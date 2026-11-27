// Columns: how many, the space between them, a line between, and (unchecking "Equal column width") the width of each
// column and the space after it, for the whole document or one section. The Left and Right presets make a narrow and a
// wide column.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { sideMargins } from "../editor/layout";
import { toUnits, unitWord } from "../units";
import { check, twipsOf, unitInput } from "./form";
import { h, modal } from "./dom";

/** The least a column may be: a quarter of an inch. */
const MIN_COL = 360;

/** Widths for a preset: one narrow column and one wide one, a third and two thirds of the room that is left. */
export function presetWidths(preset: "left" | "right", text: number, space: number): number[] {
  const room = Math.max(MIN_COL * 2, text - space);
  const narrow = Math.max(MIN_COL, Math.round(room / 3));
  const wide = room - narrow;
  return preset === "left" ? [narrow, wide] : [wide, narrow];
}

/** The text width of a section in twips (what the columns share). */
export function textWidth(ed: Editor): number {
  const info = ed.caretSectionInfo();
  const side = sideMargins(info as never, { gutterAtTop: !!ed.pageInfo.gutterAtTop });
  return info.width - side.left - side.right;
}

export function openColumns(ed: Editor) {
  const setup = ed.engine.pageSetup();
  const cur = ed.caretColumns();
  const sectionP = ed.caretParagraph();
  const several = (setup.sections?.length ?? 1) > 1 && sectionP !== undefined;
  let scope: "document" | "section" = "document";
  const text = textWidth(ed);

  const unequalWas = cur.widths.length === cur.num && cur.num > 1;
  const gap0 = cur.space > 0 || cur.num > 1 ? cur.space : 720;
  let widths: number[] = unequalWas ? [...cur.widths] : [];
  let spaces: number[] = unequalWas ? Array.from({ length: cur.num - 1 }, (_, i) => cur.spaces?.[i] ?? gap0) : [];

  const count = h("input", { type: "number", min: "1", max: "6", step: "1", id: "col-count", value: String(Math.min(6, Math.max(1, cur.num))), style: "width:100%;min-width:0" });
  const space = unitInput("col-space", gap0, unitWord() === "cm" ? t("Space between columns (cm)") : t("Space between columns (inches)"));
  space.id = "col-space";
  const equal = check("col-equal", t("Equal column width"), !unequalWas);
  const sep = h("input", { type: "checkbox", id: "col-sep", checked: cur.sep });
  const num = () => Math.min(6, Math.max(1, Math.round(parseFloat(count.value) || 1)));

  // One row for each column: its width, and the space after it.
  const rows = h("div", { class: "col-rows" });
  const rowInputs: { width: HTMLInputElement; space: HTMLInputElement | null }[] = [];
  const evenWidths = (n: number, gap: number) => {
    const w = Math.max(MIN_COL, Math.floor((text - gap * (n - 1)) / n));
    return Array.from({ length: n }, () => w);
  };
  const buildRows = () => {
    const n = num();
    if (widths.length !== n) {
      const gap = spaces[0] ?? twipsOf(space, gap0);
      widths = evenWidths(n, gap);
      spaces = Array.from({ length: n - 1 }, () => gap);
    }
    rowInputs.length = 0;
    rows.replaceChildren(
      h("div", { class: "col-row head", "aria-hidden": "true" }, h("span", {}, t("Column")), h("span", {}, t("Width")), h("span", {}, t("Spacing"))),
      ...widths.map((w, i) => {
        const wi = unitInput(`col-w${i}`, w, t("Width of column {n} ({unit})", { n: i + 1, unit: unitWord() }), toUnits(MIN_COL));
        const si = i < n - 1 ? unitInput(`col-s${i}`, spaces[i] ?? gap0, t("Space after column {n} ({unit})", { n: i + 1, unit: unitWord() })) : null;
        rowInputs.push({ width: wi, space: si });
        for (const input of [wi, si]) input?.addEventListener("input", paint);
        return h("div", { class: "col-row" }, h("span", {}, String(i + 1)), wi, si ?? h("span", {}));
      }),
    );
  };
  const readRows = () => {
    widths = rowInputs.map((r, i) => Math.max(MIN_COL, twipsOf(r.width, widths[i])));
    spaces = rowInputs.slice(0, -1).map((r, i) => twipsOf(r.space!, spaces[i] ?? gap0));
  };

  // A page drawn with the same columns: lines of text, which the browser fills down one column and across to the next.
  const preview = h("div", { class: "cp-preview", "aria-hidden": "true" });
  // Equal columns are drawn as the browser makes them (lines that fill down one column and across to the next); unequal ones as boxes of their widths.
  const cpText = h("div", { class: "cp-text" }, ...Array.from({ length: 90 }, () => h("span", { class: "cp-line" })));
  const flex = h("div", { class: "cp-flex", hidden: true });
  preview.append(h("div", { class: "cp-page" }, cpText, flex));
  const paint = () => {
    const n = num();
    const eq = equal.input.checked;
    space.disabled = n < 2 || !eq;
    sep.disabled = n < 2;
    rows.hidden = eq || n < 2;
    if (!eq && n > 1 && !rows.hidden && rowInputs.length) readRows();
    cpText.hidden = !eq;
    flex.hidden = eq;
    cpText.style.setProperty("--cp-n", String(n));
    // The page is drawn 8.5 inches across: a sixth of the preview's width is an inch and a quarter.
    cpText.style.setProperty("--cp-gap", `${Math.min(twipsOf(space, gap0) / 1440, 6.5 / Math.max(2, n)) * 13}px`);
    cpText.classList.toggle("cp-rule", sep.checked && n > 1);
    const unit = eq ? [] : widths;
    const gaps = eq ? Array.from({ length: n - 1 }, () => twipsOf(space, gap0)) : spaces;
    // The page is drawn 8.5 inches across: columns keep their share of the text width.
    const total = eq ? 1 : unit.reduce((a, b) => a + b, 0) + gaps.reduce((a, b) => a + b, 0) || 1;
    flex.replaceChildren(
      ...Array.from({ length: n }, (_, i) => {
        const w = eq ? (text - gaps.reduce((a, b) => a + b, 0)) / n : unit[i];
        const col = h("div", { class: "cp-col", style: `flex:0 0 ${((w / (eq ? text : total)) * 100).toFixed(2)}%` }, ...Array.from({ length: 24 }, () => h("span", { class: "cp-line" })));
        if (i < n - 1) {
          const g = h("div", { class: `cp-gap${sep.checked ? " cp-rule" : ""}`, style: `flex:0 0 ${(((gaps[i] ?? 0) / (eq ? text : total)) * 100).toFixed(2)}%` });
          return [col, g];
        }
        return [col];
      }).flat(),
    );
  };
  const refresh = () => {
    const n = num();
    if (!equal.input.checked && n > 1) buildRows();
    paint();
  };
  for (const el of [count, equal.input]) el.addEventListener("input", refresh);
  equal.input.addEventListener("change", refresh);
  space.addEventListener("input", paint);
  sep.addEventListener("change", paint);

  // The presets set the count and the widths at once.
  const presets = h("div", { class: "seg col-presets", role: "group", "aria-label": t("Presets") });
  const preset = (label: string, n: number, kind?: "left" | "right") => {
    const b = h("button", { type: "button" }, label);
    b.addEventListener("click", () => {
      count.value = String(n);
      const gap = twipsOf(space, gap0) || 720;
      if (kind) {
        equal.input.checked = false;
        widths = presetWidths(kind, text, gap);
        spaces = [gap];
      } else {
        equal.input.checked = true;
        widths = [];
        spaces = [];
      }
      refresh();
    });
    presets.append(b);
  };
  preset(t("One"), 1);
  preset(t("Two"), 2);
  preset(t("Three"), 3);
  preset(t("Left"), 2, "left");
  preset(t("Right"), 2, "right");

  const applyTo = several ? h("div", { class: "seg", role: "radiogroup", "aria-label": t("Apply to") }) : null;
  if (applyTo) {
    for (const [id, label] of [["document", t("Whole document")], ["section", t("This section")]] as const) {
      const b = h("button", { type: "button", role: "radio", "aria-checked": String(id === scope), class: id === scope ? "on" : "" }, label);
      b.addEventListener("click", () => {
        scope = id;
        applyTo.querySelectorAll("button").forEach((x) => {
          x.classList.toggle("on", x === b);
          x.setAttribute("aria-checked", String(x === b));
        });
      });
      applyTo.append(b);
    }
  }

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Columns") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Columns"))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", {}, t("Presets")), presets),
      h(
        "div",
        { class: "cp-row" },
        h(
          "div",
          { class: "cp-fields" },
          h("div", { class: "field" }, h("label", { for: "col-count" }, t("Number of columns")), count),
          equal.row,
          h("div", { class: "field" }, h("label", { for: "col-space" }, unitWord() === "cm" ? t("Space between columns (cm)") : t("Space between columns (inches)")), space),
          h("label", { class: "check", for: "col-sep" }, sep, h("span", {}, t("Line between columns"))),
        ),
        preview,
      ),
      rows,
      applyTo && h("div", { class: "field" }, h("label", {}, t("Apply to")), applyTo),
    ),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  if (!equal.input.checked) buildRows();
  paint();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    const n = num();
    const eq = equal.input.checked || n < 2;
    const room = text;
    let w: number[] | undefined;
    let sp: number[] | undefined;
    let twips: number;
    if (eq) {
      // Columns must stay wide enough to read: at least half an inch each.
      const maxGap = (room - n * 720) / Math.max(1, n - 1);
      twips = Math.round(Math.min(twipsOf(space, gap0), Math.max(0, maxGap)));
    } else {
      readRows();
      w = widths.slice(0, n);
      sp = spaces.slice(0, n - 1);
      // The columns and the spaces share the text width: shrink them together if they ask for more.
      const total = w.reduce((a, b) => a + b, 0) + sp.reduce((a, b) => a + b, 0);
      if (total > room) {
        const k = room / total;
        w = w.map((x) => Math.max(MIN_COL, Math.floor(x * k)));
        sp = sp.map((x) => Math.floor(x * k));
      }
      twips = sp[0] ?? gap0;
    }
    close();
    ed.setColumns(n, { space: twips, sep: sep.checked, all: scope === "document", ...(w ? { widths: w, spaces: sp } : {}) });
    ed.focus();
  });
}
