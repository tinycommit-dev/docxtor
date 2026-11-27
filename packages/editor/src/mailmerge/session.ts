// The mail merge of the open document, for the dialogs and commands of the Mailings menu: what the document says
// about its merge, the record shown in place of the template (Preview results), the highlight on merge fields, and the
// ops that start a merge, set its recipients and insert fields. Every change is an op, so it undoes like any other.
import type { Engine, MergeFieldSpec, MergeInfo, MergeMainType, Op } from "@docxtor/core";
import { t, tn, type MailMergeDataSource, type RecipientTable, type UiApi } from "@docxtor/extensions";
import type { Internals } from "../builtins/index";
import { toast } from "../ui/dom";
import { PreviewStrip } from "./strip";

export class MergeSession {
  /** Merge fields have a tint on the page (Highlight merge fields). */
  highlight = false;
  private strip: PreviewStrip;
  /** The editor was made read-only for the preview (it wasn't before). */
  /** Lets go of the reason that keeps the document from changing while a record is shown (null when none is shown). */
  private releasePreviewLock: (() => void) | null = null;
  private lastEngine: Engine | null = null;

  constructor(
    readonly x: Internals,
    readonly ui: UiApi,
  ) {
    this.strip = new PreviewStrip(this);
    // A document that was opened or created has no record shown.
    x.fe.on("open", () => {
      if (this.lastEngine !== x.fe.engine) this.leavePreviewState();
      this.lastEngine = x.fe.engine;
    });
    this.lastEngine = x.fe.engine;
  }

  get engine(): Engine | null {
    return this.x.fe.engine;
  }

  /** What the document says about its merge. Null with no document. */
  info(): MergeInfo | null {
    try {
      return this.engine?.mergeInfo() ?? null;
    } catch {
      return null;
    }
  }

  get editable(): boolean {
    return !!this.engine && !this.x.fe.readOnly;
  }

  /** The position of the record shown in place of the template, or null. */
  get previewing(): number | null {
    return this.info()?.previewing ?? null;
  }

  /** Run ops as one undo step. Returns whether they went through. */
  run(ops: Op | Op[]): boolean {
    return !!this.x.editor.exec(ops, { select: this.x.editor.sel });
  }

  // -- starting and ending a merge ---------------------------------------------------------

  /** Make the document a mail merge main document of a kind (or change the kind). */
  start(main: MergeMainType): boolean {
    return this.run({ op: "set_mail_merge", merge: { main_type: main } });
  }

  stop(): boolean {
    this.stopPreview();
    return this.run({ op: "set_mail_merge", merge: null });
  }

  /** Bring a list in from a data source and use it as the recipients. */
  async choose(source: MailMergeDataSource): Promise<boolean> {
    const abort = new AbortController();
    let table: RecipientTable | null;
    try {
      table = await source.pick({ ui: this.ui, signal: abort.signal });
    } catch (e) {
      if ((e as Error).name !== "AbortError") toast(String((e as Error).message ?? e), 6000);
      return false;
    }
    return table ? this.setRecipients(source, table) : false;
  }

  setRecipients(source: MailMergeDataSource, table: RecipientTable): boolean {
    const info = this.info();
    if (info?.previewing != null) this.stopPreview();
    if (!table.columns.length) {
      toast(t("The list has no columns."));
      return false;
    }
    const name = table.name || t(source.label).replace(/…$/, "");
    const kind = table.kind ?? source.id;
    const ok = this.run({
      op: "set_mail_merge",
      merge: {
        main_type: info?.active ? info.mainType : "formLetters",
        source: { kind, name, ...(table.table ? { table: table.table } : {}), ...(table.delimiter ? { delimiter: table.delimiter } : {}) },
        columns: table.columns,
        rows: table.rows,
      },
    });
    if (ok) toast(tn(table.rows.length, "{n} recipient from {name}", "{n} recipients from {name}", { name }));
    return ok;
  }

  /** Whether there is a list to merge with; says so when there is none. */
  needRecipients(): boolean {
    const info = this.info();
    if (info?.hasRecipients) return true;
    toast(info?.active ? t("Choose the recipients first, under Mailings, Select recipients.") : t("Start a mail merge and choose the recipients first, under Mailings."));
    return false;
  }

  // -- merge fields --------------------------------------------------------------------------

  /** Insert a merge field or rule at the caret, replacing the selection. */
  insert(spec: MergeFieldSpec): boolean {
    const ed = this.x.editor;
    const r = ed.range();
    if (!r || !this.editable) return false;
    const ops: Op[] = ed.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
    ops.push({ op: "insert_merge_field", at: r[0], field: spec });
    return !!ed.exec(ops, { onError: (m) => toast(m, 6000) });
  }

  // -- highlight -------------------------------------------------------------------------------

  setHighlight(on: boolean) {
    this.highlight = on;
    this.x.editor.canvas.classList.toggle("mm-highlight", on);
  }

  // -- preview -----------------------------------------------------------------------------------

  /** Show the record at a position of the list in place of the template. */
  preview(position: number): boolean {
    const eng = this.engine;
    if (!eng || !this.needRecipients()) return false;
    const was = this.previewing !== null;
    let shown;
    try {
      shown = eng.mergePreview(position);
    } catch (e) {
      toast(String((e as Error).message ?? e), 6000);
      return false;
    }
    if (!shown) return false;
    if (!was) {
      this.releasePreviewLock = this.x.fe.lock("mail merge preview", { scope: "document" });
    }
    this.x.editor.rerender();
    this.x.fe.refreshToolbar();
    this.strip.show(shown);
    return true;
  }

  /** Move by a number of records, or to the first (`-Infinity`) or last (`Infinity`). Starts the preview if it isn't on. */
  go(to: "first" | "last" | "previous" | "next") {
    const info = this.info();
    if (!info || !this.needRecipients()) return;
    const last = Math.max(0, info.included - 1);
    const now = info.previewing ?? -1;
    const at = to === "first" ? 0 : to === "last" ? last : to === "previous" ? Math.max(0, now - 1) : Math.min(last, now + 1);
    this.preview(info.previewing === null && (to === "previous" || to === "next") ? 0 : at);
  }

  stopPreview() {
    const eng = this.engine;
    if (!eng || this.previewing === null) return;
    try {
      eng.mergePreview(null);
    } catch {
      // Nothing is shown any more either way.
    }
    this.leavePreviewState();
    this.x.editor.rerender();
    this.x.fe.refreshToolbar();
  }

  private leavePreviewState() {
    this.strip.hide();
    this.releasePreviewLock?.();
    this.releasePreviewLock = null;
  }
}
