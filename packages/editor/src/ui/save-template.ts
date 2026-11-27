// File, Save as template: name a template, say what it is for, and choose where it is kept.
import { t } from "@docxtor/extensions";
import { h, modal } from "./dom";

export interface TemplateTarget {
  id: string;
  label: string;
  note?: string;
}

export interface SaveTemplateAnswer {
  name: string;
  description: string;
  store: string;
}

/** The dialog. `save` may reject with a message, which the dialog shows and stays open for. */
export function openSaveTemplate(opts: { name: string; stores: TemplateTarget[]; save: (answer: SaveTemplateAnswer) => Promise<void>; onClose?: () => void }) {
  const name = h("input", { type: "text", id: "tpl-name", value: opts.name, autocomplete: "off", spellcheck: "false" });
  const description = h("input", { type: "text", id: "tpl-desc", autocomplete: "off", placeholder: t("What it is for, in a few words") });
  const where = h("select", { id: "tpl-where" }, ...opts.stores.map((s) => h("option", { value: s.id }, s.label)));
  const note = h("span", { class: "help" });
  const showNote = () => (note.textContent = opts.stores.find((s) => s.id === where.value)?.note ?? "");
  where.addEventListener("change", showNote);
  showNote();
  const problem = h("p", { class: "field-error", role: "alert", hidden: true });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const save = h("button", { class: "btn primary", type: "button" }, t("Save"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Save as template"), style: "width:min(var(--dialog-s),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Save as template")), h("p", { class: "sub" }, t("New documents made from it start with this document's text, styles and page setup."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: "tpl-name" }, t("Name")), name),
      h("div", { class: "field" }, h("label", { for: "tpl-desc" }, t("Description")), description),
      opts.stores.length > 1 ? h("div", { class: "field" }, h("label", { for: "tpl-where" }, t("Keep it")), where, note) : h("div", { class: "field" }, note),
      problem,
    ),
    h("div", { class: "dialog-actions" }, cancel, save),
  );
  const close = modal(dialog, opts.onClose);
  cancel.addEventListener("click", close);
  const go = async () => {
    const clean = name.value.trim();
    if (!clean) {
      problem.textContent = t("Give the template a name.");
      problem.hidden = false;
      name.focus();
      return;
    }
    save.disabled = true;
    try {
      await opts.save({ name: clean, description: description.value.trim(), store: where.value });
      close();
    } catch (e) {
      problem.textContent = t("Couldn't save the template: {reason}", { reason: (e as Error).message });
      problem.hidden = false;
      save.disabled = false;
    }
  };
  save.addEventListener("click", () => void go());
  for (const i of [name, description]) {
    i.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        void go();
      }
    });
  }
  name.focus();
  name.select();
}
