// Table of figures: a list of the captions with one label (Figure, Table, …), with page numbers.
import { t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { captionLabels } from "./caption";
import { h, modal } from "./dom";

export function openTableOfFigures(ed: Editor) {
  if (ed.story) return;
  const caps = ed.xrefTargets().captions;
  const labels = captionLabels(ed);
  // The first label that has captions, else the first of the list.
  const first = labels.find((l) => caps.some((c) => c.label === l)) ?? labels[0];
  const select = h("select", { id: "tof-label" }, ...labels.map((l) => h("option", { value: l }, l)));
  select.value = first;
  const help = h("p", { class: "help", "aria-live": "polite" });
  const paint = () => {
    const n = caps.filter((c) => c.label === select.value).length;
    help.textContent = n ? tn(n, "{n} caption will be listed.", "{n} captions will be listed.") : t("There are no captions with this label yet.");
  };
  select.addEventListener("change", paint);
  paint();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const insert = h("button", { class: "btn primary", type: "button" }, t("Insert"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Table of figures") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Table of figures")), h("p", { class: "sub" }, t("Lists captions with page numbers, before the paragraph you are in. Update fields keeps it current."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "tof-label" }, t("Caption label")), select, help)),
    h("div", { class: "dialog-actions" }, cancel, insert),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  insert.addEventListener("click", () => {
    const label = select.value;
    close();
    ed.insertTableOfFigures(label);
  });
}
