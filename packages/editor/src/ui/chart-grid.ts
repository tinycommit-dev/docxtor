// The data of the Edit chart dialog: a grid like a spreadsheet's (categories down the side and one column per
// series, or X and Y and sizes for scatter and bubble charts). Numbers are checked as they are typed and cells can
// be pasted from a spreadsheet as tab-separated text.
import { t } from "@docxtor/extensions";
import { addSeries, isXY, parseNumber, removeSeries, rowsOf, type Draft } from "../chart/draft";
import { h, svg } from "./dom";
import { I } from "./icons";

type Col = { f: "cat" } | { f: "x" | "y" | "size"; s: number };

function columns(d: Draft): Col[] {
  if (!isXY(d.kind)) return [{ f: "cat" }, ...d.series.map((_, s) => ({ f: "y" as const, s }))];
  return d.series.flatMap((_, s): Col[] => (d.kind === "bubble" ? [{ f: "x", s }, { f: "y", s }, { f: "size", s }] : [{ f: "x", s }, { f: "y", s }]));
}

export interface Grid {
  /** The grid and its buttons. */
  el: HTMLElement;
  render(): void;
  /** How many cells hold something that isn't a number. */
  bad(): number;
  problem: HTMLElement;
}

export function createGrid(d: Draft, changed: () => void, reshaped: () => void): Grid {
  const wrap = h("div", { class: "chart-grid-wrap", tabindex: "-1" });
  const problem = h("p", { class: "chart-problem", role: "alert", "aria-live": "polite" });
  const addRow = h("button", { type: "button", class: "btn" }, t("Add row"));
  const addSer = h("button", { type: "button", class: "btn" }, t("Add series"));

  const read = (r: number, c: Col): string => (c.f === "cat" ? (d.categories[r] ?? "") : (d.series[c.s]![c.f][r] ?? ""));
  const write = (r: number, c: Col, v: string) => {
    if (c.f === "cat") d.categories[r] = v;
    else {
      const s = d.series[c.s];
      if (s) s[c.f][r] = v;
    }
  };
  const isNumeric = (c: Col) => c.f !== "cat";

  /** What a cell is called for a screen reader: its series and its category (or point). */
  const labelOf = (r: number, c: Col): string => {
    if (c.f === "cat") return t("Category {n}", { n: r + 1 });
    const name = d.series[c.s]?.name || t("Series {n}", { n: c.s + 1 });
    if (isXY(d.kind)) {
      const vars = { series: name, n: r + 1 };
      return c.f === "x" ? t("{series}, point {n}, X", vars) : c.f === "y" ? t("{series}, point {n}, Y", vars) : t("{series}, point {n}, size", vars);
    }
    return `${name}, ${d.categories[r]?.trim() || t("Category {n}", { n: r + 1 })}`;
  };
  const relabel = () => {
    const cols = columns(d);
    wrap.querySelectorAll<HTMLInputElement>("input.chart-cell[data-r]").forEach((input) => input.setAttribute("aria-label", labelOf(Number(input.dataset.r), cols[Number(input.dataset.c)]!)));
  };

  const focusCell = (r: number, c: number) => {
    const n = rowsOf(d);
    const input = wrap.querySelector<HTMLInputElement>(`input.chart-cell[data-r="${Math.min(r, n - 1)}"][data-c="${c}"]`);
    input?.focus();
    input?.select();
  };

  const ensureRows = (n: number) => {
    const grow = (list: string[], fill = "") => {
      while (list.length < n) list.push(fill);
    };
    if (!isXY(d.kind)) grow(d.categories);
    for (const s of d.series) {
      grow(s.y);
      if (isXY(d.kind)) grow(s.x);
      if (d.kind === "bubble") grow(s.size, "1");
    }
  };

  const cell = (value: string, r: number, ci: number, col: Col): HTMLInputElement => {
    const numeric = isNumeric(col);
    const input = h("input", { type: "text", class: "chart-cell", value, "aria-label": labelOf(r, col), "data-r": r, "data-c": ci, autocomplete: "off", spellcheck: "false", inputmode: numeric ? "decimal" : undefined });
    input.addEventListener("input", () => {
      write(r, col, input.value);
      if (numeric) input.toggleAttribute("aria-invalid", Number.isNaN(parseNumber(input.value)));
      if (!numeric && !isXY(d.kind)) relabel();
      changed();
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        focusCell(r + 1, ci);
      }
    });
    input.addEventListener("paste", (e) => {
      const text = e.clipboardData?.getData("text/plain") ?? "";
      if (!/[\t\n]/.test(text.replace(/\n$/, ""))) return;
      e.preventDefault();
      pasteGrid(r, ci, text);
    });
    if (numeric && Number.isNaN(parseNumber(value))) input.setAttribute("aria-invalid", "");
    return input;
  };

  const pasteGrid = (r0: number, c0: number, text: string) => {
    const lines = text.replace(/\r/g, "").replace(/\n$/, "").split("\n").map((l) => l.split("\t"));
    const need = r0 + lines.length;
    // More columns than the chart has: new series.
    const widest = Math.max(...lines.map((l) => l.length));
    const lastCol = c0 + widest;
    const width = isXY(d.kind) ? (d.kind === "bubble" ? 3 : 2) : 1;
    const have = () => (isXY(d.kind) ? d.series.length * width : d.series.length + 1);
    let guard = 0;
    while (have() < lastCol && d.kind !== "stock" && guard++ < 64) addSeries(d, t("Series {n}", { n: d.series.length + 1 }));
    ensureRows(need);
    const cols = columns(d);
    lines.forEach((cells, i) => cells.forEach((v, j) => cols[c0 + j] && write(r0 + i, cols[c0 + j]!, v.trim())));
    render();
    reshaped();
    changed();
    focusCell(r0, c0);
  };

  function render() {
    const xy = isXY(d.kind);
    const rows = rowsOf(d);
    const cols = columns(d);
    const per = d.kind === "bubble" ? 3 : 2;
    const table = h("table", { class: "chart-table" });
    const head = h("tr", {}, h("th", { scope: "col", class: "corner" }, xy ? "" : t("Category")));
    const fixed = d.kind === "stock";
    d.series.forEach((s, i) => {
      const name = h("input", { type: "text", class: "chart-cell chart-name", value: s.name, "aria-label": t("Series {n} name", { n: i + 1 }), autocomplete: "off", spellcheck: "false" });
      name.addEventListener("input", () => {
        s.name = name.value;
        relabel();
        changed();
      });
      const del = h("button", { type: "button", class: "chart-del", title: t("Remove series {n}", { n: i + 1 }), "aria-label": t("Remove series {n}", { n: i + 1 }), disabled: d.series.length <= 1 || fixed }, svg(I.close));
      del.addEventListener("click", () => {
        removeSeries(d, i);
        render();
        reshaped();
        changed();
      });
      head.append(h("th", { scope: "col", colspan: xy ? per : 1, class: "series" }, h("div", { class: "chart-name-row" }, name, del)));
    });
    head.append(h("th", { class: "end" }));
    table.append(h("thead", {}, head));
    if (xy) table.querySelector("thead")!.append(h("tr", { class: "sub" }, h("th", {}), ...d.series.flatMap(() => (d.kind === "bubble" ? [h("th", { scope: "col" }, "X"), h("th", { scope: "col" }, "Y"), h("th", { scope: "col" }, t("Size"))] : [h("th", { scope: "col" }, "X"), h("th", { scope: "col" }, "Y")])), h("th", {})));
    const body = h("tbody", {});
    for (let r = 0; r < rows; r++) {
      const tr = h("tr", {});
      if (xy) tr.append(h("td", { class: "rownum" }, String(r + 1)));
      cols.forEach((c, ci) => tr.append(h("td", {}, cell(read(r, c), r, ci, c))));
      const del = h("button", { type: "button", class: "chart-del", title: t("Remove row {n}", { n: r + 1 }), "aria-label": t("Remove row {n}", { n: r + 1 }), disabled: rows <= 1 }, svg(I.close));
      del.addEventListener("click", () => {
        d.categories.splice(r, 1);
        for (const s of d.series) {
          s.x.splice(r, 1);
          s.y.splice(r, 1);
          s.size.splice(r, 1);
        }
        render();
        changed();
      });
      tr.append(h("td", { class: "end" }, del));
      body.append(tr);
    }
    table.append(body);
    wrap.replaceChildren(table);
    addSer.disabled = fixed;
  }

  addRow.addEventListener("click", () => {
    const n = rowsOf(d);
    ensureRows(n + 1);
    if (isXY(d.kind)) for (const s of d.series) s.x[n] = String(n + 1);
    render();
    changed();
    focusCell(n, 0);
  });
  addSer.addEventListener("click", () => {
    addSeries(d, t("Series {n}", { n: d.series.length + 1 }));
    render();
    reshaped();
    changed();
    wrap.querySelectorAll<HTMLInputElement>("input.chart-name").item(d.series.length - 1)?.focus();
  });

  const bad = () => d.series.flatMap((s) => [...s.y, ...(isXY(d.kind) ? s.x : []), ...(d.kind === "bubble" ? s.size : [])]).filter((v) => Number.isNaN(parseNumber(v))).length;
  const el = h("div", { class: "chart-datapane" }, wrap, h("div", { class: "chart-grid-actions" }, addRow, addSer), problem, h("p", { class: "help" }, t("Paste cells from a spreadsheet to fill the grid.")));
  return { el, render, bad, problem };
}
