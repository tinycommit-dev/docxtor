// Add to table of contents: a line for text that is not a heading (Word's TC field). The mark is hidden in the text
// and listed when the table is built from table entry fields.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { addTocEntry } from "../editor/lists";
import { h, modal, toast } from "./dom";

export function openTocEntry(ed: Editor) {
  if (ed.story) return;
  const picked = ed.collapsed() ? "" : ed.selectedText().replace(/\s+/g, " ").trim().slice(0, 120);
  const text = h("input", { type: "text", id: "tce-text", value: picked, autocomplete: "off", spellcheck: "false" });
  const level = h("select", { id: "tce-level" }, ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => h("option", { value: String(n) }, t("Level {n}", { n }))));
  const kind = h("input", { type: "text", id: "tce-kind", value: "C", maxlength: "1", autocomplete: "off", spellcheck: "false", style: "width:4em" });
  const noPage = h("input", { type: "checkbox", id: "tce-nopage" });

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const add = h("button", { class: "btn primary", type: "button" }, t("Add"));
  const check = () => (add.disabled = !text.value.trim());
  text.addEventListener("input", check);
  check();

  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Add to table of contents") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Add to table of contents")), h("p", { class: "sub" }, t("The line is hidden in the text. A table of contents lists it when it uses table entry fields."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: "tce-text" }, t("Line in the table")), text),
      h("div", { class: "field" }, h("label", { for: "tce-level" }, t("Level")), level),
      h("div", { class: "field" }, h("label", { for: "tce-kind" }, t("Table identifier")), kind, h("span", { class: "help" }, t("One letter. Tables with another letter list only their own lines."))),
      h("label", { class: "check", for: "tce-nopage" }, noPage, h("span", {}, t("No page number for this line"))),
    ),
    h("div", { class: "dialog-actions" }, cancel, add),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  const go = () => {
    const line = text.value.trim();
    if (!line) return;
    const id = kind.value.trim().slice(0, 1).toUpperCase();
    close();
    if (addTocEntry(ed, { text: line, level: Number(level.value), ...(id && id !== "C" ? { id } : {}), noPage: noPage.checked })) toast(t("Added to the table of contents"));
  };
  add.addEventListener("click", go);
  text.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      go();
    }
  });
  text.focus();
  text.select();
}
