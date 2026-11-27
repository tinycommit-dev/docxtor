// Bookmarks and citations at the caret.
import type { Editor } from "./editor";
import type { BookmarkInfo, CitationInfo, MPos } from "@docxtor/core";

export const referenceMethods = {
  /** The bookmarks people can see, in document order. */
  bookmarks(this: Editor): BookmarkInfo[] {
    return this.engine ? this.engine.bookmarks() : [];
  },
  /**
   * A name for a new bookmark: made from the selected text (letters, digits and underscores, starting with a
   * letter, 40 characters at most) or Bookmark1, Bookmark2… , and not one already taken.
   */
  suggestBookmarkName(this: Editor): string {
    const taken = new Set(this.bookmarks().map((b) => b.name.toLowerCase()));
    const text = this.collapsed() ? "" : this.selectedText();
    const base = text
      .normalize("NFD")
      .replace(/\p{M}/gu, "")
      .replace(/[^A-Za-z0-9]+/g, "_")
      .replace(/^[^A-Za-z]+/, "")
      .slice(0, 30)
      .replace(/_+$/, "");
    if (base) {
      if (!taken.has(base.toLowerCase())) return base;
      for (let n = 2; ; n++) if (!taken.has(`${base}_${n}`.toLowerCase())) return `${base}_${n}`;
    }
    for (let n = 1; ; n++) if (!taken.has(`bookmark${n}`)) return `Bookmark${n}`;
  },
  /** Bookmark the selection (the caret's place when nothing is selected). Returns the name, or null when the engine refused. */
  addBookmark(this: Editor, name = this.suggestBookmarkName()): string | null {
    const r = this.range();
    if (!r) return null;
    if (!this.exec({ op: "insert_bookmark", from: r[0], to: r[1], name }, { select: this.sel })) return null;
    // Drawn now, so a popover can be anchored to it.
    this.bookmarkMarks.redraw();
    return name;
  },
  /** The ribbon in the margin that marks a bookmark (null for one in a header, footer or note). */
  bookmarkMarker(this: Editor, name: string): HTMLElement | null {
    return this.bookmarkMarks.marker(name);
  },
  /** Rename a bookmark; links to it follow. `onError` gets the engine's message (a bad or taken name). */
  renameBookmark(this: Editor, from: string, to: string, onError?: (message: string) => void): boolean {
    return !!this.exec({ op: "rename_bookmark", from, to }, { select: this.sel, onError });
  },
  /** Remove a bookmark. Links that point at it stay as they are. */
  removeBookmark(this: Editor, name: string): boolean {
    return !!this.exec({ op: "delete_bookmark", name }, { select: this.sel });
  },
  /**
   * The page whose header, footer or notes show the story a bookmark is in, or -1 (a bookmark in the text, or in a
   * header no page shows). A header shared by several pages gives the first.
   */
  bookmarkPage(this: Editor, b: BookmarkInfo): number {
    const st = b.story;
    if (!st) return -1;
    const pg = this.paginator;
    if (st.kind === "header" || st.kind === "footer") {
      for (let k = 0; k < pg.pages; k++) if (pg.storyAt(k, st.kind)?.part === st.part) return k;
      return -1;
    }
    const lists: ({ id: string }[] | undefined)[] = st.kind === "footnote" ? pg.notesByPage : pg.endByPage.map((a) => a?.notes);
    return lists.findIndex((list) => list?.some((n) => n.id === st.noteId));
  },
  /** Whether Go to can reach a bookmark: it is in the text, or in a header, footer or note that a page shows. */
  canSelectBookmark(this: Editor, b: BookmarkInfo): boolean {
    return b.story ? this.bookmarkPage(b) >= 0 : !!this.flow.querySelector(`[data-p="${b.p}"]`);
  },
  /** Select the text a bookmark covers (the caret for a point) and scroll to it. A bookmark in a header, footer or note opens that for editing. False when it can't be reached. */
  selectBookmark(this: Editor, b: BookmarkInfo): boolean {
    const at = { p: b.p, o: b.o };
    if (!b.story) this.ensureDrawn(b.p);
    if (b.story) {
      // Already editing it: just select.
      if (!this.root.querySelector(`[data-p="${b.p}"]`)) {
        const page = this.bookmarkPage(b);
        if (page < 0) return false;
        const zone = b.story.kind === "header" || b.story.kind === "footer" ? b.story.kind : b.story.kind === "footnote" ? "notes" : "endnotes";
        this.enterStory(zone, page, at);
        if (!this.root.querySelector(`[data-p="${b.p}"]`)) return false;
      }
      this.scrollStoryIntoView();
    } else {
      if (this.story && !this.root.querySelector(`[data-p="${b.p}"]`)) this.exitStory(false);
      const el = this.root.querySelector<HTMLElement>(`[data-p="${b.p}"]`);
      if (!el) return false;
      this.ensureDrawn(el);
      const c = this.canvas.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      // A block with no layout box (left out, folded away) reads zeros: the view stays where it is rather than going to the top of the window.
      if (r.width || r.height) this.canvas.scrollTo({ top: this.canvas.scrollTop + r.top - c.top - 120 });
    }
    this.select({ anchor: at, focus: { p: b.endP, o: b.endO } });
    this.focus();
    this.emitSelection();
    return true;
  },
  /** Scroll to the paragraph where a bookmark starts (the target of an internal link). */
  goToBookmark(this: Editor, name: string): void {
    // Bookmark names are the same whatever their case.
    const lower = name.toLowerCase();
    const el = Array.from(this.flow.querySelectorAll<HTMLElement>("[data-bm]")).find((e) => e.dataset.bm!.split(" ").some((n) => n.toLowerCase() === lower));
    if (el?.dataset.p) return this.scrollToParagraph(Number(el.dataset.p));
    // A bookmark in a header, footer or note: open that for editing.
    const inStory = this.bookmarks().find((b) => b.story && b.name.toLowerCase() === lower);
    if (inStory) this.selectBookmark(inStory);
  },
  /** The citation whose text covers a position. */
  citationAt(this: Editor, p: number, o: number): CitationInfo | null {
    return this.engine.citations().find((c) => c.p === p && o >= c.o && o < c.o + Math.max(1, c.len)) ?? null;
  },
  /** The citation the caret is in or the selection covers, if any. */
  citationAtCaret(this: Editor): CitationInfo | null {
    if (!this.engine) return null;
    const r = this.range();
    if (!r) return null;
    return this.engine.citations().find((c) => c.p === r[0].p && r[0].o >= c.o && r[0].o <= c.o + c.len && (r[1].p !== c.p || r[1].o <= c.o + c.len)) ?? null;
  },
  /** The citation a deletion at the caret would touch: the one the caret is at the end of (Backspace) or the start of (Delete), or inside. */
  citationBeside(this: Editor, at: MPos, backward: boolean): CitationInfo | null {
    if (!this.paraElement(at.p)?.querySelector(".cite")) return null;
    return this.engine.citations().find((c) => c.p === at.p && (backward ? at.o > c.o && at.o <= c.o + c.len : at.o >= c.o && at.o < c.o + c.len)) ?? null;
  },
  /** Select a citation's text as one unit. */
  selectCitation(this: Editor, c: CitationInfo): void {
    this.select({ anchor: { p: c.p, o: c.o }, focus: { p: c.p, o: c.o + c.len } });
    this.emitSelection();
  },
  /** Select the citation under a clicked element and tell the host, which shows its popover. */
  openCitation(this: Editor, el: HTMLElement): void {
    const p = Number(el.closest<HTMLElement>("[data-p]")?.dataset.p);
    const o = Number(el.dataset.o);
    if (!Number.isFinite(p) || !Number.isFinite(o)) return;
    const c = this.citationAt(p, o);
    if (!c) return;
    this.selectCitation(c);
    this.hooks.onCitation?.(c, el);
  },
  /** The element of a citation, once drawn (to anchor a popover to). */
  citationElement(this: Editor, c: CitationInfo): HTMLElement | null {
    const para = this.paraElement(c.p);
    if (!para) return null;
    return Array.from(para.querySelectorAll<HTMLElement>(".cite")).find((e) => Number(e.dataset.o) >= c.o && Number(e.dataset.o) < c.o + Math.max(1, c.len)) ?? null;
  },
  /** Cite sources after the selection (at the caret when nothing is selected). */
  insertCitation(this: Editor, tags: string[], page?: string): boolean {
    const r = this.range();
    if (!r || !tags.length) return false;
    const ok = this.exec({ op: "insert_citation", at: r[1], tags, ...(page ? { page } : {}) });
    if (ok) this.focus();
    return !!ok;
  },
  /** Change a citation (its sources, page, or omitted parts) and keep it selected. Returns the citation as it is now. */
  editCitation(this: Editor, c: CitationInfo, patch: { tags?: string[]; page?: string; suppress_author?: boolean; suppress_year?: boolean; suppress_title?: boolean }): CitationInfo | null {
    if (!this.exec({ op: "edit_citation", at: { p: c.p, o: c.o + Math.min(1, c.len) }, ...patch })) return null;
    const now = this.engine.citations().find((x) => x.p === c.p && x.o === c.o) ?? null;
    if (now) this.select({ anchor: { p: now.p, o: now.o }, focus: { p: now.p, o: now.o + now.len } });
    return now;
  },
  /** Remove a citation from the text. */
  removeCitation(this: Editor, c: CitationInfo): boolean {
    // The engine takes the citation out and writes the others again; in a note style the note goes with it.
    return !!this.exec({ op: "remove_citation", at: { p: c.p, o: c.o } }, { select: { p: c.p, o: c.o } });
  },
  /** The bibliography block in the text, if the document has one. */
  bibliographyElement(this: Editor): HTMLElement | null {
    return this.flow.querySelector<HTMLElement>('.fc[data-gallery="Bibliographies"]');
  },
  /**
   * Insert the bibliography after the block that holds the caret. A document has one: when it already has it, the view goes
   * there instead. Returns true when one was inserted.
   */
  insertBibliography(this: Editor): boolean {
    const existing = this.bibliographyElement();
    if (existing) {
      const first = existing.querySelector<HTMLElement>("[data-p]");
      if (first) this.scrollToParagraph(Number(first.dataset.p));
      return false;
    }
    const r = this.range();
    if (!r) return false;
    const table = this.paraElement(r[1].p)?.closest<HTMLElement>(".ftw");
    const after = table?.dataset.id ? Number(table.dataset.id) : r[1].p;
    return !!this.exec({ op: "insert_bibliography", after }, { select: this.sel });
  },
};

declare module "./editor" {
  interface Editor {
    bookmarks(): BookmarkInfo[];
    suggestBookmarkName(): string;
    addBookmark(name?: string): string | null;
    bookmarkMarker(name: string): HTMLElement | null;
    renameBookmark(from: string, to: string, onError?: (message: string) => void): boolean;
    removeBookmark(name: string): boolean;
    bookmarkPage(b: BookmarkInfo): number;
    canSelectBookmark(b: BookmarkInfo): boolean;
    selectBookmark(b: BookmarkInfo): boolean;
    goToBookmark(name: string): void;
    citationAt(p: number, o: number): CitationInfo | null;
    citationAtCaret(): CitationInfo | null;
    citationBeside(at: MPos, backward: boolean): CitationInfo | null;
    selectCitation(c: CitationInfo): void;
    openCitation(el: HTMLElement): void;
    citationElement(c: CitationInfo): HTMLElement | null;
    insertCitation(tags: string[], page?: string): boolean;
    editCitation(c: CitationInfo, patch: { tags?: string[]; page?: string; suppress_author?: boolean; suppress_year?: boolean; suppress_title?: boolean; }): CitationInfo | null;
    removeCitation(c: CitationInfo): boolean;
    bibliographyElement(): HTMLElement | null;
    insertBibliography(): boolean;
  }
}
