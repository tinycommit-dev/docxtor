// The Document grid part of Page setup, Layout: a grid of lines (and characters) that East Asian text sits on. The
// file stores a line pitch and what is added to the font size to get a character's width; the box shows what Word shows,
// the number of characters on a line and of lines on a page, worked out against the section's text area.
import type { DocGrid, SectionInfo } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { gridCounts, gridFor } from "../editor/grid";
import { h } from "./dom";

const TYPES: [DocGrid["type"], string][] = [
  ["default", msg("No grid")],
  ["lines", msg("Lines only")],
  ["linesAndChars", msg("Lines and characters")],
  ["snapToChars", msg("Snap to characters")],
];

/**
 * The grid controls for a section. `area()` gives the text area in points as the dialog's margins and paper stand now.
 * `patch` is the grid to write, or nothing when the controls still say what the section had.
 */
export function docGridGroup(info: SectionInfo, area: () => { width: number; height: number }): { group: HTMLElement; patch: () => DocGrid | undefined } {
  const was = info.docGrid;
  const base = info.gridBase ?? 10.5;
  const a0 = area();
  const counts = gridCounts(was, base, a0.width, a0.height);
  const kind = h("select", { id: "ps-grid" }, ...TYPES.map(([v, label]) => h("option", { value: v, selected: v === (was?.type ?? "default") }, t(label))));
  const chars = h("input", { id: "ps-grid-chars", type: "number", min: "1", max: "400", step: "1", value: String(counts.chars) });
  const lines = h("input", { id: "ps-grid-lines", type: "number", min: "1", max: "400", step: "1", value: String(counts.lines) });
  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);
  const sync = () => {
    const k = kind.value;
    lines.disabled = k === "default";
    chars.disabled = k !== "linesAndChars" && k !== "snapToChars";
  };
  kind.addEventListener("change", sync);
  sync();
  const group = h(
    "div",
    { class: "pd-group" },
    h("h3", {}, t("Document grid")),
    h("div", { class: "pd-grid" }, field(t("Grid"), kind, "ps-grid"), h("div", { class: "field" }), field(t("Characters per line"), chars, "ps-grid-chars"), field(t("Lines per page"), lines, "ps-grid-lines")),
  );
  const patch = (): DocGrid | undefined => {
    const type = kind.value as DocGrid["type"];
    const wasType = was?.type ?? "default";
    const c = Math.round(Number(chars.value) || counts.chars);
    const l = Math.round(Number(lines.value) || counts.lines);
    if (type === wasType && (type === "default" || ((type === "lines" || c === counts.chars) && l === counts.lines))) return undefined;
    const { width, height } = area();
    return gridFor(type, base, width, height, c, l);
  };
  return { group, patch };
}
