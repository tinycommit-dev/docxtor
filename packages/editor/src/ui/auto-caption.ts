// AutoCaption: give every table and picture a caption as soon as it is inserted. The settings last between sessions
// (the host keeps them); an empty label turns it off.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import type { AutoCaptions } from "../editor/lists";
import { h, modal } from "./dom";
import { captionLabels } from "./caption";

export function openAutoCaption(ed: Editor) {
  const labels = captionLabels(ed);
  const kinds: ["table" | "image", string][] = [
    ["table", t("Tables")],
    ["image", t("Pictures")],
  ];
  const rows = kinds.map(([kind, title]) => {
    const cur = ed.autoCaptions[kind];
    const label = h("select", { id: `ac-${kind}`, "aria-label": t("Label for {what}", { what: title }) }, h("option", { value: "" }, t("No caption")), ...labels.map((l) => h("option", { value: l }, l)));
    label.value = cur?.label ?? "";
    const position = h("select", { id: `ac-${kind}-pos`, "aria-label": t("Position for {what}", { what: title }) }, h("option", { value: "above" }, t("Above")), h("option", { value: "below" }, t("Below")));
    position.value = cur?.position ?? (kind === "table" ? "above" : "below");
    label.addEventListener("change", () => (position.disabled = !label.value));
    position.disabled = !label.value;
    return { kind, label, position, el: h("div", { class: "ac-row" }, h("span", { class: "ac-what" }, title), label, position) };
  });

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("Save"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("AutoCaption") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("AutoCaption")), h("p", { class: "sub" }, t("Adds a caption each time you insert a table or a picture from the menu."))),
    h("div", { class: "dialog-body" }, ...rows.map((r) => r.el)),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const next: AutoCaptions = {};
    for (const r of rows) if (r.label.value) next[r.kind] = { label: r.label.value, position: r.position.value as "above" | "below" };
    ed.autoCaptions = next;
    ed.autoCaptionsSave?.(next);
    close();
  });
}
