// The smaller table dialogs: Insert table, Split cells, Insert cells, Delete cells, Convert text to table, Convert table
// to text and Formula. Each reads what it needs when it opens and applies one undoable step.
import { msg, t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { parseLength, unitName } from "../units";
import { h, modal, toast } from "./dom";
import { cellSelection, type CellSelection } from "./table-selection";

const radios = (name: string, items: [string, string][], value: string) => {
  const inputs = items.map(([v, label]) => {
    const input = h("input", { type: "radio", name, value: v, id: `${name}-${v}`, checked: v === value });
    return { v, input, row: h("label", { class: "check", for: `${name}-${v}` }, input, h("span", {}, t(label))) };
  });
  return { el: h("div", { class: "td-radios", role: "radiogroup" }, ...inputs.map((i) => i.row)), get: () => inputs.find((i) => i.input.checked)?.v ?? value };
};

const shell = (label: string, id: string, sub: string | null, body: HTMLElement[], ok: HTMLButtonElement, cancel: HTMLButtonElement, error?: HTMLElement) =>
  h(
    "div",
    { class: "dialog td-dialog", id, role: "dialog", "aria-modal": "true", "aria-label": label },
    h("div", { class: "dialog-head" }, h("h2", {}, label), sub ? h("p", { class: "sub" }, sub) : null),
    h("div", { class: "dialog-body" }, ...body, error ?? null),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );

const buttons = (okLabel: string) => ({ cancel: h("button", { class: "btn ghost", type: "button" }, t("Cancel")), ok: h("button", { class: "btn primary", type: "button" }, okLabel) });

/** Press OK on Enter in a field. */
function enterToConfirm(dialog: HTMLElement, ok: HTMLButtonElement) {
  dialog.addEventListener("keydown", (e) => {
    const target = e.target as HTMLElement;
    if (e.key === "Enter" && target.tagName === "INPUT" && (target as HTMLInputElement).type !== "checkbox" && (target as HTMLInputElement).type !== "radio") {
      e.preventDefault();
      ok.click();
    }
  });
}

const whole = (input: HTMLInputElement, min: number, max: number, fallback: number) => {
  const n = Math.round(Number(input.value.replace(",", ".")));
  return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback;
};

// ---------------------------------------------------------------------------------------------------------------------
// Insert table
// ---------------------------------------------------------------------------------------------------------------------

/** How the new table's columns are sized. */
type Fit = "fixed" | "contents" | "window";
const FITS: [Fit, string][] = [
  [("fixed"), msg("Fixed column width")],
  [("contents"), msg("Fit to the contents")],
  [("window"), msg("Fit to the window")],
];

/** Insert table…: rows, columns and how the columns are sized. Inside a table it makes a table in the cell. */
export function openInsertTable(ed: Editor) {
  const r = ed.range();
  if (!r) return;
  const inCell = !!ed.format()?.inTable;
  const rows = h("input", { id: "it-rows", type: "number", min: "1", max: "200", step: "1", value: "3" });
  const cols = h("input", { id: "it-cols", type: "number", min: "1", max: "50", step: "1", value: "3" });
  const fit = h("select", { id: "it-fit" }, ...FITS.map(([v, label]) => h("option", { value: v, selected: v === "fixed" }, t(label))));
  const widthLabel = h("label", { for: "it-width" }, t("Column width ({unit})", { unit: unitName() }));
  const width = h("input", { id: "it-width", type: "text", inputmode: "decimal", autocomplete: "off", placeholder: t("Automatic") });
  const sync = () => {
    width.disabled = fit.value !== "fixed";
    widthLabel.classList.toggle("off", width.disabled);
  };
  fit.addEventListener("change", sync);
  sync();
  const { ok, cancel } = buttons(t("Insert"));
  const dialog = shell(
    t("Insert table"),
    "it-dialog",
    inCell ? t("The table goes in the cell the caret is in.") : null,
    [
      h("div", { class: "pd-grid" }, h("div", { class: "field" }, h("label", { for: "it-cols" }, t("Columns")), cols), h("div", { class: "field" }, h("label", { for: "it-rows" }, t("Rows")), rows)),
      h("div", { class: "field" }, h("label", { for: "it-fit" }, t("Column sizes")), fit),
      h("div", { class: "field" }, widthLabel, width, h("span", { class: "help" }, t("Leave empty to share the width of the page."))),
    ],
    ok,
    cancel,
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const px = fit.value === "fixed" ? parseLength(width.value) : null;
    const op = { op: "insert_table", after: r[1].p, rows: whole(rows, 1, 200, 3), cols: whole(cols, 1, 50, 3), autofit: fit.value, ...(px && px > 0 ? { column_width: px } : {}) };
    close();
    if (ed.exec(op)) ed.focus();
  });
  enterToConfirm(dialog, ok);
}

// ---------------------------------------------------------------------------------------------------------------------
// Split cells
// ---------------------------------------------------------------------------------------------------------------------

/** Split cells…: the cell (or the cells, joined first) into a number of columns and rows. */
export function openSplitCells(ed: Editor, sel: CellSelection | null = cellSelection(ed)) {
  if (!sel) return;
  const d = sel.details;
  const merged = sel.multi;
  // A cell that was merged splits back into the parts it covers.
  const cols = h("input", { id: "sc-cols", type: "number", min: "1", max: "63", step: "1", value: String(merged ? Math.max(2, sel.cols[1] - sel.cols[0] + 1) : Math.max(2, d.span)) });
  const rows = h("input", { id: "sc-rows", type: "number", min: "1", max: "32", step: "1", value: String(merged ? sel.rows[1] - sel.rows[0] + 1 : 1) });
  const join = h("input", { id: "sc-merge", type: "checkbox", checked: true });
  const error = h("p", { class: "dl-error", role: "alert", hidden: true });
  const { ok, cancel } = buttons(t("Split"));
  const dialog = shell(
    t("Split cells"),
    "sc-dialog",
    null,
    [
      h("div", { class: "pd-grid" }, h("div", { class: "field" }, h("label", { for: "sc-cols" }, t("Number of columns")), cols), h("div", { class: "field" }, h("label", { for: "sc-rows" }, t("Number of rows")), rows)),
      merged ? h("label", { class: "check", for: "sc-merge" }, join, h("span", {}, t("Merge the cells before splitting"))) : h("span"),
    ],
    ok,
    cancel,
    error,
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const n = whole(cols, 1, 63, 2);
    const m = whole(rows, 1, 32, 1);
    if (n === 1 && m === 1) {
      error.textContent = t("Choose more than one column or row.");
      error.hidden = false;
      return;
    }
    const onError = (message: string) => {
      error.textContent = message;
      error.hidden = false;
    };
    let at = sel.from;
    if (merged && join.checked) {
      // The cells become one; the split then works on that cell.
      const res = ed.exec({ op: "table_cells", from: sel.from, to: sel.to, action: { type: "merge" } }, { onError });
      if (!res) return;
      at = res[0].caret?.p ?? sel.from;
    }
    const res = ed.exec({ op: "table_cells", from: at, to: at, action: { type: "split", cols: n, rows: m } }, { onError, merge: merged && join.checked });
    if (!res) {
      // The split was refused: the cells go back to how they were, not left joined.
      if (merged && join.checked) ed.undo();
      return;
    }
    close();
    ed.focus();
  });
  enterToConfirm(dialog, ok);
}

// ---------------------------------------------------------------------------------------------------------------------
// Insert cells and Delete cells
// ---------------------------------------------------------------------------------------------------------------------

/** Insert cells… or Delete cells…: the others shift right or down (left or up), or a whole row or column goes in or out. */
export function openCells(ed: Editor, mode: "insert" | "delete", sel: CellSelection | null = cellSelection(ed)) {
  if (!sel) return;
  const insert = mode === "insert";
  const items: [string, string][] = insert
    ? [["right", msg("Shift cells right")], ["down", msg("Shift cells down")], ["row", msg("Insert an entire row")], ["col", msg("Insert an entire column")]]
    : [["left", msg("Shift cells left")], ["up", msg("Shift cells up")], ["row", msg("Delete the entire row")], ["col", msg("Delete the entire column")]];
  const choice = radios(`cells-${mode}`, items, insert ? "right" : "left");
  const error = h("p", { class: "dl-error", role: "alert", hidden: true });
  const { ok, cancel } = buttons(insert ? t("Insert") : t("Delete"));
  const dialog = shell(insert ? t("Insert cells") : t("Delete cells"), `cells-${mode}-dialog`, null, [choice.el], ok, cancel, error);
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const v = choice.get();
    const onError = (message: string) => {
      error.textContent = message;
      error.hidden = false;
    };
    const op =
      v === "row"
        ? { op: "table_edit", at: sel.from, action: insert ? "row_above" : "delete_row" }
        : v === "col"
          ? { op: "table_edit", at: sel.from, action: insert ? "col_left" : "delete_col" }
          : { op: "table_cell", at: sel.from, action: mode, shift: v };
    if (!ed.exec(op, { onError })) return;
    close();
    ed.focus();
  });
  enterToConfirm(dialog, ok);
}

// ---------------------------------------------------------------------------------------------------------------------
// Convert text to table and table to text
// ---------------------------------------------------------------------------------------------------------------------

const SEPARATORS: [string, string][] = [
  ["paragraphs", msg("Paragraphs")],
  ["commas", msg("Commas")],
  ["tabs", msg("Tabs")],
  ["other", msg("Another character")],
];

/** Guess what separates the pieces of the selected paragraphs: tabs if any have one, else commas. */
function guessSeparator(texts: string[]): string {
  if (texts.some((x) => x.includes("\t"))) return "tabs";
  if (texts.every((x) => x.includes(",")) && texts.length > 0) return "commas";
  return "paragraphs";
}

/** Convert text to table…: the selected paragraphs become a table, cut at paragraphs, commas, tabs or a character. */
export function openConvertToTable(ed: Editor) {
  const ids = ed.selectedParagraphs();
  if (!ids.length) return;
  const texts = ids.map((id) => ed.engine.textRange({ p: id, o: 0 }, { p: id, o: 1_000_000 }));
  const sep = radios("ct-sep", SEPARATORS, guessSeparator(texts));
  const other = h("input", { id: "ct-other", type: "text", maxlength: "1", value: ";", "aria-label": t("The separating character"), style: "width:48px;min-width:0" });
  const cols = h("input", { id: "ct-cols", type: "number", min: "1", max: "50", step: "1", value: "2" });
  const fit = h("select", { id: "ct-fit" }, ...FITS.map(([v, label]) => h("option", { value: v, selected: v === "fixed" }, t(label))));
  const note = h("p", { class: "help", id: "ct-note" });
  const split = (text: string, v: string): number => {
    const ch = v === "commas" ? "," : v === "tabs" ? "\t" : other.value || ";";
    return text.split(ch).length;
  };
  const paint = () => {
    const v = sep.get();
    const count = v === "paragraphs" ? Math.max(1, whole(cols, 1, 50, 2)) : Math.max(...texts.map((x) => split(x, v)), 1);
    const rows = v === "paragraphs" ? Math.ceil(ids.length / count) : ids.length;
    cols.disabled = v !== "paragraphs";
    if (v !== "paragraphs") cols.value = String(count);
    other.hidden = v !== "other";
    note.textContent = t("{rows} by {cols}", { rows: tn(rows, "{n} row", "{n} rows"), cols: tn(count, "{n} column", "{n} columns") });
  };
  const error = h("p", { class: "dl-error", role: "alert", hidden: true });
  const { ok, cancel } = buttons(t("Convert"));
  const dialog = shell(
    t("Convert text to table"),
    "ct-dialog",
    t("Each paragraph becomes a row, or a cell, and the table keeps its formatting."),
    [
      h("div", { class: "field" }, h("label", {}, t("Separate text at")), sep.el, h("div", { class: "td-inline" }, other)),
      h("div", { class: "pd-grid" }, h("div", { class: "field" }, h("label", { for: "ct-cols" }, t("Number of columns")), cols), h("div", { class: "field" }, h("label", { for: "ct-fit" }, t("Column sizes")), fit)),
      note,
    ],
    ok,
    cancel,
    error,
  );
  dialog.addEventListener("change", paint);
  dialog.addEventListener("input", paint);
  paint();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const v = sep.get();
    const op = { op: "convert_to_table", ids, separator: v, separator_char: v === "other" ? other.value || ";" : undefined, columns: v === "paragraphs" ? whole(cols, 1, 50, 2) : undefined, autofit: fit.value };
    const res = ed.exec(op, {
      onError: (message) => {
        error.textContent = message;
        error.hidden = false;
      },
    });
    if (!res) return;
    close();
    ed.focus();
  });
  enterToConfirm(dialog, ok);
}

/** Convert table to text…: the table (the one the caret is in) becomes paragraphs, its cells cut with a separator. */
export function openConvertToText(ed: Editor, sel: CellSelection | null = cellSelection(ed)) {
  if (!sel) return;
  const sep = radios("cx-sep", [["paragraphs", msg("Paragraph marks")], ["tabs", msg("Tabs")], ["commas", msg("Commas")], ["other", msg("Another character")]], "tabs");
  const other = h("input", { id: "cx-other", type: "text", maxlength: "1", value: ";", "aria-label": t("The separating character"), style: "width:48px;min-width:0" });
  const paint = () => (other.hidden = sep.get() !== "other");
  const { ok, cancel } = buttons(t("Convert"));
  const dialog = shell(t("Convert table to text"), "cx-dialog", t("The cells of a row go on one line, divided by the separator you choose."), [h("div", { class: "field" }, h("label", {}, t("Separate text with")), sep.el, h("div", { class: "td-inline" }, other))], ok, cancel);
  dialog.addEventListener("change", paint);
  paint();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const v = sep.get();
    close();
    if (ed.exec({ op: "convert_to_text", at: sel.from, separator: v, separator_char: v === "other" ? other.value || ";" : undefined })) ed.focus();
  });
  enterToConfirm(dialog, ok);
}

// ---------------------------------------------------------------------------------------------------------------------
// Formula
// ---------------------------------------------------------------------------------------------------------------------

const FORMATS: [string, string][] = [
  ["", msg("(none)")],
  ["0", "0"],
  ["0.00", "0.00"],
  ["#,##0", "#,##0"],
  ["#,##0.00", "#,##0.00"],
  ['$#,##0.00;($#,##0.00)', '$#,##0.00;($#,##0.00)'],
  ["0%", "0%"],
  ["0.00%", "0.00%"],
];
const FUNCTIONS = ["ABS", "AND", "AVERAGE", "COUNT", "DEFINED", "FALSE", "IF", "INT", "MAX", "MIN", "MOD", "NOT", "OR", "PRODUCT", "ROUND", "SIGN", "SUM", "TRUE"];

/** Formula…: a calculation in the cell (Word's `=` field), with the sum Word suggests, a number format and the functions and bookmarks to paste. */
export function openFormula(ed: Editor) {
  const r = ed.range();
  if (!r) return;
  const suggestion = ed.engine.suggestFormula(r[0].p);
  const formula = h("input", { id: "fm-formula", type: "text", autocomplete: "off", spellcheck: "false", value: suggestion });
  const format = h("select", { id: "fm-format" }, ...FORMATS.map(([v, label]) => h("option", { value: v }, t(label))));
  const fn = h("select", { id: "fm-function", "aria-label": t("Paste function") }, h("option", { value: "" }, t("Paste function")), ...FUNCTIONS.map((f) => h("option", { value: f }, f)));
  const marks = ed.engine.bookmarks().map((b) => b.name);
  const bm = h("select", { id: "fm-bookmark", "aria-label": t("Paste bookmark") }, h("option", { value: "" }, t("Paste bookmark")), ...marks.map((m) => h("option", { value: m }, m)));
  const preview = h("p", { class: "help", id: "fm-preview", "aria-live": "polite" });
  const body = () => formula.value.replace(/^\s*=?\s*/, "").trim();
  const show = () => {
    const res = ed.engine.evaluateFormula(body(), r[0].p);
    preview.textContent = "value" in res ? t("Result: {value}", { value: String(Math.round(res.value * 1e10) / 1e10) }) : t("Not a formula yet: {message}", { message: res.error });
  };
  const paste = (sel: HTMLSelectElement, wrap: (v: string) => string) =>
    sel.addEventListener("change", () => {
      if (!sel.value) return;
      const at = formula.selectionStart ?? formula.value.length;
      const end = formula.selectionEnd ?? at;
      formula.value = formula.value.slice(0, at) + wrap(sel.value) + formula.value.slice(end);
      sel.value = "";
      formula.focus();
      show();
    });
  paste(fn, (v) => `${v}()`);
  paste(bm, (v) => v);
  formula.addEventListener("input", show);
  show();
  const { ok, cancel } = buttons(t("Insert"));
  const dialog = shell(
    t("Formula"),
    "fm-dialog",
    t("Use numbers, + - * / ^ %, functions, and ABOVE, LEFT, RIGHT, BELOW or cells such as A1 in a table."),
    [
      h("div", { class: "field" }, h("label", { for: "fm-formula" }, t("Formula")), formula, preview),
      h("div", { class: "field" }, h("label", { for: "fm-format" }, t("Number format")), format),
      h("div", { class: "pd-grid" }, h("div", { class: "field" }, fn), h("div", { class: "field" }, bm)),
    ],
    ok,
    cancel,
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const expr = body();
    if (!expr) return;
    const picture = format.value ? ` \\# "${format.value}"` : "";
    close();
    if (ed.insertField(`= ${expr}${picture}`)) ed.focus();
    else toast(t("Could not insert the formula"));
  });
  enterToConfirm(dialog, ok);
}
