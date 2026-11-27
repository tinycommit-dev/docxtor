// Mailings, Finish and merge, Print documents: the merged records go to the browser's print dialog in one job, each record starting on
// a page of its own (each is a section of the merged document, so it keeps the page setup the template has). Nothing is opened as a
// new document: the merged document is laid out in an editor out of sight and its pages are printed.
import type { MergeRange } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import { overlayLayer, toast } from "../ui/dom";
import { ask } from "./finish";
import { formDialog } from "./form";
import { offscreenEditor } from "./offscreen";
import { recordFields, recordsProblem, recordsRange, recordsVisible } from "./records";
import type { MergeSession } from "./session";

const clearToast = () => overlayLayer().querySelector(".toast")?.remove();

/** Print the records of a range. Returns whether the print dialog was reached. */
export async function printMerged(s: MergeSession, range: MergeRange): Promise<boolean> {
  const eng = s.engine;
  const info = s.info();
  if (!eng || !info) return false;
  const answers = await ask(s, range.from ?? 0, range.to ?? info.included);
  if (!answers) return false;
  let merged;
  try {
    merged = eng.mergeToDocx(answers, range);
  } catch (e) {
    toast(String((e as Error).message ?? e), 6000);
    return false;
  }
  toast(t("Getting the pages ready…"), 60000);
  const off = await offscreenEditor(s);
  try {
    await off.fe.open(merged.bytes, `${eng.fileName || "Merged.docx"}`);
    await document.fonts.ready;
    const ed = off.fe.editor;
    ed.layoutNow();
    // The pages are copied for the print job, so the editor can go once the dialog is done.
    window.addEventListener("afterprint", () => void off.dispose(), { once: true });
    clearToast();
    toast(tn(merged.result.records, "Printing {n} record…", "Printing {n} records…"), 3000);
    ed.print();
    return true;
  } catch (e) {
    clearToast();
    await off.dispose();
    toast(String((e as Error).message ?? e), 6000);
    return false;
  }
}

export async function openPrintMerged(s: MergeSession) {
  const info = s.info();
  if (!info || !s.needRecipients()) return;
  if (info.included === 0) {
    toast(t("No recipients are included in the merge."));
    return;
  }
  const v = await formDialog(
    {
      title: t("Print merged documents"),
      sub: tn(info.included, "{n} recipient is in the merge.", "{n} recipients are in the merge."),
      ok: t("Print"),
      fields: recordFields(info),
      visible: recordsVisible,
      validate: (values) => recordsProblem(info, values),
      notes: () => t("Each record starts on a new page, and all of them go to the print dialog together."),
    },
    () => s.x.editor.focus(),
  );
  if (!v) return;
  await printMerged(s, recordsRange(info, v));
}
