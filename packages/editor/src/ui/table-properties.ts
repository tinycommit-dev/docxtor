// The Table properties dialog (Format, Table, Table properties…, and the right-click menu in a table), as Word has it:
// Table (size, alignment, indent, text wrapping, resizing, spacing and default cell margins), Row (height, breaking
// across pages, header row), Column (width), Cell (width, vertical alignment, margins, wrapping, text direction) and
// Alt text. Lengths follow the unit setting. Only what was changed is written, in one undoable step.
import type { Op, TableWidth } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { formatUnits, parseLength, unitName } from "../units";
import { openBordersShading } from "./borders-shading";
import { h, modal } from "./dom";
import { cellSelection, type CellSelection } from "./table-selection";

type Margins = [number, number, number, number];
type Tab = "table" | "row" | "column" | "cell" | "alt";

const TABS: [Tab, string][] = [
  ["table", msg("Table")],
  ["row", msg("Row")],
  ["column", msg("Column")],
  ["cell", msg("Cell")],
  ["alt", msg("Alt text")],
];
const ALIGNS: [string, string][] = [
  ["left", msg("Left")],
  ["center", msg("Centered")],
  ["right", msg("Right")],
];
const VALIGNS: [string, string][] = [
  ["top", msg("Top")],
  ["center", msg("Center")],
  ["bottom", msg("Bottom")],
];
const DIRECTIONS: [string, string][] = [
  ["lrTb", msg("Across")],
  ["tbRl", msg("Down")],
  ["btLr", msg("Up")],
];

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** A width as the two controls show it: the kind and the number in that kind's unit. */
function widthControls(id: string, w: TableWidth) {
  const kind = h(
    "select",
    { id: `${id}-kind` },
    h("option", { value: "auto", selected: w.type === "auto" }, t("Automatic")),
    h("option", { value: "dxa", selected: w.type === "dxa" }, t("Length ({unit})", { unit: unitName() })),
    h("option", { value: "pct", selected: w.type === "pct" }, t("Percent")),
  );
  const num = h("input", { id: `${id}-value`, type: "text", inputmode: "decimal", autocomplete: "off", "aria-label": t("Width"), value: w.type === "pct" ? String(Math.round(w.value / 5) / 10) : w.type === "dxa" ? formatUnits(w.value) : "" });
  const sync = () => {
    num.disabled = kind.value === "auto";
    // Switching the measure turns the number into the same size in the other one only when it can.
  };
  kind.addEventListener("change", sync);
  sync();
  return {
    el: h("div", { class: "tp-pair" }, kind, num),
    /** The width the controls say, or the old one when the number can't be read. */
    read(was: TableWidth): TableWidth {
      if (kind.value === "auto") return { type: "auto", value: 0 };
      if (kind.value === "pct") {
        const n = Number(num.value.replace(",", "."));
        return Number.isFinite(n) && n > 0 ? { type: "pct", value: Math.round(Math.min(n, 600) * 50) } : was;
      }
      const v = parseLength(num.value);
      return v !== null && v > 0 ? { type: "dxa", value: Math.max(40, v) } : was;
    },
  };
}

export function openTableProperties(ed: Editor, tab: Tab = "table", sel: CellSelection | null = cellSelection(ed)) {
  if (!sel) return;
  const d = sel.details;
  const row0 = d.rows[sel.rows[0]] ?? d.rows[0];

  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);
  const check = (id: string, label: string, on: boolean) => {
    const input = h("input", { id, type: "checkbox", checked: on });
    return { input, row: h("label", { class: "check", for: id }, input, h("span", {}, label)) };
  };
  const length = (id: string, twips: number) => h("input", { id, type: "text", inputmode: "decimal", autocomplete: "off", value: formatUnits(twips) });
  const select = (id: string, items: [string, string][], value: string) => h("select", { id }, ...items.map(([v, label]) => h("option", { value: v, selected: v === value }, t(label))));
  const group = (title: string, ...body: HTMLElement[]) => h("div", { class: "pd-group" }, h("h3", {}, title), ...body);
  const grid = (...cells: HTMLElement[]) => h("div", { class: "pd-grid" }, ...cells);

  // Table
  const width = widthControls("tp-width", d.table.width);
  const align = select("tp-align", ALIGNS, d.table.align);
  const indent = length("tp-indent", d.table.indent);
  const wrap = select("tp-wrap", [["none", msg("None")], ["around", msg("Around")]], d.table.wrap ? "around" : "none");
  const autofit = check("tp-autofit", t("Resize to fit the contents"), d.table.autofit);
  const spacingOn = check("tp-spacing-on", t("Space between cells"), d.table.spacing > 0);
  const spacing = length("tp-spacing", d.table.spacing || 15);
  const margin = (id: string, v: number) => length(id, v);
  const [mt, mr, mb, ml] = d.table.cellMargins;
  const tTop = margin("tp-mt", mt);
  const tBottom = margin("tp-mb", mb);
  const tLeft = margin("tp-ml", ml);
  const tRight = margin("tp-mr", mr);
  const borders = h("button", { class: "btn ghost pd-tabs", type: "button" }, t("Borders and shading…"));
  const paneTable = pane(
    "table",
    group(t("Size"), grid(field(t("Preferred width"), width.el, "tp-width-kind"))),
    group(t("Position"), grid(field(t("Alignment"), align, "tp-align"), field(t("Indent from left ({unit})", { unit: unitName() }), indent, "tp-indent"), field(t("Text wrapping"), wrap, "tp-wrap"))),
    group(
      t("Options"),
      autofit.row,
      spacingOn.row,
      grid(field(t("Space between cells ({unit})", { unit: unitName() }), spacing, "tp-spacing")),
      grid(field(t("Top margin ({unit})", { unit: unitName() }), tTop, "tp-mt"), field(t("Bottom margin ({unit})", { unit: unitName() }), tBottom, "tp-mb"), field(t("Left margin ({unit})", { unit: unitName() }), tLeft, "tp-ml"), field(t("Right margin ({unit})", { unit: unitName() }), tRight, "tp-mr")),
    ),
  );

  // Row
  const rowCount = sel.rows[1] - sel.rows[0] + 1;
  const rowLabel = h("p", { class: "tp-note", id: "tp-rows" }, rowCount > 1 ? t("Rows {a} to {b}", { a: sel.rows[0] + 1, b: sel.rows[1] + 1 }) : t("Row {n}", { n: sel.rows[0] + 1 }));
  const heightOn = check("tp-height-on", t("Set the height"), row0.rule !== "auto");
  const height = length("tp-height", row0.height || 360);
  const rule = select("tp-rule", [["atLeast", msg("At least")], ["exact", msg("Exactly")]], row0.rule === "exact" ? "exact" : "atLeast");
  const allowBreak = check("tp-break", t("Allow the row to break across pages"), !row0.cantSplit);
  const header = check("tp-header", t("Repeat as a header row at the top of each page"), row0.header);
  const paneRow = pane("row", rowLabel, group(t("Size"), heightOn.row, grid(field(t("Height ({unit})", { unit: unitName() }), height, "tp-height"), field(t("Height is"), rule, "tp-rule"))), group(t("Options"), allowBreak.row, header.row));

  // Column
  const colLabel = h("p", { class: "tp-note", id: "tp-cols" }, d.span > 1 ? t("Columns {a} to {b}", { a: d.col + 1, b: d.col + d.span }) : t("Column {n}", { n: d.col + 1 }));
  const colWidth = widthControls("tp-col", d.columnWidth);
  const paneCol = pane("column", colLabel, group(t("Size"), grid(field(t("Preferred width"), colWidth.el, "tp-col-kind"))));

  // Cell
  const cellWidth = widthControls("tp-cell", d.cell.width);
  const valign = select("tp-valign", VALIGNS, d.cell.valign === "both" ? "top" : d.cell.valign);
  const direction = select("tp-dir", DIRECTIONS, d.cell.textDirection === "tbRlV" ? "tbRl" : d.cell.textDirection);
  const ownMargins = check("tp-cell-margins", t("Use margins of its own"), !!d.cell.margins);
  const cm = d.cell.margins ?? d.table.cellMargins;
  const cTop = margin("tp-cmt", cm[0]);
  const cBottom = margin("tp-cmb", cm[2]);
  const cLeft = margin("tp-cml", cm[3]);
  const cRight = margin("tp-cmr", cm[1]);
  const noWrap = check("tp-nowrap", t("Keep the text on one line"), d.cell.noWrap);
  const fit = check("tp-fit", t("Fit the text to the cell"), d.cell.fitText);
  const margins = () => {
    for (const c of [cTop, cBottom, cLeft, cRight]) c.disabled = !ownMargins.input.checked;
  };
  ownMargins.input.addEventListener("change", margins);
  margins();
  const paneCell = pane(
    "cell",
    group(t("Size"), grid(field(t("Preferred width"), cellWidth.el, "tp-cell-kind"))),
    group(t("Layout"), grid(field(t("Vertical alignment"), valign, "tp-valign"), field(t("Text direction"), direction, "tp-dir"))),
    group(
      t("Cell margins"),
      ownMargins.row,
      grid(field(t("Top ({unit})", { unit: unitName() }), cTop, "tp-cmt"), field(t("Bottom ({unit})", { unit: unitName() }), cBottom, "tp-cmb"), field(t("Left ({unit})", { unit: unitName() }), cLeft, "tp-cml"), field(t("Right ({unit})", { unit: unitName() }), cRight, "tp-cmr")),
    ),
    group(t("Options"), noWrap.row, fit.row),
  );

  // Alt text
  const title = h("input", { id: "tp-title", type: "text", autocomplete: "off", value: d.table.title });
  const description = h("textarea", { id: "tp-description", rows: "4" }, d.table.description);
  const paneAlt = pane(
    "alt",
    h("p", { class: "tp-note" }, t("Alt text tells people who can't see the table what it shows.")),
    field(t("Title"), title, "tp-title"),
    field(t("Description"), description, "tp-description"),
  );

  const panes: [Tab, HTMLElement][] = [["table", paneTable], ["row", paneRow], ["column", paneCol], ["cell", paneCell], ["alt", paneAlt]];
  const buttons = TABS.map(([id, label]) => {
    const b = h("button", { type: "button", role: "tab", id: `tp-tab-${id}`, "aria-controls": `tp-pane-${id}`, "aria-selected": "false" }, t(label));
    b.addEventListener("click", () => show(id));
    return [id, b] as const;
  });
  function show(id: Tab) {
    for (const [tid, b] of buttons) {
      b.classList.toggle("on", tid === id);
      b.setAttribute("aria-selected", String(tid === id));
    }
    for (const [pid, el] of panes) el.hidden = pid !== id;
  }
  show(tab);

  const error = h("p", { class: "dl-error", role: "alert", hidden: true });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog tp-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Table properties") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Table properties"))),
    h("div", { class: "pd-tablist seg", role: "tablist" }, ...buttons.map(([, b]) => b)),
    h("div", { class: "dialog-body" }, ...panes.map(([, el]) => el), error),
    h("div", { class: "dialog-actions" }, borders, cancel, ok),
  );
  spacingOn.input.addEventListener("change", () => (spacing.disabled = !spacingOn.input.checked));
  spacing.disabled = !spacingOn.input.checked;
  heightOn.input.addEventListener("change", () => (height.disabled = rule.disabled = !heightOn.input.checked));
  height.disabled = rule.disabled = !heightOn.input.checked;
  const close = modal(dialog);

  /** The ops for what changed. */
  const collect = (): Op[] => {
    const ops: Op[] = [];
    // A field left as it was shown keeps the exact value it came from (the field rounds to two places).
    const len = (input: HTMLInputElement, old: number, min = 0) => {
      if (input.value === formatUnits(old)) return old;
      const v = parseLength(input.value);
      return v === null ? old : Math.max(min, v);
    };
    const tableFormat: Record<string, unknown> = {};
    const w = width.read(d.table.width);
    if (!same(w, d.table.width)) tableFormat.width = w;
    if (align.value !== d.table.align) tableFormat.align = align.value;
    const ind = len(indent, d.table.indent);
    if (ind !== d.table.indent) tableFormat.indent = ind;
    if ((wrap.value === "around") !== d.table.wrap) tableFormat.wrap = wrap.value;
    if (autofit.input.checked !== d.table.autofit) tableFormat.autofit = autofit.input.checked;
    const sp = spacingOn.input.checked ? Math.max(1, len(spacing, d.table.spacing || 15)) : 0;
    if (sp !== d.table.spacing) tableFormat.spacing = sp;
    const m: Margins = [len(tTop, mt), len(tRight, mr), len(tBottom, mb), len(tLeft, ml)];
    if (!same(m, d.table.cellMargins)) tableFormat.cellMargins = m;
    if (title.value !== d.table.title) tableFormat.title = title.value;
    if (description.value !== d.table.description) tableFormat.description = description.value;
    if (Object.keys(tableFormat).length) ops.push({ op: "table_format", at: sel.from, ...tableFormat });

    const rowFormat: Record<string, unknown> = {};
    const hv = heightOn.input.checked ? Math.max(20, len(height, row0.height || 360)) : 0;
    const hr = heightOn.input.checked ? rule.value : "auto";
    if (hr !== row0.rule || (hr !== "auto" && hv !== row0.height)) rowFormat.height = { value: hv, rule: hr };
    if (!allowBreak.input.checked !== row0.cantSplit) rowFormat.cantSplit = !allowBreak.input.checked;
    if (header.input.checked !== row0.header) rowFormat.header = header.input.checked;
    if (Object.keys(rowFormat).length) ops.push({ op: "table_rows", from: sel.from, to: sel.to, ...rowFormat });

    const cw = colWidth.read(d.columnWidth);
    if (!same(cw, d.columnWidth)) ops.push({ op: "table_column", at: sel.from, col: d.col, width: cw });

    const cell: Record<string, unknown> = {};
    const ew = cellWidth.read(d.cell.width);
    if (!same(ew, d.cell.width)) cell.width = ew;
    const wasV = d.cell.valign === "both" ? "top" : d.cell.valign;
    if (valign.value !== wasV) cell.valign = valign.value;
    if (direction.value !== d.cell.textDirection && !(direction.value === "tbRl" && d.cell.textDirection === "tbRlV")) cell.textDirection = direction.value;
    if (noWrap.input.checked !== d.cell.noWrap) cell.noWrap = noWrap.input.checked;
    if (fit.input.checked !== d.cell.fitText) cell.fitText = fit.input.checked;
    if (ownMargins.input.checked) {
      const cmv: Margins = [len(cTop, cm[0]), len(cRight, cm[1]), len(cBottom, cm[2]), len(cLeft, cm[3])];
      if (!d.cell.margins || !same(cmv, d.cell.margins)) cell.margins = cmv;
    } else if (d.cell.margins) cell.clearMargins = true;
    if (Object.keys(cell).length) ops.push({ op: "table_cells", from: sel.from, to: sel.to, action: { type: "format", ...cell } });
    return ops;
  };

  const apply = (): boolean => {
    const ops = collect();
    if (!ops.length) return true;
    return !!ed.exec(ops, {
      onError: (message) => {
        error.textContent = message;
        error.hidden = false;
      },
    });
  };
  ok.addEventListener("click", () => {
    if (!apply()) return;
    close();
    ed.focus();
  });
  cancel.addEventListener("click", close);
  borders.addEventListener("click", () => {
    // What is set here is applied first, then the Borders and shading dialog opens on the table.
    if (!apply()) return;
    close();
    openBordersShading(ed, undefined, "table");
  });
  dialog.addEventListener("keydown", (e) => {
    const target = e.target as HTMLElement;
    if (e.key === "Enter" && target.tagName === "INPUT" && (target as HTMLInputElement).type !== "checkbox") {
      e.preventDefault();
      ok.click();
    }
  });

  function pane(id: Tab, ...body: HTMLElement[]) {
    return h("div", { class: "pd-pane", role: "tabpanel", id: `tp-pane-${id}`, "aria-labelledby": `tp-tab-${id}` }, ...body);
  }
}
