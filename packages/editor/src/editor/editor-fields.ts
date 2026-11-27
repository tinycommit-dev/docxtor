// Fields, the contents, captions, cross-references, the index, page numbers and the engine's AI tools.
import type { Editor } from "./editor";
import { topBlocks } from "./far";
import { t } from "@docxtor/extensions";
import type { IndexOptions, MPos, Op, TocOptions, XrefShow, XrefTarget, XrefTargets } from "@docxtor/core";
import type { ModelSel } from "./selection";
import { fieldInput, updateExtensionFields, updateForOutput, updateOnOpen } from "./fields";
import { paraLength } from "./selection";

/** Engine AI tools that write page numbers, which the engine can only estimate (see `Editor.runTool`). */
const PAGED_TOOLS = new Set([
  "insert_table_of_contents",
  "change_table_of_contents",
  "insert_index",
  "change_index",
  "insert_table_of_figures",
  "insert_cross_reference",
  "insert_field",
  "set_field_code",
  "update_fields",
]);


export const fieldMethods = {
  /**
   * The page each paragraph of the main text starts on, as laid out now (for page numbers in a table of contents). With
   * tabs, every tab is laid out first so a heading of a hidden tab gets its own page, and NUMPAGES counts them all.
   */
  pageMap(this: Editor): Record<string, number> {
    // A document with tabs shows one at a time, but its page numbers are those of all the tabs one after another (as Word
    // shows the file): the other tabs are laid out for the measurement, then hidden again.
    if (this.tabs.tabs.length > 1 && !this.allTabs) {
      const scroll = this.canvas.scrollTop;
      this.showAllTabs(true);
      try {
        return this.pageMap();
      } finally {
        this.showAllTabs(false);
        this.canvas.scrollTop = scroll;
      }
    }
    const top = this.stage.getBoundingClientRect().top;
    const out: Record<string, number> = {};
    // The number the page prints, which a section may have restarted.
    const numOf = (k: number) => this.paginator.page(k).num;
    const measured = (el: HTMLElement) => {
      const y = (el.getBoundingClientRect().top - top) / this.zoom;
      out[el.dataset.p!] = numOf(this.paginator.pageAt(y));
    };
    // A paragraph that is a block of the text is on the page the paginator put it on, and so is everything inside a block that sits whole on one page;
    // the rest (in a table or a paragraph cut by a page, folded away) is measured, with its block drawn first.
    const put = (el: HTMLElement, kid: HTMLElement) => {
      const at = this.paginator.wholePageOf(el);
      if (at !== undefined) out[el.dataset.p!] = numOf(at);
      else {
        this.drawBlock(kid);
        measured(el);
      }
    };
    for (const kid of topBlocks(this.flow)) {
      if (kid.dataset.p !== undefined) {
        const at = !kid.dataset.fold ? this.paginator.pageOfBlock(kid) : undefined;
        if (at !== undefined) out[kid.dataset.p] = numOf(at);
        else put(kid, kid);
      }
      for (const el of Array.from(kid.querySelectorAll<HTMLElement>("[data-p]"))) put(el, kid);
    }
    return out;
  },
  /** Whether the document has a table of contents. */
  hasToc(this: Editor): boolean {
    return !!this.flow.querySelector('[data-fld="TOC"]');
  },
  /** Insert a table of contents before the paragraph holding the caret, with page numbers from this layout. */
  insertToc(this: Editor, title = t("Contents"), options?: TocOptions): void {
    if (this.story) return this.hooks.onError?.(t("A table of contents can only go in the main text."));
    const r = this.range();
    if (!r) return;
    const caret = this.sel;
    if (!this.exec({ op: "insert_toc", before: this.outerBlock(r[0].p), title, pages: this.pageMap(), ...(options ? { options } : {}) }, { select: caret })) return;
    // The table pushes headings down: measure again and correct the numbers, in the same undo step.
    this.exec({ op: "update_toc", pages: this.pageMap() }, { merge: true, select: caret });
  },
  /** Rebuild every table of contents from the current headings and page layout. */
  updateToc(this: Editor): void {
    const caret = this.sel;
    if (!this.exec({ op: "update_toc", pages: this.pageMap() }, { select: caret })) return;
    this.exec({ op: "update_toc", pages: this.pageMap() }, { merge: true, select: caret });
  },
  /** The top-level block holding a paragraph (the table when it is in a cell): where a list or a caption goes next to. */
  outerBlock(this: Editor, p: number): number {
    let id = p;
    let el = this.paraElement(p)?.closest<HTMLElement>(".ftw");
    while (el) {
      id = Number(el.dataset.id);
      el = el.parentElement?.closest<HTMLElement>(".ftw") ?? null;
    }
    return id;
  },
  /** What a caption would go next to: the selected image, the table the caret is in, or the caret's paragraph. */
  captionTarget(this: Editor): { id: number; kind: "image" | "table" | "paragraph" } | null {
    if (this.story) return null;
    const r = this.range();
    if (!r) return null;
    const el = this.paraElement(r[0].p);
    if (!el) return null;
    const onImage = r[1].p === r[0].p && r[1].o === r[0].o + 1 && !!el.querySelector(`.atom.img[data-o="${r[0].o}"]`);
    if (onImage) return { id: r[0].p, kind: "image" };
    const table = el.closest<HTMLElement>(".ftw");
    if (table) return { id: Number(table.dataset.id), kind: "table" };
    return { id: r[0].p, kind: el.querySelector(".atom.img") ? "image" : "paragraph" };
  },
  /** Everything a cross-reference can point at, as the document is now. */
  xrefTargets(this: Editor): XrefTargets {
    return this.engine.xrefTargets();
  },
  /** The number a new caption with this label would get next to `target`, for the preview in the Caption dialog. */
  nextCaptionNumber(this: Editor, label: string, target: { id: number; kind: string }, position: "above" | "below"): number {
    const caps = this.engine.xrefTargets().captions.filter((c) => c.label === label);
    const order = Array.from(this.flow.querySelectorAll<HTMLElement>("[data-p]")).map((e) => Number(e.dataset.p));
    let at: number;
    if (target.kind === "table") {
      const ids = Array.from(this.flow.querySelectorAll<HTMLElement>(`.ftw[data-id="${target.id}"] [data-p]`)).map((e) => Number(e.dataset.p));
      const edge = position === "above" ? ids[0] : ids[ids.length - 1];
      at = order.indexOf(edge) + (position === "above" ? -0.5 : 0.5);
    } else at = order.indexOf(target.id) + (position === "above" ? -0.5 : 0.5);
    return caps.filter((c) => order.indexOf(c.p) < at).length + 1;
  },
  /**
   * Run an engine AI tool on this document. The engine has no page layout, so tools that write page numbers (a table of contents, an
   * index, a table of figures, cross-references, fields) leave them estimated; after those, the fields are updated with the pages of
   * this layout, in the tool's own undo step, as the menu commands do. The one place the assistant and `ctx.ai.tool` run engine tools.
   */
  runTool(this: Editor, name: string, input: unknown): string {
    const out = this.engine.raw.callTool(name, JSON.stringify(input));
    if (PAGED_TOOLS.has(name) && !this.readOnly) {
      this.refresh({ keepScroll: true });
      this.runUpdateFields(true, this.sel);
    }
    return out;
  },
  /** Run `update_fields` (and once more, because the first pass can move the headings the page numbers measure). Returns how many fields changed. */
  runUpdateFields(this: Editor, merge: boolean, select?: ModelSel | MPos | null): number {
    const a = this.exec({ op: "update_fields", pages: this.pageMap(), input: fieldInput(this, true) }, { merge, ...(select !== undefined ? { select } : {}) });
    if (!a) return 0;
    const b = this.exec({ op: "update_fields", pages: this.pageMap(), input: fieldInput(this, true) }, { merge: true, ...(select !== undefined ? { select } : {}) });
    // Fields that extensions evaluate follow, in the same undo step.
    void updateExtensionFields(this, true);
    return (a[0]?.count ?? 0) + (b?.[0]?.count ?? 0);
  },
  /** Update dates, dirty fields and (when the file asks) every field, as a file opens. Leaves nothing to undo. */
  updateFieldsOnOpen(this: Editor): void {
    if (!this.engine) return;
    updateOnOpen(this);
  },
  /** Update the fields before printing or downloading (when the setting is on). Returns how many changed. */
  async updateFieldsForOutput(this: Editor): Promise<number> {
    const n = updateForOutput(this);
    if (!(this.fieldServices?.updateBeforeOutput() ?? true)) return n;
    return n + (await updateExtensionFields(this, n > 0));
  },
  /** Show field codes instead of results, or the results again. */
  showFieldCodes(this: Editor, on: boolean): void {
    this.fieldCodes.setAll(on);
    this.layoutNow();
  },
  /** The innermost field around the caret in the text (Shift+F9 toggles its code). */
  fieldAtCaret(this: Editor): { p: number; o: number; instr: string; code: string } | null {
    const f = this.sel?.focus;
    if (!f || this.story || !this.engine) return null;
    const hits = this.engine.listFields().filter((x) => !x.story && x.p === f.p && x.o <= f.o && f.o <= x.o + x.result.length);
    const best = hits.reduce<(typeof hits)[number] | null>((a, b) => (!a || b.o >= a.o ? b : a), null);
    return best ? { p: best.p, o: best.o, instr: best.instr, code: best.code } : null;
  },
  /** Show or hide the code of the field at the caret. Returns whether there is such a field. */
  toggleFieldCodeAtCaret(this: Editor): boolean {
    const at = this.fieldAtCaret();
    if (!at) return false;
    this.fieldCodes.toggleOne(at.p, at.o);
    this.layoutNow();
    return true;
  },
  /** Change the code of the field that starts at a position and work out its result again, as one undo step. */
  setFieldCode(this: Editor, at: MPos, instr: string): boolean {
    if (this.readOnly) return false;
    const ok = !!this.exec({ op: "set_field_code", at, instr, input: fieldInput(this), pages: this.pageMap() }, { select: this.sel });
    if (ok) void updateExtensionFields(this, true);
    return ok;
  },
  /** Update caption numbers, cross-references, page numbers, and rebuild the contents, tables of figures and the index; one undo step. Returns how many changed. */
  updateFields(this: Editor): number {
    if (this.readOnly) return 0;
    if (this.story) this.exitStory(false);
    return this.runUpdateFields(false, this.sel);
  },
  /** Add a numbered caption next to the selected image or table, or the caret's paragraph, and renumber. */
  insertCaption(this: Editor, o: { label: string; text: string; position: "above" | "below"; target?: { id: number; kind: string }; chapter?: { level: number; separator?: string } }): boolean {
    if (this.readOnly) return false;
    const target = o.target ?? this.captionTarget();
    if (!target) return false;
    const res = this.exec({ op: "insert_caption", after: target.id, position: o.position, label: o.label, text: o.text, ...(o.chapter ? { chapter: o.chapter } : {}) }, { select: this.sel });
    const id = res?.[0]?.created?.[0];
    if (!res || id === undefined) return false;
    // The numbers after it move up by one; same undo step.
    this.exec({ op: "update_fields", pages: this.pageMap() }, { merge: true, select: { p: id, o: 0 } });
    this.exec({ op: "update_fields", pages: this.pageMap() }, { merge: true, select: { p: id, o: 0 } });
    this.select({ p: id, o: paraLength(this.root, id) });
    this.focus();
    this.emitSelection();
    return true;
  },
  /** A cross-reference at the caret (a selection is replaced), with page numbers from this layout. */
  insertCrossReference(this: Editor, o: { target: XrefTarget; show: XrefShow; link: boolean; aboveBelow: boolean }): boolean {
    if (this.readOnly) return false;
    if (this.story) {
      this.hooks.onError?.(t("A cross-reference can only go in the main text."));
      return false;
    }
    const r = this.range();
    if (!r) return false;
    const ops: Op[] = this.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
    ops.push({ op: "insert_cross_reference", at: r[0], target: o.target, show: o.show, link: o.link, ...(o.aboveBelow ? { above_below: true } : {}), pages: this.pageMap() });
    return !!this.exec(ops);
  },
  /** Mark the selection (or `entry`) for the index: this place only, or every paragraph that has the text. Returns how many were marked. */
  markIndexEntry(this: Editor, o: { entry: string; sub?: string; see?: string; bold?: boolean; italic?: boolean; all?: boolean; text?: string }): number {
    if (this.readOnly) return 0;
    const r = this.range();
    if (!r) return 0;
    const fmt = { ...(o.bold ? { bold: true } : {}), ...(o.italic ? { italic: true } : {}), ...(o.sub ? { sub: o.sub } : {}), ...(o.see ? { see: o.see } : {}) };
    const op: Op = o.all
      ? { op: "mark_all_index_entries", text: o.text || o.entry, entry: o.entry, ...fmt, ignore_case: true }
      : { op: "mark_index_entry", at: r[1], entry: o.entry, ...fmt };
    const res = this.exec(op, { select: this.sel });
    return res ? (o.all ? (res[0]?.count ?? 0) : 1) : 0;
  },
  /** Insert the index after the paragraph (or table) holding the caret, in two columns, and fix the page numbers. */
  insertIndex(this: Editor, options?: IndexOptions): void {
    if (this.readOnly) return;
    if (this.story) return this.hooks.onError?.(t("An index can only go in the main text."));
    const r = this.range();
    if (!r) return;
    const caret = this.sel;
    if (!this.exec({ op: "insert_index", after: this.outerBlock(r[1].p), columns: 2, pages: this.pageMap(), ...(options ? { options } : {}) }, { select: caret })) return;
    this.runUpdateFields(true, caret);
  },
  /** Insert a table of figures for the captions with this label, before the paragraph holding the caret. */
  insertTableOfFigures(this: Editor, label: string): void {
    if (this.readOnly) return;
    if (this.story) return this.hooks.onError?.(t("A table of figures can only go in the main text."));
    const r = this.range();
    if (!r) return;
    const caret = this.sel;
    if (!this.exec({ op: "insert_table_of_figures", before: this.outerBlock(r[0].p), label, pages: this.pageMap() }, { select: caret })) return;
    this.runUpdateFields(true, caret);
  },
  /** Centred page numbers in the footer (unless it already has them), leaving the caret there. */
  addPageNumbers(this: Editor, page = Math.max(0, this.currentPage() - 1)): void {
    this.enterStory("footer", page);
    if (!this.story || this.storyElement?.querySelector('[data-fld="PAGE"]')) return;
    this.setPara({ align: "center" });
    this.insertField("PAGE");
  },
};

declare module "./editor" {
  interface Editor {
    pageMap(): Record<string, number>;
    hasToc(): boolean;
    insertToc(title?: string, options?: TocOptions): void;
    updateToc(): void;
    outerBlock(p: number): number;
    captionTarget(): { id: number; kind: "image" | "table" | "paragraph"; } | null;
    xrefTargets(): XrefTargets;
    nextCaptionNumber(label: string, target: { id: number; kind: string; }, position: "above" | "below"): number;
    runTool(name: string, input: unknown): string;
    runUpdateFields(merge: boolean, select?: ModelSel | MPos | null): number;
    updateFieldsOnOpen(): void;
    updateFieldsForOutput(): Promise<number>;
    showFieldCodes(on: boolean): void;
    fieldAtCaret(): { p: number; o: number; instr: string; code: string; } | null;
    toggleFieldCodeAtCaret(): boolean;
    setFieldCode(at: MPos, instr: string): boolean;
    updateFields(): number;
    insertCaption(o: { label: string; text: string; position: "above" | "below"; target?: { id: number; kind: string; }; chapter?: { level: number; separator?: string; }; }): boolean;
    insertCrossReference(o: { target: XrefTarget; show: XrefShow; link: boolean; aboveBelow: boolean; }): boolean;
    markIndexEntry(o: { entry: string; sub?: string; see?: string; bold?: boolean; italic?: boolean; all?: boolean; text?: string; }): number;
    insertIndex(options?: IndexOptions): void;
    insertTableOfFigures(label: string): void;
    addPageNumbers(page?: number): void;
  }
}
