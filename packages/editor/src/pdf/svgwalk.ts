// Draws an SVG (a chart, a shape) into the PDF as vector paths and real text instead of a picture. Each shape's outline is read
// from its attributes, moved to the page by the browser's own transform for it (`getScreenCTM`, which holds the viewBox, nested
// groups and turns), and painted with the colours the browser computed. Text is drawn by the page walker, which reads where the
// browser put each letter. An SVG with something this does not draw (filters, masks, clipping, patterns, markers, pictures,
// transparent gradient stops) is left to the picture it has always been.
import { axialShading, ellipseSegments, invert, isSimilarity, mul, parsePathData, pathOps, pointList, polySegments, radialShading, rectSegments, scaleOf, transformSegments, type Matrix, type Seg, type Stop } from "./svgdraw.ts";
import { parseColor, type Rgba } from "./textutil.ts";
import { num } from "./writer.ts";

const PT = 0.75;

/** What the painter needs from the page walker. */
export interface SvgHost {
  /** The stage's top left corner on the screen: positions of the page are counted from it. */
  origin: DOMRect;
  /** Where a page sits in the stage, in CSS pixels. */
  sheet(page: number): { left: number; top: number; h: number };
  style(el: Element): CSSStyleDeclaration;
  /** The name of a graphics state with this opacity. */
  alpha(a: number): string;
  /** Add a shading to the file's resources; returns its name. */
  shading(dict: string): string;
  /** Add operators to the page. */
  put(ops: string, raw?: boolean): void;
  /** Draw the text of a node with the walker's text code, at `scale` (user units to screen pixels). */
  text(node: Text, cs: CSSStyleDeclaration, scale: number): void;
  /** The matrix to put on a page's operators for a matrix on the stage (in CSS pixels, y down). */
  pdfMatrix(m: Matrix): Matrix;
}

const m6 = (d: DOMMatrix | DOMMatrixReadOnly): Matrix => [d.a, d.b, d.c, d.d, d.e, d.f];

const SHAPES = new Set(["path", "rect", "circle", "ellipse", "line", "polyline", "polygon"]);
const GROUPS = new Set(["g", "svg", "a", "switch"]);
/** Parts of an SVG a picture is needed for. */
const NEEDS_PICTURE = "image, foreignObject, use, symbol, pattern, mask, filter, textPath, marker";

/** Whether this drawing is one the painter can draw: nothing in it needs the browser's own rendering. */
export function canDraw(svg: SVGSVGElement, style: (el: Element) => CSSStyleDeclaration): boolean {
  if (svg.querySelector(NEEDS_PICTURE)) return false;
  for (const el of [svg as Element, ...Array.from(svg.querySelectorAll("*"))]) {
    const tag = el.tagName.toLowerCase();
    if (el.closest("defs") && !/gradient|stop/i.test(tag) && tag !== "defs") {
      // Things in <defs> that are not gradients are only used by what the picture handles.
      if (tag !== "title" && tag !== "style") return false;
      continue;
    }
    const cs = style(el);
    if (cs.display === "none") continue;
    if (cs.filter !== "none" && cs.filter !== "") return false;
    if (cs.clipPath !== "none" && cs.clipPath !== "") return false;
    if (cs.mixBlendMode !== "normal") return false;
    const maskImage = cs.getPropertyValue("mask-image");
    if (maskImage && maskImage !== "none") return false;
    for (const k of ["marker-start", "marker-mid", "marker-end"]) if (cs.getPropertyValue(k) !== "none") return false;
    if (SHAPES.has(tag)) {
      for (const k of ["fill", "stroke"] as const) {
        const v = cs.getPropertyValue(k);
        const url = /^url\(["']?#([^"')]+)["']?\)/.exec(v);
        if (!url) continue;
        const target = svg.ownerDocument.getElementById(url[1]!) ?? svg.querySelector(`#${CSS.escape(url[1]!)}`);
        if (!target || !/^(linear|radial)gradient$/i.test(target.tagName)) return false;
        if (!gradientOk(target as unknown as SVGGradientElement, style)) return false;
      }
    }
    if (tag === "text" || tag === "tspan") {
      if (tag === "text") {
        const ctm = (el as SVGGraphicsElement).getScreenCTM();
        // Text keeps its shape only under a turn and an even scale.
        if (!ctm || !isSimilarity(m6(ctm))) return false;
      }
      if (cs.getPropertyValue("fill").startsWith("url(")) return false;
    }
  }
  return true;
}

/** An element and the ones it takes its stops and settings from through `href`. */
function chain(g: Element): Element[] {
  const out: Element[] = [];
  let cur: Element | null = g;
  for (let n = 0; cur && n < 8; n++) {
    out.push(cur);
    const href: string | null = cur.getAttribute("href") ?? cur.getAttribute("xlink:href");
    cur = href?.startsWith("#") ? (cur.ownerDocument.getElementById(href.slice(1)) ?? null) : null;
  }
  return out;
}

const attrOf = (g: Element, name: string): string | null => {
  for (const e of chain(g)) if (e.hasAttribute(name)) return e.getAttribute(name);
  return null;
};

/** A gradient the painter can draw: it spreads by padding, and its stops are opaque. */
function gradientOk(g: SVGGradientElement, style: (el: Element) => CSSStyleDeclaration): boolean {
  if ((attrOf(g, "spreadMethod") ?? "pad") !== "pad") return false;
  const stops = stopsOf(g);
  if (!stops.length) return false;
  for (const e of chain(g)) for (const s of Array.from(e.children).filter((c) => c.tagName.toLowerCase() === "stop")) if (parseFloat(style(s).getPropertyValue("stop-opacity")) < 0.999) return false;
  // Lengths in percent of the viewport are not supported, only fractions of the shape and plain numbers.
  const units = attrOf(g, "gradientUnits") ?? "objectBoundingBox";
  if (units === "userSpaceOnUse") for (const k of ["x1", "y1", "x2", "y2", "cx", "cy", "r", "fx", "fy"]) if (attrOf(g, k)?.includes("%")) return false;
  return true;
}

function stopsOf(g: Element): Element[] {
  for (const e of chain(g)) {
    const stops = Array.from(e.children).filter((c) => c.tagName.toLowerCase() === "stop");
    if (stops.length) return stops;
  }
  return [];
}

/** A length that may be a fraction ("50%" is 0.5 for a shape's own box, a number as it is). */
function fraction(v: string | null, fallback: number, bbox: boolean): number {
  if (v === null || v === "") return fallback;
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return fallback;
  return v.includes("%") ? (bbox ? n / 100 : n) : n;
}

export class SvgPainter {
  private host: SvgHost;
  constructor(host: SvgHost) {
    this.host = host;
  }

  // -- matrices ---------------------------------------------------------------------------------------------------------

  /** The matrix from an element's own space to the page's points. */
  private toPage(ctm: Matrix, page: number): Matrix {
    const sheet = this.host.sheet(page);
    const o = this.host.origin;
    // Client pixels to stage pixels, then the page: x to points from the page's left, y up from its bottom.
    const stage: Matrix = [ctm[0], ctm[1], ctm[2], ctm[3], ctm[4] - o.left, ctm[5] - o.top];
    return mul([PT, 0, 0, -PT, -PT * sheet.left, PT * (sheet.top + sheet.h)], stage);
  }

  // -- the walk ---------------------------------------------------------------------------------------------------------

  /** Draw every shape and piece of text of the drawing, in order. */
  draw(svg: SVGSVGElement, page: number): void {
    this.visit(svg, page);
  }

  private visit(parent: Element, page: number) {
    for (const el of Array.from(parent.children)) {
      const tag = el.tagName.toLowerCase();
      const cs = this.host.style(el);
      if (cs.display === "none") continue;
      if (tag === "defs" || tag === "title" || tag === "desc" || tag === "metadata" || tag === "style" || tag === "clippath" || tag === "lineargradient" || tag === "radialgradient") continue;
      if (SHAPES.has(tag)) this.shape(el as SVGGeometryElement, cs, page);
      else if (tag === "text") this.text(el as SVGTextElement);
      else if (GROUPS.has(tag)) this.visit(el, page);
    }
  }

  /** The opacity of the groups an element sits in, up to (not including) the drawing itself. */
  private groupOpacity(el: Element): number {
    let a = 1;
    for (let cur = el.parentElement; cur && !(cur instanceof SVGSVGElement && !cur.parentElement?.closest("svg")); cur = cur.parentElement) a *= parseFloat(this.host.style(cur).opacity) || 1;
    return a;
  }

  // -- shapes -----------------------------------------------------------------------------------------------------------

  private segments(el: SVGGeometryElement): Seg[] {
    const tag = el.tagName.toLowerCase();
    const n = (k: string) => parseFloat(el.getAttribute(k) ?? "0") || 0;
    switch (tag) {
      case "path":
        return parsePathData(el.getAttribute("d") ?? "") ?? [];
      case "rect": {
        const rx = el.hasAttribute("rx") ? n("rx") : n("ry");
        const ry = el.hasAttribute("ry") ? n("ry") : rx;
        return rectSegments(n("x"), n("y"), n("width"), n("height"), rx, ry);
      }
      case "circle":
        return ellipseSegments(n("cx"), n("cy"), n("r"), n("r"));
      case "ellipse": {
        const rx = n("rx") || n("ry");
        return ellipseSegments(n("cx"), n("cy"), rx, n("ry") || rx);
      }
      case "line":
        return polySegments([n("x1"), n("y1"), n("x2"), n("y2")], false);
      case "polyline":
        return polySegments(pointList(el.getAttribute("points") ?? ""), false);
      default:
        return polySegments(pointList(el.getAttribute("points") ?? ""), true);
    }
  }

  private color(cs: CSSStyleDeclaration, name: "fill" | "stroke"): Rgba | null {
    const c = parseColor(cs.getPropertyValue(name));
    return c && c[3] > 0 ? c : null;
  }

  private shape(el: SVGGeometryElement, cs: CSSStyleDeclaration, page: number) {
    if (cs.visibility !== "visible") return;
    const ctmDom = el.getScreenCTM();
    if (!ctmDom) return;
    const ctm = m6(ctmDom);
    const segs = this.segments(el);
    if (!segs.length) return;
    const matrix = this.toPage(ctm, page);
    const path = pathOps(transformSegments(segs, matrix));
    const group = this.groupOpacity(el);
    const evenodd = cs.fillRule === "evenodd";

    // Fill: a colour, or a gradient.
    const fillValue = cs.getPropertyValue("fill").trim();
    const fillOpacity = (parseFloat(cs.fillOpacity) || 0) * group;
    const gradient = /^url\(["']?#([^"')]+)["']?\)/.exec(fillValue);
    const fill = gradient ? null : this.color(cs, "fill");
    const strokeColor = this.color(cs, "stroke");
    const strokeOn = !!strokeColor && parseFloat(cs.strokeWidth) > 0;
    const strokeOpacity = (parseFloat(cs.strokeOpacity) || 0) * group;

    // Open paths are not filled by a line or polyline's rule, but SVG fills them (closing them for the fill only): do the same.
    if (fill || gradient) {
      const a = (fill ? fill[3] : 1) * fillOpacity;
      if (a > 0.001) {
        if (gradient) this.gradientFill(el, gradient[1]!, path, evenodd, matrix, a);
        else this.paint(`${num(fill![0])} ${num(fill![1])} ${num(fill![2])} rg ${path} ${evenodd ? "f*" : "f"}`, a);
      }
    }
    if (strokeOn && strokeColor) {
      const a = strokeColor[3] * strokeOpacity;
      if (a > 0.001) {
        const nonScaling = cs.getPropertyValue("vector-effect") === "non-scaling-stroke";
        // The width and dashes are in the element's units, and scale with it unless the shape says its stroke does not.
        const k = nonScaling ? 1 : scaleOf(ctm);
        const width = parseFloat(cs.strokeWidth) * k * PT;
        const caps: Record<string, number> = { butt: 0, round: 1, square: 2 };
        const joins: Record<string, number> = { miter: 0, round: 1, bevel: 2 };
        let dash = "";
        const arr = cs.getPropertyValue("stroke-dasharray");
        if (arr && arr !== "none") {
          const parts = arr.split(/[\s,]+/).map((v) => parseFloat(v) * k * PT).filter((v) => Number.isFinite(v) && v >= 0);
          if (parts.length && parts.some((v) => v > 0)) dash = `[${(parts.length % 2 ? [...parts, ...parts] : parts).map(num).join(" ")}] ${num((parseFloat(cs.getPropertyValue("stroke-dashoffset")) || 0) * k * PT)} d `;
        }
        const ops =
          `${num(strokeColor[0])} ${num(strokeColor[1])} ${num(strokeColor[2])} RG ${num(width)} w ${caps[cs.strokeLinecap] ?? 0} J ${joins[cs.strokeLinejoin] ?? 0} j ` +
          `${num(Math.max(1, parseFloat(cs.getPropertyValue("stroke-miterlimit")) || 4))} M ${dash}${path} S`;
        this.paint(ops, a);
      }
    }
  }

  /** Put operators on the page in a state of their own, at an opacity. */
  private paint(ops: string, alpha: number) {
    this.host.put(alpha < 0.999 ? `q /${this.host.alpha(alpha)} gs ${ops} Q` : `q ${ops} Q`);
  }

  private gradientFill(el: SVGGeometryElement, id: string, path: string, evenodd: boolean, toPageMatrix: Matrix, alpha: number) {
    const g = (el.ownerSVGElement?.querySelector(`#${CSS.escape(id)}`) ?? el.ownerDocument.getElementById(id)) as SVGGradientElement | null;
    if (!g) return;
    const radial = g.tagName.toLowerCase() === "radialgradient";
    const units = attrOf(g, "gradientUnits") ?? "objectBoundingBox";
    const bbox = units === "objectBoundingBox";
    let user: Matrix = [1, 0, 0, 1, 0, 0];
    if (bbox) {
      const b = el.getBBox();
      if (b.width <= 0 || b.height <= 0) return;
      user = [b.width, 0, 0, b.height, b.x, b.y];
    }
    // The gradient's own transform sits inside the box's.
    const gt = chain(g).find((e) => e.hasAttribute("gradientTransform"));
    if (gt) {
      const list = (gt as SVGGradientElement).gradientTransform.baseVal;
      const matrix = list.consolidate()?.matrix;
      if (matrix) user = mul(user, m6(matrix));
    }
    const stops: Stop[] = stopsOf(g).map((s) => {
      const cs = this.host.style(s);
      const offsetAttr = s.getAttribute("offset") ?? "0";
      const offset = offsetAttr.includes("%") ? parseFloat(offsetAttr) / 100 : parseFloat(offsetAttr);
      const c = parseColor(cs.getPropertyValue("stop-color")) ?? [0, 0, 0, 1];
      return { offset: Number.isFinite(offset) ? Math.min(1, Math.max(0, offset)) : 0, rgb: [c[0], c[1], c[2]] };
    });
    const f = (k: string, d: number) => fraction(attrOf(g, k), d, bbox);
    const dict = radial
      ? radialShading(f("fx", f("cx", bbox ? 0.5 : 0)), f("fy", f("cy", bbox ? 0.5 : 0)), f("cx", bbox ? 0.5 : 0), f("cy", bbox ? 0.5 : 0), f("r", bbox ? 0.5 : 0), stops)
      : axialShading(f("x1", 0), f("y1", 0), f("x2", bbox ? 1 : 0), f("y2", 0), stops);
    const name = this.host.shading(dict);
    const cm = mul(toPageMatrix, user);
    // Clip to the shape, then paint the shading through the matrix that takes the gradient's space to the page.
    const ops = `${path} ${evenodd ? "W*" : "W"} n ${cm.map(num).join(" ")} cm /${name} sh`;
    this.paint(ops, alpha);
  }

  // -- text -------------------------------------------------------------------------------------------------------------

  private text(el: SVGTextElement) {
    const cs = this.host.style(el);
    if (cs.visibility !== "visible") return;
    const full = el.getScreenCTM();
    if (!full) return;
    // Turned or moved text is measured upright, in the drawing's own frame, and the turn is put on the page.
    const turned: { el: SVGElement; before: string }[] = [];
    for (let cur: Element | null = el; cur && !(cur instanceof SVGSVGElement); cur = cur.parentElement) {
      if (cur instanceof SVGElement && cur.hasAttribute("transform")) {
        turned.push({ el: cur, before: cur.style.transform });
        cur.style.transform = "none";
      }
    }
    try {
      const upright = el.getScreenCTM();
      if (!upright) return;
      const frame = m6(upright);
      const inverse = invert(frame);
      let cm: Matrix | null = null;
      if (turned.length && inverse) {
        // From the upright frame to where the text really is, in client pixels; then in the stage's, then the page's.
        const client = mul(m6(full), inverse);
        const o = this.host.origin;
        const stage: Matrix = [client[0], client[1], client[2], client[3], client[0] * o.left + client[2] * o.top + client[4] - o.left, client[1] * o.left + client[3] * o.top + client[5] - o.top];
        cm = this.host.pdfMatrix(stage);
      }
      if (cm) this.host.put(`q ${cm.map(num).join(" ")} cm`, true);
      const scale = scaleOf(frame);
      const walker = el.ownerDocument.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const parent = (n as Text).parentElement;
        if (parent && (n as Text).data.trim() !== "") this.host.text(n as Text, this.host.style(parent), scale);
      }
      if (cm) this.host.put("Q", true);
    } finally {
      for (const t of turned) t.el.style.transform = t.before;
    }
  }
}
