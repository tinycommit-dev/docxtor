// Sections with columns: even, unequal and vertical columns, column breaks and balancing.
import type { Paginator } from "./layout";
import { GAP, cutText } from "./layout";
import { BALANCE_ROWS, scratchRange } from "./layout-units";

export const columnMethods = {
  /**
   * Set up the columns of a section's wrapper: `n` columns `gap` apart that fill one after another in rows `rowH` high,
   * `rowGap` apart (`balance`: the columns of the last row end level).
   */
  styleCols(this: Paginator, el: HTMLElement, n: number, gap: number, balance: boolean, rowH: number, rowGap: number) {
    const s = el.style;
    s.setProperty("column-count", String(n));
    s.setProperty("column-gap", `${gap}px`);
    s.setProperty("column-fill", balance ? "balance" : "auto");
    s.setProperty("column-height", `${rowH}px`);
    s.setProperty("column-wrap", "wrap");
    s.setProperty("row-gap", `${rowGap}px`);
    s.marginTop = "0px";
    s.marginBottom = "0px";
  },
  /**
   * Lay out a section's columns, starting at flow y `top` on `page`. The browser fragments the blocks over the columns; what
   * is left to do is to line its rows up with the pages: each row is one page's text area (`H` high, the next row `R`
   * further down), and the wrapper's rows are the section's pages.
   *
   * - A section starting part-way down a page fits below the start when all of it does (columns `room` high). Otherwise the
   *   wrapper is pulled up (a negative top margin) so that its first row is the page's text area, and each column of that
   *   row starts with a spacer as high as the offset, so the text starts where the section does. The spacers are placed one
   *   column at a time (`spaceColumn`), because each moves what follows it.
   * - The wrapper always has room for whole rows. Where the content really ends is measured, and a negative bottom margin
   *   pulls what follows up to there. Columns end level (`balance`) when a continuous section break follows, as in Word.
   *
   * Returns the pull to apply to the top margin, the change in the flow's height that the blocks after it must account
   * for, and the page the last row is on. Rows on a page with footnotes end higher, to leave the notes their room
   * (`reserveRows`).
   */
  placeColumns(this: Paginator, el: HTMLElement, si: number, top: number, page: number, neutral: number, balance: boolean): { pull: number; live: number; page: number } {
    const sec = this.secs[si];
    const n = sec.cols;
    const gap = sec.gap;
    const b = this.box(page);
    const ct = this.contentTop(page);
    const H = Math.max(40, b.top + b.h - b.mB - ct);
    const R = b.mB + GAP + this.margins(si, this.box(page).num + 1, false).mT;
    const offset = top - ct > 1 ? top - ct : 0;
    const colW = (sec.w - sec.ml - sec.mr - (n - 1) * gap) / n;
    const rule = (k: number, y1: number, y2: number) => {
      if (!sec.info.cols.sep || y2 - y1 < 1) return;
      for (let c = 1; c < n; c++) this.rules.push({ page: k, x: sec.ml + c * (colW + gap) - gap / 2, y1, y2 });
    };
    el.dataset.n = String(n);
    const marks = this.breakMarks(el);

    if (offset && !marks.length) {
      const room = this.contentBottom(page) - top;
      this.styleCols(el, n, gap, balance, room, R);
      const h1 = el.offsetHeight;
      if (h1 <= room + 1) {
        const real = this.colsBottom(el, 0);
        el.style.marginBottom = `${real - h1}px`;
        rule(page, top, top + real);
        return { pull: 0, live: real - neutral, page };
      }
    }

    this.styleCols(el, n, gap, false, H, R);
    if (offset) {
      el.dataset.pull = "1";
      this.offsetColumns(el, n, gap, offset, H, R);
    }
    for (const mark of marks) this.fillRow(el, mark, n, gap, H, R);
    if (this.reserve.length) this.reserveRows(el, page, n, gap, H, R);
    let total = el.offsetHeight;
    const rows = Math.max(1, Math.ceil((total + R) / (H + R) - 0.001));
    // Level columns would undo the room kept for footnotes, so a last row with notes stays as it is.
    if (balance && rows <= BALANCE_ROWS && !(offset && rows === 1) && !((this.reserve[page + rows - 1] ?? 0) > 0)) {
      el.style.setProperty("column-fill", "balance");
      total = el.offsetHeight;
    }
    const real = this.colsBottom(el, (rows - 1) * (H + R));
    el.style.marginBottom = `${real - total}px`;
    // The rows are pages of the section: make their boxes.
    const last = page + rows - 1;
    this.box(last);
    const wTop = offset ? ct : top;
    for (let r = 0; r < rows; r++) {
      const y1 = r === 0 ? top : this.contentTop(page + r);
      rule(page + r, y1, r === rows - 1 ? wTop + real : this.contentTop(page + r) + H);
    }
    return { pull: offset ? -offset : 0, live: real - neutral, page: last };
  },
  /**
   * How far down (from the wrapper's top) the content of its last row reaches. `lastTop` is where that row begins.
   * Blocks are measured by their fragments, so a paragraph split over columns counts where each part is.
   */
  colsBottom(this: Paginator, el: HTMLElement, lastTop: number): number {
    const sc = this.scale();
    const top = el.getBoundingClientRect().top;
    let best = lastTop;
    let who: HTMLElement | null = null;
    for (let i = el.children.length - 1; i >= 0; i--) {
      const k = el.children[i] as HTMLElement;
      if (k.classList.contains("colsp")) continue;
      const rects = Array.from(k.getClientRects());
      if (!rects.length) continue;
      const max = Math.max(...rects.map((r) => (r.bottom - top) / sc));
      // This block, and so every block before it, ends above the last row.
      if (max <= lastTop + 0.5) break;
      if (max > best) {
        best = max;
        who = k;
      }
    }
    return best + (who ? parseFloat(getComputedStyle(who).marginBottom) || 0 : 0);
  },
  /** A wrapper back to one plain column. */
  resetCols(this: Paginator, el: HTMLElement) {
    if (el.dataset.un) this.clearUnequal(el);
    if (el.dataset.vt) this.clearVertical(el);
    for (const sp of Array.from(el.querySelectorAll(".colsp"))) sp.remove();
    el.removeAttribute("data-pull");
    for (const prop of ["column-count", "column-gap", "column-fill", "column-height", "column-wrap", "row-gap"]) el.style.removeProperty(prop);
    el.style.marginTop = "0px";
    el.style.marginBottom = "0px";
  },
  /** Take the vertical layout off a wrapper. */
  clearVertical(this: Paginator, el: HTMLElement) {
    delete el.dataset.vt;
    for (const prop of ["writing-mode", "width", "height", "overflow"]) el.style.removeProperty(prop);
  },
  /**
   * Lay out a section whose text runs down the page: the wrapper is one page's text area, and the browser sets the blocks in
   * it with a vertical writing mode: lines run down (or up) and each paragraph is a column to the left of the one before it.
   * A section of vertical text is one page tall: what does not fit in its width runs off its left side.
   */
  placeVertical(this: Paginator, el: HTMLElement, si: number, page: number, neutral: number): { pull: number; live: number; page: number } {
    const sec = this.secs[si];
    this.resetCols(el);
    const H = Math.max(40, this.contentBottom(page) - this.contentTop(page));
    const W = Math.max(40, sec.w - sec.ml - sec.mr);
    el.dataset.vt = sec.vert;
    const up = sec.vert === "btLr";
    // Text from the bottom up with lines from the left is `sideways-lr`; where the browser lacks it, the closest is vertical-lr.
    const mode = up ? (typeof CSS !== "undefined" && CSS.supports("writing-mode", "sideways-lr") ? "sideways-lr" : "vertical-lr") : "vertical-rl";
    el.style.writingMode = mode;
    el.style.width = `${W}px`;
    el.style.height = `${H}px`;
    el.style.overflow = "hidden";
    el.style.marginTop = "0px";
    el.style.marginBottom = "0px";
    this.box(page);
    return { pull: 0, live: H - neutral, page };
  },
  /** Take the unequal-column layout off a wrapper: its blocks go back into the flow. */
  clearUnequal(this: Paginator, el: HTMLElement) {
    delete el.dataset.un;
    el.style.removeProperty("position");
    el.style.removeProperty("height");
    for (const b of Array.from(el.children) as HTMLElement[]) {
      for (const prop of ["position", "left", "top", "width", "box-sizing"]) b.style.removeProperty(prop);
    }
  },
  /**
   * Lay out a section whose columns differ in width. CSS columns are always equal, so the paginator does it: blocks go one
   * after another into the columns of each page (left to right, page by page), each set at its column's width. A block that
   * doesn't fit under the ones above it moves to the next column; one taller than a column stands in one anyway. A block is
   * never split between two columns, since their widths differ, and columns are not made level at the end of the section.
   * The blocks are placed on the page (absolutely, in the wrapper), and the wrapper is as tall as the pages they use.
   */
  placeUnequal(this: Paginator, el: HTMLElement, si: number, top: number, page: number, neutral: number): { pull: number; live: number; page: number } {
    const sec = this.secs[si];
    const widths = sec.colW!;
    const xs = sec.colX!;
    const n = widths.length;
    this.resetCols(el);
    el.dataset.un = "1";
    el.dataset.n = String(n);
    el.style.position = "relative";
    const blocks = (Array.from(el.children) as HTMLElement[]).filter((k) => !k.classList.contains("colsp"));
    const outer = (b: HTMLElement) => b.offsetHeight + (parseFloat(b.style.marginTop) || 0) + (parseFloat(b.style.marginBottom) || 0);
    const placed: { b: HTMLElement; col: number; page: number; y: number; h: number }[] = [];
    const rows: { page: number; y1: number; y2: number }[] = [];
    let pg = page;
    let col = 0;
    let slotTop = top;
    let pending = blocks;
    let last = page;
    let guard = 0;
    while (pending.length && guard++ < 2000) {
      this.box(pg);
      const limit = this.contentBottom(pg);
      // Every pending block at this column's width: how tall each is here.
      for (const b of pending) {
        b.style.position = "absolute";
        b.style.boxSizing = "border-box";
        b.style.left = `${xs[col]}px`;
        b.style.top = "0px";
        b.style.width = `${widths[col]}px`;
      }
      const heights = pending.map(outer);
      let y = slotTop;
      let taken = 0;
      let newPage = false;
      for (let j = 0; j < pending.length; j++) {
        const b = pending[j];
        const prev = j > 0 ? pending[j - 1] : undefined;
        const startsPage = !!b.dataset.pbb || (!!prev?.dataset.pbr && !prev.dataset.cbr);
        if (taken > 0 && startsPage) {
          newPage = true;
          break;
        }
        if (taken > 0 && y + heights[j] > limit + 0.5) break;
        placed.push({ b, col, page: pg, y, h: heights[j] });
        y += heights[j];
        taken++;
        // A column break ends the column; a page break the page.
        if (b.dataset.cbr) break;
        if (b.dataset.pbr) {
          newPage = true;
          break;
        }
      }
      last = pg;
      const row = rows.find((r) => r.page === pg);
      if (row) row.y2 = Math.max(row.y2, y);
      else rows.push({ page: pg, y1: slotTop, y2: y });
      pending = pending.slice(taken);
      if (!pending.length) break;
      if (col + 1 < n && !newPage) col++;
      else {
        col = 0;
        pg++;
        slotTop = this.contentTop(pg);
      }
    }
    // Put each block where it was placed, from the wrapper's top.
    for (const { b, col: c, y } of placed) {
      b.style.left = `${xs[c]}px`;
      b.style.top = `${y - top}px`;
      b.style.width = `${widths[c]}px`;
    }
    const bottom = Math.max(...placed.filter((p) => p.page === last).map((p) => p.y + p.h), top);
    el.style.height = `${Math.max(0, bottom - top)}px`;
    el.style.marginTop = "0px";
    el.style.marginBottom = "0px";
    // Lines between the columns, on each page the section uses.
    if (sec.info.cols.sep) {
      for (const r of rows) {
        const used = new Set(placed.filter((p) => p.page === r.page).map((p) => p.col));
        for (let c = 1; c < n; c++) {
          if (!used.has(c) && !used.has(c - 1)) continue;
          const gapMid = (xs[c - 1] + widths[c - 1] + xs[c]) / 2;
          this.rules.push({ page: r.page, x: sec.ml + gapMid, y1: r.y1, y2: Math.max(r.y2, r.y1 + 1) });
        }
      }
    }
    return { pull: 0, live: bottom - top - neutral, page: last };
  },
  /** The page breaks inside a wrapper that the browser does not make (the first block's is the section's start). */
  breakMarks(this: Paginator, el: HTMLElement): HTMLElement[] {
    const first = el.firstElementChild;
    return Array.from(el.querySelectorAll<HTMLElement>(":scope > .fp[data-pbb], :scope > .fp .atom.pbr")).filter((m) => m !== first);
  },
  /**
   * Browsers do not start a new row of columns at a page break, so spacers fill what is left of the column and every
   * column after it, and what follows starts the next row. Spacers that start the first row's later columns (`offsetColumns`)
   * and come after the break are not needed any more: the fillers are what those columns hold now.
   */
  fillRow(this: Paginator, el: HTMLElement, mark: HTMLElement, n: number, gap: number, H: number, R: number) {
    const atom = mark.classList.contains("atom");
    // A page break character: the text after it starts on a line of its own, where a spacer first stands to measure by.
    const probe = atom ? this.spacer("span", 0) : null;
    if (probe) mark.after(probe);
    const er = el.getBoundingClientRect();
    const sc = this.scale();
    const r = (probe ?? mark).getClientRects()[0];
    if (!r) {
      probe?.remove();
      return;
    }
    const y = (r.top - er.top) / sc;
    const row = Math.max(0, Math.floor((y + R / 2) / (H + R)));
    const t0 = y - row * (H + R);
    const pitch = (er.width / sc + gap) / n;
    const col = Math.max(0, Math.min(n - 1, Math.floor((((r.left + r.right) / 2 - er.left) / sc + gap / 2) / pitch)));
    // Already first in a row: it starts a page.
    if (!probe && col === 0 && t0 < 1) return;
    const stale = Array.from(el.querySelectorAll<HTMLElement>(".colsp")).filter((s) => s !== probe && mark.compareDocumentPosition(s) & Node.DOCUMENT_POSITION_FOLLOWING);
    const heights = [H - t0 - 0.5, ...new Array<number>(n - col - 1).fill(H - 0.5)];
    let at: HTMLElement = probe ?? mark;
    heights.forEach((h, i) => {
      if (h < 1 && !(probe && i === 0)) return;
      const sp = i === 0 && probe ? probe : this.spacer(probe ? "span" : "div", 0);
      sp.classList.add("fill");
      sp.style.height = `${Math.max(0, h)}px`;
      if (sp !== probe) {
        if (probe) at.after(sp);
        else mark.before(sp);
      }
      at = sp;
    });
    for (const s of stale) s.remove();
  },
  /** Start every column of the first row `offset` lower than the top of the page (the wrapper stands pulled up to it). */
  offsetColumns(this: Paginator, el: HTMLElement, n: number, gap: number, offset: number, H: number, R: number) {
    el.insertBefore(this.spacer("div", offset), el.firstChild);
    for (let c = 1; c < n; c++) this.spaceColumn(el, c, n, gap, offset, H, R);
  },
  /** Put a spacer where column c (from 0) of the first row begins now, so its text starts `offset` lower. */
  spaceColumn(this: Paginator, el: HTMLElement, c: number, n: number, gap: number, offset: number, H: number, R: number): boolean {
    const er = el.getBoundingClientRect();
    const sc = this.scale();
    const pitch = (er.width / sc + gap) / n;
    // Which column (or n for any row below the first) a box is in, by its middle.
    const key = (r: DOMRect) => {
      if ((r.top - er.top) / sc >= H + R / 2) return n;
      return Math.max(0, Math.min(n - 1, Math.floor((((r.left + r.right) / 2 - er.left) / sc + gap / 2) / pitch)));
    };
    const blocks = (Array.from(el.children) as HTMLElement[]).filter((k) => !k.classList.contains("colsp"));
    const lastRect = (k: HTMLElement) => {
      const rs = k.getClientRects();
      return rs.length ? rs[rs.length - 1] : null;
    };
    // The first block reaching column c (blocks run in reading order, so this is a search).
    let lo = 0;
    let hi = blocks.length - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      const r = lastRect(blocks[mid]);
      if (r && key(r) >= c) {
        found = mid;
        hi = mid - 1;
      } else lo = mid + 1;
    }
    if (found < 0) return false;
    const block = blocks[found];
    const rects = Array.from(block.getClientRects());
    const first = key(rects[0]);
    if (first >= c) {
      // The block starts a column: if it is this one, the spacer goes before it.
      if (first !== c) return false;
      el.insertBefore(this.spacer("div", offset), block);
      return true;
    }
    if (!rects.some((r) => key(r) === c)) return false;
    if (block.classList.contains("ftw")) return this.spaceTable(block, key, c, offset);
    if (!block.classList.contains("fp")) return false;
    const at = this.glyphAt(block, (r) => key(r) >= c);
    if (!at) return false;
    const sp = this.spacer("span", offset);
    const rest = cutText(at.node, at.offset);
    rest.parentNode!.insertBefore(sp, rest);
    // Its box now covers the start of the column's first row, over the text above: only its text takes clicks.
    block.dataset.cs = "1";
    return true;
  },
  /**
   * Leave room for footnotes: on a page with notes, every column of its row ends that much higher, and what reached lower
   * moves on to the next column. Columns are done in order, since each move pushes the rest along.
   */
  reserveRows(this: Paginator, el: HTMLElement, page: number, n: number, gap: number, H: number, R: number) {
    for (let r = 0; r < Math.ceil((el.offsetHeight + R) / (H + R) - 0.001); r++) {
      const need = this.reserve[page + r] ?? 0;
      if (need < 1) continue;
      for (let c = 0; c < n; c++) this.endColumn(el, r * n + c, n, gap, H, R, H - need);
    }
  },
  /** End column `slot` (row * n + column) at `limit` below its row's top, with a spacer filling the rest of it. */
  endColumn(this: Paginator, el: HTMLElement, slot: number, n: number, gap: number, H: number, R: number, limit: number): boolean {
    const er = el.getBoundingClientRect();
    const sc = this.scale();
    const pitch = (er.width / sc + gap) / n;
    // Where a box is: its column slot, and its top and bottom from the top of its row.
    const at = (r: DOMRect) => {
      const y = (r.top - er.top) / sc;
      const row = Math.max(0, Math.floor((y + R / 2) / (H + R)));
      const col = Math.max(0, Math.min(n - 1, Math.floor((((r.left + r.right) / 2 - er.left) / sc + gap / 2) / pitch)));
      return { slot: row * n + col, top: y - row * (H + R), bottom: (r.bottom - er.top) / sc - row * (H + R) };
    };
    const past = (r: DOMRect) => {
      const p = at(r);
      return p.slot > slot || (p.slot === slot && p.bottom > limit + 0.5);
    };
    const fill = (tag: "div" | "span", top: number) => {
      const sp = this.spacer(tag, Math.max(0, H - top - 2));
      sp.classList.add("fill");
      return sp;
    };
    // Measured sizes are a little off at some zooms: a spacer too tall to fit moves to the next column, so shrink it until it stays.
    const settle = (sp: HTMLElement) => {
      for (let i = 0; i < 4; i++) {
        const r = sp.getClientRects()[0];
        if (!r || at(r).slot === slot) return;
        sp.style.height = `${Math.max(0, parseFloat(sp.style.height) - 4)}px`;
      }
    };
    for (const k of Array.from(el.children) as HTMLElement[]) {
      if (k.classList.contains("colsp")) continue;
      const rects = Array.from(k.getClientRects()).filter((r) => r.height > 0);
      if (!rects.some(past)) continue;
      const first = at(rects[0]);
      // The column already ends above the limit.
      if (first.slot > slot) return false;
      if (first.slot === slot && (first.top > limit - 0.5 || !k.classList.contains("fp") || k.dataset.kl)) {
        // The block starts below the limit (or can't be split): a spacer before it (its top margin included) fills the column.
        const top = first.top - (parseFloat(getComputedStyle(k).marginTop) || 0);
        if (top < 1) return false;
        const sp = fill("div", top);
        el.insertBefore(sp, k);
        settle(sp);
        return true;
      }
      // A paragraph running past the limit continues in the next column from the first line below it. Tables are left as they are.
      if (!k.classList.contains("fp")) return false;
      const g = this.glyphAt(k, past);
      if (!g) return false;
      const range = scratchRange("column");
      range.setStart(g.node, g.offset);
      range.setEnd(g.node, g.offset + 1);
      const r = range.getClientRects()[0];
      if (!r) return false;
      const rest = cutText(g.node, g.offset);
      const sp = fill("span", at(r).top);
      rest.parentNode!.insertBefore(sp, rest);
      settle(sp);
      return true;
    }
    return false;
  },
  /** A table that runs into column c: a spacer row before the first row that is in it. */
  spaceTable(this: Paginator, wrap: HTMLElement, key: (r: DOMRect) => number, c: number, offset: number): boolean {
    const table = wrap.querySelector("table");
    if (!table) return false;
    const rows = Array.from(table.rows);
    const row = rows.find((r) => key(r.getBoundingClientRect()) >= c);
    if (!row || row === rows[0] || key(row.getBoundingClientRect()) !== c) return false;
    const cols = Math.max(1, ...rows.map((r) => Array.from(r.cells).reduce((a, x) => a + x.colSpan, 0)));
    const tr = document.createElement("tr");
    tr.className = "pgsp colsp";
    tr.contentEditable = "false";
    const td = document.createElement("td");
    td.colSpan = cols;
    td.style.cssText = `height:${offset}px;padding:0;border:none;background:transparent`;
    tr.append(td);
    row.parentNode!.insertBefore(tr, row);
    wrap.dataset.cs = "1";
    return true;
  },
};

declare module "./layout" {
  interface Paginator {
    styleCols(el: HTMLElement, n: number, gap: number, balance: boolean, rowH: number, rowGap: number): void;
    placeColumns(el: HTMLElement, si: number, top: number, page: number, neutral: number, balance: boolean): { pull: number; live: number; page: number; };
    colsBottom(el: HTMLElement, lastTop: number): number;
    resetCols(el: HTMLElement): void;
    clearVertical(el: HTMLElement): void;
    placeVertical(el: HTMLElement, si: number, page: number, neutral: number): { pull: number; live: number; page: number; };
    clearUnequal(el: HTMLElement): void;
    placeUnequal(el: HTMLElement, si: number, top: number, page: number, neutral: number): { pull: number; live: number; page: number; };
    breakMarks(el: HTMLElement): HTMLElement[];
    fillRow(el: HTMLElement, mark: HTMLElement, n: number, gap: number, H: number, R: number): void;
    offsetColumns(el: HTMLElement, n: number, gap: number, offset: number, H: number, R: number): void;
    spaceColumn(el: HTMLElement, c: number, n: number, gap: number, offset: number, H: number, R: number): boolean;
    reserveRows(el: HTMLElement, page: number, n: number, gap: number, H: number, R: number): void;
    endColumn(el: HTMLElement, slot: number, n: number, gap: number, H: number, R: number, limit: number): boolean;
    spaceTable(wrap: HTMLElement, key: (r: DOMRect) => number, c: number, offset: number): boolean;
  }
}
