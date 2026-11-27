// Create a style from the selection (Word's Create a Style, Google Docs' Save as new style): name the formatting the
// selected text has, and the text takes the new style. One `create_style` op, one undo step.
import { t, type ExtensionHost } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal } from "./dom";
import { tileCss } from "./style-preview";

export function openCreateStyle(ed: Editor, _host: ExtensionHost, onDone?: (name: string) => void) {
  const engine = ed.engine;
  const r = ed.range();
  const f = ed.format();
  if (!engine || !r || !f) return;
  const collapsed = ed.collapsed();
  const names = new Set(engine.styles().map((s) => s.name.toLowerCase()));
  let n = 1;
  while (names.has(`style${n}`)) n++;
  const err = h("p", { class: "ms-error help", role: "alert" });
  const name = h("input", { id: "cs-name", type: "text", value: `Style${n}`, autocomplete: "off", spellcheck: "false", "aria-label": t("Name") });
  const kind = h("select", { id: "cs-kind" }, h("option", { value: "paragraph", selected: true }, t("Paragraph")), h("option", { value: "character", disabled: collapsed }, t("Character")), h("option", { value: "linked" }, t("Linked (paragraph and character)")));
  const quick = h("input", { id: "cs-quick", type: "checkbox", checked: true });
  const sample = h("div", { class: "ms-preview" }, h("div", { class: "ms-sample", style: tileCss(f.run, f.para, { min: 12, max: 24 }) }, t("Sample text")));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("Create"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Create a style from the selection") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Create a style from the selection")), h("p", { class: "sub" }, t("The selected text keeps its look and gets the new style."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: "cs-name" }, t("Name")), name),
      h("div", { class: "field" }, h("label", { for: "cs-kind" }, t("Style type")), kind),
      sample,
      h("label", { class: "check", for: "cs-quick" }, quick, h("span", {}, t("Add to the style gallery"))),
      err,
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  name.select();
  cancel.addEventListener("click", close);
  const submit = () => {
    const value = name.value.trim();
    if (!value) {
      err.textContent = t("Give the style a name.");
      name.focus();
      return;
    }
    const res = ed.exec({ op: "create_style", name: value, kind: kind.value, from: r[0], to: r[1], def: { quick: quick.checked } }, { select: ed.sel, keepExtra: true, onError: (m) => (err.textContent = m) });
    if (!res) return;
    close();
    ed.focus();
    onDone?.(value);
  };
  ok.addEventListener("click", submit);
  name.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  });
}
