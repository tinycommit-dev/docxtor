// The selection: carets, ranges and several ranges at once, focus, moving and deleting by selection.
import type { Editor } from "./editor";
import type { ModelSel } from "./selection";
import type { FormatState, MPos, RBlock, RunPatch, SectionInfo } from "@docxtor/core";
import { orderPositions, paraIn, paraLength, paragraphIds, readSelection, writeSelection } from "./selection";
import type { SelRange } from "./multi-select";
import { disjoint, findAll, isEmpty, paraText, reachOf, replaceOps, sameRange, sortRanges } from "./multi-select";
import { sideMargins } from "./layout";

export const selectionMethods = {
  /** Set the selection programmatically (model and DOM). */
  select(this: Editor, sel: ModelSel | MPos): void {
    this.extra.clear();
    this.ensureDrawn(("focus" in sel ? sel.focus : sel).p);
    this.sel = "anchor" in sel ? sel : { anchor: sel, focus: sel };
    this.pending = null;
    this.lastAuto = null;
    writeSelection(this.root, this.sel);
    this.farSoon();
  },
  /** Pull the live DOM selection (selectionchange is async and can lag behind fast input). */
  syncSel(this: Editor): void {
    if (this.composing || !this.hasFocus()) return;
    const s = readSelection(this.root);
    if (!s) return;
    const a = this.sel;
    if (!a || a.anchor.p !== s.anchor.p || a.anchor.o !== s.anchor.o || a.focus.p !== s.focus.p || a.focus.o !== s.focus.o) {
      this.sel = s;
      this.pending = null;
    }
  },
  range(this: Editor): [MPos, MPos] | null {
    this.syncSel();
    if (!this.sel) return null;
    return orderPositions(this.root, this.sel.anchor, this.sel.focus);
  },
  /** Like `range()`, but only looks: it leaves `sel` alone, so the `selectionchange` handler still sees the move (and drops the extra ranges). For tools that react to key and pointer events. */
  peekRange(this: Editor): [MPos, MPos] | null {
    let s = this.sel;
    if (!this.composing && this.hasFocus()) s = readSelection(this.root) ?? s;
    if (!s) return null;
    return orderPositions(this.root, s.anchor, s.focus);
  },
  collapsed(this: Editor): boolean {
    const s = this.sel;
    return !s || (s.anchor.p === s.focus.p && s.anchor.o === s.focus.o);
  },
  /** Ids of paragraphs touched by the selection (document order). */
  selectedParagraphs(this: Editor): number[] {
    const r = this.range();
    if (!r) return [];
    const all = Array.from(this.root.querySelectorAll<HTMLElement>("[data-p]")).map((e) => Number(e.dataset.p));
    const span = (from: MPos, to: MPos) => {
      const a = all.indexOf(from.p);
      const b = all.indexOf(to.p);
      return a < 0 || b < 0 ? [from.p] : all.slice(a, b + 1);
    };
    // With several ranges selected, every paragraph any of them touches.
    if (this.multi) return Array.from(new Set(this.ranges().flatMap((x) => span(x.from, x.to)))).sort((x, y) => all.indexOf(x) - all.indexOf(y));
    return span(r[0], r[1]);
  },
  /** Paragraph id to position in the document, for putting ranges in order. */
  paraIndex(this: Editor): Map<number, number> {
    return new Map(this.allParas().map((el, i) => [Number(el.dataset.p), i]));
  },
  /** Every selected range in document order: the real selection and the others. */
  ranges(this: Editor): SelRange[] {
    const r = this.range();
    if (!r) return [];
    const own = { from: r[0], to: r[1] };
    return this.multi ? sortRanges(this.paraIndex(), [own, ...this.extra.ranges]) : [own];
  },
  /** The range of `ranges()` that is the real selection. */
  primaryOf(this: Editor, ranges: SelRange[]): number {
    const r = this.range();
    return r ? Math.max(0, ranges.findIndex((x) => sameRange(x, { from: r[0], to: r[1] }))) : 0;
  },
  /**
   * Select several ranges at once. The one at `primary` becomes the real selection (what the toolbar shows and
   * `selection()` reports); the others are drawn on a layer of their own. Ranges that overlap an earlier one are dropped, and so are ranges
   * in a document tab that is not shown (they could be neither seen nor reached). Returns how many ranges are selected.
   */
  selectRanges(this: Editor, list: SelRange[], primary = 0): number {
    if (!this.engine || !list.length) return 0;
    if (this.tabs.tabs.length > 1 && !this.allTabs) {
      const want = list[Math.min(primary, list.length - 1)];
      list = list.filter((r) => !this.hiddenTabOf(r.from.p) && !this.hiddenTabOf(r.to.p));
      if (!list.length) return 0;
      primary = Math.max(0, list.indexOf(want));
    }
    const index = this.paraIndex();
    const ordered = list.map((r) => {
      const [from, to] = orderPositions(this.root, r.from, r.to);
      return { from, to };
    });
    const chosen = ordered[Math.min(primary, ordered.length - 1)];
    const others = disjoint(index, sortRanges(index, ordered)).filter((r) => !sameRange(r, chosen) && !this.overlaps(index, r, chosen));
    this.extra.clear();
    this.sel = { anchor: chosen.from, focus: chosen.to };
    this.pending = null;
    this.lastAuto = null;
    this.focus();
    this.extra.set(others);
    // Every range's ends are drawn (their boxes are measured) before anyone looks.
    this.updateFar();
    this.emitSelection();
    return 1 + others.length;
  },
  /** Whether two ranges share any text. */
  overlaps(this: Editor, index: Map<number, number>, a: SelRange, b: SelRange): boolean {
    const [x, y] = sortRanges(index, [a, b]);
    if (!x || !y) return false;
    const px = index.get(x.to.p)!;
    const py = index.get(y.from.p)!;
    return py < px || (py === px && y.from.o < x.to.o);
  },
  /**
   * Select every place the selected text occurs in the body (case-sensitive), keeping the real selection where
   * it is. Returns how many places are selected, the real one included (0 when nothing was done).
   */
  selectAllMatching(this: Editor): number {
    const r = this.range();
    if (!r || this.collapsed() || r[0].p !== r[1].p || this.story) return 0;
    const el = this.paraElement(r[0].p);
    const text = el ? paraText(el).slice(r[0].o, r[1].o) : "";
    if (!text) return 0;
    const own = { from: r[0], to: r[1] };
    const index = this.paraIndex();
    const others = findAll(this.flow, text).filter((m) => !this.overlaps(index, m, own));
    this.selectRanges([own, ...others], 0);
    return others.length + 1;
  },
  /** The text of every selected range, in document order, one per line. */
  rangesText(this: Editor): string {
    return this.ranges()
      .filter((r) => !isEmpty(r))
      .map((r) => this.engine.textRange(r.from, r.to))
      .join("\n");
  },
  /**
   * Replace every selected range with `text` (or only delete them when it is empty) as one undo step. Each range
   * carries on as a caret after it, so typing goes on at all of them; the real selection is the caret that follows.
   */
  replaceRanges(this: Editor, ranges: SelRange[], text: string, merge = false): boolean {
    const { ops, carets } = replaceOps(ranges, text, text && this.pending ? this.pending : null);
    if (!ops.length) return true;
    const primary = this.primaryOf(ranges);
    if (!this.exec(ops, { merge, typing: true, select: carets[primary], keepExtra: true })) return false;
    this.extra.set(carets.filter((_, i) => i !== primary).map((c) => ({ from: c, to: c })));
    return true;
  },
  /** Delete every selected range (or, for carets, the character before or after each), as one undo step. */
  deleteRanges(this: Editor, ranges: SelRange[], forward = false, merge = false): boolean {
    const reach = reachOf(this.root, ranges, forward);
    if (reach.every(isEmpty)) return true;
    const { ops, carets } = replaceOps(reach, "");
    const primary = this.primaryOf(ranges);
    if (!this.exec(ops, { merge, typing: true, select: carets[primary], keepExtra: true })) return false;
    this.extra.set(carets.filter((_, i) => i !== primary).map((c) => ({ from: c, to: c })));
    return true;
  },
  /** Input that applies to every selected range at once; false when it is none of that kind (the real selection alone handles it). */
  multiInput(this: Editor, e: InputEvent): boolean {
    const ranges = this.ranges();
    const carets = ranges.every(isEmpty);
    if (e.inputType === "insertText" || e.inputType === "insertReplacementText") {
      const text = (e.data ?? e.dataTransfer?.getData("text/plain") ?? "").replace(/\r\n?/g, "\n");
      if (!text) return true;
      const now = performance.now();
      const merge = carets && this.lastTyping > 0 && now - this.lastTyping < 1500 && !/^\s/.test(text) && this.atTypedEnd();
      this.lastTyping = now;
      this.lastDelete = 0;
      return this.replaceRanges(ranges, text, merge);
    }
    if (!e.inputType.startsWith("delete") || e.inputType === "deleteByCut") return false;
    this.lastTyping = 0;
    const single = e.inputType === "deleteContentBackward" || e.inputType === "deleteContentForward";
    return this.deleteRanges(ranges, e.inputType.includes("Forward"), single && carets && this.deleteMerge());
  },
  format(this: Editor): FormatState | null {
    const r = this.range();
    if (!r) return null;
    const at = this.collapsed() ? r[0] : { p: r[0].p, o: Math.min(r[0].o + 1, paraLength(this.root, r[0].p)) };
    // A key asks twice (the toolbar after the change, then the selection event): the answer is kept until this task ends, for the same
    // engine, the same place and no change drawn since (every change the editor makes is drawn through `apply`), so the engine is asked
    // once. Asking the engine for its revision instead would be an engine call of its own.
    const m = this.formatKept;
    if (m && m.engine === this.engine && m.applied === this.applied && m.p === at.p && m.o === at.o) return m.f;
    const f = this.engine.formatAt(at);
    if (!this.formatKept) queueMicrotask(() => (this.formatKept = null));
    this.formatKept = { engine: this.engine, applied: this.applied, p: at.p, o: at.o, f };
    return f;
  },
  emitSelection(this: Editor): void {
    this.pasteFlow.moved();
    const f = this.format();
    this.ruler.setFormat(f);
    this.syncRuler(f);
    this.updateVRuler();
    this.spell.selectionMoved();
    this.hooks.onSelection?.(f);
  },
  /** The horizontal ruler shows the width and margins of the section the caret is in (the page's section in a header or footer). */
  syncRuler(this: Editor, f: FormatState | null | undefined): void {
    if (!this.story && f) this.rulerSection = f.section;
    this.showRulerSection();
  },
  showRulerSection(this: Editor): void {
    const sections = this.pageInfo.sections;
    const last = Math.max(0, (sections?.length ?? 1) - 1);
    const i = this.story ? this.paginator.page(this.story.page).section : this.rulerSection;
    const info = sections?.[Math.min(Math.max(0, i), last)] ?? this.pageInfo;
    // The ruler's margins are the text's: the gutter is part of them.
    const side = sideMargins(info as SectionInfo, { gutterAtTop: !!this.pageInfo.gutterAtTop });
    this.ruler.setPage({ ...info, margin: [info.margin[0], side.right, info.margin[2], side.left] });
  },
  /** 1-based page number at the top of the viewport. */
  /** The page (0-based) the caret's paragraph starts on, from the page map; undefined with no caret in the main text, or while the map is out of date. */
  caretPage(this: Editor): number | undefined {
    if (this.story || !this.sel) return undefined;
    const el = this.paraElement(this.sel.focus.p);
    return el ? this.paginator.pageOf(el) : undefined;
  },
  currentPage(this: Editor): number {
    const stageTop = this.stage.getBoundingClientRect().top;
    const canvasTop = this.canvas.getBoundingClientRect().top;
    const y = (canvasTop - stageTop) / this.zoom + 80;
    return this.paginator.pageAt(y) + 1;
  },
  selectedText(this: Editor): string {
    const r = this.range();
    return r ? this.engine.textRange(r[0], r[1]) : "";
  },
  /** The text has the keyboard: the editing area itself, or the text of a shape or text box inside it. */
  hasFocus(this: Editor): boolean {
    const a = document.activeElement;
    return a === this.root || (a instanceof HTMLElement && a.classList.contains("shape-text") && this.root.contains(a));
  },
  focus(this: Editor): void {
    // The text is hidden while several pages show side by side: the keyboard goes to a page of the grid.
    if (this.grid.on) return this.grid.focus();
    this.focusRoot();
    if (this.sel) writeSelection(this.root, this.sel);
  },
  /**
   * Give the text the keyboard without moving the view. `preventScroll` is not enough in Safari: focusing the editing area brings the
   * browser's remembered selection (the caret where the user last typed, perhaps pages away) into view whatever the option says, so a
   * click on a shape or picture, or a right-click on one, scrolled away from what was clicked. The scroll offsets are put back.
   */
  focusRoot(this: Editor): void {
    // Already there: focusing again does nothing, and reading every scroll offset would make the browser lay the page out for nothing.
    if (document.activeElement === this.root) return;
    const held: [HTMLElement, number, number][] = [];
    for (let el = this.root.parentElement; el; el = el.parentElement) held.push([el, el.scrollTop, el.scrollLeft]);
    this.root.focus({ preventScroll: true });
    for (const [el, top, left] of held) {
      if (el.scrollTop !== top) el.scrollTop = top;
      if (el.scrollLeft !== left) el.scrollLeft = left;
    }
  },
  scrollToParagraph(this: Editor, id: number): void {
    this.ensureDrawn(id);
    if (this.story && !this.root.querySelector(`[data-p="${id}"]`)) this.exitStory(false);
    const el = this.root.querySelector<HTMLElement>(`[data-p="${id}"]`) ?? this.root.querySelector<HTMLElement>(`[data-id="${id}"]`);
    if (!el) return;
    this.ensureDrawn(el);
    // The caret first, then the scroll: Safari stops a smooth scroll when the selection in the text that has the keyboard changes after it started.
    this.extra.clear();
    this.sel = { anchor: { p: id, o: 0 }, focus: { p: id, o: 0 } };
    this.focus();
    const c = this.canvas.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    this.canvas.scrollTo({ top: this.canvas.scrollTop + r.top - c.top - 80, behavior: "smooth" });
  },
  toggleMark(this: Editor, mark: "bold" | "italic" | "underline" | "strike" | "smallCaps" | "caps"): void {
    const f = this.formatWithPending();
    const on = mark === "underline" ? !!f?.run.underline && f.run.underline !== "none" : !!f?.run[mark];
    const patch: RunPatch = mark === "underline" ? { underline: on ? "none" : "single" } : { [mark]: !on };
    this.setRun(patch);
  },
  /**
   * The first and last caret positions of the text being edited, from the model: the body's first and last paragraph that is
   * shown (not in a hidden tab, not folded away), or the paragraphs of the header, footer or notes open for editing. Nothing
   * here reads layout, so it holds when blocks far from the view are out of the browser's layout.
   */
  documentBounds(this: Editor): { start: MPos; end: MPos } | null {
    let first: number | null = null;
    let last: number | null = null;
    if (this.story) {
      const ids = paragraphIds(this.root);
      if (!ids.length) return null;
      first = ids[0];
      last = ids[ids.length - 1];
    } else {
      const shown = (id: number) => {
        const el = this.blocks.get(id);
        return !el || (el.dataset.tabHidden === undefined && el.dataset.fold === undefined);
      };
      const edge = (b: RBlock, atEnd: boolean): number | null => {
        if (b.t === "p") return b.id;
        const kids: RBlock[] = b.t === "c" ? b.blocks : b.t === "tbl" ? b.rows.flatMap((r) => r.cells.flatMap((c) => c.blocks)) : [];
        for (let i = 0; i < kids.length; i++) {
          const id = edge(kids[atEnd ? kids.length - 1 - i : i], atEnd);
          if (id !== null) return id;
        }
        return null;
      };
      for (const id of this.order) {
        const b = this.json.get(id);
        if (!b || !shown(id)) continue;
        first = edge(b, false);
        if (first !== null) break;
      }
      for (let i = this.order.length - 1; i >= 0; i--) {
        const id = this.order[i];
        const b = this.json.get(id);
        if (!b || !shown(id)) continue;
        last = edge(b, true);
        if (last !== null) break;
      }
    }
    if (first === null || last === null) return null;
    return { start: { p: first, o: 0 }, end: { p: last, o: paraLength(this.root, last) } };
  },
  /** Select the whole text being edited (the body, or the header, footer or footnotes open for editing). */
  selectAll(this: Editor): void {
    // In the text of a shape or text box, all of that text.
    const a = document.activeElement;
    if (a instanceof HTMLElement && a.classList.contains("shape-text") && this.root.contains(a)) {
      window.getSelection()?.selectAllChildren(a);
      return;
    }
    this.focus();
    const b = this.documentBounds();
    if (!b) {
      window.getSelection()?.selectAllChildren(this.root);
      return;
    }
    this.select({ anchor: b.start, focus: b.end });
    this.emitSelection();
  },
  /**
   * Move the caret to the start or end of the text being edited, or with `extend` move the selection's focus there. Set from
   * the model, then scrolled into view, so it does not depend on what the browser has laid out.
   */
  moveToBoundary(this: Editor, end: boolean, extend = false): void {
    const a = document.activeElement;
    if (a instanceof HTMLElement && a.classList.contains("shape-text") && this.root.contains(a)) {
      // The start or end of the box's own text. (The browser's "documentboundary" would leave the box: its editing host
      // is the whole page.)
      const s = window.getSelection();
      if (!s) return;
      const walker = document.createTreeWalker(a, NodeFilter.SHOW_TEXT);
      let first: Text | null = null;
      let last: Text | null = null;
      for (let n = walker.nextNode() as Text | null; n; n = walker.nextNode() as Text | null) {
        first ??= n;
        last = n;
      }
      const node: Node = (end ? last : first) ?? a;
      const offset = node === a ? (end ? a.childNodes.length : 0) : end ? (node as Text).length : 0;
      if (extend && s.anchorNode && a.contains(s.anchorNode)) s.setBaseAndExtent(s.anchorNode, s.anchorOffset, node, offset);
      else s.collapse(node, offset);
      return;
    }
    const b = this.documentBounds();
    if (!b) return;
    const anchor = extend ? ((this.hasFocus() ? readSelection(this.root)?.anchor : undefined) ?? this.sel?.anchor) : undefined;
    const to = end ? b.end : b.start;
    this.focus();
    this.select({ anchor: anchor ?? to, focus: to });
    this.emitSelection();
    this.scrollCaretIntoView();
  },
  /**
   * Called before the browser moves the caret with an arrow key, Page Up or Page Down: the browser
   * jumps over blocks that are out of its layout, so the page the caret is about to move into is drawn first. The drawn stretch always has the
   * caret's page and one either side (`updateFar`), so a line or a character never leaves it; a page key goes as far as the view is tall.
   */
  ensureNear(this: Editor, direction: "up" | "down" | "left" | "right" | "pageup" | "pagedown"): void {
    const pg = this.paginator;
    if (!pg || this.farKeep === null) return;
    this.updateFar();
    if (direction !== "pageup" && direction !== "pagedown") return;
    const live = this.hasFocus() ? readSelection(this.root) : this.sel;
    const el = live ? paraIn(this.flow, live.focus.p) : null;
    const at = el ? pg.pagesOf(el) : undefined;
    if (!at) return;
    const [v0, v1] = this.viewPages();
    const reach = v1 - v0 + 2;
    pg.showPages(at[0] - reach, at[1] + reach);
    this.farStamp++;
  },
  /** Delete the selection (the menu's Delete; the keyboard has its own). */
  deleteSelection(this: Editor): void {
    const r = this.range();
    if (!r || this.collapsed()) return;
    if (this.multi) {
      this.deleteRanges(this.ranges());
      return;
    }
    this.exec({ op: "delete", from: r[0], to: r[1] }, { select: r[0] });
  },
  /** The selection as Markdown (plain text in a header, footer or footnote). */
  selectionMarkdown(this: Editor): string {
    const r = this.range();
    if (!r || this.collapsed()) return "";
    return (this.story ? null : this.engine.rangeMarkdown(r[0], r[1])) ?? this.engine.textRange(r[0], r[1]);
  },
};

declare module "./editor" {
  interface Editor {
    select(sel: ModelSel | MPos): void;
    syncSel(): void;
    range(): [MPos, MPos] | null;
    peekRange(): [MPos, MPos] | null;
    collapsed(): boolean;
    selectedParagraphs(): number[];
    paraIndex(): Map<number, number>;
    ranges(): SelRange[];
    primaryOf(ranges: SelRange[]): number;
    selectRanges(list: SelRange[], primary?: number): number;
    overlaps(index: Map<number, number>, a: SelRange, b: SelRange): boolean;
    selectAllMatching(): number;
    rangesText(): string;
    replaceRanges(ranges: SelRange[], text: string, merge?: boolean): boolean;
    deleteRanges(ranges: SelRange[], forward?: boolean, merge?: boolean): boolean;
    multiInput(e: InputEvent): boolean;
    format(): FormatState | null;
    emitSelection(): void;
    syncRuler(f: FormatState | null | undefined): void;
    showRulerSection(): void;
    caretPage(): number | undefined;
    currentPage(): number;
    selectedText(): string;
    hasFocus(): boolean;
    focus(): void;
    focusRoot(): void;
    scrollToParagraph(id: number): void;
    toggleMark(mark: "bold" | "italic" | "underline" | "strike" | "smallCaps" | "caps"): void;
    documentBounds(): { start: MPos; end: MPos; } | null;
    selectAll(): void;
    moveToBoundary(end: boolean, extend?: boolean): void;
    ensureNear(direction: "up" | "down" | "left" | "right" | "pageup" | "pagedown"): void;
    deleteSelection(): void;
    selectionMarkdown(): string;
  }
}
