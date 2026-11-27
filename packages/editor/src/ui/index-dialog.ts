// Index: how the list of marked entries looks (indented or run-in, columns, right-aligned page numbers, the line before
// them, letter headings, separators, the language that sorts it, a look). Opened on an index it changes that one.
import type { IndexOptions } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { caretIndex, LEADERS, setIndex } from "../editor/lists";
import { h, modal } from "./dom";

const LANGUAGES: [number, ReturnType<typeof msg>][] = [
  [1033, msg("English (US)")],
  [2057, msg("English (UK)")],
  [3082, msg("Spanish")],
  [1036, msg("French")],
  [1031, msg("German")],
  [1040, msg("Italian")],
  [1046, msg("Portuguese (Brazil)")],
  [1043, msg("Dutch")],
  [1053, msg("Swedish")],
  [1030, msg("Danish")],
  [1044, msg("Norwegian")],
  [1035, msg("Finnish")],
];

const LOOKS: [string, ReturnType<typeof msg>][] = [
  ["", msg("From the document")],
  ["classic", msg("Classic")],
  ["modern", msg("Modern")],
  ["simple", msg("Simple")],
];

export function openIndexDialog(ed: Editor) {
  if (ed.story || !ed.engine) return;
  const at = caretIndex(ed);
  const editing = at !== null;
  const start: IndexOptions = (editing ? ed.engine.indexOptions(at!) : null) ?? { columns: 2, runIn: false, headings: false, rightAlign: true, leader: "dot" };

  const seg = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Type") });
  let runIn = start.runIn;
  const types = ([[false, t("Indented")], [true, t("Run-in")]] as const).map(([v, label]) => {
    const b = h("button", { type: "button", role: "radio" }, label);
    b.addEventListener("click", () => {
      runIn = v;
      paint();
    });
    seg.append(b);
    return [v, b] as const;
  });
  const columns = h("select", { id: "ix-columns" }, ...[1, 2, 3, 4].map((n) => h("option", { value: String(n) }, String(n))));
  columns.value = String(start.columns);
  const language = h("select", { id: "ix-language" }, h("option", { value: "" }, t("Document language")), ...LANGUAGES.map(([v, label]) => h("option", { value: String(v) }, t(label))));
  language.value = start.language ? String(start.language) : "";
  const align = h("input", { type: "checkbox", id: "ix-align", checked: start.rightAlign });
  const headings = h("input", { type: "checkbox", id: "ix-headings", checked: start.headings });
  const leader = h("select", { id: "ix-leader" }, ...LEADERS.map(([v, label]) => h("option", { value: v }, t(label))));
  leader.value = start.leader;
  const look = h("select", { id: "ix-look" }, ...LOOKS.map(([v, label]) => h("option", { value: v }, t(label))));
  const pageSep = h("input", { type: "text", id: "ix-pagesep", value: start.pageSep ?? ", ", autocomplete: "off", spellcheck: "false", style: "width:6em" });
  const rangeSep = h("input", { type: "text", id: "ix-rangesep", value: start.rangeSep ?? "–", autocomplete: "off", spellcheck: "false", style: "width:6em" });

  const paint = () => {
    for (const [v, b] of types) {
      b.classList.toggle("on", v === runIn);
      b.setAttribute("aria-checked", String(v === runIn));
    }
    // Run-in entries have no room for a column of numbers.
    align.disabled = runIn;
    leader.disabled = runIn || !align.checked;
  };
  align.addEventListener("change", paint);
  paint();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, editing ? t("Apply") : t("Insert"));
  const dialog = h(
    "div",
    { class: "dialog ix-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Index") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Index")), h("p", { class: "sub" }, editing ? t("Change the index the caret is in. Update fields keeps it current.") : t("Lists the entries you marked, with their page numbers, in columns after the paragraph you are in."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", {}, t("Type")), seg),
      h("div", { class: "ix-row" }, h("div", { class: "field" }, h("label", { for: "ix-columns" }, t("Columns")), columns), h("div", { class: "field" }, h("label", { for: "ix-language" }, t("Language")), language)),
      h("label", { class: "check", for: "ix-headings" }, headings, h("span", {}, t("A heading before each letter"))),
      h("label", { class: "check", for: "ix-align" }, align, h("span", {}, t("Right align page numbers"))),
      h("div", { class: "field" }, h("label", { for: "ix-leader" }, t("Line before the number")), leader),
      h("div", { class: "ix-row" }, h("div", { class: "field" }, h("label", { for: "ix-pagesep" }, t("Between page numbers")), pageSep), h("div", { class: "field" }, h("label", { for: "ix-rangesep" }, t("Between first and last page")), rangeSep)),
      h("div", { class: "field" }, h("label", { for: "ix-look" }, t("Look")), look),
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const options: IndexOptions = {
      columns: Number(columns.value),
      runIn,
      headings: headings.checked,
      rightAlign: align.checked,
      leader: leader.value as IndexOptions["leader"],
      ...(pageSep.value && pageSep.value !== ", " ? { pageSep: pageSep.value } : {}),
      ...(rangeSep.value && rangeSep.value !== "–" ? { rangeSep: rangeSep.value } : {}),
      ...(language.value ? { language: Number(language.value) } : {}),
      ...(start.kind ? { kind: start.kind } : {}),
      ...(look.value ? { look: look.value } : {}),
    };
    close();
    if (editing) setIndex(ed, options);
    else ed.insertIndex(options);
  });
  columns.focus();
}
