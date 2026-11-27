// The Series and Style panes of the Edit chart dialog: per series its type and axis, colour, fill and line, data
// labels, trendlines and error bars; and the Style and Change colors galleries.
import type { ChartKind, ChartModel, ChartSeries, ErrorBars, Trendline, TrendlineType } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { palettes } from "../chart/palettes";
import { resolveColor } from "../chart/colors";
import { COMBINABLE, isCombo, isStackable, isXY, previewModel, type Draft } from "../chart/draft";
import { colorPicker } from "./chart-color";
import { check, field, group, numberInput, row, select } from "./chart-controls";
import { defaultLabels, labelEditor, thumb, type Ctx } from "./chart-panes";
import { h } from "./dom";
import { hexOr } from "../editor/color.ts";

const KIND_NAME: Record<string, () => string> = { column: () => t("Column"), bar: () => t("Bar"), line: () => t("Line"), area: () => t("Area"), scatter: () => t("Scatter") };

const TREND_TYPES: [TrendlineType, () => string][] = [
  ["linear", () => t("Linear")],
  ["exp", () => t("Exponential")],
  ["log", () => t("Logarithmic")],
  ["poly", () => t("Polynomial")],
  ["power", () => t("Power")],
  ["movingAvg", () => t("Moving average")],
];

const VALUE_TYPES: [ErrorBars["valueType"], () => string][] = [
  ["fixed", () => t("Fixed value")],
  ["percentage", () => t("Percentage")],
  ["stdDev", () => t("Standard deviation")],
  ["stdErr", () => t("Standard error")],
  ["custom", () => t("Custom")],
];

/** Whether trendlines make sense for the series (Word has none for stacked, pie, radar, stock or surface charts). */
function trendOk(d: Draft, s: ChartSeries): boolean {
  if (["column", "bar", "line", "area"].includes(s.kind)) return d.grouping === "clustered";
  return s.kind === "scatter" || s.kind === "bubble";
}

const barLike = (k: ChartKind) => k === "column" || k === "bar";

export function seriesPane(ctx: Ctx): HTMLElement {
  const { d } = ctx;
  const f = d.f;
  if (!f.series.length) return h("div", { class: "chart-pane-body" });
  const i = Math.min(ctx.sel.series, f.series.length - 1);
  ctx.sel.series = i;
  const s = f.series[i]!;
  const picker = select(f.series.map((_, k) => [String(k), d.series[k]?.name || t("Series {n}", { n: k + 1 })] as [string, string]), String(i), (v) => {
    ctx.sel.series = Number(v);
    ctx.sel.point = 0;
    ctx.rerender();
  }, t("Series"));
  const body = h("div", { class: "chart-pane-body" }, field(t("Series"), picker));

  // Type and axis, for combination charts.
  if (COMBINABLE.includes(d.kind as never) || isCombo(d)) {
    const kinds = COMBINABLE.filter((k) => k !== "bar" || d.kind === "bar").filter((k) => k !== "bar" || true);
    const typeSel = select(kinds.map((k) => [k, KIND_NAME[k]!()] as [string, string]), s.kind as string, (v) => {
      s.kind = v as ChartKind;
      s.noLine = undefined;
      s.fill = undefined;
      s.marker = s.kind === "line" || s.kind === "scatter" ? true : undefined;
      if (isXY(s.kind) && !d.series[i]!.x.length) d.series[i]!.x = d.series[i]!.y.map((_, r) => String(r + 1));
      ctx.changed();
      ctx.rerender();
    });
    const second = check(t("Secondary axis"), !!s.secondary, (on) => {
      s.secondary = on || undefined;
      if (on && !f.y2Axis) f.y2Axis = {};
      ctx.changed();
      ctx.rerender();
    });
    body.append(group(t("Type and axis"), row(field(t("Chart type"), typeSel), second)));
  }

  // Colour: of the series, or of one point (slices, varied columns).
  const perPoint = d.kind === "pie" || d.kind === "doughnut" || (f.varyColors && f.series.length === 1);
  const colorBox = group(perPoint ? t("Point colors") : t("Color"));
  if (perPoint) {
    const n = Math.max(d.categories.length, d.series[i]?.y.length ?? 0);
    ctx.sel.point = Math.min(ctx.sel.point, Math.max(0, n - 1));
    const pointSel = select(Array.from({ length: n }, (_, k) => [String(k), d.categories[k] || t("Point {n}", { n: k + 1 })] as [string, string]), String(ctx.sel.point), (v) => {
      ctx.sel.point = Number(v);
      ctx.rerender();
    }, t("Point"));
    const cp = colorPicker(f.theme, t("Point color"), (spec) => {
      const list = (s.pointColors ??= []);
      while (list.length <= ctx.sel.point) list.push(null);
      list[ctx.sel.point] = spec ?? null;
      if (list.every((c) => !c)) s.pointColors = undefined;
      ctx.changed();
    });
    cp.show(s.pointColors?.[ctx.sel.point] ?? undefined);
    colorBox.append(field(t("Point"), pointSel), cp.el);
  } else {
    const cp = colorPicker(f.theme, t("Series color"), (spec) => {
      s.color = spec;
      ctx.changed();
    });
    cp.show(s.color);
    colorBox.append(cp.el);
    if (isStackable(d.kind) === false && d.kind !== "bubble" && !isCombo(d)) void 0;
    if (f.series.length === 1 && barLike(d.kind)) {
      colorBox.append(check(t("Vary colors by point"), !!f.varyColors, (on) => {
        f.varyColors = on || undefined;
        if (!on) s.pointColors = undefined;
        ctx.changed();
        ctx.rerender();
      }));
    }
  }
  body.append(colorBox);

  // Fill and line.
  const kind = s.kind;
  const fills = barLike(kind) || kind === "area" || kind === "bubble" || (kind === "radar" && f.filled);
  const lines = kind === "line" || kind === "scatter" || kind === "radar";
  if (fills || lines) {
    const cells: (Node | null)[] = [];
    if (fills) {
      cells.push(
        field(t("Fill"), select([["", t("Solid")], ["none", t("Outline only")], ["soft", t("See-through")], ["gradient", t("Gradient")]] as [string, string][], s.fill ?? "", (v) => {
          s.fill = (v || undefined) as ChartSeries["fill"];
          ctx.changed();
        })),
      );
    }
    cells.push(field(fills ? t("Outline width, points") : t("Line width, points"), numberInput(s.lineWidth, (v) => {
      s.lineWidth = v;
      ctx.changed();
    }, { placeholder: t("Automatic"), min: 0, max: 100, label: t("Line width") })));
    body.append(group(fills ? t("Fill and outline") : t("Line"), row(...cells.slice(0, 2)), ...(lines ? [h("div", { class: "chart-checks" },
      kind === "radar" ? null : check(t("Smooth line"), !!s.smooth, (on) => ((s.smooth = on || undefined), ctx.changed())),
      check(t("Show markers"), !!s.marker, (on) => ((s.marker = on || undefined), ctx.changed())),
      kind === "radar" ? null : check(t("No line"), !!s.noLine, (on) => ((s.noLine = on || undefined), ctx.changed())),
    )] : [])));
  }
  if (barLike(kind)) {
    body.append(
      group(
        t("Columns"),
        row(
          field(t("Gap width, percent"), numberInput(f.gapWidth, (v) => ((f.gapWidth = v === undefined ? undefined : Math.round(v)), ctx.changed()), { placeholder: t("Automatic"), min: 0, max: 500 })),
          field(t("Overlap, percent"), numberInput(f.overlap, (v) => ((f.overlap = v === undefined ? undefined : Math.round(v)), ctx.changed()), { placeholder: t("Automatic"), min: -100, max: 100 })),
        ),
      ),
    );
  }
  if (d.kind === "bubble") {
    body.append(
      group(
        t("Bubbles"),
        row(
          field(t("Scale, percent"), numberInput(f.bubbleScale, (v) => ((f.bubbleScale = v === undefined ? undefined : Math.round(v)), ctx.changed()), { placeholder: "100", min: 0, max: 300 })),
          field(t("Size represents"), select([["area", t("Area")], ["width", t("Width")]] as [string, string][], f.sizeRepresents ?? "area", (v) => ((f.sizeRepresents = v as "area" | "width"), ctx.changed()))),
        ),
        check(t("Show negative bubbles"), !!f.showNegativeBubbles, (on) => ((f.showNegativeBubbles = on || undefined), ctx.changed())),
      ),
    );
  }

  // Data labels of this series.
  if (d.kind !== "surface") {
    body.append(
      group(
        t("Data labels"),
        labelEditor(
          s.kind,
          d.grouping,
          () => s.labels,
          (fn) => {
            if (s.labels) fn(s.labels);
            ctx.changed();
          },
          (on) => {
            s.labels = on ? (s.labels ?? { ...defaultLabels(s.kind) }) : undefined;
            ctx.changed();
          },
          ctx.rerender,
        ),
      ),
    );
  }

  // Trendlines.
  if (trendOk(d, s)) body.append(trendPane(ctx, s));
  // Error bars.
  if (["column", "bar", "line", "area", "scatter", "bubble"].includes(s.kind)) body.append(errorPane(ctx, s));
  return body;
}

function trendPane(ctx: Ctx, s: ChartSeries): HTMLElement {
  const list = s.trendlines ?? [];
  const box = group(t("Trendlines"));
  list.forEach((tl, k) => {
    const row1 = row(
      field(t("Type"), select(TREND_TYPES.map(([v, l]) => [v, l()] as [string, string]), tl.type, (v) => {
        tl.type = v as TrendlineType;
        ctx.changed();
        ctx.rerender();
      })),
      tl.type === "poly" ? field(t("Order"), numberInput(tl.order ?? 2, (v) => ((tl.order = v === undefined ? 2 : Math.min(6, Math.max(2, Math.round(v)))), ctx.changed()), { min: 2, max: 6 })) : tl.type === "movingAvg" ? field(t("Period"), numberInput(tl.period ?? 2, (v) => ((tl.period = v === undefined ? 2 : Math.max(2, Math.round(v))), ctx.changed()), { min: 2 })) : null,
    );
    const row2 = row(
      field(t("Forecast forward"), numberInput(tl.forward, (v) => ((tl.forward = v), ctx.changed()), { placeholder: "0", min: 0 })),
      field(t("Forecast backward"), numberInput(tl.backward, (v) => ((tl.backward = v), ctx.changed()), { placeholder: "0", min: 0 })),
    );
    const del = h("button", { type: "button", class: "btn" }, t("Remove trendline"));
    del.addEventListener("click", () => {
      list.splice(k, 1);
      s.trendlines = list.length ? list : undefined;
      ctx.changed();
      ctx.rerender();
    });
    box.append(
      h("div", { class: "chart-item" }, row1, tl.type === "movingAvg" ? null : row2, h("div", { class: "chart-checks" },
        tl.type === "movingAvg" ? null : check(t("Show equation on chart"), !!tl.equation, (on) => ((tl.equation = on || undefined), ctx.changed())),
        tl.type === "movingAvg" ? null : check(t("Show R-squared value"), !!tl.rSquared, (on) => ((tl.rSquared = on || undefined), ctx.changed())),
      ), del),
    );
  });
  const add = h("button", { type: "button", class: "btn" }, t("Add trendline"));
  add.addEventListener("click", () => {
    s.trendlines = [...list, { type: "linear" } as Trendline];
    ctx.changed();
    ctx.rerender();
  });
  box.append(add);
  return box;
}

function errorPane(ctx: Ctx, s: ChartSeries): HTMLElement {
  const xy = isXY(s.kind);
  const list = s.errorBars ?? [];
  const box = group(t("Error bars"));
  const max = xy ? 2 : 1;
  list.forEach((eb, k) => {
    const dirSel = xy ? field(t("Direction"), select([["x", t("X")], ["y", t("Y")]] as [string, string][], eb.dir, (v) => ((eb.dir = v as "x" | "y"), ctx.changed()))) : null;
    const type = field(t("Direction of the bars"), select([["both", t("Both")], ["plus", t("Plus")], ["minus", t("Minus")]] as [string, string][], eb.barType, (v) => ((eb.barType = v as ErrorBars["barType"]), ctx.changed())));
    const kindSel = field(t("Error amount"), select(VALUE_TYPES.map(([v, l]) => [v, l()] as [string, string]), eb.valueType, (v) => {
      eb.valueType = v as ErrorBars["valueType"];
      if (v === "percentage" && eb.value === undefined) eb.value = 5;
      if (v === "custom" && !eb.plus && !eb.minus) {
        eb.plus = Array.from({ length: s.values.length || 1 }, () => 1);
        eb.minus = [...eb.plus];
      }
      ctx.changed();
      ctx.rerender();
    }));
    const amount =
      eb.valueType === "custom"
        ? row(
            field(t("Plus amounts, separated by commas"), listInput(eb.plus ?? [], (v) => ((eb.plus = v), ctx.changed()))),
            field(t("Minus amounts, separated by commas"), listInput(eb.minus ?? [], (v) => ((eb.minus = v), ctx.changed()))),
          )
        : eb.valueType === "stdErr"
          ? null
          : field(eb.valueType === "percentage" ? t("Percentage") : eb.valueType === "stdDev" ? t("Multiple of standard deviation") : t("Value"), numberInput(eb.value ?? 1, (v) => ((eb.value = v ?? 1), ctx.changed()), { min: 0 }));
    const del = h("button", { type: "button", class: "btn" }, t("Remove error bars"));
    del.addEventListener("click", () => {
      list.splice(k, 1);
      s.errorBars = list.length ? list : undefined;
      ctx.changed();
      ctx.rerender();
    });
    box.append(h("div", { class: "chart-item" }, row(dirSel, type), kindSel, amount, check(t("No end cap"), !!eb.noCap, (on) => ((eb.noCap = on || undefined), ctx.changed())), del));
  });
  if (list.length < max) {
    const add = h("button", { type: "button", class: "btn" }, xy && list.length ? t("Add error bars for the other direction") : t("Add error bars"));
    add.addEventListener("click", () => {
      const other = list[0]?.dir === "x" ? "y" : list[0]?.dir === "y" ? "x" : xy ? "x" : "y";
      s.errorBars = [...list, { dir: other, barType: "both", valueType: "fixed", value: 1 } as ErrorBars];
      ctx.changed();
      ctx.rerender();
    });
    box.append(add);
  }
  return box;
}

function listInput(values: number[], onChange: (v: number[]) => void): HTMLInputElement {
  const i = h("input", { type: "text", autocomplete: "off", spellcheck: "false", value: values.join(", ") });
  i.addEventListener("input", () => {
    const parts = i.value.split(/[,;\s]+/).filter(Boolean).map(Number);
    const ok = parts.every((n) => Number.isFinite(n) && n >= 0);
    i.toggleAttribute("aria-invalid", !ok);
    if (ok) onChange(parts);
  });
  return i;
}

// -- Style and Change colors ------------------------------------------------------------------------------------

/** The model the engine merges options into: the draft's data and formatting (colours as stored). */
function engineModel(d: Draft): ChartModel {
  const m = previewModel(d);
  return { ...m, series: m.series.map((s, i) => ({ ...s, color: d.f.series[i]?.color, lineColor: d.f.series[i]?.lineColor, pointColors: d.f.series[i]?.pointColors })) };
}

export function stylePane(ctx: Ctx): HTMLElement {
  const { d } = ctx;
  const body = h("div", { class: "chart-pane-body" });
  const current = d.f.style;
  const LOOK_STYLE = [2, 10, 18, 26, 34, 42, 6, 14];
  const looks = h("div", { class: "chart-gallery", role: "group", "aria-label": t("Style") });
  for (let n = 1; n <= 8; n++) {
    let model: ChartModel;
    try {
      model = ctx.engine.chartMerge(engineModel(d), { look: n });
    } catch {
      continue;
    }
    const b = h("button", { type: "button", class: "chart-tile", "aria-pressed": String(current === LOOK_STYLE[n - 1]), "aria-label": t("Style {n}", { n }) }, h("span", { class: "chart-thumb" }, thumb(resolved(model, d))), h("span", {}, t("Style {n}", { n })));
    b.addEventListener("click", () => {
      try {
        d.f = { ...ctx.engine.chartMerge(engineModel(d), { look: n }), theme: d.f.theme };
      } catch (e) {
        ctx.fail(String((e as Error).message ?? e));
        return;
      }
      ctx.changed();
      ctx.rerender();
    });
    looks.append(b);
  }
  const pal = h("div", { class: "chart-palettes", role: "group", "aria-label": t("Change colors") });
  const stub = { ...engineModel(d), series: Array.from({ length: 6 }, (_, i) => ({ ...(d.f.series[0] ?? { name: "", values: [], kind: d.kind }), name: String(i), color: undefined, pointColors: undefined })), categories: ["a"], kind: "column" as const, varyColors: false };
  const rows: [string, string, string[]][] = [];
  for (const p of palettes(ctx.extraPalettes)) {
    let chips: string[] = [];
    if (p.colors) {
      rows.push([p.id, p.label(), Array.from({ length: Math.min(6, Math.max(p.colors.length, 2)) }, (_, i) => resolveColor(p.colors![i % p.colors!.length], d.f.theme) ?? "888888")]);
      continue;
    }
    try {
      const m = ctx.engine.chartMerge(stub as ChartModel, { palette: p.id });
      chips = m.series.map((s) => resolveColor(s.color, d.f.theme) ?? "888888");
    } catch {
      continue;
    }
    rows.push([p.id, p.label(), chips]);
  }
  const addRows = (title: string, ids: string[]) => {
    const g = h("div", { class: "chart-palette-group" }, h("p", { class: "chart-sub" }, title));
    for (const [id, label, chips] of rows.filter((r) => ids.includes(r[0]))) {
      const b = h("button", { type: "button", class: "chart-palette", "aria-pressed": String(d.f.palette === id), "aria-label": label, title: label }, ...chips.map((c) => h("span", { class: "chart-chip", style: `background:${hexOr(c)}` })));
      b.addEventListener("click", () => {
        const own = ctx.extraPalettes.find((x) => `ext:${x.id}` === id);
        if (own) {
          applyColors(d, own.colors);
          ctx.changed();
          ctx.rerender();
          return;
        }
        try {
          d.f = { ...ctx.engine.chartMerge(engineModel(d), { palette: id }), theme: d.f.theme };
        } catch (e) {
          ctx.fail(String((e as Error).message ?? e));
          return;
        }
        ctx.changed();
        ctx.rerender();
      });
      g.append(b);
    }
    return g;
  };
  pal.append(addRows(t("Colorful"), rows.filter((r) => r[0].startsWith("colorful")).map((r) => r[0])), addRows(t("Monochromatic"), rows.filter((r) => r[0].startsWith("mono")).map((r) => r[0])));
  const extras = rows.filter((r) => r[0].startsWith("ext:")).map((r) => r[0]);
  if (extras.length) pal.append(addRows(t("From extensions"), extras));
  body.append(group(t("Style"), looks), group(t("Change colors"), pal));
  return body;
}

/** Give the series (or, for pies and varied colours, the points) the colours of a palette, in order. */
function applyColors(d: Draft, colors: string[]) {
  const varied = (d.kind === "pie" || d.kind === "doughnut" || d.f.varyColors) && d.f.series.length === 1;
  d.f.palette = undefined;
  d.f.series.forEach((s, i) => {
    if (varied) s.pointColors = Array.from({ length: Math.max(d.categories.length, 1) }, (_, k) => colors[k % colors.length]!);
    else {
      s.color = colors[i % colors.length];
      s.pointColors = undefined;
    }
  });
}

/** A model from the engine, ready to draw: colours resolved against the theme. */
function resolved(m: ChartModel, d: Draft): ChartModel {
  const rc = (c: string | undefined) => (c ? resolveColor(c, d.f.theme) : undefined);
  return { ...m, series: m.series.map((s) => ({ ...s, color: rc(s.color), lineColor: rc(s.lineColor), pointColors: s.pointColors?.map((c) => (c ? (rc(c) ?? null) : null)) as (string | null)[] | undefined })) };
}
