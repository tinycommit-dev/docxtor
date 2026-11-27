// Mailings, Finish and merge: merge the document with the recipients into a new document (a section for each record) or into
// a PDF, for every record, the one on show or a range, after answering the questions the Ask and Fill-in fields have.
import type { MergeAnswers, MergeRange } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import type { DocxtorEditor, DocxtorEditorOptions } from "../docxtor-editor";
import { exportPdf } from "../pdf/export";
import { overlayLayer, toast } from "../ui/dom";
import { downloadBytes } from "./download";
import { formDialog, type FormField } from "./form";
import { recordFields, recordsProblem, recordsRange, recordsVisible } from "./records";
import type { MergeSession } from "./session";

export const stemOf = (name: string) => name.replace(/\.[^.]+$/, "") || t("Untitled");

/** The questions the merge asks. Null when the person gave up. */
export async function ask(s: MergeSession, from: number, to: number): Promise<MergeAnswers | null> {
  const eng = s.engine!;
  const prompts = eng.mergePrompts();
  const answers: MergeAnswers = { once: {}, records: {} };
  if (!prompts.length) return answers;
  const field = (p: (typeof prompts)[number]): FormField => ({ id: p.key, label: p.prompt, kind: "text", value: p.default });
  const once = prompts.filter((p) => p.once);
  const each = prompts.filter((p) => !p.once);
  if (once.length) {
    const v = await formDialog({ title: t("Questions for this merge"), sub: t("Each answer is used for every record."), ok: t("Continue"), fields: once.map(field) }, () => s.x.editor.focus());
    if (!v) return null;
    for (const p of once) answers.once![p.key] = String(v[p.key]);
  }
  if (each.length) {
    const data = eng.mergeData();
    for (let pos = from; pos < to; pos++) {
      const row = data?.rows[data.order[pos]] ?? [];
      const rest = to - pos > 1;
      const v = await formDialog(
        {
          title: t("Record {n} of {total}", { n: pos + 1, total: data?.order.length ?? to }),
          sub: row.slice(0, 3).filter(Boolean).join(", "),
          ok: t("Continue"),
          fields: [...each.map(field), ...(rest ? [{ id: "__rest", label: t("Use these answers for the records that are left"), kind: "check" as const }] : [])],
        },
        () => s.x.editor.focus(),
      );
      if (!v) return null;
      const mine: Record<string, string> = {};
      for (const p of each) mine[p.key] = String(v[p.key]);
      if (v.__rest) {
        Object.assign(answers.once!, mine);
        // Earlier records keep what they were given; this one and the rest take these answers.
        break;
      }
      answers.records![pos] = mine;
    }
  }
  return answers;
}

/** Lay the merged document out in an editor out of sight and write it as a PDF. */
async function mergedPdf(s: MergeSession, bytes: Uint8Array, name: string): Promise<Uint8Array> {
  // A second editor of the same kind (made from the first one's class, which imports this file).
  const DocxtorEditor = s.x.fe.constructor as new (options: DocxtorEditorOptions) => DocxtorEditor;
  const host = document.createElement("div");
  host.style.cssText = "position:fixed;left:-30000px;top:0;width:1400px;height:1000px;overflow:hidden;pointer-events:none";
  overlayLayer();
  document.body.append(host);
  const fe = new DocxtorEditor({ toolbar: false, menuBar: false, toolbarAsk: false, fonts: false, spelling: false, autoformat: false });
  try {
    fe.mount(host);
    await fe.ready;
    await fe.open(bytes, name);
    await document.fonts.ready;
    const out = await exportPdf(fe.editor);
    if (out.substituted.length) toast(t("A font wasn't available, so a similar one was used: {fonts}.", { fonts: out.substituted.join(", ") }), 9000);
    return out.bytes;
  } finally {
    // The extensions go first, while the document they look at is still there.
    await fe.extensions.dispose();
    fe.destroy();
    host.remove();
  }
}

export async function runMerge(s: MergeSession, output: "docx" | "pdf", range: MergeRange): Promise<boolean> {
  const eng = s.engine;
  const info = s.info();
  if (!eng || !info) return false;
  const total = info.included;
  const from = range.from ?? 0;
  const to = range.to ?? total;
  const answers = await ask(s, from, to);
  if (!answers) return false;
  let merged;
  try {
    merged = eng.mergeToDocx(answers, range);
  } catch (e) {
    toast(String((e as Error).message ?? e), 6000);
    return false;
  }
  const stem = stemOf(eng.fileName);
  const done = tn(merged.result.records, "Merged {n} record into {copies} documents", "Merged {n} records into {copies} documents", { copies: merged.result.copies });
  if (output === "docx") {
    await s.x.fe.openNewDocument(merged.bytes, `${t("{name} merged", { name: stem })}.docx`);
    toast(merged.result.skipped ? `${done}. ${tn(merged.result.skipped, "{n} was left out.", "{n} were left out.")}` : done, 4000);
    return true;
  }
  toast(t("Making the PDF…"), 60000);
  try {
    const pdf = await mergedPdf(s, merged.bytes, `${stem}.docx`);
    overlayLayer().querySelector(".toast")?.remove();
    downloadBytes(pdf, `${t("{name} merged", { name: stem })}.pdf`, "application/pdf");
    toast(done, 4000);
    return true;
  } catch (e) {
    overlayLayer().querySelector(".toast")?.remove();
    toast(String((e as Error).message ?? e), 6000);
    return false;
  }
}

export async function openFinish(s: MergeSession) {
  const info = s.info();
  if (!info || !s.needRecipients()) return;
  if (info.included === 0) {
    toast(t("No recipients are included in the merge."));
    return;
  }
  const v = await formDialog(
    {
      title: t("Finish and merge"),
      sub: tn(info.included, "{n} recipient is in the merge.", "{n} recipients are in the merge."),
      ok: t("Merge"),
      fields: [
        {
          id: "output",
          label: t("Make"),
          kind: "select",
          value: "docx",
          options: [
            { value: "docx", label: t("A new document, a section for each record") },
            { value: "pdf", label: t("A PDF document") },
          ],
        },
        ...recordFields(info),
      ],
      visible: recordsVisible,
      validate: (values) => recordsProblem(info, values),
    },
    () => s.x.editor.focus(),
  );
  if (!v) return;
  await runMerge(s, v.output === "pdf" ? "pdf" : "docx", recordsRange(info, v));
}
