// Several selections at once. A browser keeps one DOM selection, so Docxtor keeps the others itself: the real
// selection stays the primary one (the toolbar, the bubble and `selection()` follow it) and the rest are a
// list of model ranges in `ExtraSelection`, drawn on a layer over the stage.
//
// Like the non-printing characters (marks.ts) and the spelling underlines (spell-layer.ts), the layer sits
// outside the editable flow: nothing in the text, the layout or the model changes, and printing leaves it
// out. The ranges are model positions, so they survive a reflow; the layer is drawn again after every layout.
// A range with no length is a caret: after typing over several ranges, each one carries on as a caret.
//
// This file also holds the pure parts of editing every range at once: finding the matches of a text, and
// building the ops (last range first, so earlier positions stay valid) with the carets they leave.
import type { MPos, Op, RunPatch } from "@docxtor/core";
import { isFar } from "./far";
import { isZeroWidth, modelToDom, paraLength, textBefore } from "./selection";

/** A selected stretch of the text, `from` before `to` in document order (equal for a caret). */
export interface SelRange {
  from: MPos;
  to: MPos;
}

export const isEmpty = (r: SelRange) => r.from.p === r.to.p && r.from.o === r.to.o;
export const samePos = (a: MPos, b: MPos) => a.p === b.p && a.o === b.o;
export const sameRange = (a: SelRange, b: SelRange) => samePos(a.from, b.from) && samePos(a.to, b.to);

/** A paragraph's text as the model counts it: list labels and spacers left out, an object as one character. */
export function paraText(p: HTMLElement): string {
  let text = "";
  const walk = (n: Node) => {
    for (const c of Array.from(n.childNodes)) {
      if (c.nodeType === Node.TEXT_NODE) text += (c as Text).data;
      else if (c.nodeType === Node.ELEMENT_NODE) {
        const el = c as Element;
        if (isZeroWidth(el)) continue;
        if (el.classList.contains("atom")) text += "￼";
        else walk(el);
      }
    }
  };
  walk(p);
  return text;
}

/** Every place `needle` occurs in the paragraphs of `root` (case-sensitive, not overlapping), in document order. */
export function findAll(root: HTMLElement, needle: string): SelRange[] {
  const out: SelRange[] = [];
  if (!needle) return out;
  for (const el of Array.from(root.querySelectorAll<HTMLElement>("[data-p]"))) {
    const text = paraText(el);
    const p = Number(el.dataset.p);
    for (let i = text.indexOf(needle); i >= 0; i = text.indexOf(needle, i + needle.length)) out.push({ from: { p, o: i }, to: { p, o: i + needle.length } });
  }
  return out;
}

/** Put ranges in document order (`index` says where each paragraph is) and drop those in paragraphs that are gone. */
export function sortRanges(index: Map<number, number>, ranges: SelRange[]): SelRange[] {
  const at = (m: MPos) => index.get(m.p);
  return ranges
    .filter((r) => at(r.from) !== undefined && at(r.to) !== undefined)
    .sort((a, b) => at(a.from)! - at(b.from)! || a.from.o - b.from.o);
}

/** Drop the ranges that overlap an earlier one (the input is in document order). */
export function disjoint(index: Map<number, number>, ranges: SelRange[]): SelRange[] {
  const out: SelRange[] = [];
  for (const r of ranges) {
    const last = out[out.length - 1];
    if (last) {
      const lp = index.get(last.to.p)!;
      const rp = index.get(r.from.p)!;
      if (rp < lp || (rp === lp && r.from.o < last.to.o)) continue;
    }
    out.push(r);
  }
  return out;
}

/** Where a position at or after `b` ends up when `a`..`b` is replaced by `len` characters. */
export function mapAfter(x: MPos, a: MPos, b: MPos, len: number): MPos {
  return x.p === b.p ? { p: a.p, o: a.o + len + x.o - b.o } : x;
}

/**
 * The ops that replace each range (in document order) with `text`, or only delete them when it is empty, and
 * the caret each one leaves, in the same order. The last range goes first, so the others keep their positions.
 */
export function replaceOps(ranges: SelRange[], text: string, format?: RunPatch | null): { ops: Op[]; carets: MPos[] } {
  const ops: Op[] = [];
  for (let i = ranges.length - 1; i >= 0; i--) {
    const r = ranges[i];
    if (!isEmpty(r)) ops.push({ op: "delete", from: r.from, to: r.to });
    if (text) ops.push({ op: "insert_text", at: r.from, text, ...(format ? { format } : {}) });
  }
  const carets = ranges.map((r, i) => {
    let x: MPos = { p: r.from.p, o: r.from.o + text.length };
    for (let j = i - 1; j >= 0; j--) x = mapAfter(x, ranges[j].from, ranges[j].to, text.length);
    return x;
  });
  return { ops, carets };
}

/** The ranges to delete for Backspace (or Delete, `forward`): a selected range as it is, a caret takes a character. */
export function reachOf(root: HTMLElement, ranges: SelRange[], forward: boolean): SelRange[] {
  return ranges.map((r) => {
    if (!isEmpty(r)) return r;
    const at = r.from;
    if (forward) {
      const len = paraLength(root, at.p);
      if (at.o >= len) return r;
      const s = textBefore(root, { p: at.p, o: Math.min(len, at.o + 2) }, 2);
      return { from: at, to: { p: at.p, o: at.o + (s.length === 2 && /[\uD800-\uDBFF]/.test(s[0]) ? 2 : 1) } };
    }
    if (at.o <= 0) return r;
    const s = textBefore(root, at, 2);
    return { from: { p: at.p, o: at.o - (s.length === 2 && /[\uDC00-\uDFFF]/.test(s[1]) && /[\uD800-\uDBFF]/.test(s[0]) ? 2 : 1) }, to: at };
  });
}

// ---------------------------------------------------------------------------
// The layer
// ---------------------------------------------------------------------------

export interface ExtraSelectionHost {
  stage: HTMLElement;
  /** The element taking input now (the flow, or a header, footer or notes area). */
  root(): HTMLElement;
}

export class ExtraSelection {
  readonly el: HTMLElement;
  private list: SelRange[] = [];
  private frame = 0;
  private scratch = document.createRange();
  /** Called when the set of ranges changes (so the editor can tell whoever is interested). */
  onChange: (() => void) | null = null;

  constructor(private host: ExtraSelectionHost) {
    this.el = document.createElement("div");
    this.el.className = "xs-layer";
    this.el.setAttribute("aria-hidden", "true");
    host.stage.append(this.el);
  }

  /** The other selected ranges (not the real selection), in document order. */
  get ranges(): readonly SelRange[] {
    return this.list;
  }

  /** Show these ranges. The caller keeps them in document order and apart from the real selection. */
  set(ranges: SelRange[]) {
    const had = this.list.length;
    this.list = ranges;
    window.cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.draw();
    if (had || ranges.length) this.onChange?.();
  }

  clear() {
    if (!this.list.length) return;
    this.set([]);
  }

  /** The text moved, changed or was zoomed: draw again in the next frame. */
  invalidate() {
    if (!this.list.length || this.frame) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      this.draw();
    });
  }

  destroy() {
    window.cancelAnimationFrame(this.frame);
    this.el.remove();
  }

  /** Whether a point on screen is on one of the drawn ranges. */
  hit(x: number, y: number): boolean {
    return Array.from(this.el.children).some((c) => {
      const r = c.getBoundingClientRect();
      return x >= r.left - 1 && x <= r.right + 1 && y >= r.top - 1 && y <= r.bottom + 1;
    });
  }

  private draw() {
    if (!this.list.length) {
      this.el.replaceChildren();
      return;
    }
    const root = this.host.root();
    const s = this.host.stage.getBoundingClientRect();
    const sc = s.width / (this.host.stage.offsetWidth || 1) || 1;
    const frag = document.createDocumentFragment();
    const put = (cls: string, r: DOMRect, r0: SelRange) => {
      const el = document.createElement("div");
      el.className = cls;
      el.dataset.p = String(r0.from.p);
      el.dataset.from = String(r0.from.o);
      el.dataset.to = String(r0.to.o);
      el.style.cssText = `left:${((r.left - s.left) / sc).toFixed(1)}px;top:${((r.top - s.top) / sc).toFixed(1)}px;width:${(r.width / sc).toFixed(1)}px;height:${(r.height / sc).toFixed(1)}px`;
      frag.append(el);
    };
    for (const r of this.list) {
      if (isEmpty(r)) {
        const c = this.caretRect(root, r.from);
        if (c) put("xs-caret", c, r);
        continue;
      }
      for (const box of this.boxes(root, r)) put("xs-r", box, r);
    }
    this.el.replaceChildren(frag);
  }

  /** The rectangles of a range, one per line of text; a range over several paragraphs is read paragraph by paragraph. */
  private boxes(root: HTMLElement, r: SelRange): DOMRect[] {
    const pieces: SelRange[] = [];
    if (r.from.p === r.to.p) pieces.push(r);
    else {
      let on = false;
      for (const el of Array.from(root.querySelectorAll<HTMLElement>("[data-p]"))) {
        const p = Number(el.dataset.p);
        if (p === r.from.p) {
          on = true;
          pieces.push({ from: r.from, to: { p, o: paraLength(root, p) } });
        } else if (p === r.to.p) {
          pieces.push({ from: { p, o: 0 }, to: r.to });
          break;
        } else if (on && !isFar(el)) pieces.push({ from: { p, o: 0 }, to: { p, o: paraLength(root, p) } });
      }
    }
    const out: DOMRect[] = [];
    for (const piece of pieces) {
      const a = modelToDom(root, piece.from);
      const b = modelToDom(root, piece.to);
      // An end in a block left out of the layout would read zeros (the pages of every range's ends stay drawn, so this is only a safeguard).
      if (!a || !b || isFar(a.node.parentElement) || isFar(b.node.parentElement)) continue;
      try {
        this.scratch.setStart(a.node, a.offset);
        this.scratch.setEnd(b.node, b.offset);
      } catch {
        continue;
      }
      const rects = Array.from(this.scratch.getClientRects()).filter((x) => x.width >= 0.5 && x.height >= 0.5);
      // An element inside the range reports its own box too: keep only the rectangles nothing else contains, or the wash would be drawn twice.
      for (const x of rects) {
        const covered = rects.some((y) => y !== x && y.left <= x.left + 0.5 && y.right >= x.right - 0.5 && y.top <= x.top + 0.5 && y.bottom >= x.bottom - 0.5 && (y.width > x.width + 0.5 || y.height > x.height + 0.5));
        if (!covered && !out.some((y) => Math.abs(y.left - x.left) < 0.5 && Math.abs(y.top - x.top) < 0.5 && Math.abs(y.width - x.width) < 0.5 && Math.abs(y.height - x.height) < 0.5)) out.push(x);
      }
    }
    return out;
  }

  /** A thin box where a caret at `at` would be drawn. */
  private caretRect(root: HTMLElement, at: MPos): DOMRect | null {
    const dom = modelToDom(root, at);
    if (!dom || isFar(dom.node.nodeType === Node.ELEMENT_NODE ? (dom.node as Element) : dom.node.parentElement)) return null;
    try {
      this.scratch.setStart(dom.node, dom.offset);
      this.scratch.setEnd(dom.node, dom.offset);
    } catch {
      return null;
    }
    let r = Array.from(this.scratch.getClientRects()).find((x) => x.height > 0) ?? (this.scratch.getBoundingClientRect().height > 0 ? this.scratch.getBoundingClientRect() : null);
    let x = r?.left ?? 0;
    if (!r && at.o > 0) {
      // No box for the point itself (the end of a line): the right edge of the character before it.
      const before = modelToDom(root, { p: at.p, o: at.o - 1 });
      if (before) {
        try {
          this.scratch.setStart(before.node, before.offset);
          this.scratch.setEnd(dom.node, dom.offset);
          r = Array.from(this.scratch.getClientRects()).reverse().find((y) => y.height > 0) ?? null;
          x = r?.right ?? 0;
        } catch {
          r = null;
        }
      }
    }
    if (!r) return null;
    return new DOMRect(x, r.top, 1.5, r.height);
  }
}
