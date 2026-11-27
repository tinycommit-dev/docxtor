// Set numbering value: start a new list at a number, or carry on the previous list and, if wanted, skip ahead.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal } from "./dom";

export function openNumberingValue(ed: Editor) {
  const range = ed.range();
  const state = ed.format();
  if (!range || state?.listValue === undefined) return;
  const id = range[0].p;
  const now = state.listValue;
  let how: "new" | "continue" = "new";

  const valueInput = h("input", { type: "number", id: "nv-value", min: "0", step: "1", value: "1", style: "width:100%;min-width:0" });
  const skip = h("input", { type: "checkbox", id: "nv-skip" });
  const error = h("p", { class: "dl-error", role: "alert", hidden: true });
  const hint = h("span", { class: "help" }, t("This item is number {n} now.", { n: now }));

  const how_ = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Numbering") });
  const choices: ["new" | "continue", string][] = [["new", t("Start new list")], ["continue", t("Continue previous list")]];
  for (const [value, label] of choices) {
    const b = h("button", { type: "button", role: "radio", "aria-checked": "false", "data-v": value }, label);
    b.addEventListener("click", () => {
      how = value;
      if (value === "continue") valueInput.value = String(now);
      else if (valueInput.value === String(now)) valueInput.value = "1";
      paint();
    });
    how_.append(b);
  }
  const skipRow = h("label", { class: "check", for: "nv-skip" }, skip, h("span", {}, t("Advance value (skip numbers)")));
  skip.addEventListener("change", paint);

  function paint() {
    how_.querySelectorAll("button").forEach((b) => {
      const on = (b as HTMLElement).dataset.v === how;
      b.classList.toggle("on", on);
      b.setAttribute("aria-checked", String(on));
    });
    skipRow.hidden = how !== "continue";
    // Carrying on shows the number the list has reached, unless numbers are to be skipped.
    valueInput.disabled = how === "continue" && !skip.checked;
    error.hidden = true;
  }
  paint();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog nv-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Set numbering value") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Set numbering value"))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, how_, hint),
      skipRow,
      h("div", { class: "field" }, h("label", { for: "nv-value" }, t("Set value to")), valueInput),
      error,
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const value = Math.max(0, Math.round(parseFloat(valueInput.value) || 0));
    const op =
      how === "new"
        ? { op: "set_numbering_value", id, value }
        : skip.checked
          ? { op: "set_numbering_value", id, value, continue_previous: true }
          : { op: "continue_numbering", id };
    const res = ed.exec(op, {
      select: ed.range() ? { anchor: range[0], focus: range[1] } : undefined,
      onError: (message) => {
        error.textContent = message;
        error.hidden = false;
      },
    });
    if (!res) return;
    close();
    ed.focus();
  });
}
