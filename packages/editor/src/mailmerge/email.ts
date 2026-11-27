// Mailings, Finish and merge, Send email messages: as Word does. The dialog asks which column holds the address, the subject line,
// the mail format and which records; then each recipient's merged document becomes a message (the body as a web page or plain text,
// or the document attached as a .docx or a .pdf) and a sender, one of the extensions' mail senders, takes the messages.
import { Engine, type MergeInfo, type MergeRange } from "@docxtor/core";
import { t, tn, type EmailFormat, type EmailMessage, type EmailSender } from "@docxtor/extensions";
import { exportPdf } from "../pdf/export";
import { h, modal, overlayLayer, toast } from "../ui/dom";
import { ask, stemOf } from "./finish";
import { formDialog } from "./form";
import { offscreenEditor, type Offscreen } from "./offscreen";
import { recordFields, recordsCount, recordsProblem, recordsRange, recordsVisible } from "./records";
import { safeFileName } from "./eml";
import { fillSubject, guessEmailColumn, validAddress } from "./email-fields";
import type { MergeSession } from "./session";

const FORMATS: { value: EmailFormat; label: () => string }[] = [
  { value: "html", label: () => t("HTML") },
  { value: "text", label: () => t("Plain text") },
  { value: "docx", label: () => t("Attachment as a Word document (.docx)") },
  { value: "pdf", label: () => t("Attachment as a PDF document (.pdf)") },
];

/** The format a sender takes: the one asked for, or the first it can send. */
export const formatFor = (sender: EmailSender, wanted: EmailFormat): EmailFormat => (!sender.formats || sender.formats.includes(wanted) ? wanted : sender.formats[0]);

const clearToast = () => overlayLayer().querySelector(".toast")?.remove();
const tick = () => new Promise<void>((r) => setTimeout(r, 0));

interface Skipped {
  record: number;
  to: string;
  why: string;
}

export interface EmailRun {
  sender: EmailSender;
  /** The column that holds the address. */
  to: string;
  subject: string;
  format: EmailFormat;
  range: MergeRange;
}

/** Make the messages and hand them to the sender. Returns whether the sending went through for everyone. */
export async function sendEmails(s: MergeSession, run: EmailRun): Promise<boolean> {
  const eng = s.engine;
  const info = s.info();
  const data = eng?.mergeData();
  if (!eng || !info || !data) return false;
  const from = run.range.from ?? 0;
  const to = run.range.to ?? info.included;
  const answers = await ask(s, from, to);
  if (!answers) return false;
  const col = data.columns.findIndex((c) => c === run.to);
  const format = formatFor(run.sender, run.format);
  const stem = stemOf(eng.fileName);
  const messages: EmailMessage[] = [];
  const skipped: Skipped[] = [];
  let pdf: Offscreen | null = null;
  try {
    for (let pos = from; pos < to; pos++) {
      const row = data.rows[data.order[pos]] ?? [];
      const address = (row[col] ?? "").trim();
      toast(t("Preparing message {n} of {total}…", { n: pos - from + 1, total: to - from }), 60000);
      await tick();
      if (!validAddress(address)) {
        skipped.push({ record: pos + 1, to: address, why: address ? t("This is not an email address.") : t("There is no email address.") });
        continue;
      }
      let merged;
      try {
        merged = eng.mergeToDocx(answers, { from: pos, to: pos + 1 });
      } catch (err) {
        // A rule leaving the record out is the engine's one expected answer; anything else is a real error, said as it is.
        const text = err instanceof Error ? err.message : String(err);
        skipped.push({ record: pos + 1, to: address, why: /left out by a rule/.test(text) ? t("A rule leaves this record out.") : text });
        continue;
      }
      const subject = fillSubject(run.subject, data.columns, row);
      const message: EmailMessage = { to: address, subject, record: pos };
      if (format === "html" || format === "text") {
        const copy = Engine.open(merged.bytes, `${stem}.docx`);
        try {
          message.text = copy.text();
          if (format === "html") message.html = copy.exportHtml();
        } finally {
          copy.dispose();
        }
      } else {
        const name = `${safeFileName(subject, stem)}.${format}`;
        if (format === "docx") message.attachments = [{ name, type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", bytes: merged.bytes }];
        else {
          pdf ??= await offscreenEditor(s);
          await pdf.fe.open(merged.bytes, `${stem}.docx`);
          await document.fonts.ready;
          message.attachments = [{ name, type: "application/pdf", bytes: (await exportPdf(pdf.fe.editor)).bytes }];
        }
      }
      messages.push(message);
    }
  } catch (e) {
    clearToast();
    toast(String((e as Error).message ?? e), 6000);
    return false;
  } finally {
    await pdf?.dispose();
    clearToast();
  }
  if (!messages.length) {
    await report({ sent: 0, failed: [], skipped, title: t("Nothing to send") });
    return false;
  }
  // Two messages with the same file name would overwrite each other in a mail app's attachment list; number the repeats.
  const seen = new Map<string, number>();
  for (const m of messages) {
    for (const a of m.attachments ?? []) {
      const n = (seen.get(a.name) ?? 0) + 1;
      seen.set(a.name, n);
      if (n > 1) a.name = a.name.replace(/(\.[^.]+)$/, ` (${n})$1`);
    }
  }
  const outcomes: ({ ok: true } | { ok: false; error: string } | undefined)[] = new Array(messages.length).fill(undefined);
  const abort = new AbortController();
  try {
    await run.sender.send(messages, {
      ui: s.ui,
      signal: abort.signal,
      name: stem,
      progress: (i, outcome) => {
        if (i >= 0 && i < messages.length) outcomes[i] = outcome;
      },
    });
  } catch (e) {
    const why = String((e as Error).message ?? e);
    for (let i = 0; i < outcomes.length; i++) outcomes[i] ??= { ok: false, error: why };
  }
  const failed = messages.flatMap((m, i) => {
    const o = outcomes[i];
    return o && !o.ok ? [{ record: m.record + 1, to: m.to, why: o.error }] : [];
  });
  const sent = messages.length - failed.length;
  if (!failed.length && !skipped.length) {
    toast((run.sender.summary ?? ((n) => tn(n, "{n} message sent.", "{n} messages sent.")))(sent), 5000);
    return true;
  }
  await report({ sent, failed, skipped, title: failed.length ? t("Some messages did not go") : t("Some records were left out") });
  return !failed.length;
}

/** A dialog that says what went through and lists what did not. */
function report(r: { sent: number; failed: Skipped[]; skipped: Skipped[]; title: string }): Promise<void> {
  return new Promise((resolve) => {
    const list = (items: Skipped[]) => h("ul", { class: "mm-report" }, ...items.map((i) => h("li", {}, t("Record {n}", { n: i.record }), i.to ? ` (${i.to})` : "", ": ", i.why)));
    const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
    const dialog = h(
      "div",
      { class: "dialog mm-dialog", role: "dialog", "aria-modal": "true", "aria-label": r.title },
      h("div", { class: "dialog-head" }, h("h2", {}, r.title), h("p", { class: "sub" }, tn(r.sent, "{n} message went through.", "{n} messages went through."))),
      h(
        "div",
        { class: "dialog-body" },
        r.failed.length ? h("div", { class: "field" }, h("label", {}, tn(r.failed.length, "{n} message did not go", "{n} messages did not go")), list(r.failed)) : null,
        r.skipped.length ? h("div", { class: "field" }, h("label", {}, tn(r.skipped.length, "{n} record was left out", "{n} records were left out")), list(r.skipped)) : null,
      ),
      h("div", { class: "dialog-actions" }, ok),
    );
    const close = modal(dialog, resolve);
    ok.addEventListener("click", close);
    ok.focus();
  });
}

export async function openSendEmail(s: MergeSession) {
  const info: MergeInfo | null = s.info();
  if (!info || !s.needRecipients()) return;
  if (info.included === 0) {
    toast(t("No recipients are included in the merge."));
    return;
  }
  const senders = s.x.fe.extensions.mailSenders.list() as EmailSender[];
  if (!senders.length) {
    toast(t("No way to send email is turned on. Look under Extensions."));
    return;
  }
  const data = s.engine!.mergeData()!;
  const sender = (id: unknown) => senders.find((x) => x.id === id) ?? senders[0];
  const v = await formDialog(
    {
      title: t("Send email messages"),
      sub: tn(info.included, "{n} recipient is in the merge.", "{n} recipients are in the merge."),
      ok: t("Send"),
      fields: [
        { id: "sender", label: t("Send with"), kind: "select", value: senders[0].id, options: senders.map((x) => ({ value: x.id, label: t(x.label) })) },
        { id: "address", label: t("To"), kind: "select", value: guessEmailColumn(data), options: data.columns.map((c) => ({ value: c, label: c })), help: t("The column that holds each recipient's email address.") },
        { id: "subject", label: t("Subject line"), kind: "text", help: t("Put a column name between « and » to fill it in for each recipient, like «First Name».") },
        { id: "format", label: t("Mail format"), kind: "select", value: "html", options: FORMATS.map((f) => ({ value: f.value, label: f.label() })) },
        ...recordFields(info),
      ],
      visible: recordsVisible,
      validate: (values) => (!String(values.subject).trim() ? t("Type a subject line.") : recordsProblem(info, values)),
      notes: (values) => {
        const snd = sender(values.sender);
        const lines = [snd.note ? t(snd.note) : ""];
        const f = formatFor(snd, values.format as EmailFormat);
        if (f !== values.format) lines.push(t("This way of sending can't take that format, so the messages go as: {format}.", { format: FORMATS.find((x) => x.value === f)!.label() }));
        if (snd.warnAbove && snd.limitNote && recordsCount(info, values) > snd.warnAbove) lines.push(t(snd.limitNote));
        return lines.filter(Boolean).join(" ");
      },
    },
    () => s.x.editor.focus(),
  );
  if (!v) return;
  await sendEmails(s, { sender: sender(v.sender), to: String(v.address), subject: String(v.subject).trim(), format: v.format as EmailFormat, range: recordsRange(info, v) });
}
