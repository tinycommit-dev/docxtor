// Measuring: line counts per block, the lines and glyphs of a paragraph, and where a page mark goes.
import type { Paginator } from "./layout";
import type { BlockLines, BlockRec } from "./layout-types";
import { FAR, isFar, lastBlock, nextBlock, prevBlock, topBlockOf } from "./far";
import { linesIn, paragraphsOf } from "./lines";
import { scratchRange } from "./layout-units";

export const measureMethods = {
  /** The top-level blocks the pages are made of, in order (a wrapper of columns is one; the blocks inside a content control are each one). */
  pageBlockList(this: Paginator): HTMLElement[] {
    return this.pageBlocks();
  },
  countBlock(this: Paginator, el: HTMLElement): BlockLines {
    const out: BlockLines = { ids: [], counts: [], main: 0, boxes: 0 };
    // A block folded away or in a tab not shown shows no lines.
    if (el.dataset.fold || el.closest("[data-tab-hidden]")) return out;
    if (isFar(el)) this.drawFar?.(el);
    for (const fp of paragraphsOf(el)) {
      const n = linesIn(fp);
      out.ids.push(fp.dataset.p === undefined ? -1 : Number(fp.dataset.p));
      out.counts.push(n);
      if (fp.closest(".shape-text")) out.boxes += n;
      else out.main += n;
    }
    return out;
  },
  /**
   * The lines the paragraphs of a top-level block show: what the page map recorded when it measured the block, else counted from the
   * page (the block must be drawn, which is asked for first) and recorded. A page-sized change that has not been laid out yet (the map is
   * not ready) is counted from the page and not recorded.
   */
  blockLines(this: Paginator, el: HTMLElement): BlockLines {
    this.wantLines = true;
    const rec = this.mapReady ? this.recs.get(el) : undefined;
    if (rec?.lines) return rec.lines;
    const out = this.countBlock(el);
    if (rec) rec.lines = out;
    return out;
  },
  /** The lines of the text of the whole document (and of the boxes in it), from the page map. */
  lineTotals(this: Paginator): { main: number; boxes: number } {
    const out = { main: 0, boxes: 0 };
    for (const el of this.pageBlocks()) {
      const l = this.blockLines(el);
      out.main += l.main;
      out.boxes += l.boxes;
    }
    return out;
  },
  /** Record the lines of the blocks this layout measured, when something wants them (so a block that leaves the layout later still has its count). */
  recordLines(this: Paginator) {
    if (!this.wantLines) return;
    for (const el of this.reset) {
      const r = this.recs.get(el);
      if (r && !r.lines && el.isConnected) r.lines = this.countBlock(el);
    }
  },
  /**
   * The top-level blocks that may reach into the band [y0, y1] of the stage, from the page map: those of the pages the band touches, and the block
   * before them (it may run on to the first). Undefined when the map is not up to date, or the text is one unbroken sheet.
   */
  blocksBetween(this: Paginator, y0: number, y1: number): HTMLElement[] | undefined {
    if (!this.mapReady || this.cont) return undefined;
    return this.blocksOnPages(this.firstPageBelow(y0), this.pageAt(y1));
  },
  /** The top-level blocks of pages `k0` to `k1` (the block before them too: it may run on to the first), from the page map. */
  blocksOnPages(this: Paginator, k0: number, k1: number): HTMLElement[] {
    const flowChild = (b: HTMLElement | undefined): HTMLElement | null => topBlockOf(this.flow, b ?? null);
    const start = flowChild(this.firstOnPage[Math.max(0, k0)]);
    const end = flowChild(this.firstOnPage[k1 + 1]);
    const out: HTMLElement[] = [];
    let c: HTMLElement | null = start ? (prevBlock(start) ?? start) : lastBlock(this.flow);
    // The first block of a page that starts nothing is the first of a later one: the blocks that are on it come before.
    for (; c && c !== end; c = nextBlock(c)) out.push(c);
    return out;
  },
  /** The page-map record of the top-level block that holds `el` (the block itself, or the one it is in, a content control's blocks included). */
  recOf(this: Paginator, el: Element): BlockRec | undefined {
    for (let n: Element | null = el; n && n !== this.flow; n = n.parentElement) {
      const r = this.recs.get(n as HTMLElement);
      if (r) return r;
    }
    return undefined;
  },
  /** Draw, in one go, the blocks left out of the layout that hold any of `els` the page map cannot place (see `markPage`). */
  drawFarFor(this: Paginator, els: HTMLElement[]) {
    if (!this.farN || !this.drawFar) return;
    let want: Set<HTMLElement> | null = null;
    for (const el of els) {
      if (this.recOf(el)?.whole) continue;
      const block = topBlockOf(this.flow, el);
      if (block?.hasAttribute(FAR)) (want ??= new Set()).add(block);
    }
    if (want) this.farDrawAll(want);
  },
  /**
   * The page something inside the text is on (a footnote reference, a page field). The page map answers when the block holding it sits whole
   * on one page; a block cut by a page, a table cut by a page or a wrapper of columns has to be measured, and is drawn first.
   */
  markPage(this: Paginator, el: HTMLElement, top: number, sc: number): number {
    const r = this.recOf(el);
    if (r?.whole) {
      // In tests every answer the map gives is compared with the page's.
      if (this.verify && !isFar(el)) {
        const measured = this.pageAt((el.getBoundingClientRect().top - top) / sc);
        if (measured !== r.page) throw new Error(`The page map puts a mark on page ${r.page}, the page has it on ${measured} (${this.lastPlan})`);
      }
      return r.page;
    }
    if (this.drawFar) {
      const block = topBlockOf(this.flow, el);
      if (block) this.drawFar(block);
    }
    return this.pageAt((el.getBoundingClientRect().top - top) / sc);
  },
  /**
   * How much the flow is drawn larger than it is laid out (the zoom). It is the editor's own figure, exact: the ratio of a
   * rectangle to `offsetHeight` (a whole pixel) is off by up to half a pixel over the flow's height, so a page end worked out far
   * down a long document would land that far off. The measured ratio is the answer where there is no editor to ask.
   */
  scale(this: Paginator): number {
    const z = this.zoomOf?.();
    if (typeof z === "number" && z > 0) return z;
    const r = this.flow.getBoundingClientRect();
    return this.flow.offsetHeight ? r.height / this.flow.offsetHeight : 1;
  },
  /** Visual line boxes of an element, in flow coordinates (live DOM). */
  lines(this: Paginator, el: HTMLElement): { top: number; bottom: number }[] {
    const range = scratchRange("lines");
    range.selectNodeContents(el);
    return this.groupLines(this.outsideShapes(el, Array.from(range.getClientRects())));
  },
  /**
   * The text inside a shape or text box is not a line of the paragraph that holds it, and neither is a shape that
   * floats or is placed over the text: leave their boxes out of the paragraph's lines.
   */
  outsideShapes(this: Paginator, el: HTMLElement, rects: DOMRect[]): DOMRect[] {
    const shapes = Array.from(el.querySelectorAll<HTMLElement>(".atom.shape"));
    if (!shapes.length) return rects;
    const boxes = shapes.map((a) => ({ r: a.getBoundingClientRect(), flow: a.dataset.wrap === "inline" || a.dataset.wrap === "topBottom" }));
    const same = (a: DOMRect, b: DOMRect) => Math.abs(a.left - b.left) < 0.6 && Math.abs(a.top - b.top) < 0.6 && Math.abs(a.right - b.right) < 0.6 && Math.abs(a.bottom - b.bottom) < 0.6;
    const within = (a: DOMRect, b: DOMRect) => a.left >= b.left - 0.6 && a.right <= b.right + 0.6 && a.top >= b.top - 0.6 && a.bottom <= b.bottom + 0.6;
    return rects.filter((r) => !boxes.some((b) => (same(r, b.r) ? !b.flow : within(r, b.r))));
  },
  /**
   * Client rects (one per text fragment) → visual lines, in flow coordinates. In columns (`col` says which column a box is
   * in) lines at the same height in different columns stay apart, and come out in reading order: page, column, height.
   */
  groupLines(this: Paginator, rects: DOMRect[], col?: (r: DOMRect) => number): { top: number; bottom: number; col: number }[] {
    const fr = this.flow.getBoundingClientRect();
    const sc = this.scale();
    const out: { top: number; bottom: number; col: number }[] = [];
    for (const r of rects) {
      if (r.height <= 0 || r.width <= 0) continue;
      const top = (r.top - fr.top) / sc;
      const bottom = (r.bottom - fr.top) / sc;
      const c = col ? col(r) : 0;
      const line = out.find((l) => l.col === c && (Math.abs(l.top - top) < 3 || (top >= l.top - 1 && bottom <= l.bottom + 1)));
      if (line) {
        line.top = Math.min(line.top, top);
        line.bottom = Math.max(line.bottom, bottom);
      } else out.push({ top, bottom, col: c });
    }
    if (col) out.sort((a, b) => this.pageAt(a.top) - this.pageAt(b.top) || a.col - b.col || a.top - b.top);
    else out.sort((a, b) => a.top - b.top);
    // merge overlapping fragments (mixed font sizes on one line)
    const merged: { top: number; bottom: number; col: number }[] = [];
    for (const l of out) {
      const last = merged[merged.length - 1];
      if (last && last.col === l.col && l.top < last.bottom - 2) last.bottom = Math.max(last.bottom, l.bottom);
      else merged.push({ ...l });
    }
    return merged;
  },
  /** First DOM position whose glyph starts at or below `y` (flow coords). */
  positionAtLine(this: Paginator, el: HTMLElement, y: number): { node: Node; offset: number } | null {
    const fr = this.flow.getBoundingClientRect();
    const sc = this.scale();
    const topOf = (r: DOMRect) => (r.top - fr.top) / sc;
    const range = scratchRange("position");
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT, {
      acceptNode: (n) => {
        if (n.nodeType === Node.ELEMENT_NODE) {
          const e = n as Element;
          if (e.classList.contains("lbl")) return NodeFilter.FILTER_REJECT;
          if (e.classList.contains("atom")) return NodeFilter.FILTER_ACCEPT;
          return NodeFilter.FILTER_SKIP;
        }
        return NodeFilter.FILTER_ACCEPT;
      },
    });
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (n.nodeType === Node.TEXT_NODE && (n as Text).parentElement?.closest(".atom.shape")) continue;
      if (n.nodeType === Node.ELEMENT_NODE) {
        const r = (n as Element).getBoundingClientRect();
        if (r.height > 0 && topOf(r) >= y - 1) return { node: n, offset: -1 };
        continue;
      }
      const t = n as Text;
      const len = t.data.length;
      if (!len) continue;
      range.setStart(t, len - 1);
      range.setEnd(t, len);
      const lastRect = range.getClientRects()[0];
      if (!lastRect || topOf(lastRect) < y - 1) continue;
      let lo = 0;
      let hi = len - 1;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        range.setStart(t, mid);
        range.setEnd(t, mid + 1);
        const r = range.getClientRects()[0];
        if (r && topOf(r) >= y - 1) hi = mid;
        else lo = mid + 1;
      }
      return { node: t, offset: lo };
    }
    return null;
  },
  /** The first character of a paragraph (in text order) whose box satisfies `test`; `test` must hold from some point on. */
  glyphAt(this: Paginator, p: HTMLElement, test: (r: DOMRect) => boolean): { node: Text; offset: number } | null {
    const nodes: Text[] = [];
    const ends: number[] = [];
    let total = 0;
    const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT, {
      acceptNode: (n) => {
        for (let e = n.parentElement; e && e !== p; e = e.parentElement) if (e.classList.contains("lbl") || e.classList.contains("pgsp") || e.classList.contains("atom") || e.classList.contains("ghost")) return NodeFilter.FILTER_REJECT;
        return (n as Text).data.length ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      },
    });
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      nodes.push(n as Text);
      total += (n as Text).data.length;
      ends.push(total);
    }
    if (!total) return null;
    const locate = (i: number) => {
      let lo = 0;
      let hi = nodes.length - 1;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (ends[mid] > i) hi = mid;
        else lo = mid + 1;
      }
      return { node: nodes[lo], offset: i - (lo ? ends[lo - 1] : 0) };
    };
    const range = scratchRange("glyph");
    const rectOf = (i: number) => {
      for (let j = i; j < total && j < i + 8; j++) {
        const { node, offset } = locate(j);
        range.setStart(node, offset);
        range.setEnd(node, offset + 1);
        const r = range.getClientRects()[0];
        if (r && r.width > 0) return r;
      }
      return null;
    };
    let lo = 0;
    let hi = total - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      const r = rectOf(mid);
      if (r && test(r)) {
        found = mid;
        hi = mid - 1;
      } else lo = mid + 1;
    }
    return found < 0 ? null : locate(found);
  },
  /** The visual lines of a body paragraph (spacers left out), in flow coordinates; an empty paragraph is one line. */
  textLines(this: Paginator, el: HTMLElement, col?: (r: DOMRect) => number): { top: number; bottom: number; col: number }[] {
    const range = scratchRange("text");
    const rects: DOMRect[] = [];
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT, {
      acceptNode: (n) => {
        if (n.nodeType === Node.TEXT_NODE) return NodeFilter.FILTER_ACCEPT;
        const e = n as Element;
        if (e.classList.contains("pgsp")) return NodeFilter.FILTER_REJECT;
        return e.classList.contains("atom") || e.classList.contains("lbl") ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
      },
    });
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (n.nodeType === Node.ELEMENT_NODE) rects.push((n as Element).getBoundingClientRect());
      else {
        range.selectNodeContents(n);
        rects.push(...Array.from(range.getClientRects()));
      }
    }
    const found = this.groupLines(rects, col);
    if (found.length) return found;
    const cs = getComputedStyle(el);
    const box = el.getBoundingClientRect();
    const fr = this.flow.getBoundingClientRect();
    const sc = this.scale();
    const size = parseFloat(cs.fontSize) || 15;
    const line = parseFloat(cs.lineHeight) || size * 1.2;
    const top = (box.top - fr.top) / sc + (parseFloat(cs.borderTopWidth) || 0) + (parseFloat(cs.paddingTop) || 0);
    return [{ top, bottom: top + line, col: col ? col(box) : 0 }];
  },
};

declare module "./layout" {
  interface Paginator {
    pageBlockList(): HTMLElement[];
    countBlock(el: HTMLElement): BlockLines;
    blockLines(el: HTMLElement): BlockLines;
    lineTotals(): { main: number; boxes: number; };
    recordLines(): void;
    blocksBetween(y0: number, y1: number): HTMLElement[] | undefined;
    blocksOnPages(k0: number, k1: number): HTMLElement[];
    recOf(el: Element): BlockRec | undefined;
    drawFarFor(els: HTMLElement[]): void;
    markPage(el: HTMLElement, top: number, sc: number): number;
    scale(): number;
    lines(el: HTMLElement): { top: number; bottom: number; }[];
    outsideShapes(el: HTMLElement, rects: DOMRect[]): DOMRect[];
    groupLines(rects: DOMRect[], col?: (r: DOMRect) => number): { top: number; bottom: number; col: number; }[];
    positionAtLine(el: HTMLElement, y: number): { node: Node; offset: number; } | null;
    glyphAt(p: HTMLElement, test: (r: DOMRect) => boolean): { node: Text; offset: number; } | null;
    textLines(el: HTMLElement, col?: (r: DOMRect) => number): { top: number; bottom: number; col: number; }[];
  }
}
