// Headers, footers and notes: entering and leaving a story, footnotes and endnotes, and printing.
import { Editor } from "./editor";
import type { StoryZone } from "./layout";
import type { MPos } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { storyLink } from "./story-links";
import { paraLength, writeSelection } from "./selection";
import { layoutTabs } from "./tabs";
import { renderBlocks } from "./render";
import { updateForOutput } from "./fields";
import * as printing from "./print";

export const storyMethods = {
  /** Edit the header/footer of a page (created if missing) or the footnotes shown on it. */
  enterStory(this: Editor, zone: StoryZone, page: number, caret?: MPos): void {
    this.extra.clear();
    if (this.readOnly || this.noStories) return;
    const notesZone = zone === "notes" || zone === "endnotes";
    // A continuous layout (Pageless, Draft, Reading, Outline) shows no headers or footers: there is nothing on screen to edit.
    if (!notesZone && this.continuous) return;
    if (this.story) this.exitStory(false);
    let variant: string | undefined;
    if (!notesZone) {
      variant = this.paginator.variantAt(page, zone);
      if (!variant) {
        // Make the version this page would show, in the page's own section (later sections without their
        // own show it too, as in Word).
        const box = this.paginator.page(page);
        const sec = this.pageInfo.sections?.[box.section] ?? this.pageInfo;
        variant = sec.titlePage && box.first ? "first" : this.pageInfo.evenAndOddHeaders && box.num % 2 === 0 ? "even" : "default";
        const section = (this.pageInfo.sections?.length ?? 1) > 1 ? this.sectionAnchor(box.section) : undefined;
        try {
          const res = this.engine.apply({ op: "add_header_footer", kind: zone, variant, ...(section !== undefined ? { section } : {}) }, false);
          caret ??= res[0]?.caret;
        } catch (e) {
          this.hooks.onError?.(String((e as Error).message ?? e));
          return;
        }
        this.applyOut(this.engine.renderChanged());
        this.hooks.onChange?.();
      }
    }
    const noteIds = zone === "notes" ? (this.paginator.notesByPage[page] ?? []).map((n) => n.id) : zone === "endnotes" ? (this.paginator.endByPage[page]?.notes ?? []).map((n) => n.id) : undefined;
    if (notesZone && !noteIds?.length) return;
    this.bodySel = this.sel;
    this.story = { zone, page, variant, noteIds };
    const el = document.createElement("div");
    el.className = zone === "notes" ? "notes story-edit" : zone === "endnotes" ? "notes endnotes story-edit" : `hf ${zone === "header" ? "hdr" : "ftr"} story-edit`;
    el.contentEditable = "true";
    el.spellcheck = this.nativeSpell;
    el.setAttribute("role", "textbox");
    el.setAttribute("aria-multiline", "true");
    el.setAttribute("aria-label", zone === "header" ? t("Header") : zone === "footer" ? t("Footer") : zone === "endnotes" ? t("Endnotes") : t("Footnotes"));
    el.dataset.label = zone === "header" ? "Header" : zone === "footer" ? "Footer" : zone === "endnotes" ? "Endnotes" : "Footnotes";
    // A header or footer shared with the section before says so, like Word's "Same as Previous".
    if (storyLink(this)?.linked) el.dataset.linked = t("Same as previous");
    this.storyEl = el;
    // A citation in a note (Chicago notes) is one unit here too: a click selects it and opens its popover.
    el.addEventListener("click", (e) => {
      const cite = (e.target as HTMLElement).closest<HTMLElement>(".cite");
      if (cite && !this.readOnly && !e.shiftKey && !e.altKey && !e.metaKey && !e.ctrlKey && window.getSelection()?.isCollapsed !== false) this.openCitation(cite);
    });
    this.paginator.story = { zone, page, el };
    this.renderStory();
    this.root = el;
    this.flow.contentEditable = "false";
    this.stage.classList.add("story-mode");
    this.layoutNow();
    const paras = el.querySelectorAll<HTMLElement>("[data-p]");
    const last = paras[paras.length - 1];
    const at = caret ?? (last ? { p: Number(last.dataset.p), o: paraLength(el, Number(last.dataset.p)) } : null);
    this.sel = at ? { anchor: at, focus: at } : null;
    el.focus({ preventScroll: true });
    if (this.sel) {
      writeSelection(el, this.sel);
      this.scrollCaretIntoView();
    }
    this.hooks.onStory?.(this.story);
    this.emitSelection();
  },
  /** Back to the body text. */
  exitStory(this: Editor, restoreCaret = true): void {
    if (!this.story) return;
    // The keyboard in the story's box would be left on the page body when the box goes: it comes back to the text.
    const hadKeys = !!this.storyEl?.contains(document.activeElement);
    this.story = null;
    this.paginator.story = null;
    this.storyEl?.remove();
    this.storyEl = null;
    this.root = this.flow;
    this.flow.contentEditable = this.readOnly ? "false" : "true";
    this.stage.classList.remove("story-mode");
    this.sel = this.bodySel;
    this.layoutNow();
    if (restoreCaret || (hadKeys && this.flow.isConnected)) this.focus();
    this.hooks.onStory?.(null);
    this.emitSelection();
  },
  renderStory(this: Editor): void {
    const st = this.story;
    const el = this.storyEl;
    if (!st || !el) return;
    const page = this.engine.pageSetup();
    // The part the page shows; when its section has none of this kind, the last section's (see enterStory).
    const shown = st.zone === "notes" || st.zone === "endnotes" ? undefined : this.paginator.storyAt(st.page, st.zone);
    const blocks =
      st.zone === "notes"
        ? page.notes.filter((n) => st.noteIds?.includes(n.id)).flatMap((n) => n.blocks)
        : st.zone === "endnotes"
          ? page.endnotes.filter((n) => st.noteIds?.includes(n.id)).flatMap((n) => n.blocks)
          : (shown ?? (st.zone === "header" ? page.headers : page.footers)[st.variant ?? "default"])?.blocks ?? [];
    if (!blocks.length) {
      // Removed (e.g. undone): nothing left to edit.
      queueMicrotask(() => this.exitStory());
      return;
    }
    el.replaceChildren(...renderBlocks(blocks, this.env));
    layoutTabs(el, page.defaultTab || 720);
  },
  onDoubleClick(this: Editor, e: MouseEvent): void {
    const t = e.target as HTMLElement;
    const math = t.closest<HTMLElement>(".atom.math");
    if (math && this.root.contains(math) && !this.readOnly) {
      // An equation is one unit: a double-click opens it for editing.
      const p = Number(math.closest<HTMLElement>("[data-p]")?.dataset.p);
      const o = Number(math.dataset.o);
      if (Number.isFinite(p) && Number.isFinite(o)) {
        e.preventDefault();
        this.select({ anchor: { p, o }, focus: { p, o: o + 1 } });
        const eq = this.equationAtCaret();
        if (eq) this.hooks.onEquation?.(eq, math);
        return;
      }
    }
    const guide = t.closest<HTMLElement>(".atom.ruby");
    if (guide && this.root.contains(guide) && !this.readOnly) {
      // A phonetic guide is one unit: a double-click opens the Phonetic guide box on it.
      const p = Number(guide.closest<HTMLElement>("[data-p]")?.dataset.p);
      const o = Number(guide.dataset.o);
      if (Number.isFinite(p) && Number.isFinite(o)) {
        e.preventDefault();
        this.select({ anchor: { p, o }, focus: { p, o: o + 1 } });
        const found = this.rubyAtCaret();
        if (found) this.hooks.onRuby?.(found, guide);
        return;
      }
    }
    // A field code that is showing opens for editing.
    const code = t.closest<HTMLElement>(".fc-first");
    if (code && this.flow.contains(code) && !this.readOnly) {
      e.preventDefault();
      this.fieldCodes.edit(code);
      return;
    }
    if (this.storyEl?.contains(t)) return;
    const ref = t.closest<HTMLElement>(".atom.note[data-note]");
    if (ref && this.flow.contains(ref)) {
      e.preventDefault();
      if (ref.dataset.nk === "endnote") this.editEndnote(ref.dataset.note!);
      else this.editNote(ref.dataset.note!);
      return;
    }
    const z = this.zoneAt(e.clientY, e.clientX);
    if (z.zone === "body") return;
    e.preventDefault();
    // The endnote under the pointer gets the caret (the notes of a page are one area).
    this.enterStory(z.zone, z.page, z.zone === "endnotes" ? this.endnoteAt(z.page, e.clientY) : undefined);
  },
  /**
   * The page and zone (header, footer, footnotes or body text) under a client y coordinate. With the x
   * coordinate too, the desk beside a page narrower than the widest is body, not margin.
   */
  zoneAt(this: Editor, clientY: number, clientX?: number): { page: number; zone: StoryZone | "body"; } {
    const z = this.paginator.zoneAt(this.stageY(clientY));
    if (clientX !== undefined && z.zone !== "body") {
      const r = this.stage.getBoundingClientRect();
      const sc = this.stage.offsetWidth ? r.width / this.stage.offsetWidth : 1;
      const x = (clientX - r.left) / sc;
      const b = this.paginator.page(z.page);
      if (x < b.left || x > b.left + b.w) return { page: z.page, zone: "body" as const };
    }
    return z;
  },
  /** Stage y (unzoomed) of a client y coordinate. */
  stageY(this: Editor, clientY: number): number {
    const r = this.stage.getBoundingClientRect();
    const sc = this.stage.offsetHeight ? r.height / this.stage.offsetHeight : 1;
    return (clientY - r.top) / sc;
  },
  /** Open the notes area holding a footnote, caret at the end of its text. */
  editNote(this: Editor, noteId: string): void {
    const page = this.paginator.notesByPage.findIndex((list) => list?.some((n) => n.id === noteId));
    if (page < 0) return;
    const note = this.paginator.notesByPage[page].find((n) => n.id === noteId)!;
    const lastBlock = note.blocks[note.blocks.length - 1];
    const caret = lastBlock && "segs" in lastBlock ? { p: lastBlock.id, o: lastBlock.len } : undefined;
    this.enterStory("notes", page, caret);
  },
  /** Add a footnote at the caret and put the caret in its text. With `mark` the note shows that text (`*`, a symbol) instead of a number. */
  insertFootnote(this: Editor, mark?: string): void {
    if (this.readOnly) return;
    if (this.story) {
      this.hooks.onError?.(t("Footnotes can only be added to the main text."));
      return;
    }
    const r = this.range();
    if (!r) return;
    const after = { p: r[1].p, o: r[1].o + 1 + (mark ? mark.length : 0) };
    let res;
    try {
      res = this.engine.apply(mark ? { op: "insert_custom_note", at: r[1], kind: "footnote", mark } : { op: "insert_footnote", at: r[1] }, false);
    } catch (e) {
      this.hooks.onError?.(String((e as Error).message ?? e));
      return;
    }
    this.sel = { anchor: after, focus: after };
    this.applyOut(this.engine.renderChanged());
    this.layoutNow();
    this.hooks.onChange?.();
    const caret = res[0]?.caret;
    const page = caret ? this.paginator.notesByPage.findIndex((list) => list?.some((n) => n.blocks.some((b) => b.id === caret.p))) : -1;
    if (page >= 0) this.enterStory("notes", page, caret);
  },
  /** The end of the endnote on page `page` under a client y (the last one when none is). */
  endnoteAt(this: Editor, page: number, clientY: number): MPos | undefined {
    const sheet = this.sheets.children[page] as HTMLElement | undefined;
    const paras = Array.from(sheet?.querySelectorAll<HTMLElement>(".endnotes [data-p]") ?? []);
    if (!paras.length) return undefined;
    const hit = paras.find((p) => {
      const r = p.getBoundingClientRect();
      return clientY >= r.top && clientY <= r.bottom;
    });
    const el = hit ?? paras[paras.length - 1];
    const id = Number(el.dataset.p);
    return { p: id, o: paraLength(el.parentElement ?? el, id) };
  },
  /** Open the endnotes area holding an endnote, caret at the end of its text. */
  editEndnote(this: Editor, noteId: string): void {
    const page = this.paginator.endByPage.findIndex((a) => a?.notes.some((n) => n.id === noteId));
    if (page < 0) return;
    const note = this.paginator.endByPage[page]!.notes.find((n) => n.id === noteId)!;
    const lastBlock = note.blocks[note.blocks.length - 1];
    const caret = lastBlock && "segs" in lastBlock ? { p: lastBlock.id, o: lastBlock.len } : undefined;
    this.enterStory("endnotes", page, caret);
    this.scrollStoryIntoView();
  },
  /** Scroll the area being edited into view (when it is a note far from the caret). */
  scrollStoryIntoView(this: Editor): void {
    const el = this.storyEl;
    if (!el) return;
    const c = this.canvas.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    if (r.top < c.top + 40 || r.bottom > c.bottom - 40) this.canvas.scrollTop += r.top - c.top - c.height / 2 + r.height / 2;
  },
  /** Add an endnote at the caret and put the caret in its text, at the end of the document. With `mark` the note shows that text instead of a number. */
  insertEndnote(this: Editor, mark?: string): void {
    if (this.readOnly) return;
    if (this.story) {
      this.hooks.onError?.(t("Endnotes can only be added to the main text."));
      return;
    }
    const r = this.range();
    if (!r) return;
    const after = { p: r[1].p, o: r[1].o + 1 + (mark ? mark.length : 0) };
    let res;
    try {
      res = this.engine.apply(mark ? { op: "insert_custom_note", at: r[1], kind: "endnote", mark } : { op: "insert_endnote", at: r[1] }, false);
    } catch (e) {
      this.hooks.onError?.(String((e as Error).message ?? e));
      return;
    }
    this.sel = { anchor: after, focus: after };
    this.applyOut(this.engine.renderChanged());
    this.layoutNow();
    this.hooks.onChange?.();
    const caret = res[0]?.caret;
    const page = caret ? this.paginator.endByPage.findIndex((a) => a?.notes.some((n) => n.blocks.some((b) => b.id === caret.p))) : -1;
    if (page >= 0) {
      this.enterStory("endnotes", page, caret);
      this.scrollStoryIntoView();
    }
  },
  /**
   * Cmd/Ctrl-click on a note's mark: from the text to the note, and from the note back to its mark. Returns
   * whether it did anything.
   */
  followNoteMark(this: Editor, mark: HTMLElement): boolean {
    if (this.story) {
      // The note's own mark (it has no id): back to the reference it belongs to.
      if (mark.dataset.note || !this.bodySel) return false;
      const back = this.bodySel;
      this.exitStory(false);
      this.sel = back;
      this.focus();
      this.scrollCaretIntoView();
      return true;
    }
    const id = mark.dataset.note;
    if (!id) return false;
    const para = mark.closest<HTMLElement>("[data-p]");
    if (para) {
      const at = { p: Number(para.dataset.p), o: Number(mark.dataset.o) + 1 };
      this.sel = { anchor: at, focus: at };
    }
    if (mark.dataset.nk === "endnote") this.editEndnote(id);
    else this.editNote(id);
    return true;
  },
  /** Print the pages exactly as they look here (see print.ts). Whatever view is on, it prints the pages. */
  print(this: Editor): void {
    updateForOutput(this);
    void this.asPages(() => printing.print(this));
  },
  /** Print every tab of a document with tabs (Print prints the one that is open, as Docs does). */
  printAllTabs(this: Editor): void {
    this.showAllTabs(true);
    updateForOutput(this);
    void this.asPages(() => printing.print(this, () => this.showAllTabs(false)));
  },
  /** Lay out every tab, or only the open one again. */
  showAllTabs(this: Editor, on: boolean): void {
    if (this.allTabs === on) return;
    this.allTabs = on;
    this.markTabs();
    this.layoutNow();
  },
  /**
   * Run something that needs the document as pages (printing, the PDF): in a view with no pages (Pageless, Reading, Outline) or
   * with headings folded, the pages come back for the duration, and the view afterwards.
   */
  async asPages<T>(this: Editor, fn: () => T | Promise<T>): Promise<T> {
    const spec = this.contSpec;
    const f = this.folds;
    if (!this.engine || (!spec && !f.active && !f.folded.size)) return fn();
    const saved = { folded: new Set(f.folded), levels: f.levels, outline: f.outline, firstLine: f.firstLine };
    f.folded.clear();
    f.levels = 0;
    f.outline = false;
    f.firstLine = false;
    if (spec) {
      this.paginator.setContinuous(null);
      this.stage.classList.remove("cont");
      this.reloadPageSetup();
    } else this.layoutNow();
    try {
      return await fn();
    } finally {
      for (const id of saved.folded) f.folded.add(id);
      f.levels = saved.levels;
      f.outline = saved.outline;
      f.firstLine = saved.firstLine;
      if (spec && this.contSpec === spec) {
        this.stage.classList.add("cont");
        this.paginator.setContinuous(this.continuousLayout(spec, this.setup ?? this.engine.pageSetup()));
        this.reloadPageSetup();
      } else this.layoutNow();
    }
  },
  /**
   * Print only the selected text, laid out on pages of its own (see print.ts). Returns false when nothing is selected
   * in the body.
   */
  printSelection(this: Editor): boolean {
    const r = this.story || this.collapsed() ? null : this.range();
    if (!r || !this.engine) return false;
    updateForOutput(this);
    return printing.printSelection(this, r[0], r[1], (host) => new Editor(host));
  },
};

declare module "./editor" {
  interface Editor {
    enterStory(zone: StoryZone, page: number, caret?: MPos): void;
    exitStory(restoreCaret?: boolean): void;
    renderStory(): void;
    onDoubleClick(e: MouseEvent): void;
    zoneAt(clientY: number, clientX?: number): { page: number; zone: StoryZone | "body"; };
    stageY(clientY: number): number;
    editNote(noteId: string): void;
    insertFootnote(mark?: string): void;
    endnoteAt(page: number, clientY: number): MPos | undefined;
    editEndnote(noteId: string): void;
    scrollStoryIntoView(): void;
    insertEndnote(mark?: string): void;
    followNoteMark(mark: HTMLElement): boolean;
    print(): void;
    printAllTabs(): void;
    showAllTabs(on: boolean): void;
    asPages<T>(fn: () => T | Promise<T>): Promise<T>;
    printSelection(): boolean;
  }
}
