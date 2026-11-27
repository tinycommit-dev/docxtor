// Edit chart: the type, data, layout, axes, series and style of the selected chart, with a live preview. Five tabs:
// Chart (type, title, data), Layout (title, legend, labels, gridlines), Axes, Series (colour, type and axis, labels,
// trendlines, error bars) and Style (the Style and Change colors galleries). The data is a grid like a spreadsheet's
// (numbers are checked as they are typed and cells can be pasted); Apply sends one `set_chart` op, so one undo takes
// it all back.
import { t, type ChartPaletteContribution } from "@docxtor/extensions";
import type { ChartModel, SetChartOp } from "@docxtor/core";
import { drawChart } from "../chart/draw";
import { editChartEx } from "../chart/chartex-atom";
import { diffOptions, draftOf, isStackable, isXY, parseNumber, previewModel, rowsOf, type Draft } from "../chart/draft";
import type { Editor } from "../editor/editor";
import { createGrid } from "./chart-grid";
import { axesPane, chartPane, layoutPane, type Ctx } from "./chart-panes";
import { seriesPane, stylePane } from "./chart-series-pane";
import { h, modal } from "./dom";

export { parseNumber };

type TabId = "chart" | "layout" | "axes" | "series" | "style";

export function openChartDialog(ed: Editor, at: { p: number; o: number }, extraPalettes: ChartPaletteContribution[] = []) {
  if (editChartEx(ed, at)) return;
  const model = ed.engine.chartAt(at);
  if (!model) return;
  const base: ChartModel = model;
  const d: Draft = draftOf(base);
  const kindTouched = { value: false };
  let tab: TabId = "chart";
  let shown: TabId | null = null;

  const previewBox = h("div", { class: "chart-preview", "aria-label": t("Preview") });
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const panel = h("div", { class: "chart-panel pd-pane", role: "tabpanel", id: "chart-panel", tabindex: "-1" });
  const message = h("p", { class: "chart-note", role: "alert", "aria-live": "polite" });

  const ctx: Ctx = {
    d,
    base,
    engine: ed.engine,
    extraPalettes,
    sel: { series: 0, axis: "y", point: 0, title: d.title },
    changed: () => preview(),
    reshaped: () => undefined,
    rerender: () => show(),
    fail: (m) => (message.textContent = m),
    grid: undefined as never,
  };
  ctx.grid = createGrid(d, () => preview(), () => undefined);

  function preview() {
    const n = ctx.grid.bad();
    ctx.grid.problem.textContent = n ? t("Some values aren't numbers. Use digits, a dot or comma for decimals, or leave a cell empty.") : "";
    apply.disabled = n > 0 || d.series.length === 0 || rowsOf(d) === 0;
    message.textContent = "";
    previewBox.replaceChildren(drawChart(previewModel(d), 300, 220));
  }

  const TABS: [TabId, () => string][] = [
    ["chart", () => t("Chart")],
    ["layout", () => t("Layout")],
    ["axes", () => t("Axes")],
    ["series", () => t("Series")],
    ["style", () => t("Style")],
  ];
  const tablist = h("div", { class: "chart-tablist seg", role: "tablist", "aria-label": t("Chart settings") });
  const tabButtons = TABS.map(([id, label]) => {
    const b = h("button", { type: "button", role: "tab", id: `chart-tab-${id}`, "aria-controls": "chart-panel", "aria-selected": "false", tabindex: "-1", "data-tab": id }, label());
    b.addEventListener("click", () => {
      tab = id;
      show();
    });
    tablist.append(b);
    return [id, b] as const;
  });
  tablist.addEventListener("keydown", (e) => {
    const i = TABS.findIndex(([id]) => id === tab);
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : e.key === "Home" ? -99 : e.key === "End" ? 99 : 0;
    if (!step) return;
    e.preventDefault();
    const next = Math.max(0, Math.min(TABS.length - 1, Math.abs(step) > 1 ? (step < 0 ? 0 : TABS.length - 1) : (i + step + TABS.length) % TABS.length));
    tab = TABS[next]![0];
    show();
    tabButtons[next]![1].focus();
  });

  function show() {
    for (const [id, b] of tabButtons) {
      const on = id === tab;
      b.classList.toggle("on", on);
      b.setAttribute("aria-selected", String(on));
      b.tabIndex = on ? 0 : -1;
    }
    panel.setAttribute("aria-labelledby", `chart-tab-${tab}`);
    // Drawing the same pane again (a choice that changes what it shows) keeps its place; another pane starts at the top.
    const scroll = shown === tab ? panel.scrollTop : 0;
    shown = tab;
    const pane = tab === "chart" ? chartPane(ctx, kindTouched) : tab === "layout" ? layoutPane(ctx) : tab === "axes" ? axesPane(ctx) : tab === "series" ? seriesPane(ctx) : stylePane(ctx);
    panel.replaceChildren(pane);
    if (tab === "chart") ctx.grid.render();
    panel.scrollTop = scroll;
    preview();
  }

  const dialog = h(
    "div",
    { class: "dialog chart-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Edit chart") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Edit chart"))),
    h("div", { class: "dialog-body chart-body" }, h("div", { class: "chart-form" }, tablist, panel), h("div", { class: "chart-side" }, h("div", { class: "field" }, h("label", {}, t("Preview")), previewBox), message)),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);

  apply.addEventListener("click", () => {
    if (apply.disabled) return;
    const xy = isXY(d.kind);
    let n = rowsOf(d);
    // Empty rows at the end are the spare room of the grid, not data.
    const blank = (r: number) => (xy ? d.series.every((s) => !s.x[r]?.trim() && !s.y[r]?.trim() && !s.size[r]?.trim()) : !d.categories[r]?.trim() && d.series.every((s) => !s.y[r]?.trim()));
    while (n > 1 && blank(n - 1)) n--;
    const nums = (list: string[]) => Array.from({ length: n }, (_, i) => parseNumber(list[i] ?? ""));
    const op: SetChartOp = {
      op: "set_chart",
      at,
      title: d.title,
      categories: xy ? undefined : Array.from({ length: n }, (_, i) => d.categories[i] ?? ""),
      series: d.series.map((s, i) => ({ name: s.name.trim() || t("Series {n}", { n: i + 1 }), values: nums(s.y), x_values: xy ? nums(s.x) : undefined, sizes: d.kind === "bubble" ? nums(s.size) : undefined })),
    };
    if (kindTouched.value && (d.kind !== base.kind || base.combo)) op.kind = d.kind as SetChartOp["kind"];
    if (isStackable(d.kind) && d.grouping !== base.grouping) op.grouping = d.grouping;
    const options = diffOptions(base, d);
    if (options) op.options = options;
    const res = ed.exec({ ...op }, { select: { anchor: at, focus: { p: at.p, o: at.o + 1 } }, onError: (m) => (message.textContent = m) });
    if (res) close();
  });

  show();
  (panel.querySelector<HTMLElement>(".chart-type.on") ?? tabButtons[0]![1]).focus();
}
