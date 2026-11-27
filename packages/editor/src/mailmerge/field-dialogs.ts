// The dialogs that put merge fields in the document: Insert merge field (the columns of the list, with options), Address block and
// Greeting line (with a preview for each recipient), the rules (If, Ask, Fill-in, Next record if, Skip record if, Set bookmark),
// and Match fields.
import { Engine, type MergeFieldSpec } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { h, modal, onChoose, toast } from "../ui/dom";
import { COMPARISONS, formDialog, type FormValues } from "./form";
import type { MergeSession } from "./session";

let seq = 0;

const CASES: { value: string; label: string }[] = [
  { value: "", label: msg("As in the list") },
  { value: "upper", label: msg("UPPER CASE") },
  { value: "lower", label: msg("lower case") },
  { value: "first_cap", label: msg("First letter capital") },
  { value: "caps", label: msg("Each Word Capital") },
];

/** Mailings, Insert merge field: pick a column and insert it as often as needed; the dialog stays open. */
export function openInsertField(s: MergeSession) {
  const info = s.info();
  if (!info || !s.needRecipients()) return;
  const vocab = Engine.mergeVocabulary();
  const id = `mmi-${++seq}`;
  let kind: "columns" | "address" = "columns";
  let current = info.columns[0] ?? "";
  const list = h("div", { class: "xr-list mm-fields", role: "listbox", "aria-label": t("Fields"), tabindex: "0" });
  const seg = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Insert") });
  const before = h("input", { type: "text", id: `${id}-before`, spellcheck: "false", autocomplete: "off" });
  const after = h("input", { type: "text", id: `${id}-after`, spellcheck: "false", autocomplete: "off" });
  const caseSel = h("select", { id: `${id}-case` }, ...CASES.map((c) => h("option", { value: c.value }, t(c.label))));
  const picture = h("input", { type: "text", id: `${id}-picture`, spellcheck: "false", autocomplete: "off", placeholder: "#,##0.00   MMMM d, yyyy" });
  const insert = h("button", { class: "btn primary", type: "button" }, t("Insert"));
  const close = h("button", { class: "btn ghost", type: "button" }, t("Close"));
  const names = () => (kind === "columns" ? info.columns : vocab.fields);
  const paint = () => {
    list.replaceChildren(
      ...names().map((n) => {
        const on = n === current;
        const b = h("button", { type: "button", role: "option", class: `xr-item${on ? " on" : ""}`, "aria-selected": String(on), tabindex: "-1" }, h("span", { class: "xr-title" }, n));
        b.addEventListener("mousedown", (e) => e.preventDefault());
        onChoose(
          b,
          () => {
            current = n;
            paint();
          },
          go,
        );
        return b;
      }),
    );
    list.querySelector<HTMLElement>(".xr-item.on")?.scrollIntoView({ block: "nearest" });
    seg.querySelectorAll("button").forEach((b, i) => {
      const on = (i === 0) === (kind === "columns");
      b.classList.toggle("on", on);
      b.setAttribute("aria-checked", String(on));
    });
  };
  for (const [k, label] of [["columns", t("Database fields")], ["address", t("Address fields")]] as const) {
    const b = h("button", { type: "button", role: "radio" }, label);
    b.addEventListener("click", () => {
      kind = k;
      current = names()[0] ?? "";
      paint();
    });
    seg.append(b);
  }
  list.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const all = names();
    const i = all.indexOf(current);
    current = all[Math.min(all.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))] ?? current;
    paint();
  });
  const go = () => {
    if (!current) return;
    const spec: MergeFieldSpec = { kind: "field", name: current, ...(before.value ? { before: before.value } : {}), ...(after.value ? { after: after.value } : {}), ...(caseSel.value ? { case: caseSel.value as "upper" } : {}), ...(picture.value.trim() ? { picture: picture.value.trim() } : {}) };
    if (s.insert(spec)) toast(t("Inserted {field}", { field: current }), 1400);
  };
  insert.addEventListener("click", go);
  const options = h(
    "details",
    { class: "mm-options" },
    h("summary", {}, t("Options")),
    h("div", { class: "field" }, h("label", { for: `${id}-before` }, t("Text before")), before, h("p", { class: "help" }, t("Only written when the field isn't empty."))),
    h("div", { class: "field" }, h("label", { for: `${id}-after` }, t("Text after")), after),
    h("div", { class: "field" }, h("label", { for: `${id}-case` }, t("Capitals")), caseSel),
    h("div", { class: "field" }, h("label", { for: `${id}-picture` }, t("Number or date format")), picture, h("p", { class: "help" }, t("A picture such as #,##0.00 or MMMM d, yyyy."))),
  );
  const dialog = h(
    "div",
    { class: "dialog mm-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Insert merge field") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Insert merge field")), h("p", { class: "sub" }, t("The field shows each recipient's value when you merge. The dialog stays open, so you can insert several."))),
    h("div", { class: "dialog-body" }, seg, list, options),
    h("div", { class: "dialog-actions" }, close, insert),
  );
  const done = modal(dialog, () => s.x.editor.focus());
  close.addEventListener("click", done);
  paint();
}

/** Mailings, Address block: the recipient's name and postal address in a format, with a preview. */
export function openAddressBlock(s: MergeSession) {
  const info = s.info();
  if (!info || !s.needRecipients()) return;
  const vocab = Engine.mergeVocabulary();
  const spec = (v: FormValues): MergeFieldSpec => ({
    kind: "address_block",
    name: String(v.name),
    company: !!v.company,
    country: String(v.country) as "never",
    ...(v.country === "different" && String(v.except).trim() ? { except: String(v.except).trim() } : {}),
  });
  const match = h("button", { class: "btn ghost", type: "button" }, t("Match fields…"));
  match.addEventListener("click", () => void s.x.fe.command("mailings.match"));
  void formDialog(
    {
      title: t("Insert address block"),
      sub: t("The recipient's name and postal address, laid out for an envelope or a letter."),
      ok: t("Insert"),
      fields: [
        { id: "name", label: t("How the name is written"), kind: "select", options: vocab.nameFormats.map(([id, example]) => ({ value: id, label: example })), value: "first_last" },
        { id: "company", label: t("Put the company name on its own line"), kind: "check" },
        {
          id: "country",
          label: t("The country or region"),
          kind: "select",
          value: "never",
          options: [
            { value: "never", label: t("Never include it") },
            { value: "always", label: t("Always include it") },
            { value: "different", label: t("Only include it if it isn't:") },
          ],
        },
        { id: "except", label: t("Your own country or region"), kind: "text", value: "United States" },
      ],
      visible: (id, v) => id !== "except" || v.country === "different",
      preview: { count: () => s.info()?.included ?? 0, text: (v, pos) => s.engine!.mergeFieldPreview(Engine.mergeFieldInstr(spec(v)), pos) },
      extra: [match],
    },
    () => s.x.editor.focus(),
  ).then((v) => {
    if (v) s.insert(spec(v));
  });
}

const SALUTATIONS = [
  { value: "Dear ", label: msg("Dear") },
  { value: "To ", label: msg("To") },
  { value: "Hello ", label: msg("Hello") },
  { value: "Hi ", label: msg("Hi") },
  { value: "", label: msg("Nothing before the name") },
];
const GREETING_NAMES = [
  { value: "title_last", label: msg("Mr. Randall") },
  { value: "first", label: msg("Joshua") },
  { value: "first_last", label: msg("Joshua Randall") },
  { value: "title_first_last", label: msg("Mr. Joshua Randall") },
];
const PUNCTUATION = [
  { value: ",", label: msg("Comma") },
  { value: ":", label: msg("Colon") },
  { value: "", label: msg("Nothing") },
];
const INVALID = [
  { value: "Dear Sir or Madam,", label: msg("Dear Sir or Madam,") },
  { value: "To whom it may concern:", label: msg("To whom it may concern:") },
  { value: "Dear Customer,", label: msg("Dear Customer,") },
];

/** Mailings, Greeting line: "Dear Mr. Randall," with the words and punctuation chosen, and a line for a recipient with no name. */
export function openGreetingLine(s: MergeSession) {
  const info = s.info();
  if (!info || !s.needRecipients()) return;
  const spec = (v: FormValues): MergeFieldSpec => ({ kind: "greeting_line", salutation: String(v.salutation), name: String(v.name), punctuation: String(v.punctuation), invalid: String(v.invalid) });
  const match = h("button", { class: "btn ghost", type: "button" }, t("Match fields…"));
  match.addEventListener("click", () => void s.x.fe.command("mailings.match"));
  void formDialog(
    {
      title: t("Insert greeting line"),
      sub: t("A greeting that follows each recipient's name."),
      ok: t("Insert"),
      fields: [
        { id: "salutation", label: t("The words before the name"), kind: "select", options: SALUTATIONS.map((o) => ({ value: o.value, label: t(o.label) })) },
        { id: "name", label: t("How the name is written"), kind: "select", options: GREETING_NAMES.map((o) => ({ value: o.value, label: t(o.label) })) },
        { id: "punctuation", label: t("What ends the greeting"), kind: "select", options: PUNCTUATION.map((o) => ({ value: o.value, label: t(o.label) })) },
        { id: "invalid", label: t("For a recipient with no name"), kind: "select", options: INVALID.map((o) => ({ value: o.value, label: t(o.label) })) },
      ],
      preview: { count: () => s.info()?.included ?? 0, text: (v, pos) => s.engine!.mergeFieldPreview(Engine.mergeFieldInstr(spec(v)), pos) },
      extra: [match],
    },
    () => s.x.editor.focus(),
  ).then((v) => {
    if (v) s.insert(spec(v));
  });
}

/** Match fields: which column stands for each of Word's address fields. */
export function openMatchFields(s: MergeSession) {
  const eng = s.engine;
  const data = eng?.mergeData();
  if (!eng || !data || !s.needRecipients()) return;
  const none = "";
  void formDialog(
    {
      title: t("Match fields"),
      sub: t("The address block and the greeting line look for these fields. Docxtor matched the columns it recognised; change any that are wrong."),
      ok: t("OK"),
      fields: data.matched.map(([field, column]) => ({
        id: field,
        label: field,
        kind: "select" as const,
        value: data.mapping[field] ?? column ?? none,
        options: [{ value: none, label: t("Not matched") }, ...data.columns.map((c) => ({ value: c, label: c }))],
      })),
    },
    () => s.x.editor.focus(),
  ).then((v) => {
    if (!v) return;
    const mapping: Record<string, string> = {};
    for (const [field] of data.matched) {
      const col = String(v[field] ?? "");
      if (col) mapping[field] = col;
    }
    s.run({ op: "set_merge_recipients", mapping });
  });
}

// -- the rules ---------------------------------------------------------------------------------

const withValue = (op: string) => op !== "empty" && op !== "not_empty";

/** Fields for a rule that compares a column: which column, how, and to what. */
function compareFields(columns: string[]) {
  return [
    { id: "field", label: t("Column"), kind: "select" as const, options: columns.map((c) => ({ value: c, label: c })) },
    { id: "op", label: t("Comparison"), kind: "select" as const, options: COMPARISONS.map((c) => ({ value: c.value, label: t(c.label) })) },
    { id: "value", label: t("Compare to"), kind: "text" as const },
  ];
}

export type Rule = "if" | "ask" | "fillIn" | "nextIf" | "skipIf" | "set";

/** Mailings, Rules: the dialogs for the rules that ask for something. */
export async function openRule(s: MergeSession, rule: Rule) {
  const info = s.info();
  if (!info || !s.needRecipients()) return;
  const cmp = compareFields(info.columns);
  const visible = (id: string, v: FormValues) => id !== "value" || withValue(String(v.op));
  const need = (v: FormValues, ...ids: string[]) => (ids.some((i) => !String(v[i]).trim()) ? t("Fill in the boxes that are not marked optional.") : null);
  let out: FormValues | null = null;
  let spec: ((v: FormValues) => MergeFieldSpec) | null = null;
  const back = () => s.x.editor.focus();
  switch (rule) {
    case "if":
      out = await formDialog({ title: t("Insert If…Then…Else"), sub: t("Writes one text when the column compares as you say, and another when it doesn't."), ok: t("Insert"), fields: [...cmp, { id: "then", label: t("Insert this text"), kind: "textarea", rows: 2 }, { id: "otherwise", label: t("Otherwise insert this text (optional)"), kind: "textarea", rows: 2 }], visible, validate: (v) => need(v, "then") }, back);
      spec = (v) => ({ kind: "if", field: String(v.field), op: String(v.op), value: String(v.value), then: String(v.then), otherwise: String(v.otherwise) });
      break;
    case "ask":
      out = await formDialog({ title: t("Insert Ask"), sub: t("Asks a question when the merge starts and keeps the answer under a bookmark name."), ok: t("Insert"), fields: [{ id: "bookmark", label: t("Bookmark"), kind: "text" }, { id: "prompt", label: t("Prompt"), kind: "text" }, { id: "default", label: t("Default answer (optional)"), kind: "text" }, { id: "once", label: t("Ask once for the whole merge"), kind: "check" }], validate: (v) => need(v, "bookmark", "prompt") }, back);
      spec = (v) => ({ kind: "ask", bookmark: String(v.bookmark), prompt: String(v.prompt), default: String(v.default), once: !!v.once });
      break;
    case "fillIn":
      out = await formDialog({ title: t("Insert Fill-in"), sub: t("Asks a question when the merge starts and inserts the answer here."), ok: t("Insert"), fields: [{ id: "prompt", label: t("Prompt"), kind: "text" }, { id: "default", label: t("Default answer (optional)"), kind: "text" }, { id: "once", label: t("Ask once for the whole merge"), kind: "check" }], validate: (v) => need(v, "prompt") }, back);
      spec = (v) => ({ kind: "fill_in", prompt: String(v.prompt), default: String(v.default), once: !!v.once });
      break;
    case "nextIf":
      out = await formDialog({ title: t("Insert Next record if"), sub: t("Goes on to the next record in the same document when the column compares as you say."), ok: t("Insert"), fields: cmp, visible }, back);
      spec = (v) => ({ kind: "next_if", field: String(v.field), op: String(v.op), value: String(v.value) });
      break;
    case "skipIf":
      out = await formDialog({ title: t("Insert Skip record if"), sub: t("Leaves a recipient out of the merge when the column compares as you say."), ok: t("Insert"), fields: cmp, visible }, back);
      spec = (v) => ({ kind: "skip_if", field: String(v.field), op: String(v.op), value: String(v.value) });
      break;
    case "set":
      out = await formDialog({ title: t("Insert Set bookmark"), sub: t("Gives a bookmark a value for the rest of the merge. Cross-reference the bookmark to show it."), ok: t("Insert"), fields: [{ id: "bookmark", label: t("Bookmark"), kind: "text" }, { id: "value", label: t("Value"), kind: "text" }], validate: (v) => need(v, "bookmark", "value") }, back);
      spec = (v) => ({ kind: "set", bookmark: String(v.bookmark), value: String(v.value) });
      break;
  }
  if (out && spec) s.insert(spec(out));
}
