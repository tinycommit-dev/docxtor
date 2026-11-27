// Splitting a paragraph, a table row or a table across pages with spacers.
import type { Paginator } from "./layout";
import { cutText } from "./layout";
import type { Given, NoteLine } from "./layout-types";
import { SPLIT_ROOM, layoutUnits, scratchRange } from "./layout-units";

export const splitMethods = {
  /**
   * Put the queued spacers in the page, in document order. A text node cut once goes on in the node that holds the text
   * after the cut: later positions in it are counted from the cut.
   */
  flushSpacers(this: Paginator) {
    const moved = new Map<Node, { node: Text; from: number }>();
    for (const { node, offset, h } of this.queued) {
      if (offset === -1) {
        this.insertSpacer({ node, offset }, h);
        continue;
      }
      const at = moved.get(node);
      const rest = this.insertSpacer(at ? { node: at.node, offset: offset - at.from } : { node, offset }, h);
      if (rest) moved.set(node, { node: rest, from: offset });
    }
    this.queued = [];
    this.queuedH = 0;
  },
  /** Insert a spacer at a position; returns the text node that holds the text after it, when the position is in text. */
  insertSpacer(this: Paginator, pos: { node: Node; offset: number }, h: number): Text | null {
    const sp = document.createElement("span");
    sp.className = "pgsp";
    sp.contentEditable = "false";
    sp.style.height = `${h}px`;
    if (pos.offset === -1) {
      pos.node.parentNode!.insertBefore(sp, pos.node);
      return null;
    }
    const t = pos.node as Text;
    const rest = cutText(t, pos.offset);
    rest.parentNode!.insertBefore(sp, rest);
    return rest;
  },
  /**
   * Cut a paragraph at the page ends it crosses. With `defer` (a paragraph of the flow, during `pass`) the spacers are
   * queued, not inserted: the lines are measured once, and where a spacer would go the lines after it move down by its
   * height (it takes a line of its own, as tall as the gap to the next page), exactly as measuring again would show.
   */
  splitParagraph(this: Paginator, el: HTMLElement, push: number, page: number, isTop: boolean, defer = false, notes?: { lines: NoteLine[]; room: number[]; given: Given[]; from: number }): { added: number; page: number } | null {
    let added = 0;
    let guard = 0;
    // Deferred: positions are the page as it is (no queued spacer in it) moved by the queued spacers above this
    // paragraph (`queuedH`) and, for each line, this paragraph's own queued spacers above it (`shift`).
    const off = push + this.queuedH;
    // With `notes` (the pass gives pages their notes' room as it fills them), the lines come with their references, where the pass has them.
    let sim: NoteLine[] | null = notes && defer ? notes.lines : defer ? this.lines(el).map((l) => ({ top: l.top + off, bottom: l.bottom + off, shift: 0 })) : null;
    // The room given to this paragraph's notes on the page it is on starts here in `notes.given` (on its first page, what the pass gave it).
    let pageFrom = notes?.from ?? 0;
    // How far the paragraph's first line's letters sit below the top of its box: the leading the browser puts above a line of it, which
    // is what Word's line box at the top margin also has above the letters (measured, since the browser splits the leading unevenly).
    let leadAbove = -1;
    while (guard++ < 200) {
      const lines = sim ?? this.lines(el).map((l) => ({ top: l.top + push, bottom: l.bottom + push, shift: 0 }));
      if (lines.length < 2) return added ? { added, page } : null;
      // Widow/orphan control: keep at least two lines on each side when possible
      // (unless the paragraph turned it off, which allows a single line on either side).
      const control = el.dataset.wc !== "0";
      // The first line that goes on to the next page with the page ending at `limit`: -1 when all of it stays, null when all of it goes.
      const fitFor = (limit: number): number | null => {
        let f = lines.findIndex((l) => l.bottom > limit + 0.5);
        if (f < 0) return -1;
        if (control && f < 2 && !isTop && added === 0) return null;
        if (control && lines.length - f < 2 && f >= 3) f -= 1;
        return f;
      };
      // The lines that go on take their notes' room with them (`takeBack`), and the page is cut again without it.
      let fit: number | null = null;
      for (let tries = 0; ; tries++) {
        if (notes && sim && tries < 3) this.roomOnPage(sim, page, notes.room, notes.given);
        fit = fitFor(this.contentBottom(page));
        const f = fit;
        if (!notes || !sim || tries >= 3 || !this.takeBack(page, notes.room, notes.given, pageFrom, (l) => f === null || (f >= 0 && lines.indexOf(l) >= f))) break;
      }
      if (fit === null) return null;
      if (fit < 0) return { added, page };
      if (fit < 1) return added ? { added, page } : null;
      const pos = this.positionAtLine(el, sim ? lines[fit].top - off - lines[fit].shift : lines[fit].top - push);
      if (!pos) return added ? { added, page } : null;
      // In whole layout units (Chrome keeps lengths in 1/64 px): the spacer then moves what follows by exactly h, so the
      // lines worked out for a deferred spacer are where measuring would find them, however many pages they add up over.
      // Word puts the line's box at the top margin, as for a page that starts with a whole paragraph: the letters sit below the margin by
      // the leading above them. That is the first line's distance from the paragraph's box (no more than half the step between lines).
      if (leadAbove < 0) {
        const r = el.getBoundingClientRect();
        const cs = getComputedStyle(el);
        const boxTop = (r.top - this.flow.getBoundingClientRect().top) / this.scale() + (parseFloat(cs.paddingTop) || 0) + (parseFloat(cs.borderTopWidth) || 0) + (sim ? off : push);
        const half = Math.max(0, (lines[1].top - lines[0].top - (lines[0].bottom - lines[0].top)) / 2);
        leadAbove = Math.min(Math.max(0, lines[0].top - boxTop), half);
      }
      const lead = leadAbove;
      const h = layoutUnits(this.contentTop(page + 1) - lines[fit].top + lead);
      if (h <= 0) return added ? { added, page } : null;
      if (sim) {
        this.queued.push({ ...pos, h });
        this.queuedH += h;
        // The spacer is a line of its own (measuring would find its box), and every line from the cut on moves down by h.
        const at = lines[fit];
        sim = [...lines.slice(0, fit), { top: at.top, bottom: at.top + h, shift: at.shift }, ...lines.slice(fit).map((l) => ({ ...l, top: l.top + h, bottom: l.bottom + h, shift: l.shift + h }))];
      } else this.insertSpacer(pos, h);
      added += h;
      page += 1;
      isTop = true;
      if (notes) pageFrom = notes.given.length;
    }
    return { added, page };
  },
  /**
   * If a paragraph has content after its page break, push that content to the next page
   * with a spacer and return the added height; return null when the break ends the paragraph.
   */
  breakInside(this: Paginator, el: HTMLElement, push: number, page: number): number | null {
    const atom = el.querySelector(".atom.pbr, .atom.cbr");
    if (!atom) return null;
    const after = scratchRange("after");
    after.setStartAfter(atom);
    after.setEnd(el, el.childNodes.length);
    const frag = after.cloneContents();
    frag.querySelectorAll(".lbl,.pgsp,br[data-ph]").forEach((x) => x.remove());
    if (!frag.textContent?.trim() && !frag.querySelector(".atom")) return null;
    const before = this.hOf(el);
    const sp = document.createElement("span");
    sp.className = "pgsp";
    sp.contentEditable = "false";
    sp.style.height = "0px";
    atom.after(sp);
    // The spacer starts on its own line; size it so the following text starts the next page.
    const top = (sp.getBoundingClientRect().top - this.flow.getBoundingClientRect().top) / this.scale() + push;
    sp.style.height = `${Math.max(0, this.contentTop(page + 1) - top)}px`;
    return this.hOf(el) - before;
  },
  /** The rows of a table that count for pagination (not spacers or repeated headers). */
  bodyRows(this: Paginator, table: HTMLTableElement): HTMLTableRowElement[] {
    return Array.from(table.rows).filter((r) => !r.classList.contains("pgsp") && !r.classList.contains("hdr-repeat"));
  },
  /** A row may split across pages: it isn't marked `cantSplit` and no cell spans rows (a merge would be torn). */
  canSplitRow(this: Paginator, row: HTMLTableRowElement, rows: HTMLTableRowElement[]): boolean {
    if (row.dataset.cs || row.dataset.exact) return false;
    if (Array.from(row.cells).some((c) => c.rowSpan > 1)) return false;
    // A cell above that runs down into this row.
    const at = rows.indexOf(row);
    for (let k = 0; k < at; k++) if (Array.from(rows[k].cells).some((c) => c.rowSpan > 1 && k + c.rowSpan > at)) return false;
    return true;
  },
  /**
   * Break a row at the bottom of `page`: in every cell the first block (or line of a paragraph) that crosses the limit and
   * everything after it moves to the top of the next page, by a spacer in the cell. The row is as tall as its tallest cell,
   * so it grows by what the cells pushed. Returns whether anything moved.
   */
  splitRow(this: Paginator, row: HTMLTableRowElement, push: number, page: number): boolean {
    const limit = this.contentBottom(page);
    const next = this.contentTop(page + 1);
    const fr = this.flow.getBoundingClientRect();
    const sc = this.scale();
    let moved = false;
    const gapBefore = (child: HTMLElement, top: number) => {
      const sp = document.createElement("div");
      sp.className = "pgsp";
      sp.contentEditable = "false";
      sp.style.height = `${Math.max(1, next - top)}px`;
      child.before(sp);
      moved = true;
    };
    for (const td of Array.from(row.cells)) {
      if (td.classList.contains("gap")) continue;
      const box = (td.querySelector(":scope > .clip") as HTMLElement | null) ?? td;
      for (const child of Array.from(box.children) as HTMLElement[]) {
        if (child.classList.contains("pgsp") || child.tagName === "svg") continue;
        const r = child.getBoundingClientRect();
        const top = (r.top - fr.top) / sc + push;
        const bottom = (r.bottom - fr.top) / sc + push;
        if (bottom <= limit + 0.5) continue;
        if (top >= limit - 0.5) gapBefore(child, top);
        else if (child.classList.contains("fp")) {
          const done = this.splitParagraph(child, push, page, false);
          // A paragraph that cannot be cut (one line, or widow control) goes whole.
          if (done && done.added) moved = true;
          else gapBefore(child, top);
        } else gapBefore(child, top);
        break;
      }
    }
    return moved;
  },
  splitTable(this: Paginator, wrap: HTMLElement, push: number, page: number): { added: number; page: number } | null {
    const table = wrap.querySelector("table");
    if (!table) return null;
    // How far the table grows is what it grew by, not the sum of the spacers put in: the browser puts a table's rows on whole pixels, and
    // the height beside them (borders, the fraction the table's own place gives it) moves with them, so a spacer of 227.9375 px grew a
    // table by 227.4375. A pass that cuts the table and then measures the blocks after it (every pass that starts part-way down does)
    // sees that growth; one that measured them before the cut (a pass from the top) must add the same, or the two disagree by the
    // difference and a part-way pass has to start over at every table it cuts.
    const h0 = this.hOf(wrap);
    let added = 0;
    let guard = 0;
    const holes: [number, number][] = [];
    const cutRows: HTMLTableRowElement[] = [];
    const cols = Math.max(1, ...Array.from(table.rows).map((r) => Array.from(r.cells).reduce((a, c) => a + c.colSpan, 0)));
    const finish = () => {
      // A row that was cut at a page shows nothing in the gap between the pages: its cells' fill and lines stop at the cut.
      if (holes.length) {
        const top = this.topOf(wrap) + push + table.offsetTop;
        const stops: string[] = ["#000 0px"];
        for (const [a, b] of holes) stops.push(`#000 ${a - top}px`, `transparent ${a - top}px`, `transparent ${b - top}px`, `#000 ${b - top}px`);
        table.style.setProperty("mask-image", `linear-gradient(to bottom, ${stops.join(",")})`);
        table.dataset.cut = "1";
        // Word closes the row at the page end and opens it again on the next page: draw the row's bottom line at the cut
        // and again at the top of the continuation.
        const wrapTop = this.topOf(wrap) + push;
        holes.forEach(([a, b], k) => {
          const cell = cutRows[k].cells[0];
          const cs = cell ? getComputedStyle(cell) : null;
          if (!cs || cs.borderBottomStyle === "none" || cs.borderBottomStyle === "hidden") return;
          for (const y of [a, b]) {
            const line = document.createElement("div");
            line.className = "pgsp cutline";
            line.contentEditable = "false";
            line.style.cssText = `position:absolute;left:${table.offsetLeft}px;width:${table.offsetWidth}px;top:${y - wrapTop}px;height:0;border-top:${cs.borderBottomWidth} ${cs.borderBottomStyle} ${cs.borderBottomColor}`;
            wrap.append(line);
          }
        });
      }
      return { added: this.hOf(wrap) - h0, page };
    };
    while (guard++ < 500) {
      const wrapTop = this.topOf(wrap) + push;
      const rows = this.bodyRows(table);
      const limit = this.contentBottom(page);
      const idx = rows.findIndex((r) => wrapTop + table.offsetTop + r.offsetTop + r.offsetHeight > limit + 0.5);
      if (idx < 0) return finish();
      const row = rows[idx];
      const rowTop = wrapTop + table.offsetTop + row.offsetTop;
      const atTop = rowTop <= this.contentTop(page) + 1;
      // A row that is allowed to split is cut at the page end when enough of it fits (or it is taller than a page).
      const room = limit - rowTop;
      const fitsAPage = row.offsetHeight <= this.contentBottom(page) - this.contentTop(page);
      if (this.canSplitRow(row, rows) && (room >= SPLIT_ROOM || !fitsAPage || rowTop < this.contentTop(page) - 1)) {
        const before = this.hOf(wrap);
        if (this.splitRow(row, push, page)) {
          added += this.hOf(wrap) - before;
          holes.push([limit, this.contentTop(page + 1)]);
          cutRows.push(row);
          page += 1;
          continue;
        }
      }
      if (atTop || rowTop < this.contentTop(page) - 1) {
        // A row taller than a page that cannot be cut: let it overflow, then continue after its bottom.
        const rowBottom = rowTop + row.offsetHeight;
        if (idx === rows.length - 1) return added ? finish() : null;
        page = Math.max(page + 1, this.pageFor(rowBottom));
        continue;
      }
      const h = this.contentTop(page + 1) - rowTop;
      if (h <= 0) return added ? finish() : null;
      const tr = document.createElement("tr");
      tr.className = "pgsp";
      tr.contentEditable = "false";
      const td = document.createElement("td");
      td.colSpan = cols;
      td.style.cssText = `height:${h}px;padding:0;border:none;background:transparent`;
      tr.append(td);
      row.parentNode!.insertBefore(tr, row);
      added += h;
      // Repeat header rows at the top of the continuation, like Word.
      const headers = rows.filter((r) => r.dataset.header);
      if (headers.length && !headers.includes(row)) {
        for (const hr of headers) {
          const copy = hr.cloneNode(true) as HTMLTableRowElement;
          copy.classList.add("hdr-repeat");
          copy.contentEditable = "false";
          copy.setAttribute("aria-hidden", "true");
          copy.querySelectorAll("[data-p]").forEach((x) => x.removeAttribute("data-p"));
          row.parentNode!.insertBefore(copy, row);
          added += copy.offsetHeight;
        }
      }
      page += 1;
    }
    return finish();
  },
  spacer(this: Paginator, tag: "div" | "span", h: number): HTMLElement {
    const sp = document.createElement(tag);
    sp.className = "pgsp colsp";
    sp.contentEditable = "false";
    sp.style.height = `${h}px`;
    return sp;
  },
};

declare module "./layout" {
  interface Paginator {
    flushSpacers(): void;
    insertSpacer(pos: { node: Node; offset: number; }, h: number): Text | null;
    splitParagraph(el: HTMLElement, push: number, page: number, isTop: boolean, defer?: boolean, notes?: { lines: NoteLine[]; room: number[]; given: Given[]; from: number; }): { added: number; page: number; } | null;
    breakInside(el: HTMLElement, push: number, page: number): number | null;
    bodyRows(table: HTMLTableElement): HTMLTableRowElement[];
    canSplitRow(row: HTMLTableRowElement, rows: HTMLTableRowElement[]): boolean;
    splitRow(row: HTMLTableRowElement, push: number, page: number): boolean;
    splitTable(wrap: HTMLElement, push: number, page: number): { added: number; page: number; } | null;
    spacer(tag: "div" | "span", h: number): HTMLElement;
  }
}
