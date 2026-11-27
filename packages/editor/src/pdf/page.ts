// Walks a laid-out copy of the pages and turns what it finds into the operators of each PDF page.
//
// The browser has already done the layout: every line of text sits where it should and every box has its
// size. This reads those positions back (Range.getClientRects for text, getBoundingClientRect and computed
// style for boxes, borders and backgrounds) and draws the same thing in PDF coordinates. Words are placed one
// by one at the positions the browser measured, so the PDF breaks lines where the screen does even when a
// font had to be replaced.
import { splitFamilies } from "./fontcss.ts";
import type { FontKey, FontSet, Pick } from "./fontset.ts";
import type { ImageStore } from "./images.ts";
import { MARGIN, rasterChar, rasterInk, rasterMath, rasterSvg } from "./raster.ts";
import { pageStream, type Item, type StructNode, type StructTree } from "./structure.ts";
import { canDraw, SvgPainter } from "./svgwalk.ts";
import { glyphHex, isInvisible, isSpace, literalBytes, mathItalic, parseColor, shownChars, type Rgba } from "./textutil.ts";
import { num } from "./writer.ts";

const PT = 0.75;
const SMALL_CAPS = 0.7;
/** Pixels per CSS pixel for pictures made from drawings. */
const DRAW_SCALE = 2;

/** Stacking, from the back: the sheets, objects behind the text, the text, objects in front. */
const LAYERS = 4;

/** Brackets and bars an equation stretches, drawn as lines when tall: ( [ { ⟨ ⌈ ⌊ | ‖ and their right-hand twins. */
const MIRRORED = new Map([
  [0x29, 0x28],
  [0x5d, 0x5b],
  [0x7d, 0x7b],
  [0x27e9, 0x27e8],
  [0x2309, 0x2308],
  [0x230b, 0x230a],
]);
const FENCES = new Set([0x28, 0x5b, 0x7b, 0x27e8, 0x2308, 0x230a, 0x7c, 0x2016, ...MIRRORED.keys()]);

/** A MathML length (`2px`, `0.1em`, `50%`, `thin`, a plain number) in pixels; `fallback` when there is none. */
export function lengthOf(v: string | null, size: number, fallback: number): number {
  if (v === null || !v.trim()) return fallback;
  const s = v.trim().toLowerCase();
  if (s === "thin") return fallback / 2;
  if (s === "medium") return fallback;
  if (s === "thick") return fallback * 2;
  const n = parseFloat(s);
  if (!Number.isFinite(n)) return fallback;
  if (s.endsWith("em")) return n * size;
  if (s.endsWith("ex")) return n * size * 0.5;
  if (s.endsWith("pt")) return n / PT;
  if (s.endsWith("%")) return (fallback * n) / 100;
  return n;
}

export interface LinkOut {
  page: number;
  /** x0, y0, x1, y1 in points on the page, y up. */
  rect: [number, number, number, number];
  href: string;
  /** The Link element of a tagged PDF this annotation is the content of. */
  node?: StructNode;
  /** What the link says, for the annotation's description. */
  text?: string;
}

/** Decides the structure element of each element of the pages, for a tagged PDF (see tagmap.ts). */
export interface Tagging {
  tree: StructTree;
  /** The element an element belongs to, given its parent's; null for an artifact. */
  enter(el: Element, parent: StructNode | null): StructNode | null;
}

/** A sheet of paper in the stage: where it sits and how big it is, in CSS pixels. */
export interface Sheet {
  left: number;
  top: number;
  w: number;
  h: number;
}

export interface WalkOptions {
  stage: HTMLElement;
  boxes: Sheet[];
  fonts: FontSet;
  images: ImageStore;
  /** The name of a graphics state with this opacity. */
  alpha(a: number): string;
  /** Let the browser breathe now and then. */
  tick(): Promise<void>;
  /** The advance of a character in a standard font, in 1/1000 em. */
  stdAdvance(p: Extract<Pick, { kind: "std" }>): number;
  /** The box a character's ink fills in a standard font, [left, bottom, right, top] in 1/1000 em, y up. */
  stdBounds?(p: Extract<Pick, { kind: "std" }>): [number, number, number, number] | null;
  /** Tag the content for a tagged PDF; without it everything is drawn untagged. */
  tagging?: Tagging;
  /** Draw charts and shapes as vector paths and text where it can, instead of pictures. Default: yes. */
  vectors?: boolean;
}

interface Box {
  left: number;
  top: number;
  w: number;
  h: number;
}

interface Deco {
  lines: string[];
  style: string;
  color: Rgba;
  thickness: number | null;
  offset: number | null;
}

interface State {
  /** The structure element the content belongs to (null: an artifact, whatever it holds). */
  tag: StructNode | null;
  layer: number;
  /** Every operator goes on this page (inside a transformed or faded group); null to choose by position. */
  page: number | null;
  deco: Deco[];
  alpha: number;
  /** Inside a group: the layer can't change. */
  locked: boolean;
}

interface CharInfo {
  o: number;
  l: number;
  shown: { cp: number; pick: Pick }[];
  small: boolean;
}

interface Unit {
  chars: CharInfo[];
  /** A character no font covers, drawn as a picture. */
  raster: boolean;
  /** Whitespace follows it. */
  space: boolean;
}

/** The font a computed style asks for. */
export function fontKeyOf(cs: CSSStyleDeclaration): FontKey {
  const w = parseInt(cs.fontWeight, 10);
  return { families: splitFamilies(cs.fontFamily), weight: Number.isFinite(w) ? w : 400, italic: cs.fontStyle.startsWith("italic") || cs.fontStyle.startsWith("oblique") };
}

const SKIP = ".page-num, .vruler, .np-layer, .bm-layer, .fm-layer, .sp-layer, script, style, template, noscript";

export class PageWalker {
  /** Operators per page and layer, each with the structure element it belongs to. */
  readonly layers: Item[][][];
  readonly links: LinkOut[] = [];
  /** The shadings (colour gradients) the drawings use, for the file's resources. */
  readonly shadings: { name: string; dict: string }[] = [];
  /** How many screen pixels a unit of the SVG being drawn is: its text is sized by it. */
  private svgScale = 1;
  private o: WalkOptions;
  /** The CSS letter spacing of the text being drawn, in pixels. */
  private spacing = 0;
  private origin: DOMRect;
  private range = document.createRange();
  private n = 0;
  private styles = new WeakMap<Element, CSSStyleDeclaration>();
  private keys = new Map<string, FontKey>();
  private ascents = new Map<string, number>();
  private probe: HTMLElement;
  private probeText: HTMLElement;
  private probeMark: HTMLElement;
  /** Measures characters in the fonts the page uses (for operators drawn as pictures). */
  private inkProbe = document.createElement("canvas").getContext("2d")!;

  constructor(opts: WalkOptions) {
    this.o = opts;
    this.origin = opts.stage.getBoundingClientRect();
    this.layers = opts.boxes.map(() => Array.from({ length: LAYERS }, () => [] as Item[]));
    this.probe = document.createElement("div");
    this.probe.style.cssText = "position:absolute;left:0;top:0;visibility:hidden;white-space:pre;line-height:normal;pointer-events:none";
    this.probeText = document.createElement("span");
    this.probeText.textContent = "Hxgp";
    this.probeMark = document.createElement("span");
    this.probeMark.style.cssText = "display:inline-block;width:0;height:0";
    this.probe.append(this.probeText, this.probeMark);
    opts.stage.parentElement?.append(this.probe);
  }

  // -- geometry ----------------------------------------------------------------------------------

  private toBox(r: DOMRect): Box {
    return { left: r.left - this.origin.left, top: r.top - this.origin.top, w: r.width, h: r.height };
  }

  private rects(range: Range | Element): Box[] {
    return Array.from(range.getClientRects())
      .filter((r) => r.width > 0 || r.height > 0)
      .map((r) => this.toBox(r));
  }

  /** The page that owns a vertical position in the stage (a page owns the gap below it). */
  pageAt(y: number): number {
    const boxes = this.o.boxes;
    let lo = 0;
    let hi = boxes.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (boxes[mid].top <= y) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  }

  private X(page: number, x: number): number {
    return (x - this.o.boxes[page].left) * PT;
  }

  private Y(page: number, y: number): number {
    const b = this.o.boxes[page];
    return (b.top + b.h - y) * PT;
  }

  /** Where an element sits: its page and its distance from the top of the page in points. */
  locate(el: Element): { page: number; x: number; y: number } {
    const r = this.toBox(el.getBoundingClientRect());
    const page = this.pageAt(r.top + 1);
    return { page, x: this.X(page, r.left), y: this.Y(page, r.top) };
  }

  /** Add operators to a page. `tag` is the element they are the content of; left out, they are an artifact. */
  private put(page: number, layer: number, s: string, tag?: StructNode | null, raw = false, artifact = false) {
    this.layers[page][layer].push({ s, tag: tag ?? undefined, raw, artifact, seq: tag ? this.o.tagging?.tree.next() : undefined });
  }

  /** The operators of a page as its content stream, with marked content when the PDF is tagged. */
  stream(page: number, tree: StructTree | null): string {
    return pageStream(this.layers[page].flat(), page, tree);
  }

  // -- colour and shapes -------------------------------------------------------------------------

  /** Operators that select a fill colour (and opacity, wrapped by the caller when needed). */
  private fillOps(c: Rgba): string {
    return `${num(c[0])} ${num(c[1])} ${num(c[2])} rg`;
  }

  /** Wrap operators in a graphics state with the opacity of a colour, when it isn't opaque. */
  private withAlpha(ops: string, c: Rgba, st: State): string {
    const a = c[3] * st.alpha;
    return a < 0.999 && st.alpha !== a ? `q /${this.o.alpha(a)} gs ${ops} Q` : ops;
  }

  /** Fill a rectangle given in stage pixels, once on each page it touches. Decoration unless `tagged` (a fraction bar is content). */
  private fill(x: number, y: number, w: number, h: number, c: Rgba, st: State, tagged = false) {
    if (w <= 0 || h <= 0 || c[3] <= 0.001) return;
    const on = (page: number, y0: number, y1: number) => {
      if (y1 <= y0) return;
      const ops = `${this.fillOps(c)} ${num(this.X(page, x))} ${num(this.Y(page, y1))} ${num(w * PT)} ${num((y1 - y0) * PT)} re f`;
      this.put(page, st.layer, this.withAlpha(ops, c, st), tagged ? st.tag : null);
    };
    if (st.page !== null) return on(st.page, y, y + h);
    for (let p = this.pageAt(y); p < this.o.boxes.length; p++) {
      const b = this.o.boxes[p];
      if (b.top >= y + h) break;
      on(p, Math.max(y, b.top), Math.min(y + h, b.top + b.h));
    }
  }

  /** Stroke a straight line given in stage pixels (on the page of its start, or the forced page). */
  private stroke(x1: number, y1: number, x2: number, y2: number, width: number, c: Rgba, dash: number[] | null, st: State) {
    if (c[3] <= 0.001 || width <= 0) return;
    const page = st.page ?? this.pageAt((y1 + y2) / 2);
    const ops =
      `${num(c[0])} ${num(c[1])} ${num(c[2])} RG ${num(width * PT)} w ${dash ? `[${dash.map((d) => num(d * PT)).join(" ")}] 0 d ` : ""}` +
      `${num(this.X(page, x1))} ${num(this.Y(page, y1))} m ${num(this.X(page, x2))} ${num(this.Y(page, y2))} l S`;
    this.put(page, st.layer, `q ${this.withAlpha(ops, c, st)} Q`);
  }

  // -- the walk ----------------------------------------------------------------------------------

  async run(): Promise<void> {
    await this.visit(this.o.stage, { tag: this.o.tagging?.tree.root ?? null, layer: 2, page: null, deco: [], alpha: 1, locked: false });
    this.probe.remove();
  }

  private style(el: Element): CSSStyleDeclaration {
    let cs = this.styles.get(el);
    if (!cs) this.styles.set(el, (cs = getComputedStyle(el)));
    return cs;
  }

  private async visit(el: Element, parent: State): Promise<void> {
    if ((++this.n & 127) === 0) await this.o.tick();
    if (el.matches(SKIP)) return;
    const cs = this.style(el);
    if (cs.display === "none" || cs.display === "table-column" || cs.display === "table-column-group") return;
    let st = parent;
    // The element of the structure tree this belongs to.
    if (this.o.tagging) {
      const tag = this.o.tagging.enter(el, parent.tag);
      if (tag !== parent.tag) st = { ...st, tag };
    }
    // Where this element sits in the stack: the sheets are behind everything, objects with a z-index around the text.
    if (!st.locked) {
      let layer = st.layer;
      if (el.classList.contains("sheets")) layer = 0;
      else if (layer !== 0 && cs.position !== "static" && cs.zIndex !== "auto") {
        const z = parseInt(cs.zIndex, 10);
        layer = z < 0 ? 1 : z > 0 ? 3 : 2;
      }
      if (layer !== st.layer) st = { ...st, layer };
    }
    const closers: { page: number; layer: number }[] = [];
    const open = (page: number, ops: string) => {
      this.put(page, st.layer, `q ${ops}`, undefined, true);
      closers.push({ page, layer: st.layer });
    };

    // A transform is undone here and applied to the PDF instead, so everything inside is measured upright.
    if (cs.transform !== "none" && !(el instanceof SVGElement && el.ownerSVGElement)) {
      const m = new DOMMatrix(cs.transform);
      if (m.is2D) {
        const [ox, oy] = cs.transformOrigin.split(/\s+/).map(parseFloat);
        (el as HTMLElement).style.transform = "none";
        const r = this.toBox(el.getBoundingClientRect());
        const px = r.left + (ox || 0);
        const py = r.top + (oy || 0);
        const mat: [number, number, number, number, number, number] = [m.a, m.b, m.c, m.d, m.e + px - (m.a * px + m.c * py), m.f + py - (m.b * px + m.d * py)];
        const cx = r.left + r.w / 2;
        const cy = r.top + r.h / 2;
        const page = st.page ?? this.pageAt(mat[1] * cx + mat[3] * cy + mat[5]);
        const [a, b, c, d, e, f] = this.pdfMatrix(page, mat);
        open(page, `${num(a)} ${num(b)} ${num(c)} ${num(d)} ${num(e)} ${num(f)} cm`);
        st = { ...st, page, locked: true };
      }
    }
    // Fading applies to everything inside.
    const opacity = parseFloat(cs.opacity);
    if (opacity < 0.999) {
      const r = this.toBox(el.getBoundingClientRect());
      const page = st.page ?? this.pageAt(r.top + r.h / 2);
      const alpha = st.alpha * opacity;
      open(page, `/${this.o.alpha(alpha)} gs`);
      st = { ...st, page, alpha, locked: true };
    }
    // Content that sticks out of a box with hidden overflow is cut off.
    if ((cs.overflowX !== "visible" || cs.overflowY !== "visible") && !cs.display.startsWith("table") && !el.matches(".stage, .flow, .sheets, .sheet, .cols")) {
      const r = this.toBox(el.getBoundingClientRect());
      if (r.w > 0 && r.h > 0) {
        const page = st.page ?? this.pageAt(r.top + r.h / 2);
        open(page, `${num(this.X(page, r.left))} ${num(this.Y(page, r.top + r.h))} ${num(r.w * PT)} ${num(r.h * PT)} re W n`);
        st = { ...st, page, locked: true };
      }
    }
    // Text decoration reaches the text inside, but not into boxes that sit apart from the line.
    if (cs.textDecorationLine !== "none") {
      const color = parseColor(cs.textDecorationColor) ?? parseColor(cs.color) ?? [0, 0, 0, 1];
      const thickness = cs.textDecorationThickness;
      const offset = cs.textUnderlineOffset;
      st = {
        ...st,
        deco: [
          ...st.deco,
          { lines: cs.textDecorationLine.split(/\s+/), style: cs.textDecorationStyle, color, thickness: /px$/.test(thickness) ? parseFloat(thickness) : null, offset: /px$/.test(offset) ? parseFloat(offset) : null },
        ],
      };
    } else if (st.deco.length && (cs.display.startsWith("inline-") || cs.float !== "none" || cs.position === "absolute" || cs.position === "fixed")) {
      st = { ...st, deco: [] };
    }

    this.paintBox(el, cs, st);

    const tag = el.tagName.toLowerCase();
    if (tag === "img") this.image(el as HTMLImageElement, st);
    else if (tag === "svg" && !(el.parentElement instanceof SVGElement)) this.drawing(el as SVGElement, st, "svg");
    else if (tag === "math") this.drawing(el, st, "math");
    else {
      if (el.hasAttribute("data-href")) this.link(el, st);
      for (let child = el.firstChild; child; child = child.nextSibling) {
        if (child.nodeType === 1) await this.visit(child as Element, st);
        else if (child.nodeType === 3) this.text(child as Text, cs, st);
      }
    }
    for (let i = closers.length - 1; i >= 0; i--) this.put(closers[i].page, closers[i].layer, "Q", undefined, true);
  }

  /** A matrix in stage pixels (y down) as the matrix to apply on a page in points (y up). */
  private pdfMatrix(page: number, m: [number, number, number, number, number, number]): [number, number, number, number, number, number] {
    const [a, b, c, d, e, f] = m;
    const bx = this.o.boxes[page];
    const L = bx.left;
    const TH = bx.top + bx.h;
    return [a, -b, -c, d, PT * (a * L + c * TH + e - L), PT * (TH * (1 - d) - b * L - f)];
  }

  // -- boxes -------------------------------------------------------------------------------------

  private paintBox(el: Element, cs: CSSStyleDeclaration, st: State) {
    if (cs.visibility !== "visible") return;
    const bg = parseColor(cs.backgroundColor);
    const inline = cs.display === "inline";
    if (el.matches(".tab-fill[data-leader]")) this.leader(el as HTMLElement, cs, st);
    if (el.classList.contains("notes")) {
      const r = this.toBox(el.getBoundingClientRect());
      this.fill(r.left, r.top, r.w * 0.33, 0.75, [0, 0, 0, 1], st);
    }
    if (inline) {
      const rects = this.rects(el);
      if (bg && bg[3] > 0) for (const r of rects) this.fill(r.left, r.top, r.w, r.h, bg, st);
      // A box around a run (Word's text border): top and bottom on every line, the ends on the first and the last.
      if (cs.borderTopStyle !== "none" || cs.borderLeftStyle !== "none" || cs.borderRightStyle !== "none")
        rects.forEach((r, i) => this.borders(r, cs, st, (side) => (side === "Left" && i > 0) || (side === "Right" && i < rects.length - 1)));
      return;
    }
    const r = this.toBox(el.getBoundingClientRect());
    if (r.w <= 0 && r.h <= 0) return;
    if (bg && bg[3] > 0) this.fill(r.left, r.top, r.w, r.h, bg, st);
    this.borders(r, cs, st);
  }

  private borders(r: Box, cs: CSSStyleDeclaration, st: State, skip?: (side: string) => boolean) {
    const sides = [
      ["Top", r.left, r.top, r.w, 0],
      ["Bottom", r.left, r.top + r.h, r.w, 0],
      ["Left", r.left, r.top, 0, r.h],
      ["Right", r.left + r.w, r.top, 0, r.h],
    ] as const;
    for (const [side, x, y, w, h] of sides) {
      const width = parseFloat(cs.getPropertyValue(`border-${side.toLowerCase()}-width`));
      const style = cs.getPropertyValue(`border-${side.toLowerCase()}-style`);
      if (!width || style === "none" || style === "hidden" || skip?.(side)) continue;
      const color = parseColor(cs.getPropertyValue(`border-${side.toLowerCase()}-color`));
      if (!color || color[3] <= 0) continue;
      const horizontal = side === "Top" || side === "Bottom";
      // The border sits inside the box: the right and bottom ones end at its edge.
      const x0 = side === "Right" ? x - width : x;
      const y0 = side === "Bottom" ? y - width : y;
      const len = horizontal ? w : h;
      if (style === "dashed" || style === "dotted") {
        const cx = horizontal ? x : x0 + width / 2;
        const cy = horizontal ? y0 + width / 2 : y;
        const dash = style === "dotted" ? [width, width] : [width * 3, width * 2];
        this.stroke(cx, cy, horizontal ? cx + len : cx, horizontal ? cy : cy + len, width, color, dash, st);
      } else if (style === "double" && width >= 3) {
        const t = width / 3;
        // Two lines, one at each edge of the border.
        for (const off of [0, width - t]) {
          if (horizontal) this.fill(x, y0 + off, len, t, color, st);
          else this.fill(x0 + off, y, t, len, color, st);
        }
      } else if (horizontal) this.fill(x, y0, len, width, color, st);
      else this.fill(x0, y, width, len, color, st);
    }
  }

  /** The dots, dashes or line a right or centred tab draws between its two sides. */
  private leader(el: HTMLElement, cs: CSSStyleDeclaration, st: State) {
    const r = this.toBox(el.getBoundingClientRect());
    const color = parseColor(cs.color) ?? [0, 0, 0, 1];
    const em = parseFloat(cs.fontSize);
    const kind = el.dataset.leader;
    if (kind === "dot" || kind === "middleDot") {
      const cy = r.top + r.h - 0.3 * em - 1.5;
      for (let x = r.left + 0.225 * em; x < r.left + r.w - 0.2; x += 0.45 * em) this.fill(x - 0.6, cy - 0.6, 1.2, 1.2, color, st);
    } else if (kind === "hyphen") {
      const y = r.top + r.h - 0.3 * em - 1;
      for (let x = r.left; x < r.left + r.w - 0.25 * em; x += 0.5 * em) this.fill(x, y, 0.25 * em, 1, color, st);
    } else if (kind === "underscore" || kind === "heavy") this.fill(r.left, r.top + r.h - 0.12 * em - 1, r.w, 1, color, st);
  }

  private link(el: Element, st: State) {
    const href = el.getAttribute("data-href");
    if (!href) return;
    for (const r of this.rects(el)) {
      const page = st.page ?? this.pageAt(r.top + r.h / 2);
      this.links.push({ page, rect: [this.X(page, r.left), this.Y(page, r.top + r.h), this.X(page, r.left + r.w), this.Y(page, r.top)], href, node: st.tag?.type === "Link" ? st.tag : undefined, text: (el.textContent ?? "").replace(/\s+/g, " ").trim() });
    }
  }

  // -- pictures ----------------------------------------------------------------------------------

  private place(name: string, r: Box, st: State) {
    const page = st.page ?? this.pageAt(r.top + r.h / 2);
    this.put(page, st.layer, `q ${num(r.w * PT)} 0 0 ${num(r.h * PT)} ${num(this.X(page, r.left))} ${num(this.Y(page, r.top + r.h))} cm /${name} Do Q`, st.tag);
  }

  private image(img: HTMLImageElement, st: State) {
    const cs = this.style(img);
    if (cs.visibility !== "visible" || !img.complete || !img.naturalWidth) return;
    const r = this.toBox(img.getBoundingClientRect());
    if (r.w <= 0 || r.h <= 0) return;
    this.place(this.o.images.fromImage(img, r.w, r.h, DRAW_SCALE).name, r, st);
  }

  /** A drawing the page makes with SVG or MathML: vector paths and text when it can be drawn so, else a picture. */
  private drawing(el: Element, st: State, kind: "svg" | "math") {
    const cs = this.style(el);
    if (cs.visibility !== "visible") return;
    const r = this.toBox(el.getBoundingClientRect());
    if (r.w <= 0 || r.h <= 0) return;
    if (kind === "svg" && this.o.vectors !== false && canDraw(el as SVGSVGElement, (e) => this.style(e))) {
      this.svgVector(el as SVGSVGElement, st, r);
      return;
    }
    if (kind === "math" && this.o.vectors !== false && this.mathVector(el, st)) return;
    const ref = this.o.images.fromCanvas(() => (kind === "svg" ? rasterSvg(el as SVGElement, r.w, r.h, DRAW_SCALE) : rasterMath(el, r.w, r.h, DRAW_SCALE)));
    this.place(ref.name, { left: r.left - MARGIN, top: r.top - MARGIN, w: r.w + 2 * MARGIN, h: r.h + 2 * MARGIN }, st);
  }

  /** Draw an SVG as paths and text (see svgwalk.ts). */
  private svgVector(svg: SVGSVGElement, st: State, r: Box) {
    const page = st.page ?? this.pageAt(r.top + r.h / 2);
    const painter = new SvgPainter({
      origin: this.origin,
      sheet: (p) => this.o.boxes[p]!,
      style: (e) => this.style(e),
      alpha: (a) => this.o.alpha(a * st.alpha),
      shading: (dict) => {
        const name = `Sh${this.shadings.length + 1}`;
        this.shadings.push({ name, dict });
        return name;
      },
      put: (ops, raw) => this.put(page, st.layer, ops, st.tag, !!raw),
      text: (node, cs, scale) => {
        this.svgScale = scale;
        try {
          this.text(node, cs, { ...st, page });
        } finally {
          this.svgScale = 1;
        }
      },
      pdfMatrix: (m) => this.pdfMatrix(page, m),
    });
    painter.draw(svg, page);
  }

  // -- equations ---------------------------------------------------------------------------------

  /**
   * An equation as text and lines instead of a picture. The browser has laid it out, and each letter, digit and
   * operator sits in an element of its own whose box is tight around its ink. Letters and digits are drawn as text
   * where the browser put them. An operator the browser stretched or enlarged (a tall bracket, a big sum) is its
   * character scaled to the box it fills, or lines for a tall bracket. Fraction bars and root signs have no
   * character and are drawn as lines. The whole equation is planned first: when any part of it can't be drawn
   * so, nothing is drawn and the caller makes a picture of it instead.
   */
  private mathVector(el: Element, st: State): boolean {
    const plan: (() => void)[] = [];
    if (!this.planMath(el, st, plan)) return false;
    for (const step of plan) step();
    return true;
  }

  private planMath(el: Element, st: State, plan: (() => void)[]): boolean {
    for (let c = el.firstChild; c; c = c.nextSibling) {
      if (c.nodeType === 3) {
        const node = c as Text;
        if (!node.data.trim()) continue;
        const cs = this.style(el);
        if (el.localName === "mo") {
          if (!this.planOperator(el, node, cs, st, plan)) return false;
        } else plan.push(() => this.text(node, cs, st));
        continue;
      }
      if (c.nodeType !== 1) continue;
      const child = c as Element;
      const cs = this.style(child);
      if (cs.display === "none") continue;
      // Anything the equation holds that isn't MathML (a picture, a drawing) is left to the picture.
      if (child.namespaceURI !== "http://www.w3.org/1998/Math/MathML") return false;
      plan.push(() => this.paintBox(child, cs, st));
      if (!this.planMath(child, st, plan)) return false;
      if (child.localName === "mfrac") plan.push(() => this.fractionBar(child, cs, st));
      else if (child.localName === "msqrt" || child.localName === "mroot") plan.push(() => this.radical(child, cs, st));
    }
    return true;
  }

  /** The box a picked character's ink fills, in em, [left, bottom, right, top] with y up. */
  private glyphBox(p: Pick): [number, number, number, number] | null {
    if (p.kind === "ttf") {
      const b = p.font.ttf.glyphBounds(p.gid);
      return b && [b[0] / p.unitsPerEm, b[1] / p.unitsPerEm, b[2] / p.unitsPerEm, b[3] / p.unitsPerEm];
    }
    const b = this.o.stdBounds?.(p);
    return b ? [b[0] / 1000, b[1] / 1000, b[2] / 1000, b[3] / 1000] : null;
  }

  /** An operator: as text, or scaled to the box the browser stretched it to, or as lines for a tall bracket. */
  private planOperator(mo: Element, node: Text, cs: CSSStyleDeclaration, st: State, plan: (() => void)[]): boolean {
    const chars = [...node.data.trim()];
    // A word set as an operator (lim, sin) is ordinary text.
    if (chars.length !== 1) {
      plan.push(() => this.text(node, cs, st));
      return true;
    }
    const cp = chars[0].codePointAt(0)!;
    const ink = this.toBox(mo.getBoundingClientRect());
    const size = parseFloat(cs.fontSize);
    if (ink.w <= 0 || ink.h <= 0 || !(size > 0)) {
      plan.push(() => this.text(node, cs, st));
      return true;
    }
    const key = this.fontKey(cs);
    const pick = this.o.fonts.pick(key, cp);
    const box = pick && this.glyphBox(pick);
    const fence = FENCES.has(cp) && ink.h > size * 1.8;
    if (!pick || !box) {
      // No font has it: at its own size it is a pictured character like any other; stretched, a fence is lines and
      // anything else a sharp picture of the character filling its box.
      if (fence) plan.push(() => this.fence(cp, ink, cs, st));
      else if (ink.h <= size * 1.3 && ink.w <= size * 1.6) plan.push(() => this.text(node, cs, st));
      else plan.push(() => this.inkPicture(chars[0], ink, cs, st));
      return true;
    }
    const naturalH = (box[3] - box[1]) * size;
    // The box runs the operator's advance across, so a bar or a "!" is wider than its ink without being stretched.
    // Only an operator that stretches sideways (an arrow or brace over or under something, a stretchy accent) is.
    const sideways = mo.getAttribute("stretchy") === "true" || /^m(?:over|under|underover)$/.test(mo.parentElement?.localName ?? "");
    const across = sideways && ink.w > this.advance(pick, size) * 1.25 + 1;
    const stretched = ink.h > naturalH * 1.35 + 1 || across;
    if (!stretched) plan.push(() => this.text(node, cs, st));
    else if (fence) plan.push(() => this.fence(cp, ink, cs, st));
    else plan.push(() => this.scaledGlyph(pick, box, key, ink, across, cs, st));
    return true;
  }

  /** A character drawn so its ink fills a box: a big sum, a stretched arrow or accent. */
  private scaledGlyph(p: Pick, b: [number, number, number, number], key: FontKey, box: Box, across: boolean, cs: CSSStyleDeclaration, st: State) {
    const color = parseColor(cs.color) ?? [0, 0, 0, 1];
    if (color[3] <= 0.001 || cs.visibility !== "visible") return;
    const size = parseFloat(cs.fontSize);
    const inkW = (b[2] - b[0]) * size;
    const inkH = (b[3] - b[1]) * size;
    // The box is tight around the ink up and down, but runs the operator's whole advance across: a sum or
    // an integral made bigger keeps its shape and sits in the middle; an arrow or brace made wider is stretched.
    const sy = box.h / inkH;
    const wide = across && box.w / inkW > sy * 1.6;
    const sx = wide ? (box.w * 0.96) / inkW : sy;
    const left = box.left + (box.w - inkW * sx) / 2;
    const page = st.page ?? this.pageAt(box.top + box.h / 2);
    const x = this.X(page, left - b[0] * size * sx);
    const y = this.Y(page, box.top + b[3] * size * sy);
    let shown: string;
    if (p.kind === "ttf") {
      p.font.use(p.gid, p.cp);
      shown = glyphHex([p.gid]);
    } else shown = literalBytes([p.code]);
    this.o.fonts.noteUse(key, p);
    // Text state outlives ET: the scaling and spacing a run before left behind are set back.
    const ops = `${this.fillOps(color)} BT 100 Tz 0 Tc ${num(sx)} 0 0 ${num(sy)} ${num(x)} ${num(y)} Tm /${p.font.resource} ${num(size * PT)} Tf ${shown} Tj ET`;
    this.put(page, st.layer, this.withAlpha(ops, color, st), st.tag);
  }

  /** A character no font file has, as a picture filling the box the equation stretched it to. */
  private inkPicture(text: string, box: Box, cs: CSSStyleDeclaration, st: State) {
    if (box.w <= 0 || box.h <= 0 || cs.visibility !== "visible") return;
    const font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
    const color = cs.color;
    // As for a scaled glyph: taller keeps its shape and sits in the middle, wider is stretched.
    this.inkProbe.font = font;
    const m = this.inkProbe.measureText(text);
    const inkW = m.actualBoundingBoxLeft + m.actualBoundingBoxRight;
    const inkH = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent;
    if (!(inkW > 0) || !(inkH > 0)) return;
    const sy = box.h / inkH;
    const w = box.w / inkW > sy * 1.6 ? box.w * 0.96 : Math.min(box.w, inkW * sy);
    const target = { left: box.left + (box.w - w) / 2, top: box.top, w, h: box.h };
    const ref = this.o.images.fromCanvas(async () => rasterInk(text, font, color, target.w, target.h, DRAW_SCALE * 2), `ink:${text}|${font}|${color}|${target.w.toFixed(1)}x${target.h.toFixed(1)}`);
    this.place(ref.name, target, st);
  }

  /** Stroke a path given as moves, lines and curves in stage pixels, on one page. */
  private path(cmds: (["M" | "L", number, number] | ["C", number, number, number, number, number, number])[], width: number, color: Rgba, st: State, page: number) {
    if (color[3] <= 0.001 || width <= 0) return;
    let d = "";
    for (const c of cmds) {
      if (c[0] === "C") d += `${num(this.X(page, c[1]))} ${num(this.Y(page, c[2]))} ${num(this.X(page, c[3]))} ${num(this.Y(page, c[4]))} ${num(this.X(page, c[5]))} ${num(this.Y(page, c[6]))} c `;
      else d += `${num(this.X(page, c[1]))} ${num(this.Y(page, c[2]))} ${c[0] === "M" ? "m" : "l"} `;
    }
    const ops = `${num(color[0])} ${num(color[1])} ${num(color[2])} RG ${num(width * PT)} w 1 j 1 J ${d}S`;
    this.put(page, st.layer, `q ${this.withAlpha(ops, color, st)} Q`, st.tag);
  }

  /** A tall bracket, brace or bar as lines filling the box the browser stretched it to. */
  private fence(cp: number, box: Box, cs: CSSStyleDeclaration, st: State) {
    const color = parseColor(cs.color) ?? [0, 0, 0, 1];
    if (cs.visibility !== "visible") return;
    const size = parseFloat(cs.fontSize);
    const t = Math.max(0.6, size * 0.07);
    const page = st.page ?? this.pageAt(box.top + box.h / 2);
    const top = box.top + t / 2;
    const bottom = box.top + box.h - t / 2;
    const mid = (top + bottom) / 2;
    // The box runs the bracket's whole advance; its ink is the middle of it, narrower than the box.
    const inset = Math.max(t / 2, box.w * 0.22);
    const l = box.left + inset;
    const r = box.left + box.w - inset;
    const cx = (l + r) / 2;
    // Mirrored shapes are drawn as their left-hand twin, flipped.
    const flip = MIRRORED.has(cp);
    const X = (x: number) => (flip ? l + r - x : x);
    const kind = flip ? MIRRORED.get(cp)! : cp;
    const k = 4 / 3;
    let cmds: Parameters<PageWalker["path"]>[0];
    switch (kind) {
      case 0x28: // (
        cmds = [["M", X(r), top], ["C", X(r + (l - r) * k), top + (bottom - top) * 0.28, X(r + (l - r) * k), bottom - (bottom - top) * 0.28, X(r), bottom]];
        break;
      case 0x5b: // [
        cmds = [["M", X(r), top], ["L", X(l), top], ["L", X(l), bottom], ["L", X(r), bottom]];
        break;
      case 0x2308: // ⌈
        cmds = [["M", X(r), top], ["L", X(l), top], ["L", X(l), bottom]];
        break;
      case 0x230a: // ⌊
        cmds = [["M", X(l), top], ["L", X(l), bottom], ["L", X(r), bottom]];
        break;
      case 0x7b: {
        // {
        const q = Math.min((bottom - top) * 0.12, (r - l) * 1.5);
        cmds = [
          ["M", X(r), top],
          ["C", X(cx), top, X(cx), top, X(cx), top + q],
          ["L", X(cx), mid - q],
          ["C", X(cx), mid, X(l), mid, X(l), mid],
          ["C", X(l), mid, X(cx), mid, X(cx), mid + q],
          ["L", X(cx), bottom - q],
          ["C", X(cx), bottom, X(cx), bottom, X(r), bottom],
        ];
        break;
      }
      case 0x27e8: // ⟨
        cmds = [["M", X(r), top], ["L", X(l), mid], ["L", X(r), bottom]];
        break;
      case 0x7c: // |
        cmds = [["M", cx, top], ["L", cx, bottom]];
        break;
      case 0x2016: {
        // ‖
        const a = l + (r - l) / 3;
        const b = r - (r - l) / 3;
        this.path([["M", a, top], ["L", a, bottom]], t, color, st, page);
        cmds = [["M", b, top], ["L", b, bottom]];
        break;
      }
      default:
        return;
    }
    this.path(cmds, t, color, st, page);
  }

  /** The line between the top and the bottom of a fraction (none when its linethickness is 0). */
  private fractionBar(el: Element, cs: CSSStyleDeclaration, st: State) {
    if (cs.visibility !== "visible" || el.children.length < 2) return;
    const size = parseFloat(cs.fontSize);
    const t = lengthOf(el.getAttribute("linethickness"), size, Math.max(0.5, size * 0.066));
    if (!(t > 0)) return;
    const color = parseColor(cs.color) ?? [0, 0, 0, 1];
    const r = this.toBox(el.getBoundingClientRect());
    const top = this.toBox(el.children[0].getBoundingClientRect());
    const bottom = this.toBox(el.children[1].getBoundingClientRect());
    // On the math axis, as the browser draws it (in line with the "=" beside it); halfway between the two parts
    // when the axis can't be found, or isn't between them.
    const numBottom = top.top + top.h;
    const axis = this.mathAxis(el, cs);
    const y = axis !== null && axis > numBottom && axis < bottom.top ? axis : (numBottom + bottom.top) / 2;
    const pl = parseFloat(cs.paddingLeft) || 0;
    const pr = parseFloat(cs.paddingRight) || 0;
    this.fill(r.left + pl, y - t / 2, r.w - pl - pr, t, color, { ...st, page: st.page ?? this.pageAt(y) }, true);
  }

  /**
   * Where the math axis of an element is, in stage pixels: its baseline (found with an empty space put beside it for a
   * moment) less the height of the middle of a minus sign. Null in a place where nothing can be put beside it.
   */
  private mathAxis(el: Element, cs: CSSStyleDeclaration): number | null {
    const parent = el.parentElement;
    if (!parent || !/^(?:math|mrow|mstyle|mpadded|mphantom|msqrt|menclose|mtd|merror)$/.test(parent.localName)) return null;
    this.inkProbe.font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
    const minus = this.inkProbe.measureText("\u2212");
    const height = (minus.actualBoundingBoxAscent - minus.actualBoundingBoxDescent) / 2;
    if (!(height > 0)) return null;
    // An mspace has no width, height or depth of its own: its top is the baseline of the row.
    const probe = document.createElementNS("http://www.w3.org/1998/Math/MathML", "mspace");
    parent.insertBefore(probe, el.nextSibling);
    const baseline = this.toBox(probe.getBoundingClientRect()).top;
    probe.remove();
    return baseline - height;
  }

  /** The root sign: a tick, a long stroke down and up, and the line over what is under the root. */
  private radical(el: Element, cs: CSSStyleDeclaration, st: State) {
    if (cs.visibility !== "visible") return;
    const root = el.localName === "mroot";
    const under = root ? (el.children[0] ? [el.children[0]] : []) : Array.from(el.children);
    const boxes = under.map((c) => this.toBox(c.getBoundingClientRect())).filter((b) => b.w > 0 || b.h > 0);
    const r = this.toBox(el.getBoundingClientRect());
    if (r.w <= 0 || r.h <= 0) return;
    const size = parseFloat(cs.fontSize);
    const t = Math.max(0.5, size * 0.055);
    const color = parseColor(cs.color) ?? [0, 0, 0, 1];
    const baseLeft = boxes.length ? Math.min(...boxes.map((b) => b.left)) : r.left + r.w;
    const baseRight = boxes.length ? Math.max(...boxes.map((b) => b.left + b.w)) : r.left + r.w;
    const barY = r.top + t * 1.5;
    const bottom = r.top + r.h - t / 2;
    const signR = baseLeft - Math.max(0.5, size * 0.04);
    const signL = Math.max(root ? r.left : r.left + t / 2, signR - size * 0.78);
    const w = signR - signL;
    if (w <= 0) return;
    const h = bottom - barY;
    const page = st.page ?? this.pageAt(r.top + r.h / 2);
    this.path(
      [
        ["M", signL, bottom - h * 0.4],
        ["L", signL + w * 0.2, bottom - h * 0.48],
        ["L", signL + w * 0.48, bottom],
        ["L", signR, barY],
        ["L", Math.max(baseRight, signR) + t, barY],
      ],
      t,
      color,
      st,
      page,
    );
  }

  // -- text --------------------------------------------------------------------------------------

  private fontKey(cs: CSSStyleDeclaration): FontKey {
    const id = `${cs.fontFamily}|${cs.fontWeight}|${cs.fontStyle}`;
    let k = this.keys.get(id);
    if (!k) this.keys.set(id, (k = fontKeyOf(cs)));
    return k;
  }

  /** How far the baseline sits below the top of the text's box, in the font and size an element uses. */
  private ascent(cs: CSSStyleDeclaration): number {
    const sig = `${cs.fontFamily}|${cs.fontSize}|${cs.fontWeight}|${cs.fontStyle}|${cs.fontStretch}|${cs.fontVariantCaps}`;
    let a = this.ascents.get(sig);
    if (a === undefined) {
      this.probeText.style.cssText = `font-family:${cs.fontFamily};font-size:${cs.fontSize};font-weight:${cs.fontWeight};font-style:${cs.fontStyle};font-stretch:${cs.fontStretch};font-variant-caps:${cs.fontVariantCaps}`;
      this.range.selectNodeContents(this.probeText);
      const t = this.range.getBoundingClientRect();
      const m = this.probeMark.getBoundingClientRect();
      a = m.bottom - t.top;
      this.ascents.set(sig, a);
    }
    return a;
  }

  private advance(p: Pick, size: number): number {
    return p.kind === "ttf" ? (p.font.ttf.advance(p.gid) / p.unitsPerEm) * size : (this.o.stdAdvance(p) / 1000) * size;
  }

  /** The line around the letters of the text being drawn (-webkit-text-stroke), when it has one. */
  private outline: { width: number; color: Rgba; hollow: boolean } | null = null;

  private text(node: Text, cs: CSSStyleDeclaration, st: State) {
    const raw = node.data;
    if (!raw || cs.visibility !== "visible") return;
    this.range.selectNodeContents(node);
    const lines = this.rects(this.range);
    if (!lines.length) return;
    // A letter on its own in an equation is drawn in italic, as the browser draws it (MathML's math-auto).
    const italic = mathItalic(cs, raw);
    const key = italic ? { ...this.fontKey(cs), italic: true } : this.fontKey(cs);
    const transform = cs.textTransform === "math-auto" ? "none" : cs.textTransform;
    // Text in an SVG is sized in the drawing's units, coloured by `fill`, and faded by its own opacity.
    const inSvg = !!node.parentElement && node.parentElement instanceof SVGElement;
    const k = inSvg ? this.svgScale : 1;
    const size = parseFloat(cs.fontSize) * k;
    this.spacing = (cs.letterSpacing === "normal" ? 0 : parseFloat(cs.letterSpacing) || 0) * k;
    let color = parseColor(inSvg ? cs.fill : cs.color) ?? [0, 0, 0, 1];
    if (inSvg) color = [color[0], color[1], color[2], color[3] * (parseFloat(cs.fillOpacity) || 0) * (parseFloat(cs.opacity) || 0)];
    // Outline text (a line and no fill) is stroked; text filled with a gradient takes the colour of its first stop.
    const strokeWidth = parseFloat(cs.getPropertyValue("-webkit-text-stroke-width")) || 0;
    const strokeColor = parseColor(cs.getPropertyValue("-webkit-text-stroke-color")) ?? color;
    if (cs.getPropertyValue("-webkit-background-clip") === "text" || cs.backgroundClip === "text") {
      const first = /rgba?\([^)]*\)/.exec(cs.backgroundImage);
      const c = first && parseColor(first[0]);
      if (c) color = c;
    }
    const hollow = color[3] <= 0.001 && strokeWidth > 0;
    this.outline = strokeWidth > 0 ? { width: strokeWidth, color: strokeColor, hollow } : null;
    if (hollow) color = strokeColor;
    const ascent = this.ascent(cs) * k;
    const fonts = this.o.fonts;

    // Split the text into words and single pictured characters, each with the font that draws it.
    const units: Unit[] = [];
    let cur: Unit | null = null;
    let wordStart = true;
    let o = 0;
    for (const ch of raw) {
      const cp = ch.codePointAt(0)!;
      const l = ch.length;
      if (isSpace(cp)) {
        if (cur) cur.space = true;
        cur = null;
        wordStart = true;
      } else if (isInvisible(cp)) {
        if (cur) cur.chars[cur.chars.length - 1].l += l;
      } else {
        const shown = shownChars(ch, transform, cs.fontVariantCaps, wordStart);
        const picks = shown.cps.map((c) => ({ cp: c, pick: fonts.pick(key, c) }));
        wordStart = false;
        // Characters no font file covers are drawn as one picture per run, so the browser shapes and orders them
        // (Arabic joins its letters, Hebrew runs right to left, an emoji sequence stays one emoji).
        const raster = picks.some((p) => !p.pick);
        if (!cur || cur.raster !== raster) units.push((cur = { chars: [], raster, space: false }));
        cur.chars.push({ o, l, shown: raster ? [] : (picks as CharInfo["shown"]), small: shown.small });
      }
      o += l;
    }

    // A space at the start of the text stands between this text and what comes before it (another element's
    // words): without it the words run together when the text is copied or searched.
    if (isSpace(raw.codePointAt(0)!)) {
      const space = fonts.pick(key, 0x20);
      if (space) {
        this.range.setStart(node, 0);
        this.range.setEnd(node, 1);
        const own = this.rects(this.range)[0];
        const box = own ?? (units.length && !units[0].raster ? this.fragments(node, units[0])[0]?.box : undefined);
        if (box) this.emitRun([{ o: 0, l: 1, shown: [{ cp: 0x20, pick: space }], small: false }], { ...box, w: own ? own.w : 0 }, false, key, size, color, ascent, st, false);
      }
    }

    for (const u of units) {
      if (u.raster) {
        for (const f of this.fragments(node, u)) {
          const last = f.chars[f.chars.length - 1];
          this.rasterGlyph(raw.slice(f.chars[0].o, last.o + last.l), f.box, cs, st);
        }
        continue;
      }
      for (const f of this.fragments(node, u)) {
        this.emitRun(f.chars, f.box, f.last && u.space, key, size, color, ascent, st, false);
        // The hyphen drawn at the end of a line: part of the page, not of the text.
        const dash = f.hyphen && fonts.pick(key, 0x2d);
        if (f.hyphen && dash) this.emitRun([{ o: 0, l: 1, shown: [{ cp: 0x2d, pick: dash }], small: false }], f.hyphen, false, key, size, color, ascent, st, true);
      }
    }

    for (const d of st.deco) for (const line of lines) this.decorate(d, line, ascent, size, st);
  }

  /**
   * The pieces of a word, one per line it is set on (a long word can break across lines). Where hyphenation broke
   * it, the browser reports the hyphen it draws as an extra box on the last letter of the line and again on the
   * first letter of the next; that box comes back as `hyphen`.
   */
  private fragments(node: Text, u: Unit): { chars: CharInfo[]; box: Box; last: boolean; hyphen?: Box }[] {
    const a = u.chars[0];
    const z = u.chars[u.chars.length - 1];
    this.range.setStart(node, a.o);
    this.range.setEnd(node, z.o + z.l);
    const rects = this.rects(this.range);
    if (rects.length === 0) return [];
    if (rects.length === 1) return [{ chars: u.chars, box: rects[0], last: true }];
    // Set on several lines: find where each line starts, character by character.
    const out: { chars: CharInfo[]; box: Box; last: boolean; hyphen?: Box }[] = [];
    let start = 0;
    let box: Box | null = null;
    let before: Box[] = [];
    u.chars.forEach((c, i) => {
      this.range.setStart(node, c.o);
      this.range.setEnd(node, c.o + c.l);
      let mine = this.rects(this.range);
      let hyphen: Box | undefined;
      if (i > 0 && mine.length >= 2 && before.length >= 2) {
        const h = before[before.length - 1];
        if (Math.abs(mine[0].left - h.left) < 0.01 && Math.abs(mine[0].top - h.top) < 0.01) {
          hyphen = h;
          mine = mine.slice(1);
        }
      }
      before = this.rects(this.range);
      const r = mine[0];
      if (!r) return;
      if (box && Math.abs(r.top - box.top) > Math.max(2, box.h / 2)) {
        out.push({ chars: u.chars.slice(start, i), box, last: false, hyphen });
        start = i;
        box = r;
      } else if (box) {
        const right = Math.max(box.left + box.w, r.left + r.w);
        box = { left: Math.min(box.left, r.left), top: box.top, w: right - Math.min(box.left, r.left), h: box.h };
      } else box = r;
    });
    if (box) out.push({ chars: u.chars.slice(start), box, last: true });
    return out;
  }

  private emitRun(chars: CharInfo[], box: Box, space: boolean, key: FontKey, size: number, color: Rgba, ascent: number, st: State, artifact: boolean) {
    if (color[3] <= 0.001 || !chars.length) return;
    // The width the glyphs have by themselves; the difference to the measured width is spread over them.
    let natural = 0;
    let glyphs = 0;
    for (const c of chars)
      for (const s of c.shown) {
        natural += this.advance(s.pick, size * (c.small ? SMALL_CAPS : 1));
        glyphs++;
      }
    // The glyphs are stretched to the measured width first (a stand-in font is narrower or wider than the one on
    // screen, and stretched letters read better than spread ones); what is left, including the CSS letter
    // spacing, is spread between the letters. Real fonts differ by a hair (kerning) and are only spaced.
    const ls = this.spacing;
    const target = box.w - ls * glyphs;
    let tz = 1;
    if (glyphs > 1 && natural > 0 && Math.abs(target / natural - 1) > 0.02) tz = Math.max(0.6, Math.min(1.6, target / natural));
    const tc = glyphs > 1 ? (((target - natural * tz) / glyphs + ls) * PT) / tz : 0;
    const page = st.page ?? this.pageAt(box.top + box.h / 2);
    const x = this.X(page, box.left);
    const y = this.Y(page, box.top + ascent);

    let synthBold = false;
    let synthItalic = false;
    let font = "";
    let fontSize = 0;
    let shows = "";
    let pending: string[] = [];
    let pendingStd = false;
    const flush = () => {
      if (!pending.length) return;
      shows += pendingStd ? `${literalBytes(pending.map(Number))} Tj ` : `${glyphHex(pending.map(Number))} Tj `;
      pending = [];
    };
    const draw = (p: Pick, small: boolean) => {
      const sz = size * (small ? SMALL_CAPS : 1) * PT;
      if (p.font.resource !== font || sz !== fontSize) {
        flush();
        shows += `/${p.font.resource} ${num(sz)} Tf `;
        font = p.font.resource;
        fontSize = sz;
      }
      pendingStd = p.kind === "std";
      if (p.kind === "ttf") {
        p.font.use(p.gid, p.cp);
        pending.push(String(p.gid));
        synthBold ||= p.synthBold;
        synthItalic ||= p.synthItalic;
      } else pending.push(String(p.code));
    };
    for (const c of chars)
      for (const s of c.shown) {
        draw(s.pick, c.small);
        this.o.fonts.noteUse(key, s.pick);
      }
    if (space) {
      const sp = this.o.fonts.pick(key, 0x20);
      if (sp) draw(sp, false);
    }
    flush();
    const skew = synthItalic ? 0.2126 : 0;
    let ops = `${this.fillOps(color)} BT ${num(1)} 0 ${num(skew)} 1 ${num(x)} ${num(y)} Tm ${num(tz * 100)} Tz ${num(tc)} Tc ${shows}ET`;
    if (this.outline) {
      // A line round the letters (-webkit-text-stroke); hollow letters have no fill.
      const o = this.outline;
      ops = `q ${num(o.color[0])} ${num(o.color[1])} ${num(o.color[2])} RG ${num(o.width * PT)} w ${o.hollow ? 1 : 2} Tr ${ops} Q`;
    } else if (synthBold) ops = `q ${num(color[0])} ${num(color[1])} ${num(color[2])} RG ${num(size * PT * 0.03)} w 2 Tr ${ops} Q`;
    // The hyphen at the end of a line belongs to the page, not to the text.
    this.put(page, st.layer, this.withAlpha(ops, color, st), artifact ? null : st.tag, false, artifact);
  }

  private rasterGlyph(text: string, box: Box, cs: CSSStyleDeclaration, st: State) {
    if (box.w <= 0 || box.h <= 0) return;
    const font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
    const color = cs.color;
    const dir = cs.direction;
    const ref = this.o.images.fromCanvas(async () => rasterChar(text, font, color, dir, box.w, box.h, DRAW_SCALE), `ch:${text}|${font}|${color}|${dir}|${box.w.toFixed(1)}x${box.h.toFixed(1)}`);
    this.place(ref.name, box, st);
  }

  /** Underline, strike-through or overline for one line of text. */
  private decorate(d: Deco, line: Box, ascent: number, size: number, st: State) {
    const base = line.top + ascent;
    const thick = d.thickness ?? Math.max(1, size / 16);
    const x1 = line.left;
    const x2 = line.left + line.w;
    const draw = (y: number) => {
      if (d.style === "double") {
        this.stroke(x1, y - thick, x2, y - thick, thick, d.color, null, st);
        this.stroke(x1, y + thick, x2, y + thick, thick, d.color, null, st);
      } else if (d.style === "dotted") this.stroke(x1, y, x2, y, thick, d.color, [thick, thick * 1.5], st);
      else if (d.style === "dashed") this.stroke(x1, y, x2, y, thick, d.color, [thick * 4, thick * 2], st);
      else if (d.style === "wavy") {
        const step = Math.max(2, thick * 2.5);
        for (let x = x1; x < x2; x += step) {
          const to = Math.min(x2, x + step);
          const up = (Math.round((x - x1) / step) & 1) === 0;
          this.stroke(x, y + (up ? -step / 3 : step / 3), to, y + (up ? step / 3 : -step / 3), thick, d.color, null, st);
        }
      } else this.stroke(x1, y, x2, y, thick, d.color, null, st);
    };
    for (const l of d.lines) {
      if (l === "underline") draw(base + (d.offset ?? size * 0.1) + thick / 2);
      else if (l === "line-through") draw(base - size * 0.3);
      else if (l === "overline") draw(line.top + thick / 2);
    }
  }
}
