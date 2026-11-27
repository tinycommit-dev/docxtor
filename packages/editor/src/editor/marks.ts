// Non-printing characters: ¶ at the end of paragraphs, a dot for each space, an arrow for each tab, ↵ for
// line breaks and a labelled dotted rule for page, column and section breaks.
//
// The marks are drawn on a layer of their own that sits over the stage, outside the editable flow and
// outside the sheets. Nothing in the text changes, so pagination, selection mapping, copying and the
// engine never see them, and printing leaves the layer out. The marks come from where the browser put
// the text (Range rects), for the part of the document in view and a screen above and below it, redrawn
// in the next frame after a layout, an edit, a zoom or a scroll out of the drawn part.
import { t } from "@docxtor/extensions";
import { isFar } from "./far";

export interface MarksHost {
  /** The scrolling area, the stage that holds sheets, text and this layer, and the editable flow. */
  canvas: HTMLElement;
  stage: HTMLElement;
  flow: HTMLElement;
  sheets: HTMLElement;
  /** The top-level blocks that may reach into a band of the stage, from the page map (undefined: not known, look through the flow). */
  blocksBetween?(y0: number, y1: number): HTMLElement[] | undefined;
  /** The right edge (stage px) of the sheet under a stage y. */
  pageRight(y: number): number;
}

/** How a section starts, as the page setup names it, and the words for its break. */
const SECTION_BREAKS: Record<string, () => string> = {
  nextPage: () => t("Section break (next page)"),
  continuous: () => t("Section break (continuous)"),
  evenPage: () => t("Section break (even page)"),
  oddPage: () => t("Section break (odd page)"),
  nextColumn: () => t("Section break (next column)"),
};

interface Point {
  x: number;
  y: number;
  h: number;
}

const SVG = "http://www.w3.org/2000/svg";

/** The paragraphs of the body, headers, footers and notes that reach into [y0, y1] of the stage. */
export function paragraphsBetween(flow: HTMLElement, sheets: HTMLElement, y0: number, y1: number, between?: (y0: number, y1: number) => HTMLElement[] | undefined): HTMLElement[] {
  const out: HTMLElement[] = [];
  // The page map knows which blocks are on the pages the band touches: those are looked at, and only those that are drawn (a block left out of the layout reads zeros).
  const mapped = between?.(y0, y1);
  if (mapped) {
    for (const k of mapped) {
      if (isFar(k) || k.offsetTop + k.offsetHeight < y0 || k.offsetTop > y1) continue;
      if (k.classList.contains("fp")) out.push(k);
      else out.push(...Array.from(k.querySelectorAll<HTMLElement>(".fp")));
    }
  } else {
    const kids = flow.children;
    let lo = 0;
    let hi = kids.length - 1;
    // The blocks stack in order: find the first one whose bottom is below y0.
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const k = kids[mid] as HTMLElement;
      if (k.offsetTop + k.offsetHeight >= y0) hi = mid;
      else lo = mid + 1;
    }
    for (let i = lo; i < kids.length; i++) {
      const k = kids[i] as HTMLElement;
      if (k.offsetTop > y1) break;
      if (k.classList.contains("fp")) out.push(k);
      else out.push(...Array.from(k.querySelectorAll<HTMLElement>(".fp")));
    }
  }
  // The sheets stack in order too (a row of them side by side shares a top): bisect, then step back a little for the row the bisection may have landed in the middle of. Walking every sheet (a novel has 1,400) is a cost per call that grows with the document.
  const pages = sheets.children;
  let a = 0;
  let b = pages.length - 1;
  while (a < b) {
    const mid = (a + b) >> 1;
    const k = pages[mid] as HTMLElement;
    if (k.offsetTop + k.offsetHeight >= y0) b = mid;
    else a = mid + 1;
  }
  for (let i = Math.max(0, a - 3); i < pages.length; i++) {
    const sheet = pages[i] as HTMLElement;
    if (sheet.offsetTop > y1) break;
    if (sheet.offsetTop + sheet.offsetHeight < y0) continue;
    out.push(...Array.from(sheet.querySelectorAll<HTMLElement>(".hf .fp, .notes .fp")));
  }
  return out;
}

export class NonPrinting {
  readonly el: HTMLElement;
  private on = false;
  /** Only the labelled rules for page, column and section breaks (the Draft view shows these always). */
  private breaks = false;
  private frame = 0;
  /** The stretch of the stage (y, px) that is drawn now. */
  private built: [number, number] | null = null;
  private range = document.createRange();

  constructor(private host: MarksHost) {
    this.el = document.createElement("div");
    this.el.className = "np-layer";
    this.el.setAttribute("aria-hidden", "true");
    this.el.hidden = true;
    host.stage.append(this.el);
  }

  get shown(): boolean {
    return this.on;
  }

  show(on: boolean) {
    if (on === this.on) return;
    this.on = on;
    this.host.canvas.classList.toggle("show-marks", on);
    this.sync();
  }

  /** Show only the rules for breaks, without the other marks (they are all there while the marks are on). */
  showBreaks(on: boolean) {
    if (on === this.breaks) return;
    this.breaks = on;
    this.sync();
  }

  private sync() {
    const visible = this.on || this.breaks;
    this.el.hidden = !visible;
    window.cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.built = null;
    if (visible) this.invalidate();
    else this.el.replaceChildren();
  }

  /** Draw again in the next frame (the text moved, changed or was zoomed). */
  invalidate() {
    if (!this.on && !this.breaks) return;
    this.built = null;
    if (this.frame) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      this.redraw();
    });
  }

  /** The view scrolled: draw again when it has left the drawn stretch. */
  scrolled() {
    if ((!this.on && !this.breaks) || this.frame) return;
    const v = this.view();
    const h = v.bottom - v.top;
    if (this.built && v.top > this.built[0] + h * 0.1 && v.bottom < this.built[1] - h * 0.1) return;
    this.invalidate();
  }

  private view() {
    const c = this.host.canvas.getBoundingClientRect();
    const s = this.host.stage.getBoundingClientRect();
    const sc = s.width / (this.host.stage.offsetWidth || 1) || 1;
    return { top: (c.top - s.top) / sc, bottom: (c.bottom - s.top) / sc, s, sc };
  }

  redraw() {
    if (!this.on && !this.breaks) return;
    const v = this.view();
    const h = v.bottom - v.top;
    const y0 = Math.max(0, v.top - h * 0.5);
    const y1 = v.bottom + h * 0.5;
    const ctx = new Draw(this.host, this.range, v.s, v.sc, y0, !this.on);
    for (const fp of paragraphsBetween(this.host.flow, this.host.sheets, y0, y1, this.host.blocksBetween)) {
      const r = fp.getBoundingClientRect();
      if ((r.bottom - v.s.top) / v.sc < y0 || (r.top - v.s.top) / v.sc > y1) continue;
      ctx.paragraph(fp);
    }
    this.el.replaceChildren(ctx.finish(this.host.stage.offsetWidth, y1 - y0));
    this.built = [y0, y1];
  }
}

/** One drawing pass: reads where the text is and collects the marks. */
class Draw {
  private frag = document.createDocumentFragment();
  /** Dot strokes by width: the paths hold a zero-length stroke for each space. */
  private dots = new Map<number, string[]>();
  private count = 0;

  constructor(
    private host: MarksHost,
    private range: Range,
    private s: DOMRect,
    private sc: number,
    private y0: number,
    /** Only the rules for breaks. */
    private rulesOnly = false,
  ) {}

  private x = (px: number) => (px - this.s.left) / this.sc;
  private y = (px: number) => (px - this.s.top) / this.sc;

  private box(r: DOMRect): Point {
    return { x: this.x(r.left), y: this.y(r.top), h: r.height / this.sc };
  }

  private mark(cls: string, text: string, p: Point, size: number, left = p.x): HTMLElement {
    const el = document.createElement("span");
    el.className = `np-m ${cls}`;
    el.textContent = text;
    el.style.cssText = `left:${left.toFixed(1)}px;top:${p.y.toFixed(1)}px;height:${p.h.toFixed(1)}px;line-height:${p.h.toFixed(1)}px;font-size:${size.toFixed(1)}px`;
    this.frag.append(el);
    return el;
  }

  /** The rect of one character of a text node, if it takes room on a line. */
  private char(text: Text, i: number): DOMRect | null {
    this.range.setStart(text, i);
    this.range.setEnd(text, i + 1);
    const r = this.range.getBoundingClientRect();
    return r.height > 0 ? r : null;
  }

  /** Where text would continue right after `prev` on its line (the start of the content when nothing comes before). */
  private after(prev: Element | null, fp: HTMLElement, cs: CSSStyleDeclaration, fs: number): Point {
    if (prev instanceof HTMLElement && prev.classList.contains("pgsp")) return this.after(prev.previousElementSibling, fp, cs, fs);
    if (prev) {
      const text = prev.classList.contains("s") ? prev.firstChild : null;
      if (text instanceof Text && text.data.length) {
        const c = this.char(text, text.data.length - 1);
        if (c) {
          const p = this.box(c);
          // After a line break the next position is the start of the next line.
          if (text.data.endsWith("\n")) return { ...this.start(fp, cs, fs, false), y: p.y + this.lineStep(cs, p.h), h: p.h };
          return { x: this.x(c.right), y: p.y, h: p.h };
        }
      }
      const r = prev.getBoundingClientRect();
      if (r.height > 0) return { x: this.x(r.right), y: this.y(r.top), h: r.height / this.sc };
    }
    return this.start(fp, cs, fs, true);
  }

  private lineStep(cs: CSSStyleDeclaration, fallback: number): number {
    const lh = parseFloat(cs.lineHeight);
    return Number.isFinite(lh) && lh > 0 ? lh : fallback;
  }

  /** The start of a paragraph's text (where an empty paragraph shows its mark), by its alignment. */
  private start(fp: HTMLElement, cs: CSSStyleDeclaration, fs: number, first: boolean): Point {
    const r = fp.getBoundingClientRect();
    const left = this.x(r.left) + (parseFloat(cs.borderLeftWidth) || 0) + (parseFloat(cs.paddingLeft) || 0);
    const right = this.x(r.right) - (parseFloat(cs.borderRightWidth) || 0) - (parseFloat(cs.paddingRight) || 0);
    const lh = this.lineStep(cs, fs * 1.2);
    const top = this.y(r.top) + (parseFloat(cs.borderTopWidth) || 0) + (parseFloat(cs.paddingTop) || 0);
    const label = fp.querySelector<HTMLElement>(":scope > .lbl");
    if (label && first) {
      const l = label.getBoundingClientRect();
      return { x: this.x(l.right), y: this.y(l.top), h: l.height / this.sc };
    }
    let x = left + (first ? parseFloat(cs.textIndent) || 0 : 0);
    if (cs.textAlign === "center") x = (left + right) / 2;
    else if (cs.textAlign === "right" || cs.textAlign === "end") x = right - fs * 0.6;
    return { x, y: top, h: lh };
  }

  paragraph(fp: HTMLElement) {
    const cs = getComputedStyle(fp);
    const fs = parseFloat(cs.fontSize) || 14.67;
    const rect = fp.getBoundingClientRect();
    const right = this.x(rect.right) - (parseFloat(cs.borderRightWidth) || 0) - (parseFloat(cs.paddingRight) || 0);
    const stroke = Math.max(1, Math.min(2.4, fs * 0.09));
    for (const span of this.rulesOnly ? [] : Array.from(fp.querySelectorAll<HTMLElement>(".s"))) {
      const text = span.firstChild;
      if (!(text instanceof Text)) continue;
      const d = text.data;
      if (d === "\t") {
        const c = this.char(text, 0);
        if (c) this.mark("np-tab", "→", this.box(c), fs * 0.8, this.x(c.left) + 1);
      } else if (d === "\n") {
        const c = this.char(text, 0);
        if (c) this.mark("np-br", "↵", this.box(c), fs * 0.8, this.x(c.left) + 1);
      } else {
        for (let i = d.indexOf(" "); i >= 0; i = d.indexOf(" ", i + 1)) {
          const c = this.char(text, i);
          if (!c || c.width < 0.5) continue;
          // Centred on the space, at the height of a middle dot.
          const x = this.x(c.left + c.width / 2);
          const y = this.y(c.top + c.height * 0.58) - this.y0;
          const w = stroke;
          let path = this.dots.get(w);
          if (!path) this.dots.set(w, (path = []));
          path.push(`M${x.toFixed(1)} ${y.toFixed(1)}h0`);
          this.count++;
        }
      }
    }
    for (const atom of Array.from(fp.querySelectorAll<HTMLElement>(".atom.pbr, .atom.cbr"))) {
      const column = atom.classList.contains("cbr");
      const at = this.after(atom.previousElementSibling, fp, cs, fs);
      this.rule(column ? "np-column" : "np-page", column ? "column" : "page", column ? t("Column break") : t("Page break"), at, right, fp, 1);
    }
    // The paragraph mark, after the last content (a trailing placeholder <br> is not content).
    let last = fp.lastElementChild;
    while (last && (last.classList.contains("pgsp") || (last.tagName === "BR" && (last as HTMLElement).dataset.ph === "1"))) last = last.previousElementSibling;
    const end = this.after(last, fp, cs, fs);
    const pm = this.rulesOnly ? null : this.mark("np-para", "¶", end, fs, end.x + 1);
    const sb = fp.dataset.sb;
    const after: Point = { ...end, x: end.x + fs * 0.6 + 4 };
    if (sb && SECTION_BREAKS[sb]) {
      this.rule("np-sect", sb, SECTION_BREAKS[sb](), after, right, fp, 2);
      if (pm) pm.dataset.sb = sb;
    } else if ((fp.nextElementSibling as HTMLElement | null)?.dataset.pbb) {
      // Insert, Break, Page break makes the next paragraph start a page; that is where the page ends.
      this.rule("np-page", "page", t("Page break"), after, right, fp, 1);
    }
  }

  /** A dotted rule (two for a section break) with its label, from `at` to the end of the text, or the page when the text is too narrow. */
  private rule(cls: string, kind: string, label: string, at: Point, right: number, fp: HTMLElement, lines: 1 | 2) {
    const x0 = at.x + 6;
    // The label is about 6.5px a letter at 11px; the rule needs some room on each side.
    const need = label.length * 6.5 + 40;
    let x1 = right;
    if (x1 - x0 < need && !fp.closest(".cols, td")) x1 = Math.max(x1, this.host.pageRight(at.y) - 12);
    if (x1 - x0 < 24) return;
    const el = document.createElement("div");
    el.className = `np-brk ${cls}`;
    el.dataset.kind = kind;
    el.style.cssText = `left:${x0.toFixed(1)}px;top:${at.y.toFixed(1)}px;width:${(x1 - x0).toFixed(1)}px;height:${at.h.toFixed(1)}px`;
    const side = () => {
      const i = document.createElement("i");
      i.className = lines === 2 ? "double" : "single";
      return i;
    };
    const text = document.createElement("b");
    text.textContent = label;
    el.append(side(), text, side());
    this.frag.append(el);
  }

  finish(width: number, height: number): DocumentFragment {
    if (this.count) {
      const svg = document.createElementNS(SVG, "svg");
      svg.setAttribute("class", "np-dots");
      svg.setAttribute("width", String(width));
      svg.setAttribute("height", String(Math.ceil(height)));
      svg.style.top = `${this.y0}px`;
      svg.dataset.n = String(this.count);
      for (const [w, d] of this.dots) {
        const path = document.createElementNS(SVG, "path");
        path.setAttribute("d", d.join(""));
        path.setAttribute("stroke-width", String(w));
        svg.append(path);
      }
      this.frag.prepend(svg);
    }
    return this.frag;
  }
}
