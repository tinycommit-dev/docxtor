// Mark index entry: the text to list in the index, a sub-entry, a "See …" reference, and how the page number looks.
// The mark is a hidden field after the selection; Mark all marks the first occurrence in every paragraph.
import { t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal, toast } from "./dom";

export function openIndexEntry(ed: Editor) {
  if (ed.story) return;
  const picked = ed.collapsed() ? "" : ed.selectedText().replace(/\s+/g, " ").trim().slice(0, 120);
  const entry = h("input", { type: "text", id: "ix-entry", value: picked, autocomplete: "off", spellcheck: "false" });
  const sub = h("input", { type: "text", id: "ix-sub", autocomplete: "off", spellcheck: "false" });
  const see = h("input", { type: "text", id: "ix-see", placeholder: t("Another entry, such as Fruit"), autocomplete: "off", spellcheck: "false" });
  const bold = h("input", { type: "checkbox", id: "ix-bold" });
  const italic = h("input", { type: "checkbox", id: "ix-italic" });

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const markAll = h("button", { class: "btn", type: "button" }, t("Mark all"));
  const mark = h("button", { class: "btn primary", type: "button" }, t("Mark"));
  const check = () => {
    const none = !entry.value.trim();
    mark.disabled = none;
    markAll.disabled = none;
  };
  entry.addEventListener("input", check);
  check();

  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Mark index entry") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Mark index entry")), h("p", { class: "sub" }, t("The entry is hidden in the text and listed, with its page number, when you insert the index."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: "ix-entry" }, t("Main entry")), entry),
      h("div", { class: "field" }, h("label", { for: "ix-sub" }, t("Subentry")), sub),
      h("div", { class: "field" }, h("label", { for: "ix-see" }, t("Cross-reference")), see, h("p", { class: "help" }, t("Shows “See …” in place of a page number."))),
      h("div", { class: "field" }, h("label", {}, t("Page number format")), h("div", { class: "ix-checks" }, h("label", { class: "check", for: "ix-bold" }, bold, h("span", {}, t("Bold"))), h("label", { class: "check", for: "ix-italic" }, italic, h("span", {}, t("Italic"))))),
    ),
    h("div", { class: "dialog-actions" }, cancel, markAll, mark),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  const run = (all: boolean) => {
    const main = entry.value.trim();
    if (!main) return;
    const ref = see.value.trim();
    close();
    const n = ed.markIndexEntry({
      entry: main,
      sub: sub.value.trim() || undefined,
      see: ref ? `${t("See")} ${ref}` : undefined,
      bold: bold.checked,
      italic: italic.checked,
      all,
      text: picked || main,
    });
    if (n) toast(all ? tn(n, "Marked {n} place for the index", "Marked {n} places for the index") : t("Marked for the index"));
  };
  mark.addEventListener("click", () => run(false));
  markAll.addEventListener("click", () => run(true));
  for (const el of [entry, sub, see]) {
    el.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        run(false);
      }
    });
  }
  entry.focus();
  entry.select();
}
