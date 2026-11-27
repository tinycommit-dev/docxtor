// Drop caps (Insert, Drop cap): None, Dropped, In margin, and Options (the letter's font, how many lines it drops and how far
// it sits from the text). The first letter moves into a frame of its own, as Word does it, so the file opens the same in Word.
import type { Editor } from "../editor/editor";
import { t } from "@docxtor/extensions";
import { h, modal } from "./dom";
import { choice, field, grid, twipsOf, unitInput, unitLabel } from "./form";

export type DropKind = "none" | "drop" | "margin";

export interface DropCapState {
  kind: DropKind;
  lines: number;
  /** Distance from the text, in twips. */
  distance: number;
  font: string;
}

/** The drop cap of the paragraph the caret is in (or the one the caret's paragraph is the text of). */
export function dropCapState(ed: Editor): DropCapState {
  const none: DropCapState = { kind: "none", lines: 3, distance: 0, font: "" };
  const p = ed.caretParagraph();
  if (p === undefined) return none;
  const el = ed.paraElement(p);
  const frame = el?.classList.contains("dropcap") ? el : el?.previousElementSibling?.classList.contains("dropcap") ? (el.previousElementSibling as HTMLElement) : null;
  if (!frame) return none;
  const gap = Number(frame.dataset.gap ?? 0);
  return {
    kind: frame.dataset.drop === "margin" ? "margin" : "drop",
    lines: Number(frame.dataset.lines ?? 3) || 3,
    distance: Math.round(gap * 15),
    font: frame.querySelector<HTMLElement>(".s")?.style.fontFamily.split(",")[0].replace(/["']/g, "") ?? "",
  };
}

/** Make or take away the drop cap of the caret's paragraph. */
export function setDropCap(ed: Editor, kind: DropKind, extra: { lines?: number; distance?: number; font?: string } = {}) {
  const p = ed.caretParagraph();
  if (p === undefined || ed.readOnly) return;
  const now = dropCapState(ed);
  ed.exec({ op: "set_drop_cap", id: p, kind, ...(kind === "none" ? {} : { lines: extra.lines ?? now.lines, ...(extra.distance !== undefined ? { distance: extra.distance } : {}), ...(extra.font ? { font: extra.font } : {}) }) });
  ed.reloadPageSetup();
}

export function openDropCap(ed: Editor) {
  const now = dropCapState(ed);
  const position = choice<DropKind>(
    t("Position"),
    [
      ["none", t("None")],
      ["drop", t("Dropped")],
      ["margin", t("In margin")],
    ],
    now.kind,
  );
  const font = h("input", { id: "dc-font", type: "text", value: now.font, placeholder: t("Same as the text"), autocomplete: "off", spellcheck: "false", "aria-label": t("Font"), style: "width:100%;min-width:0" });
  const lines = h("input", { id: "dc-lines", type: "number", min: "1", max: "10", step: "1", value: String(now.lines), "aria-label": t("Lines to drop"), style: "width:100%;min-width:0" });
  const distance = unitInput("dc-distance", now.distance, unitLabel(t("Distance from text")));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Drop cap") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Drop cap"))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", {}, t("Position")), position.el),
      field(t("Font"), font, "dc-font"),
      grid(field(t("Lines to drop"), lines, "dc-lines"), h("div", { class: "field" }, h("label", { for: "dc-distance" }, unitLabel(t("Distance from text"))), distance)),
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const n = Math.max(1, Math.min(10, Math.round(parseFloat(lines.value) || 3)));
    const kind = position.value;
    const d = twipsOf(distance, now.distance);
    close();
    setDropCap(ed, kind, { lines: n, distance: d, font: font.value.trim() || undefined });
    ed.focus();
  });
}
