// Formatting what is selected: runs, paragraphs, lists, links, tables, breaks, ruby and equations.
import type { Editor, EquationAtCaret, ListKind, RubyAtCaret } from "./editor";
import { applyFormat } from "./format-painter";
import type { BreakKind, FormatState, MPos, Op, ParaPatch, RunFmt, RunPatch } from "@docxtor/core";
import { isEmpty } from "./multi-select";
import type { VisualAlign } from "./align";
import { fileAlign } from "./align";
import { t } from "@docxtor/extensions";
import { autoCaption } from "./lists";

/** The numbered list presets of the engine's `set_list_style`. */
const NUMBERED_STYLES = ["decimal", "paren", "outline", "upper", "roman", "zero"];

export const formatMethods = {
  /** Remember the formatting at the selection: the text's, and its paragraph's (Format, Copy formatting). */
  copyFormat(this: Editor): void {
    this.painter.copy();
  },
  /** Apply the remembered formatting to the selection (see format-painter.ts for what goes where). */
  pasteFormat(this: Editor): void {
    if (this.painter.copied) applyFormat(this, this.painter.copied, "command");
  },
  /** Superscript or subscript on, or back to the baseline when it's already on. */
  toggleVertAlign(this: Editor, v: "superscript" | "subscript"): void {
    const on = this.formatWithPending()?.run.vertAlign === v;
    this.setRun({ vertAlign: on ? "baseline" : v });
  },
  /** One step up or down Word's list of font sizes. */
  stepFontSize(this: Editor, d: 1 | -1): void {
    const cur = this.formatWithPending()?.run.size ?? 11;
    const sizes = [6, 7, 8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 48, 60, 72, 96];
    const next = d > 0 ? (sizes.find((s) => s > cur) ?? cur + 4) : ([...sizes].reverse().find((s) => s < cur) ?? Math.max(1, cur - 1));
    this.setRun({ size: next });
  },
  setRun(this: Editor, patch: RunPatch): void {
    const r = this.range();
    if (!r) return;
    // With several ranges selected, the formatting goes onto all of them in one step.
    const chosen = this.multi ? this.ranges().filter((x) => !isEmpty(x)) : [];
    if (chosen.length) {
      this.exec(chosen.map((x) => ({ op: "format_text", from: x.from, to: x.to, format: patch })), { select: this.sel, keepExtra: true });
      return;
    }
    if (this.collapsed()) {
      // Applies to the next typed text.
      this.pending = { ...(this.pending ?? {}), ...patch };
      this.hooks.onSelection?.(this.formatWithPending());
      this.focus();
      return;
    }
    this.exec({ op: "format_text", from: r[0], to: r[1], format: patch }, { select: this.sel });
  },
  formatWithPending(this: Editor): FormatState | null {
    const f = this.format();
    if (!f || !this.pending) return f;
    // The pending patch speaks of the same properties as the run does, except for the pieces of a theme colour and the
    // OpenType settings, which are written differently.
    const { underlineTheme: _u, themeColor: _c, style, ...p } = this.pending;
    return { ...f, charStyle: style === undefined ? f.charStyle : style || undefined, run: { ...f.run, ...(p as Partial<RunFmt>), underline: p.underline ?? f.run.underline } };
  },
  setPara(this: Editor, patch: ParaPatch): void {
    const ids = this.selectedParagraphs();
    if (ids.length) this.exec({ op: "format_paragraphs", ids, format: patch }, { select: this.sel, keepExtra: true });
  },
  /**
   * Indents from the ruler. In a list, the left and first-line indents move every item of the selected items' list
   * levels, as in Word and Google Docs, not only the item with the caret; the right indent is the selection's. "The
   * list" is the run of items around the selection: Word files often give unrelated lists pages apart the same list id,
   * and those stay where they are. Headers and footers are drawn without `data-p`, so their items are never counted.
   */
  setIndents(this: Editor, patch: ParaPatch): void {
    const ids = this.selectedParagraphs();
    if (!ids.length) return;
    if (patch.indRight === undefined) {
      const paras = this.allParas();
      const at = new Map(paras.map((el, i) => [Number(el.dataset.p), i]));
      const listOf = (el: HTMLElement) => el.dataset.ln?.split(":")[0];
      const seen = new Set(ids);
      for (const id of ids) {
        const i = at.get(id);
        const ln = i === undefined ? undefined : paras[i].dataset.ln;
        if (i === undefined || !ln) continue;
        const list = listOf(paras[i]);
        for (const step of [-1, 1]) {
          for (let k = i + step; k >= 0 && k < paras.length && listOf(paras[k]) === list; k += step) {
            if (paras[k].dataset.ln === ln) seen.add(Number(paras[k].dataset.p));
          }
        }
      }
      ids.splice(0, ids.length, ...[...seen].sort((x, y) => (at.get(x) ?? 0) - (at.get(y) ?? 0)));
    }
    this.exec({ op: "format_paragraphs", ids, format: patch }, { select: this.sel, keepExtra: true });
  },
  /**
   * Align the selected paragraphs by what shows on screen: left means the left side, also in a paragraph that runs
   * right to left (where the file says `right`, see align.ts).
   */
  setAlign(this: Editor, visual: VisualAlign): void {
    this.setParaWith({}, visual);
  },
  /**
   * Apply a paragraph patch (and further ops, such as a change to the default tab width) as one undo step. `align` is
   * by what shows on screen: each paragraph gets the file's word for it in the direction it ends up with.
   */
  setParaWith(this: Editor, patch: ParaPatch, align?: VisualAlign, more: Op[] = []): void {
    const ids = this.selectedParagraphs();
    if (!ids.length) return;
    const groups = new Map<string, number[]>();
    for (const id of ids) {
      const rtl = patch.bidi ?? !!this.engine.formatAt({ p: id, o: 0 })?.para.bidi;
      const a = align ? fileAlign(align, rtl) : "";
      groups.set(a, [...(groups.get(a) ?? []), id]);
    }
    const ops: Op[] = [...groups].map(([a, list]) => ({ op: "format_paragraphs", ids: list, format: a ? { ...patch, align: a } : patch }));
    this.exec([...ops, ...more], { select: this.sel, keepExtra: true });
  },
  /** Make the selected paragraphs run left to right or right to left. */
  setDirection(this: Editor, rtl: boolean): void {
    this.setPara({ bidi: rtl });
  },
  setStyle(this: Editor, style: string): void {
    const ids = this.selectedParagraphs();
    if (ids.length) this.exec({ op: "set_style", ids, style }, { select: this.sel, keepExtra: true });
  },
  /**
   * The Bulleted list and Numbered list keys: disc bullets or decimal numbering for paragraphs that
   * aren't in that kind of list yet (a checklist or the other kind switches over), and off when
   * they all are. They use the presets, so the galleries show what's in use.
   */
  toggleList(this: Editor, kind: ListKind): void {
    const ids = this.selectedParagraphs();
    if (!ids.length) return;
    const inKind = ids.every((id) => this.engine.formatAt({ p: id, o: 0 })?.list === kind);
    if (inKind) this.exec({ op: "set_list_style", ids, style: "none" }, { select: this.sel, keepExtra: true });
    else this.applyListStyle(ids, kind === "bullet" ? "disc" : "decimal");
  },
  /**
   * Give paragraphs a list style as one undo step. Paragraphs that start a numbered list right
   * after an item of a numbered list in the same style carry on its numbering; a paragraph in
   * between, or another style, starts a list at 1.
   */
  applyListStyle(this: Editor, ids: number[], style: string): void {
    this.exec(this.listStyleOps(ids, style), { select: this.sel, keepExtra: true });
  },
  /** The ops that give paragraphs a list style (see `applyListStyle`). */
  listStyleOps(this: Editor, ids: number[], style: string): Op[] {
    const ops: Op[] = [{ op: "set_list_style", ids, style }];
    if (NUMBERED_STYLES.includes(style) && !this.engine.formatAt({ p: ids[0], o: 0 })?.list) {
      const order = this.allParas().map((p) => Number(p.dataset.p));
      const prev = order[order.indexOf(ids[0]) - 1];
      const f = prev === undefined ? null : this.engine.formatAt({ p: prev, o: 0 });
      if (f?.list === "decimal" && f.listStyle === style) ops.push({ op: "continue_numbering", id: ids[0] });
    }
    return ops;
  },
  /** Give the selected paragraphs a list style (a preset id, or "none" to leave the list). */
  setListStyle(this: Editor, style: string): void {
    const ids = this.selectedParagraphs();
    if (ids.length) this.applyListStyle(ids, style);
  },
  /** Checklist on, or off when the selection already is one. */
  toggleChecklist(this: Editor): void {
    this.setListStyle(this.format()?.list === "checklist" ? "none" : "checklist");
  },
  /** Number the paragraph at the caret from 1 again. */
  restartNumbering(this: Editor): void {
    const r = this.range();
    if (r) this.exec({ op: "restart_numbering", id: r[0].p }, { select: this.sel });
  },
  /** Carry on from the list above (the engine says so when there is none). */
  continueNumbering(this: Editor): void {
    const r = this.range();
    if (r) this.exec({ op: "continue_numbering", id: r[0].p }, { select: this.sel });
  },
  /** The checklist items among `ids` (default: the selection), with whether each is ticked. */
  checklistItems(this: Editor, ids = this.selectedParagraphs()): { id: number; checked: boolean }[] {
    const items: { id: number; checked: boolean }[] = [];
    for (const id of ids) {
      const f = this.engine.formatAt({ p: id, o: 0 });
      if (f?.list === "checklist") items.push({ id, checked: !!f.checked });
    }
    return items;
  },
  /** Tick or untick checklist items (default: the selected ones; all ticked unless they all are already). */
  setChecked(this: Editor, checked?: boolean, ids?: number[]): void {
    const items = this.checklistItems(ids);
    if (!items.length) return;
    const on = checked ?? !items.every((i) => i.checked);
    this.exec({ op: "set_checked", ids: items.map((i) => i.id), checked: on }, { select: this.sel });
  },
  indent(this: Editor, delta: number): void {
    const ids = this.selectedParagraphs();
    if (ids.length) this.exec({ op: "indent", ids, delta }, { select: this.sel, keepExtra: true });
  },
  clearFormatting(this: Editor): void {
    const r = this.range();
    if (!r) return;
    const ops: Op[] = [{ op: "format_paragraphs", ids: this.selectedParagraphs(), format: { clear: true, style: "Normal" } }];
    const chosen = this.multi ? this.ranges().filter((x) => !isEmpty(x)) : this.collapsed() ? [] : [{ from: r[0], to: r[1] }];
    ops.unshift(...chosen.map((x): Op => ({ op: "format_text", from: x.from, to: x.to, format: { clear: true } })));
    this.exec(ops, { select: this.sel, keepExtra: true });
  },
  /** Link the selection to `url`; with only a caret, `label` (the URL itself without one) is inserted as the linked text. */
  setLink(this: Editor, url: string | null, label?: string): void {
    const r = this.range();
    if (!r) return;
    if (this.collapsed() && url) {
      const at = r[0];
      const len = (label ?? url).length;
      this.exec([
        { op: "insert_text", at, text: label ?? url },
        { op: "set_link", from: at, to: { p: at.p, o: at.o + len }, url },
      ], { select: { p: at.p, o: at.o + len } });
      return;
    }
    const chosen = this.multi ? this.ranges().filter((x) => !isEmpty(x)) : [{ from: r[0], to: r[1] }];
    if (chosen.some((x) => x.from.p !== x.to.p)) {
      this.hooks.onError?.(t("Links must stay within one paragraph."));
      return;
    }
    this.exec(chosen.map((x): Op => ({ op: "set_link", from: x.from, to: x.to, url })), { select: this.sel, keepExtra: true });
  },
  /** The link at the caret (or the start of the selection): its target and its whole range. */
  linkAt(this: Editor): { href: string; from: MPos; to: MPos } | null {
    const r = this.range();
    const el = r && this.paraElement(r[0].p);
    if (!r || !el) return null;
    const at = r[0];
    const len = (x: HTMLElement) => (x.classList.contains("atom") ? 1 : (x.textContent ?? "").length);
    const segs = Array.from(el.querySelectorAll<HTMLElement>(".lnk"));
    const hit = segs.find((x) => {
      const o = Number(x.dataset.o);
      return at.o >= o && (at.o < o + len(x) || (at.o === o + len(x) && this.collapsed()));
    });
    if (!hit) return null;
    const href = hit.dataset.href!;
    // Contiguous segments with the same target make up the link.
    let i = segs.indexOf(hit);
    let j = i;
    const next = (a: HTMLElement, b: HTMLElement) => b.dataset.href === href && Number(b.dataset.o) === Number(a.dataset.o) + len(a);
    while (i > 0 && next(segs[i - 1], segs[i])) i--;
    while (j < segs.length - 1 && next(segs[j], segs[j + 1])) j++;
    const last = segs[j];
    return { href, from: { p: at.p, o: Number(segs[i].dataset.o) }, to: { p: at.p, o: Number(last.dataset.o) + len(last) } };
  },
  /** Open the link at the caret: a web address in a new tab, a bookmark by scrolling to it. */
  followLink(this: Editor): void {
    const l = this.linkAt();
    if (l) this.openAddress(l.href);
  },
  /**
   * Open an address the document holds (a link, a chip's Open button): the one place that does. A bookmark in the
   * document scrolls to it. Anything else goes to the host first, which opens its own addresses (the Docxtor app, its
   * documents); then only web and mail addresses open, in a new tab. Others (`javascript:`, `data:`, `file:`, an app's
   * own scheme) are refused with a notice, since the address comes from a file someone else may have made. The editor
   * never changes the page's own address. False when nothing was opened.
   */
  openAddress(this: Editor, href: string): boolean {
    // A bookmark's name has no slash: `#/…` is an app's own address (older file chips saved one), for the host.
    if (href.startsWith("#") && !href.startsWith("#/")) {
      this.goToBookmark(href.slice(1));
      return true;
    }
    if (this.hooks.openAddress?.(href)) return true;
    let url: URL | null = null;
    try {
      url = new URL(href);
    } catch {
      /* not an address */
    }
    if (!url || !["http:", "https:", "mailto:"].includes(url.protocol)) {
      this.hooks.onError?.(t("Docxtor only opens web and email addresses. This link goes to “{address}”.", { address: href.length > 80 ? `${href.slice(0, 80)}…` : href }));
      return false;
    }
    window.open(url.href, "_blank", "noopener");
    return true;
  },
  /** Remove the link at the caret (the whole link) or the links in the selection, keeping the text. */
  removeLink(this: Editor): void {
    if (this.collapsed()) {
      const l = this.linkAt();
      if (!l) return;
      this.select({ anchor: l.from, focus: l.to });
    }
    this.setLink(null);
  },
  insertTable(this: Editor, rows: number, cols: number): void {
    const r = this.range();
    if (!r) return;
    const res = this.exec({ op: "insert_table", after: r[1].p, rows, cols });
    if (res) {
      this.focus();
      autoCaption(this, "table");
    }
  },
  /** Run a table edit (a row or column added or deleted, the table deleted) at the caret; false when nothing changed. */
  tableAction(this: Editor, action: string): boolean {
    const r = this.range();
    return !!r && !!this.exec({ op: "table_edit", at: r[0].p, action });
  },
  insertPageBreak(this: Editor): void {
    const r = this.range();
    if (!r) return;
    this.exec([{ op: "split_paragraph", at: r[0] }]);
    const id = this.sel?.focus.p;
    if (id !== undefined) this.exec({ op: "format_paragraphs", ids: [id], format: { pageBreakBefore: true } }, { select: { p: id, o: 0 } });
  },
  /**
   * Insert a page, column or section break at the caret (a selection is replaced). The caret ends up
   * after the break, which for a section break is the start of the new section.
   */
  insertBreak(this: Editor, kind: BreakKind): void {
    if (this.readOnly) return;
    const r = this.range();
    if (!r) return;
    const ops: Op[] = this.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
    ops.push({ op: "insert_break", at: r[0], kind });
    if (!this.exec(ops)) return;
    // Sections changed (or may have): headers, footers, margins and line numbers follow them.
    if (kind !== "page" && kind !== "column") this.reloadPageSetup();
  },
  /** The phonetic guide that is exactly what is selected (a click on a guide selects it as one unit), if any. */
  rubyAtCaret(this: Editor): RubyAtCaret | null {
    const r = this.range();
    if (!r || r[0].p !== r[1].p || r[1].o !== r[0].o + 1) return null;
    const ruby = this.engine.rubyAt(r[0]);
    return ruby ? { at: r[0], ruby } : null;
  },
  /** The element of a phonetic guide, once drawn (to anchor its box to). */
  rubyElement(this: Editor, at: MPos): HTMLElement | null {
    return this.paraElement(at.p)?.querySelector<HTMLElement>(`.atom.ruby[data-o="${at.o}"]`) ?? null;
  },
  /** The equation that is exactly what is selected (a click on an equation selects it as one unit), if any. */
  equationAtCaret(this: Editor): EquationAtCaret | null {
    const r = this.range();
    if (!r || r[0].p !== r[1].p || r[1].o !== r[0].o + 1) return null;
    const eq = this.engine.equationAt(r[0]);
    return eq ? { at: r[0], latex: eq.latex, display: eq.display } : null;
  },
  /** The element of an equation, once drawn (to anchor its editor to). */
  equationElement(this: Editor, at: MPos): HTMLElement | null {
    return this.paraElement(at.p)?.querySelector<HTMLElement>(`.atom.math[data-o="${at.o}"]`) ?? null;
  },
  /**
   * Put an equation written in LaTeX at the caret (a selection is replaced). With `display` it gets a line of its
   * own. The message of a LaTeX error goes to `onError` when given; returns whether it was inserted.
   */
  insertEquation(this: Editor, latex: string, display: boolean, onError?: (message: string) => void): boolean {
    const r = this.range();
    if (!r || this.readOnly) return false;
    const ops: Op[] = this.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
    ops.push({ op: "insert_equation", at: r[0], latex, display });
    return !!this.exec(ops, { onError });
  },
  /** Change the equation at a position (and, when `display` is given, move it onto a line of its own or back into the text). */
  setEquation(this: Editor, at: MPos, latex: string, display?: boolean, onError?: (message: string) => void): boolean {
    if (this.readOnly) return false;
    return !!this.exec({ op: "set_equation", at, latex, ...(display === undefined ? {} : { display }) }, { onError });
  },
  /** A horizontal line across the text width at the caret (a selection is replaced); the caret ends up on the line after it. */
  insertHorizontalLine(this: Editor): void {
    if (this.readOnly) return;
    const r = this.range();
    if (!r) return;
    const ops: Op[] = this.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
    ops.push({ op: "insert_horizontal_line", at: r[0] });
    this.exec(ops);
  },
};

declare module "./editor" {
  interface Editor {
    copyFormat(): void;
    pasteFormat(): void;
    toggleVertAlign(v: "superscript" | "subscript"): void;
    stepFontSize(d: 1 | -1): void;
    setRun(patch: RunPatch): void;
    formatWithPending(): FormatState | null;
    setPara(patch: ParaPatch): void;
    setIndents(patch: ParaPatch): void;
    setAlign(visual: VisualAlign): void;
    setParaWith(patch: ParaPatch, align?: VisualAlign, more?: Op[]): void;
    setDirection(rtl: boolean): void;
    setStyle(style: string): void;
    toggleList(kind: ListKind): void;
    applyListStyle(ids: number[], style: string): void;
    listStyleOps(ids: number[], style: string): Op[];
    setListStyle(style: string): void;
    toggleChecklist(): void;
    restartNumbering(): void;
    continueNumbering(): void;
    checklistItems(ids?: number[]): { id: number; checked: boolean; }[];
    setChecked(checked?: boolean, ids?: number[]): void;
    indent(delta: number): void;
    clearFormatting(): void;
    setLink(url: string | null, label?: string): void;
    linkAt(): { href: string; from: MPos; to: MPos; } | null;
    followLink(): void;
    openAddress(href: string): boolean;
    removeLink(): void;
    insertTable(rows: number, cols: number): void;
    tableAction(action: string): boolean;
    insertPageBreak(): void;
    insertBreak(kind: BreakKind): void;
    rubyAtCaret(): RubyAtCaret | null;
    rubyElement(at: MPos): HTMLElement | null;
    equationAtCaret(): EquationAtCaret | null;
    equationElement(at: MPos): HTMLElement | null;
    insertEquation(latex: string, display: boolean, onError?: (message: string) => void): boolean;
    setEquation(at: MPos, latex: string, display?: boolean, onError?: (message: string) => void): boolean;
    insertHorizontalLine(): void;
  }
}
