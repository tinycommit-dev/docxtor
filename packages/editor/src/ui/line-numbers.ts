// Line numbers: on or off, when to start again, how often to show one, and which sections.
import type { LineNumbers, Op } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal } from "./dom";

type Restart = "newPage" | "newSection" | "continuous";
const RESTARTS: [Restart, string][] = [
  ["newPage", msg("Each page")],
  ["newSection", msg("Each section")],
  ["continuous", msg("Continuous")],
];

export function openLineNumbers(ed: Editor) {
  const setup = ed.engine.pageSetup();
  const info = setup.sections?.[ed.caretSection()];
  const cur: LineNumbers | undefined = info?.lineNumbers;
  const sectionP = ed.caretParagraph();
  const several = (setup.sections?.length ?? 1) > 1 && sectionP !== undefined;
  let restart: Restart = (RESTARTS.some(([r]) => r === cur?.restart) ? cur!.restart : "newPage") as Restart;
  let scope: "document" | "section" = "document";

  const show = h("input", { type: "checkbox", id: "ln-show", checked: !!cur });
  const countBy = h("input", { type: "number", min: "1", step: "1", value: String(cur?.countBy ?? 1), id: "ln-count", "aria-label": t("Count by") });

  const group = (label: string, items: [string, string][], get: () => string, set: (v: string) => void) => {
    const el = h("div", { class: "seg", role: "radiogroup", "aria-label": label });
    const buttons = items.map(([v, text]) => {
      const b = h("button", { type: "button", role: "radio" }, t(text));
      b.addEventListener("click", () => {
        set(v);
        paint();
      });
      el.append(b);
      return [v, b] as const;
    });
    const paint = () => {
      for (const [v, b] of buttons) {
        b.classList.toggle("on", v === get());
        b.setAttribute("aria-checked", String(v === get()));
      }
    };
    paint();
    return el;
  };
  const restartGroup = group(t("Restart"), RESTARTS, () => restart, (v) => (restart = v as Restart));
  const applyTo = several
    ? group(t("Apply to"), [["document", msg("Whole document")], ["section", msg("This section")]], () => scope, (v) => (scope = v as typeof scope))
    : null;

  const dim = () => {
    const off = !show.checked;
    countBy.disabled = off;
    restartGroup.querySelectorAll("button").forEach((b) => ((b as HTMLButtonElement).disabled = off));
    restartGroup.classList.toggle("off", off);
  };
  show.addEventListener("change", dim);
  dim();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Line numbers") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Line numbers"))),
    h(
      "div",
      { class: "dialog-body" },
      h("label", { class: "check", for: "ln-show" }, show, h("span", {}, t("Show line numbers"))),
      h("div", { class: "field" }, h("label", {}, t("Restart")), restartGroup),
      h("div", { class: "field" }, h("label", { for: "ln-count" }, t("Count by")), countBy),
      applyTo && h("div", { class: "field" }, h("label", {}, t("Apply to")), applyTo),
    ),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    const section = scope === "section" && sectionP !== undefined ? { section: sectionP } : {};
    const op: Op = show.checked
      ? { op: "set_line_numbers", on: true, count_by: Math.max(1, Math.round(parseFloat(countBy.value) || 1)), restart, ...section }
      : { op: "set_line_numbers", on: false, ...section };
    close();
    ed.exec(op);
    ed.reloadPageSetup();
    ed.focus();
  });
}
