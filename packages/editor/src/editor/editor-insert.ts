// Sections and columns at the caret, margins and rulers, and inserting pictures, text, Markdown and fragments.
import type { Editor } from "./editor";
import type { Columns, Fragment, MPos, Op, PageSetup, SectionInfo } from "@docxtor/core";
import { pictureExtension } from "./picture";
import { t } from "@docxtor/extensions";
import { tw } from "./render";
import { autoCaption } from "./lists";
import { fieldInput, updateExtensionFields } from "./fields";

export const insertMethods = {
  /** Index into `pageInfo.sections` of the section holding the caret; the last one in a header, footer or note. */
  caretSection(this: Editor): number {
    const last = Math.max(0, (this.pageInfo.sections?.length ?? 1) - 1);
    if (this.story) return last;
    const i = this.format()?.section;
    return i === undefined ? last : Math.min(Math.max(0, i), last);
  },
  /** Size, margins and the rest of the section holding the caret (the document's page setup when it lists no sections). */
  caretSectionInfo(this: Editor): SectionInfo | PageSetup {
    const p = this.pageInfo;
    return p.sections?.[this.caretSection()] ?? p;
  },
  /** Columns of the section holding the caret: how many, the gap between them (twips) and whether a line is drawn. */
  caretColumns(this: Editor): Columns {
    const info = this.caretSectionInfo();
    const c = info.cols as Columns | number | undefined;
    if (c && typeof c === "object") return c;
    return { num: typeof c === "number" ? c : 1, space: 720, sep: false, widths: [] };
  },
  /**
   * Put the caret's section (a document with one section: all of it) in `count` columns. `all` changes every section
   * instead. The gap and the line between columns stay as they are unless given.
   */
  setColumns(this: Editor, count: number, o: { space?: number; sep?: boolean; all?: boolean; widths?: number[]; spaces?: number[] } = {}): void {
    if (this.readOnly || !this.engine) return;
    const several = (this.pageInfo.sections?.length ?? 1) > 1;
    const anchor = several && !o.all ? this.caretParagraph() : undefined;
    this.exec({
      op: "set_columns",
      count,
      ...(o.space !== undefined ? { space: o.space } : {}),
      ...(o.sep !== undefined ? { sep: o.sep } : {}),
      ...(o.widths ? { widths: o.widths, ...(o.spaces ? { spaces: o.spaces } : {}) } : {}),
      ...(anchor !== undefined ? { section: anchor } : {}),
    });
    this.reloadPageSetup();
  },
  /** A paragraph inside the caret's section, to address that section in ops; undefined in a header, footer or note. */
  caretParagraph(this: Editor): number | undefined {
    return this.story ? undefined : this.range()?.[0].p;
  },
  /** A paragraph inside section i, to address that section in ops. */
  sectionAnchor(this: Editor, i: number): number | undefined {
    const end = this.pageInfo.sections?.[i]?.end;
    if (end !== undefined) return end;
    const ps = this.flow.querySelectorAll<HTMLElement>(":scope > [data-p], :scope > .cols > [data-p], :scope > .far-box > [data-p]");
    const last = ps[ps.length - 1];
    return last ? Number(last.dataset.p) : undefined;
  },
  /** Set the top and bottom margins of the section that owns a page (the vertical ruler's drag), or of the whole document when it has one section. */
  setMargins(this: Editor, page: number, top: number, bottom: number): void {
    const si = this.paginator.page(page).section;
    const info = this.pageInfo.sections?.[si] ?? this.pageInfo;
    const margins: [number, number, number, number] = [top, info.margin[1], bottom, info.margin[3]];
    const several = (this.pageInfo.sections?.length ?? 1) > 1;
    const section = several ? this.sectionAnchor(si) : undefined;
    this.exec({ op: "set_page_setup", ...(section !== undefined ? { section } : {}), margins });
    this.reloadPageSetup();
    this.updateVRuler();
  },
  /** The page the vertical ruler stands beside: the caret's page when it is in view, otherwise the first page in view. */
  rulerPage(this: Editor): number {
    const pg = this.paginator;
    const clamp = (k: number) => Math.max(0, Math.min(this.pages - 1, k));
    if (this.story) return clamp(this.story.page);
    const sr = this.stage.getBoundingClientRect();
    const cr = this.canvas.getBoundingClientRect();
    const sc = this.stage.offsetHeight ? sr.height / this.stage.offsetHeight : 1;
    const viewTop = (cr.top - sr.top) / sc;
    const viewBottom = (cr.bottom - sr.top) / sc;
    const first = clamp(pg.firstPageBelow(viewTop));
    if (!this.sel) return first;
    let y: number | null = null;
    const s = window.getSelection();
    if (this.hasFocus() && s && s.rangeCount) {
      const r = s.getRangeAt(0).getBoundingClientRect();
      if (r.height > 0) y = (r.top + r.height / 2 - sr.top) / sc;
    }
    if (y === null) {
      const el = this.root.querySelector<HTMLElement>(`[data-p="${this.sel.focus.p}"]`);
      if (el) y = (el.getBoundingClientRect().top - sr.top) / sc + 1;
    }
    if (y === null) return first;
    const k = clamp(pg.pageAt(y));
    const b = pg.page(k);
    return b.top + b.h > viewTop && b.top < viewBottom ? k : first;
  },
  /** Put the vertical ruler beside the right page, with the margins of the section that owns it. */
  updateVRuler(this: Editor): void {
    if (!this.engine || !this.paginator || !this.rulerShown) return;
    const k = this.rulerPage();
    const info = this.pageInfo.sections?.[this.paginator.page(k).section] ?? this.pageInfo;
    this.vruler.update(k, info.margin[0], info.margin[2]);
  },
  async insertImage(this: Editor, file: File): Promise<void> {
    await this.insertPicture({ bytes: new Uint8Array(await file.arrayBuffer()), name: file.name, type: file.type });
  },
  /**
   * Put a picture (bytes and a name) at the caret, as large as it is and no wider than the page allows. A format the
   * browser cannot show (TIFF, EMF, WMF) is measured from what the engine shows in its place.
   */
  async insertPicture(this: Editor, pic: { bytes: Uint8Array; name: string; type?: string; alt?: string }): Promise<void> {
    if (this.readOnly) return;
    const r = this.range();
    if (!r) return;
    const ext = pictureExtension(pic.bytes);
    if (!ext) {
      this.hooks.onError?.(t("That file isn't a picture Docxtor can use. Use a PNG, JPEG, GIF, WebP, BMP, TIFF, SVG, EMF or WMF picture."));
      return;
    }
    const media = this.engine.addMedia(ext, pic.bytes);
    const img = new Image();
    img.src = this.engine.mediaUrl(media);
    await img.decode().catch(() => undefined);
    const sec = this.caretSectionInfo();
    const maxW = (tw(sec.width) - tw(sec.margin[1]) - tw(sec.margin[3])) * 0.75;
    let w = (img.naturalWidth || 400) * 0.75;
    let h = (img.naturalHeight || 300) * 0.75;
    if (w > maxW) {
      h *= maxW / w;
      w = maxW;
    }
    if (this.exec({ op: "insert_image", at: r[0], media, width: Math.round(w), height: Math.round(h), alt: pic.alt ?? pic.name })) autoCaption(this, "image");
  },
  /** Turn the browser's own spellcheck on or off (the body and any header, footer or notes being edited). */
  setNativeSpellcheck(this: Editor, on: boolean): void {
    this.nativeSpell = on;
    this.flow.spellcheck = on;
    if (this.storyEl) this.storyEl.spellcheck = on;
  },
  /**
   * Replace the text between two positions of one paragraph as one undo step, with the formatting of the
   * text it replaces, and leave the caret after it. Returns whether the edit was made.
   */
  replaceText(this: Editor, from: MPos, to: MPos, text: string): boolean {
    if (this.readOnly || !this.engine) return false;
    // Typing at the end of the old text gives the new text its formatting; then the old text goes.
    const caret = { p: from.p, o: from.o + text.length };
    this.lastTyping = 0;
    return !!this.exec(
      [
        { op: "insert_text", at: to, text },
        { op: "delete", from, to },
      ],
      { select: caret },
    );
  },
  /** Type text at the caret (replacing a selection) as the keyboard would: one undo step. Returns whether anything was inserted. */
  insertText(this: Editor, text: string): boolean {
    if (this.readOnly || !this.engine || !text) return false;
    const r = this.range();
    if (!r) return false;
    const fmt = this.pending ? { format: this.pending } : {};
    const ops: Op[] = this.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
    ops.push({ op: "insert_text", at: r[0], text, ...fmt });
    this.lastTyping = 0;
    return !!this.exec(ops);
  },
  insertMarkdownAtCaret(this: Editor, md: string): void {
    const r = this.range();
    if (!r) return;
    // Several ranges: the text goes in place of each of them (as plain text).
    if (this.multi) {
      this.replaceRanges(this.ranges(), md.replace(/\\(.)/g, "$1").replace(/\n{2,}/g, "\n"));
      return;
    }
    const ops: Op[] = [];
    if (!this.collapsed()) ops.push({ op: "delete", from: r[0], to: r[1] });
    ops.push({ op: "paste_markdown", at: r[0], markdown: md });
    this.exec(ops);
    // Pasted text can bring a font the document didn't use before.
    this.hooks.onFontsMayChange?.();
  },
  /** Insert a fragment (Docxtor's clipboard format, or HTML or RTF read by the engine) at the caret, replacing the selection, as one undo step. */
  insertFragmentAtCaret(this: Editor, fragment: Fragment, mode: "keep" | "merge" = "keep"): boolean {
    const r = this.range();
    if (!r || this.readOnly) return false;
    const ops: Op[] = [];
    if (!this.collapsed()) ops.push({ op: "delete", from: r[0], to: r[1] });
    ops.push({ op: "insert_fragment", at: r[0], fragment, mode });
    const done = !!this.exec(ops);
    // Pasted text can bring a font the document didn't use before.
    if (done) this.hooks.onFontsMayChange?.();
    return done;
  },
  /** Insert a field (`PAGE`, `NUMPAGES`, `DATE`) at the caret. */
  insertField(this: Editor, field: string, result?: string): boolean {
    const r = this.range();
    if (!r) return false;
    const ops: Op[] = this.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
    ops.push({ op: "insert_field", at: r[0], field, ...(result ? { result } : {}), input: fieldInput(this) });
    const ok = !!this.exec(ops);
    if (ok) void updateExtensionFields(this, true);
    return ok;
  },
};

declare module "./editor" {
  interface Editor {
    caretSection(): number;
    caretSectionInfo(): SectionInfo | PageSetup;
    caretColumns(): Columns;
    setColumns(count: number, o?: { space?: number; sep?: boolean; all?: boolean; widths?: number[]; spaces?: number[]; }): void;
    caretParagraph(): number | undefined;
    sectionAnchor(i: number): number | undefined;
    setMargins(page: number, top: number, bottom: number): void;
    rulerPage(): number;
    updateVRuler(): void;
    insertImage(file: File): Promise<void>;
    insertPicture(pic: { bytes: Uint8Array; name: string; type?: string; alt?: string; }): Promise<void>;
    setNativeSpellcheck(on: boolean): void;
    replaceText(from: MPos, to: MPos, text: string): boolean;
    insertText(text: string): boolean;
    insertMarkdownAtCaret(md: string): void;
    insertFragmentAtCaret(fragment: Fragment, mode?: "keep" | "merge"): boolean;
    insertField(field: string, result?: string): boolean;
  }
}
