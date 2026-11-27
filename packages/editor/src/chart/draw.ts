// Draws a chart from the engine's model as SVG. No chart library: columns and bars (clustered, stacked,
// percent), lines, areas, scatter, bubble, stock, surface, radar, pie and doughnut, and combinations of the
// first few, with axes, gridlines, tick marks, a legend, a title, data labels, trendlines and error bars. Colours
// come from the file (or the theme's accents, or Word's Office palette as CSS tokens), text is the document's own
// font, and every size is in points like the drawing in Word.
import type { Axis, ChartKind, ChartModel, ChartSeries, Trendline } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import { drawSurface, surfaceBands } from "./draw-surface";
import { autoRange, formatNumber, frac, general, niceTicks, unitDivisor, type TickOptions, type Ticks } from "./format";
import { AXIS_TITLE_FONT, FONT, LEGEND_ROW, PAD, PT_PX, TITLE_FONT, color, el, fillAttrs, finite, isDark, sliceColor, smoothPath, textWidth, tip, type Box } from "./svg";
import { errorAmounts, fitTrend } from "./trend";
import { hex } from "../editor/color.ts";

const KIND_TITLE: Record<ChartKind, () => string> = {
  column: () => t("Column chart"),
  bar: () => t("Bar chart"),
  line: () => t("Line chart"),
  pie: () => t("Pie chart"),
  doughnut: () => t("Doughnut chart"),
  area: () => t("Area chart"),
  scatter: () => t("Scatter chart"),
  radar: () => t("Radar chart"),
  bubble: () => t("Bubble chart"),
  stock: () => t("Stock chart"),
  surface: () => t("Surface chart"),
  other: () => t("Chart"),
};

/** The text a screen reader gets: what kind of chart, its title and a first look at the data. */
export function chartLabel(m: ChartModel): string {
  const kind = m.combo ? t("Combination chart") : KIND_TITLE[m.kind]();
  const head = m.title ? `${kind}: ${m.title}` : kind;
  const n = m.series.length;
  const names = m.series
    .slice(0, 4)
    .map((s) => s.name)
    .filter(Boolean)
    .join(", ");
  const xy = m.kind === "scatter" || m.kind === "bubble";
  const count = xy ? Math.max(0, ...m.series.map((s) => s.values.length)) : m.categories.length;
  const pts = xy ? tn(count, "{n} point", "{n} points") : tn(count, "{n} category", "{n} categories");
  return `${head}. ${tn(n, "{n} series", "{n} series")}${names ? ` (${names}${n > 4 ? t(", and more") : ""})` : ""}, ${pts}.`;
}

/** The chart's data as a table, for people who can't see the drawing. */
export function chartTable(m: ChartModel): HTMLElement {
  const wrap = document.createElement("span");
  wrap.className = "chart-sr";
  const table = document.createElement("table");
  table.className = "chart-data";
  wrap.append(table);
  if (m.title) table.createCaption().textContent = m.title;
  const head = table.createTHead().insertRow();
  const th = (text: string) => Object.assign(document.createElement("th"), { textContent: text, scope: "col" });
  if (m.kind === "scatter" || m.kind === "bubble") {
    head.append(...[t("Series"), "X", "Y", ...(m.kind === "bubble" ? [t("Size")] : [])].map(th));
    for (const s of m.series) s.values.forEach((y, i) => {
      const r = table.insertRow();
      r.insertCell().textContent = s.name;
      r.insertCell().textContent = general(s.xValues?.[i] ?? NaN);
      r.insertCell().textContent = finite(y) ? general(y) : "";
      if (m.kind === "bubble") r.insertCell().textContent = finite(s.sizes?.[i]) ? general(s.sizes![i]!) : "";
    });
    return wrap;
  }
  head.append(Object.assign(document.createElement("th"), { textContent: "", scope: "col" }));
  for (const s of m.series) head.append(th(s.name));
  m.categories.forEach((c, i) => {
    const r = table.insertRow();
    const row = document.createElement("th");
    row.scope = "row";
    row.textContent = c;
    r.append(row);
    for (const s of m.series) r.insertCell().textContent = finite(s.values[i]) ? general(s.values[i]!) : "";
  });
  return wrap;
}

// ---------------------------------------------------------------------------------------------

/** Draw a chart `w` × `h` points. */
export function drawChart(m: ChartModel, wPt: number, hPt: number): SVGSVGElement {
  const W = Math.max(40, wPt * PT_PX);
  const H = Math.max(40, hPt * PT_PX);
  const root = el("svg", { class: "chart-svg", viewBox: `0 0 ${W} ${H}`, width: W, height: H, role: "img", "aria-label": chartLabel(m), "data-kind": m.kind });
  if (m.kind === "other") {
    root.append(
      el("rect", { class: "chart-ph", x: 0.5, y: 0.5, width: W - 1, height: H - 1, rx: 2 }),
      el("text", { class: "chart-t chart-title", x: W / 2, y: H / 2 - 4, "text-anchor": "middle" }, m.title || t("Chart")),
      el("text", { class: "chart-t", x: W / 2, y: H / 2 + 16, "text-anchor": "middle" }, t("Docxtor keeps this chart as it is.")),
    );
    return root;
  }
  const g = new Drawer(m, root, W, H);
  g.run();
  return root;
}

interface LegendEntry {
  name: string;
  color: string;
  kind: ChartKind;
  marker: boolean;
  dashed?: boolean;
}

let clips = 0;

const AXIS_LABEL_W = 8;

/** The settings of an axis, whatever the file leaves out. */
function axisOf(m: ChartModel, which: "x" | "y" | "y2"): Axis {
  return (which === "x" ? m.xAxis : which === "y" ? m.yAxis : m.y2Axis) ?? {};
}

function tickOptions(a: Axis): TickOptions {
  return { min: a.min, max: a.max, step: a.majorUnit, minorStep: a.minorUnit, log: a.logBase };
}

export class Drawer {
  left = PAD;
  top = PAD;
  right: number;
  bottom: number;

  constructor(
    readonly m: ChartModel,
    readonly svg: SVGSVGElement,
    readonly W: number,
    readonly H: number,
  ) {
    this.right = W - PAD;
    this.bottom = H - PAD;
  }

  add(...n: (Node | null | undefined | false)[]) {
    for (const x of n) if (x) this.svg.append(x);
  }

  /** The Excel number format of the value axis: the axis' own, else the data's. */
  private valueFormat(axis: Axis = axisOf(this.m, "y")): string | undefined {
    if (this.m.grouping === "percent" && this.m.kind !== "line" && this.m.kind !== "scatter") return "0%";
    return axis.numberFormat ?? this.m.axes.dataFormat;
  }

  /** A tick label of an axis (display units applied). */
  private tickLabel(v: number, axis: Axis, fmt: string | undefined): string {
    return formatNumber(v / unitDivisor(axis.displayUnits), fmt);
  }

  run() {
    const m = this.m;
    this.add(el("rect", { class: "chart-bg", x: 0, y: 0, width: this.W, height: this.H }));
    this.title();
    this.legend();
    if (m.series.length === 0) return;
    switch (m.kind) {
      case "pie":
      case "doughnut":
        return this.pie();
      case "radar":
        return this.radar();
      case "scatter":
      case "bubble":
        return this.scatter();
      case "surface":
        return drawSurface(this);
      default:
        return this.cartesian();
    }
  }

  // -- title and legend --------------------------------------------------------------------------

  private title() {
    if (!this.m.title) return;
    const lines = this.m.title.split("\n");
    const y0 = this.top;
    lines.forEach((line, i) => this.add(el("text", { class: "chart-t chart-title", x: this.W / 2, y: y0 + TITLE_FONT * 0.85 + i * TITLE_FONT * 1.2, "text-anchor": "middle" }, line)));
    if (!this.m.titleOverlay) this.top += lines.length * TITLE_FONT * 1.2 + 8;
  }

  private legendEntries(): LegendEntry[] {
    const m = this.m;
    if (m.kind === "surface") return surfaceBands(m).map((b) => ({ name: b.label, color: b.color, kind: "column" as ChartKind, marker: false }));
    if (m.kind === "pie" || m.kind === "doughnut") {
      const s = m.series[0];
      return s ? m.categories.map((c, i) => ({ name: c, color: sliceColor(m, s, i), kind: m.kind, marker: false })) : [];
    }
    if (m.varyColors && m.series.length === 1) return m.categories.map((c, i) => ({ name: c, color: s0Color(m, i), kind: "column" as ChartKind, marker: false }));
    const out: LegendEntry[] = m.series.map((s, i) => ({ name: s.name, color: color(m, i, s), kind: s.kind, marker: !!s.marker }));
    // Trendlines are listed after the series, as Word lists them.
    m.series.forEach((s, i) => {
      for (const tl of s.trendlines ?? []) out.push({ name: trendName(tl, s), color: color(m, i, s), kind: "line", marker: false, dashed: true });
    });
    return out;
  }

  private legend() {
    const lg = this.m.legend;
    const entries = this.legendEntries();
    if (!lg || entries.length === 0) return;
    const overlay = !!this.m.legendOverlay;
    const sw = 10;
    const gap = 6;
    const widths = entries.map((e) => sw + gap + textWidth(e.name) + 14);
    const draw = (e: LegendEntry, x: number, y: number) => {
      const line = e.kind === "line" || e.kind === "scatter" || e.kind === "radar" || e.kind === "stock";
      const mark = line
        ? el("g", {}, el("line", { x1: x, y1: y, x2: x + sw + 4, y2: y, stroke: e.color, "stroke-width": e.dashed ? 1.75 : 2.25, "stroke-linecap": "round", "stroke-dasharray": e.dashed ? "3 3" : undefined }), e.marker ? el("circle", { cx: x + (sw + 4) / 2, cy: y, r: 3, fill: e.color }) : null)
        : el("rect", { class: "chart-key", x, y: y - sw / 2, width: sw, height: sw, fill: e.color });
      this.add(mark, el("text", { class: "chart-t chart-legend-text", x: x + sw + gap + (line ? 4 : 0), y: y + FONT * 0.35 }, e.name));
    };
    const pos = lg.position;
    if (pos === "t" || pos === "b") {
      const avail = this.right - this.left;
      const rows: number[][] = [[]];
      let used = 0;
      entries.forEach((_, i) => {
        if (used + widths[i]! > avail && rows[rows.length - 1]!.length) {
          rows.push([]);
          used = 0;
        }
        rows[rows.length - 1]!.push(i);
        used += widths[i]!;
      });
      const h = rows.length * LEGEND_ROW;
      const y0 = pos === "t" ? this.top : this.bottom - h;
      rows.forEach((row, r) => {
        const total = row.reduce((a, i) => a + widths[i]!, 0) - 14;
        let x = (this.left + this.right) / 2 - total / 2;
        for (const i of row) {
          draw(entries[i]!, x, y0 + r * LEGEND_ROW + LEGEND_ROW / 2);
          x += widths[i]!;
        }
      });
      if (!overlay) {
        if (pos === "t") this.top += h + 4;
        else this.bottom -= h + 4;
      }
    } else {
      const maxW = Math.max(...widths) - 14;
      const w = maxW + 8;
      const h = entries.length * LEGEND_ROW;
      const x0 = pos === "l" ? this.left : this.right - maxW;
      const y0 = pos === "tr" ? this.top + 4 : (this.top + this.bottom) / 2 - h / 2;
      entries.forEach((e, i) => draw(e, x0, y0 + i * LEGEND_ROW + LEGEND_ROW / 2));
      if (!overlay) {
        if (pos === "l") this.left += w + 8;
        else this.right -= w + 8;
      }
    }
  }

  // -- value ranges ------------------------------------------------------------------------------

  /** [min, max] of the values of some series: stacks add up, percent charts run 0 to 1. */
  private extent(series: ChartSeries[], stackable: boolean): [number, number] {
    const m = this.m;
    if (stackable && m.grouping === "percent") return [0, 1];
    const n = Math.max(m.categories.length, ...series.map((s) => s.values.length), 0);
    let lo = Infinity;
    let hi = -Infinity;
    if (stackable && m.grouping === "stacked") {
      for (let i = 0; i < n; i++) {
        let pos = 0;
        let neg = 0;
        for (const s of series) {
          const v = s.values[i];
          if (!finite(v)) continue;
          if (v >= 0) pos += v;
          else neg += v;
        }
        lo = Math.min(lo, neg, 0);
        hi = Math.max(hi, pos, 0);
      }
    } else {
      for (const s of series) {
        for (const v of s.values) {
          if (finite(v)) {
            lo = Math.min(lo, v);
            hi = Math.max(hi, v);
          }
        }
        // Error bars reach past the values.
        for (const eb of s.errorBars ?? []) {
          if (eb.dir === "x") continue;
          const all = s.values.filter(finite);
          s.values.forEach((v, i) => {
            if (!finite(v)) return;
            const a = errorAmounts(eb, i, v, all);
            lo = Math.min(lo, v - a.minus);
            hi = Math.max(hi, v + a.plus);
          });
        }
      }
    }
    if (lo === Infinity) return [0, 1];
    return [lo, hi];
  }

  private ticksFor(series: ChartSeries[], stackable: boolean, includeZero: boolean, target: number, axis: Axis): Ticks {
    const [lo, hi] = this.extent(series, stackable);
    const [a, b] = autoRange(lo, hi, includeZero);
    if (this.m.grouping === "percent" && stackable) return niceTicks(0, 1, target, { min: axis.min ?? 0, max: axis.max ?? 1, step: axis.majorUnit ?? 0.2, minorStep: axis.minorUnit });
    return niceTicks(a, b, target, tickOptions(axis));
  }

  // -- columns, bars, lines, areas, stocks --------------------------------------------------------

  private cartesian() {
    const m = this.m;
    const horizontal = m.kind === "bar";
    const prim = m.series.filter((s) => !s.secondary);
    const sec = m.series.filter((s) => s.secondary);
    const barLike = (s: ChartSeries) => s.kind === "column" || s.kind === "bar";
    const areaLike = (s: ChartSeries) => s.kind === "area";
    const stackKinds = (s: ChartSeries) => barLike(s) || areaLike(s) || (s.kind === "line" && m.grouping !== "clustered");
    const stackable = prim.some(stackKinds);
    const includeZero = prim.some((s) => barLike(s) || areaLike(s));
    const xa = axisOf(m, "x");
    const ya = axisOf(m, "y");
    const y2a = axisOf(m, "y2");
    const base = prim.length ? prim : m.series;
    const pt = this.ticksFor(base, stackable, includeZero, 5, ya);
    const st = sec.length ? this.ticksFor(sec, false, sec.some((s) => barLike(s) || areaLike(s)), 5, y2a) : null;
    const n = Math.max(m.categories.length, ...m.series.map((s) => s.values.length));
    const fmt = this.valueFormat(ya);
    const fmt2 = y2a.numberFormat ?? m.axes.dataFormat;

    // Room for axis titles and tick labels.
    let l = this.left;
    let r = this.right;
    let t0 = this.top;
    let b = this.bottom;
    const vTitle = horizontal ? xa.title : ya.title;
    const hTitle = horizontal ? ya.title : xa.title;
    if (vTitle) l += 18;
    if (hTitle) b -= 18;
    if (!horizontal && y2a.title && st) r -= 18;
    const valLabels = pt.values.map((v) => this.tickLabel(v, ya, fmt));
    const valW = Math.max(...valLabels.map((s) => textWidth(s))) + AXIS_LABEL_W;
    const catW = Math.max(0, ...m.categories.map((c) => textWidth(c)));
    const hideVal = !!ya.hidden || ya.labelPos === "none";
    const hideCat = !!xa.hidden || xa.labelPos === "none";
    const valHigh = ya.labelPos === "high";
    let rotate = false;
    let skip = 1;
    if (!horizontal) {
      if (!hideVal) {
        if (valHigh) r -= valW;
        else l += valW;
      }
      if (st && !y2a.hidden && y2a.labelPos !== "none") r -= Math.max(...st.values.map((v) => textWidth(this.tickLabel(v, y2a, fmt2)))) + AXIS_LABEL_W;
      const plotW = r - l;
      const slot = plotW / Math.max(1, n);
      if (!hideCat && catW + 6 > slot) {
        // Turned sideways when that fits below the bars, else every other (third...) label.
        if (catW * 0.72 + 14 <= this.H * 0.32) rotate = true;
        else skip = Math.ceil((catW + 10) / slot);
      }
      if (!hideCat) b -= rotate ? catW * 0.72 + 14 : FONT + 10;
      t0 += 6;
    } else {
      if (!hideCat) l += Math.min(catW, (r - l) * 0.35) + 10;
      if (!hideVal) b -= FONT + 10;
      r -= 10;
      t0 += 4;
    }
    const plot: Box = { x: l, y: t0, w: Math.max(10, r - l), h: Math.max(10, b - t0) };
    this.add(...this.axisTitles(plot, horizontal, !!st));

    const band = (horizontal ? plot.h : plot.w) / Math.max(1, n);
    const reverse = !!xa.reverse;
    const midCat = m.series.every((s) => s.kind === "area") && !horizontal;
    const uStep = midCat && n > 1 ? plot.w / (n - 1) : band;
    // Position along the category axis for unit `u` (0 = start of the first slot).
    const uPx = (u: number): number => {
      if (midCat) return plot.x + (reverse ? n - 1 - u : u) * uStep;
      if (!horizontal) return reverse ? plot.x + plot.w - u * band : plot.x + u * band;
      return reverse ? plot.y + u * band : plot.y + plot.h - u * band;
    };
    const valLen = horizontal ? plot.w : plot.h;
    const valPx = (v: number, tk: Ticks, rev = false): number => {
      let f = frac(tk, v);
      if (rev) f = 1 - f;
      return horizontal ? plot.x + f * valLen : plot.y + plot.h - f * valLen;
    };
    const vp = (v: number) => valPx(v, pt, !!ya.reverse);
    const center = (i: number) => (midCat ? uPx(i) : uPx(i + 0.5));

    // Gridlines.
    const grid = el("g", { class: "chart-gridlines" });
    const valLine = (p: number, cls: string) => grid.append(horizontal ? el("line", { class: cls, x1: p, y1: plot.y, x2: p, y2: plot.y + plot.h }) : el("line", { class: cls, x1: plot.x, y1: p, x2: plot.x + plot.w, y2: p }));
    if (ya.majorGrid || (ya.majorGrid === undefined && m.axes.grid)) for (const v of pt.values) valLine(vp(v), "chart-grid");
    if (ya.minorGrid) for (const v of pt.minor) valLine(vp(v), "chart-grid minor");
    if (st && y2a.majorGrid) for (const v of st.values) valLine(valPx(v, st, !!y2a.reverse), "chart-grid");
    const catLine = (u: number, cls: string) => {
      const p = uPx(u);
      grid.append(horizontal ? el("line", { class: cls, x1: plot.x, y1: p, x2: plot.x + plot.w, y2: p }) : el("line", { class: cls, x1: p, y1: plot.y, x2: p, y2: plot.y + plot.h }));
    };
    if (xa.majorGrid) for (let u = 0; u <= (midCat ? n - 1 : n); u++) catLine(u, "chart-grid");
    if (xa.minorGrid) for (let u = 0; u < n; u++) catLine(midCat ? u + 0.5 : u + 0.5, "chart-grid minor");
    this.add(grid);

    // Value axis labels, tick marks and the line itself.
    const axis = el("g", { class: "chart-axes" });
    const tickLen = (k: string | undefined) => (k === "out" ? [5, 0] : k === "in" ? [0, 5] : k === "cross" ? [5, 5] : [0, 0]);
    if (!ya.hidden) {
      const [out, inn] = tickLen(ya.majorTick);
      const [mout, minn] = tickLen(ya.minorTick);
      const edge = horizontal ? plot.y + plot.h : valHigh ? plot.x + plot.w : plot.x;
      const tick = (v: number, o: number, i: number) => {
        const p = vp(v);
        if (o + i === 0) return;
        axis.append(horizontal ? el("line", { class: "chart-tick", x1: p, y1: edge + o, x2: p, y2: edge - i }) : el("line", { class: "chart-tick", x1: edge - (valHigh ? -i : o), y1: p, x2: edge + (valHigh ? -o : i), y2: p }));
      };
      for (const v of pt.values) tick(v, out, inn);
      for (const v of pt.minor) tick(v, mout, minn);
      if (!hideVal) {
        pt.values.forEach((v, i) => {
          const p = vp(v);
          const label = valLabels[i]!;
          if (horizontal) axis.append(el("text", { class: "chart-t", x: p, y: valHigh ? plot.y - 6 : plot.y + plot.h + FONT + 4, "text-anchor": "middle" }, label));
          else axis.append(el("text", { class: "chart-t", x: valHigh ? plot.x + plot.w + AXIS_LABEL_W : plot.x - AXIS_LABEL_W, y: p + FONT * 0.35, "text-anchor": valHigh ? "start" : "end" }, label));
        });
      }
    }
    if (st && !horizontal && !y2a.hidden) {
      const [out, inn] = tickLen(y2a.majorTick);
      for (const v of st.values) {
        const p = valPx(v, st, !!y2a.reverse);
        if (y2a.labelPos !== "none") axis.append(el("text", { class: "chart-t", x: plot.x + plot.w + AXIS_LABEL_W, y: p + FONT * 0.35, "text-anchor": "start" }, this.tickLabel(v, y2a, fmt2)));
        if (out + inn) axis.append(el("line", { class: "chart-tick", x1: plot.x + plot.w - inn, y1: p, x2: plot.x + plot.w + out, y2: p }));
      }
    }
    this.add(axis);

    // Category axis line at the value 0 (or the bottom/left edge when the range doesn't include it).
    const zero = Math.min(Math.max(0, pt.min), pt.max);
    const zeroPx = pt.log ? valPx(pt.min, pt, !!ya.reverse) : vp(zero);
    if (!xa.hidden) {
      this.add(horizontal ? el("line", { class: "chart-axis", x1: zeroPx, y1: plot.y, x2: zeroPx, y2: plot.y + plot.h }) : el("line", { class: "chart-axis", x1: plot.x, y1: zeroPx, x2: plot.x + plot.w, y2: zeroPx }));
      const [out, inn] = tickLen(xa.majorTick);
      if (out + inn) {
        const ticks = el("g", {});
        for (let u = 0; u <= (midCat ? n - 1 : n); u++) {
          const p = uPx(u);
          ticks.append(horizontal ? el("line", { class: "chart-tick", x1: zeroPx - out, y1: p, x2: zeroPx + inn, y2: p }) : el("line", { class: "chart-tick", x1: p, y1: zeroPx - inn, x2: p, y2: zeroPx + out }));
        }
        this.add(ticks);
      }
    }

    // Category labels: next to the axis, at the low end of the chart (the default look) or the high end.
    if (!hideCat) {
      const lab = el("g", {});
      const high = xa.labelPos === "high";
      const nextTo = (xa.labelPos ?? "nextTo") === "nextTo" && pt.min < 0 && pt.max > 0;
      for (let i = 0; i < n; i += skip) {
        const text = m.categories[i] ?? "";
        const c = center(i);
        if (horizontal) {
          const x = high ? plot.x + plot.w + 8 : nextTo ? zeroPx - 8 : plot.x - 8;
          lab.append(el("text", { class: "chart-t", x, y: c + FONT * 0.35, "text-anchor": high ? "start" : "end" }, text));
        } else {
          const y = high ? plot.y - 6 : nextTo ? zeroPx + FONT + 6 : plot.y + plot.h + FONT + 6;
          if (rotate) lab.append(el("text", { class: "chart-t", x: c, y: plot.y + plot.h + 10, "text-anchor": "end", transform: `rotate(-45 ${c} ${plot.y + plot.h + 10})` }, text));
          else lab.append(el("text", { class: "chart-t", x: c, y, "text-anchor": "middle" }, text));
        }
      }
      this.add(lab);
    }

    // Series, in drawing order: areas, then bars, then lines.
    const idx = new Map(m.series.map((s, i) => [s, i] as const));
    const layer = (pred: (s: ChartSeries) => boolean) => m.series.filter(pred);
    const tickOf = (s: ChartSeries) => (s.secondary && st ? st : pt);
    const revOf = (s: ChartSeries) => (s.secondary && st ? !!y2a.reverse : !!ya.reverse);
    const px = (s: ChartSeries) => (v: number) => valPx(v, tickOf(s), revOf(s));

    const areas = layer(areaLike);
    if (areas.length) this.areas(areas, idx, center, valPx, pt, n, plot);
    const bars = layer(barLike);
    if (bars.length) this.bars(bars, idx, uPx, valPx, pt, horizontal, n, plot);
    for (const s of layer((x) => x.kind === "line")) this.line(s, idx.get(s)!, center, px(s), n, horizontal, plot);
    for (const s of layer((x) => x.kind === "stock")) this.stock(s, idx.get(s)!, layer((x) => x.kind === "stock"), center, px(s), band, plot);
    // Scatter series in a combination chart sit on the categories: X = 1 is the middle of the first one.
    for (const s of layer((x) => x.kind === "scatter")) this.comboScatter(s, idx.get(s)!, (x) => uPx(x - 0.5), px(s), horizontal, plot);
  }

  /** The text of a data label. */
  private labelText(s: ChartSeries, i: number, v: number, extra: { pct?: number; size?: number; x?: number } = {}): string | null {
    const l = s.labels;
    if (!l) return null;
    const m = this.m;
    const parts: string[] = [];
    if (l.series) parts.push(s.name);
    if (l.category) parts.push(m.kind === "scatter" || m.kind === "bubble" ? (finite(extra.x) ? general(extra.x) : "") : (m.categories[i] ?? ""));
    if (l.value) parts.push(formatNumber(v, l.numberFormat ?? this.valueFormat()));
    if (l.percent && extra.pct !== undefined) parts.push(`${Math.round(extra.pct * 100)}%`);
    if (l.bubbleSize && finite(extra.size)) parts.push(formatNumber(extra.size, l.numberFormat));
    const text = parts.filter((p) => p !== "").join(l.separator ?? ", ");
    return text || null;
  }

  /** A data label: text at a place, with its legend key when asked for. */
  private labelEl(s: ChartSeries, si: number, text: string, x: number, y: number, anchor: string, cls = ""): SVGElement {
    const lines = text.split("\n");
    const t = el("text", { class: `chart-t chart-label${cls ? ` ${cls}` : ""}`, x, y: y - ((lines.length - 1) * FONT * 1.1) / 2, "text-anchor": anchor });
    lines.forEach((line, k) => {
      const span = el("tspan", { x, dy: k === 0 ? 0 : FONT * 1.1 });
      if (k === 0 && s.labels?.legendKey) span.append(el("tspan", { fill: color(this.m, si, s) }, "■ "));
      span.append(line);
      t.append(span);
    });
    return t;
  }

  /** Error bars of a series along the value axis (or the X axis for scatter and bubble charts). */
  private errBars(s: ChartSeries, pts: { i: number; v: number; at: number; end?: number }[], toPx: (v: number) => number, horizontalValue: boolean, dir: "x" | "y" = "y"): SVGElement | null {
    const bars = (s.errorBars ?? []).filter((e) => e.dir === dir);
    if (!bars.length) return null;
    const g = el("g", { class: "chart-errbars" });
    const all = pts.map((p) => p.v);
    for (const eb of bars) {
      for (const p of pts) {
        const a = errorAmounts(eb, p.i, p.v, all);
        const base = p.end ?? p.v;
        const p0 = toPx(base - a.minus);
        const p1 = toPx(base + a.plus);
        const cap = eb.noCap ? 0 : 4;
        if (horizontalValue) {
          g.append(el("line", { class: "chart-err", x1: p0, y1: p.at, x2: p1, y2: p.at }));
          if (cap) {
            if (a.minus) g.append(el("line", { class: "chart-err", x1: p0, y1: p.at - cap, x2: p0, y2: p.at + cap }));
            if (a.plus) g.append(el("line", { class: "chart-err", x1: p1, y1: p.at - cap, x2: p1, y2: p.at + cap }));
          }
        } else {
          g.append(el("line", { class: "chart-err", x1: p.at, y1: p0, x2: p.at, y2: p1 }));
          if (cap) {
            if (a.minus) g.append(el("line", { class: "chart-err", x1: p.at - cap, y1: p0, x2: p.at + cap, y2: p0 }));
            if (a.plus) g.append(el("line", { class: "chart-err", x1: p.at - cap, y1: p1, x2: p.at + cap, y2: p1 }));
          }
        }
      }
    }
    return g;
  }

  /** The trendlines of a series, clipped to the plot. `toPx` maps (x, y) in data units to the page. */
  private trendlines(s: ChartSeries, si: number, xs: number[], ys: number[], toPx: (x: number, y: number) => { x: number; y: number }, plot: Box) {
    if (!s.trendlines?.length) return;
    const clipId = `chart-clip-${++clips}`;
    const g = el("g", { class: "chart-trendlines", "clip-path": `url(#${clipId})` });
    const labels = el("g", { class: "chart-trendlabels" });
    this.add(el("clipPath", { id: clipId }, el("rect", { x: plot.x, y: plot.y, width: plot.w, height: plot.h })));
    const col = color(this.m, si, s);
    for (const tl of s.trendlines) {
      const fit = fitTrend(tl, xs, ys);
      if (!fit) continue;
      let pts: { x: number; y: number }[];
      if (fit.points) pts = fit.points.map((p) => toPx(p.x, p.y));
      else {
        const x0 = Math.min(...xs) - (tl.backward ?? 0);
        const x1 = Math.max(...xs) + (tl.forward ?? 0);
        const N = 80;
        pts = [];
        for (let k = 0; k <= N; k++) {
          const x = x0 + ((x1 - x0) * k) / N;
          const y = fit.f!(x);
          if (Number.isFinite(y)) pts.push(toPx(x, y));
        }
      }
      if (pts.length < 2) continue;
      g.append(el("path", { class: "chart-trend", d: "M" + pts.map((p) => `${p.x} ${p.y}`).join("L"), stroke: col, fill: "none" }, tip(trendName(tl, s))));
      if (tl.equation || tl.rSquared) {
        const end = pts[pts.length - 1]!;
        const lines = [tl.equation ? fit.equation : "", tl.rSquared ? `R² = ${Number(fit.r2.toPrecision(4))}` : ""].filter(Boolean);
        lines.forEach((line, k) => labels.append(el("text", { class: "chart-t chart-eq", x: Math.min(plot.x + plot.w - 2, end.x), y: Math.max(plot.y + FONT, end.y - 6 - (lines.length - 1 - k) * (FONT + 2)), "text-anchor": "end" }, line)));
      }
    }
    this.add(g, labels);
  }

  private axisTitles(plot: Box, horizontal: boolean, hasSecond: boolean): SVGElement[] {
    const m = this.m;
    const xa = axisOf(m, "x");
    const ya = axisOf(m, "y");
    const out: SVGElement[] = [];
    const bottomTitle = horizontal ? ya.title : xa.title;
    const leftTitle = horizontal ? xa.title : ya.title;
    if (bottomTitle) out.push(el("text", { class: "chart-t chart-axis-title", x: plot.x + plot.w / 2, y: this.bottom - 4, "text-anchor": "middle" }, bottomTitle));
    if (leftTitle) {
      const x = this.left + AXIS_TITLE_FONT * 0.8;
      const y = plot.y + plot.h / 2;
      out.push(el("text", { class: "chart-t chart-axis-title", x, y, "text-anchor": "middle", transform: `rotate(-90 ${x} ${y})` }, leftTitle));
    }
    const rightTitle = hasSecond && !horizontal ? axisOf(m, "y2").title : undefined;
    if (rightTitle) {
      const x = this.right - AXIS_TITLE_FONT * 0.3;
      const y = plot.y + plot.h / 2;
      out.push(el("text", { class: "chart-t chart-axis-title", x, y, "text-anchor": "middle", transform: `rotate(90 ${x} ${y})` }, rightTitle));
    }
    return out;
  }

  private bars(series: ChartSeries[], idx: Map<ChartSeries, number>, uPx: (u: number) => number, valPx: (v: number, t: Ticks, rev?: boolean) => number, pt: Ticks, horizontal: boolean, n: number, plot: Box) {
    const m = this.m;
    const stacked = m.grouping !== "clustered";
    const k = series.length;
    // Fractions of a slot, as Word draws them: the file's gap and overlap, else 219% and -27% (150% and 100% stacked).
    const gapFrac = (m.gapWidth ?? (stacked ? 150 : 219)) / 100;
    const overlap = (m.overlap ?? (stacked ? 100 : -27)) / 100;
    const barFrac = stacked ? 1 / (1 + gapFrac) : 1 / (k - (k - 1) * overlap + gapFrac);
    const groupFrac = stacked ? barFrac : barFrac * (k - (k - 1) * overlap);
    const g = el("g", { class: "chart-bars" });
    const labels = el("g", { class: "chart-labels" });
    const rev = !!axisOf(m, "y").reverse;
    const rect = (u0: number, u1: number, v0: number, v1: number) => {
      const a = uPx(u0);
      const b = uPx(u1);
      const c = valPx(v0, pt, rev);
      const d = valPx(v1, pt, rev);
      return horizontal ? { x: Math.min(c, d), y: Math.min(a, b), w: Math.abs(d - c), h: Math.abs(b - a) } : { x: Math.min(a, b), y: Math.min(c, d), w: Math.abs(b - a), h: Math.abs(d - c) };
    };
    const fmt = m.grouping === "percent" ? "0%" : this.valueFormat();
    const errs = series.map(() => [] as { i: number; v: number; at: number; end?: number }[]);
    for (let i = 0; i < n; i++) {
      let pos = 0;
      let neg = 0;
      const total = stacked && m.grouping === "percent" ? series.reduce((a, s) => a + Math.abs(finite(s.values[i]) ? s.values[i]! : 0), 0) : 1;
      series.forEach((s, j) => {
        const raw = s.values[i];
        if (!finite(raw)) return;
        const v = m.grouping === "percent" ? (total ? raw / total : 0) : raw;
        let u0: number;
        let v0: number;
        let v1: number;
        if (stacked) {
          u0 = i + 0.5 - groupFrac / 2;
          if (v >= 0) {
            v0 = pos;
            pos += v;
            v1 = pos;
          } else {
            v0 = neg;
            neg += v;
            v1 = neg;
          }
        } else {
          u0 = i + 0.5 - groupFrac / 2 + j * barFrac * (1 - overlap);
          v0 = pt.log ? pt.min : Math.min(Math.max(0, pt.min), pt.max);
          v1 = v;
        }
        const r = rect(u0, u0 + barFrac, v0, v1);
        const si = idx.get(s)!;
        const fill = hex(s.pointColors?.[i]) ?? (m.varyColors && m.series.length === 1 ? color(m, i) : color(m, si, s));
        g.append(el("rect", { class: "chart-bar", x: r.x, y: r.y, width: Math.max(0, r.w), height: Math.max(0, r.h), ...fillAttrs(this.svg, s, fill) }, tip(`${s.name}, ${m.categories[i] ?? ""}: ${formatNumber(raw, this.valueFormat())}`)));
        if (s.errorBars?.length) errs[j]!.push({ i, v: raw, at: horizontal ? r.y + r.h / 2 : r.x + r.w / 2, end: stacked ? v1 : undefined });
        const text = this.labelText({ ...s, labels: s.labels && m.grouping === "percent" ? { ...s.labels, numberFormat: s.labels.numberFormat ?? fmt } : s.labels }, i, m.grouping === "percent" ? v : raw);
        if (text) {
          const posn = s.labels?.position ?? (stacked ? "ctr" : "outEnd");
          const up = v >= 0 !== (horizontal ? false : false);
          const dark = isDark(fill);
          const cx = r.x + r.w / 2;
          const cy = r.y + r.h / 2 + FONT * 0.35;
          const inside = posn !== "outEnd";
          const cls = inside && dark && s.fill !== "none" ? "on-dark" : "";
          let x: number;
          let y: number;
          let anchor = "middle";
          if (horizontal) {
            y = cy;
            const toRight = v >= 0 !== rev;
            if (posn === "ctr") x = cx;
            else if (posn === "inEnd") {
              x = toRight ? r.x + r.w - 4 : r.x + 4;
              anchor = toRight ? "end" : "start";
            } else if (posn === "inBase") {
              x = toRight ? r.x + 4 : r.x + r.w - 4;
              anchor = toRight ? "start" : "end";
            } else {
              x = toRight ? r.x + r.w + 4 : r.x - 4;
              anchor = toRight ? "start" : "end";
            }
          } else {
            x = cx;
            const upward = v >= 0 !== rev;
            if (posn === "ctr") y = cy;
            else if (posn === "inEnd") y = upward ? r.y + FONT + 1 : r.y + r.h - 4;
            else if (posn === "inBase") y = upward ? r.y + r.h - 4 : r.y + FONT + 1;
            else y = upward ? r.y - 4 : r.y + r.h + FONT;
          }
          void up;
          labels.append(this.labelEl(s, si, text, x, y, anchor, cls));
        }
      });
    }
    this.add(g);
    series.forEach((s, j) => {
      const e = this.errBars(s, errs[j]!, (v) => valPx(v, pt, rev), horizontal);
      if (e) this.add(e);
    });
    this.add(labels);
    // Trendlines follow the middle of the group's bars.
    series.forEach((s) => {
      if (!s.trendlines?.length || m.grouping !== "clustered") return;
      const xs: number[] = [];
      const ys: number[] = [];
      s.values.forEach((v, i) => {
        if (finite(v)) {
          xs.push(i + 1);
          ys.push(v);
        }
      });
      this.trendlines(s, idx.get(s)!, xs, ys, (x, y) => (horizontal ? { x: valPx(y, pt, rev), y: uPx(x - 0.5) } : { x: uPx(x - 0.5), y: valPx(y, pt, rev) }), plot);
    });
  }

  private line(s: ChartSeries, si: number, center: (i: number) => number, valPx: (v: number) => number, n: number, horizontal: boolean, plot: Box) {
    const m = this.m;
    const col = color(m, si, s);
    const pts: ({ x: number; y: number; v: number } | null)[] = [];
    // Stacked lines add up; percent lines show shares (rare, drawn plainly as values here).
    for (let i = 0; i < n; i++) {
      const v = s.values[i];
      if (!finite(v)) {
        pts.push(null);
        continue;
      }
      const c = center(i);
      const p = valPx(v);
      pts.push(horizontal ? { x: p, y: c, v } : { x: c, y: p, v });
    }
    const g = el("g", { class: "chart-line" });
    const sw = s.lineWidth ? s.lineWidth * PT_PX : undefined;
    if (!s.noLine) {
      // Runs between gaps.
      let run: { x: number; y: number }[] = [];
      const flush = () => {
        if (run.length > 1) g.append(el("path", { class: "chart-stroke", d: s.smooth ? smoothPath(run) : "M" + run.map((p) => `${p.x} ${p.y}`).join("L"), stroke: col, "stroke-width": sw, fill: "none" }));
        run = [];
      };
      for (const p of pts) {
        if (p) run.push(p);
        else flush();
      }
      flush();
    }
    const fmt = this.valueFormat();
    pts.forEach((p, i) => {
      if (!p) return;
      if (s.marker) g.append(el("circle", { class: "chart-marker", cx: p.x, cy: p.y, r: 3.5, fill: col }, tip(`${s.name}, ${m.categories[i] ?? ""}: ${formatNumber(p.v, fmt)}`)));
      else g.append(el("circle", { class: "chart-hit", cx: p.x, cy: p.y, r: 6 }, tip(`${s.name}, ${m.categories[i] ?? ""}: ${formatNumber(p.v, fmt)}`)));
      const text = this.labelText(s, i, p.v);
      if (text) g.append(...this.pointLabel(s, si, text, p.x, p.y, s.labels?.position ?? (horizontal ? "r" : "r"), 8));
    });
    this.add(g);
    const errPts = pts.flatMap((p, i) => (p ? [{ i, v: p.v, at: horizontal ? p.y : p.x }] : []));
    const e = this.errBars(s, errPts, valPx, horizontal);
    if (e) this.add(e);
    if (s.trendlines?.length && m.grouping === "clustered") {
      const xs: number[] = [];
      const ys: number[] = [];
      s.values.forEach((v, i) => {
        if (finite(v)) {
          xs.push(i + 1);
          ys.push(v);
        }
      });
      this.trendlines(s, si, xs, ys, (x, y) => (horizontal ? { x: valPx(y), y: center(x - 1) } : { x: center(x - 1), y: valPx(y) }), plot);
    }
  }

  /** A label beside a point: above, below, left, right or centred on it. */
  private pointLabel(s: ChartSeries, si: number, text: string, x: number, y: number, pos: string, off: number): SVGElement[] {
    let lx = x;
    let ly = y;
    let anchor = "middle";
    switch (pos) {
      case "t":
        ly = y - off;
        break;
      case "b":
        ly = y + off + FONT * 0.6;
        break;
      case "l":
        lx = x - off;
        ly = y + FONT * 0.35;
        anchor = "end";
        break;
      case "ctr":
        ly = y + FONT * 0.35;
        break;
      default:
        lx = x + off;
        ly = y + FONT * 0.35;
        anchor = "start";
    }
    return [this.labelEl(s, si, text, lx, ly, anchor)];
  }

  private areas(series: ChartSeries[], idx: Map<ChartSeries, number>, center: (i: number) => number, valPx: (v: number, t: Ticks, rev?: boolean) => number, pt: Ticks, n: number, _plot: Box) {
    const m = this.m;
    const stacked = m.grouping !== "clustered";
    const base: number[] = new Array(n).fill(0);
    const total = (i: number) => series.reduce((a, s) => a + Math.abs(finite(s.values[i]) ? s.values[i]! : 0), 0);
    const g = el("g", { class: "chart-areas" });
    const floor = Math.min(Math.max(0, pt.min), pt.max);
    const rev = !!axisOf(m, "y").reverse;
    const labels: SVGElement[] = [];
    for (const s of series) {
      const top: { x: number; y: number }[] = [];
      const bottom: { x: number; y: number }[] = [];
      for (let i = 0; i < n; i++) {
        let v = finite(s.values[i]) ? s.values[i]! : 0;
        if (m.grouping === "percent") v = total(i) ? v / total(i) : 0;
        const lo = stacked ? base[i]! : floor;
        const hi = stacked ? base[i]! + v : v;
        if (stacked) base[i] = hi;
        top.push({ x: center(i), y: valPx(hi, pt, rev) });
        bottom.push({ x: center(i), y: valPx(lo, pt, rev) });
        const text = this.labelText(s, i, finite(s.values[i]) ? s.values[i]! : 0);
        if (text && finite(s.values[i])) labels.push(this.labelEl(s, idx.get(s)!, text, center(i), (top[i]!.y + bottom[i]!.y) / 2 + FONT * 0.35, "middle"));
      }
      const d = `M${top.map((p) => `${p.x} ${p.y}`).join("L")}L${[...bottom].reverse().map((p) => `${p.x} ${p.y}`).join("L")}Z`;
      const col = color(m, idx.get(s)!, s);
      const attrs = fillAttrs(this.svg, s, col);
      g.append(el("path", { class: "chart-area", d, ...attrs }, tip(s.name)), el("path", { class: "chart-stroke", d: `M${top.map((p) => `${p.x} ${p.y}`).join("L")}`, stroke: col, fill: "none" }));
      const e = this.errBars(s, s.values.flatMap((v, i) => (finite(v) ? [{ i, v, at: top[i]!.x, end: stacked ? base[i]! : undefined }] : [])), (v) => valPx(v, pt, rev), false);
      if (e) g.append(e);
    }
    this.add(g, ...labels);
  }

  /** A stock series: high-low lines, up-down bars and the close mark. The series come in the order of the stock type. */
  private stock(s: ChartSeries, si: number, group: ChartSeries[], center: (i: number) => number, valPx: (v: number) => number, band: number, _plot: Box) {
    const m = this.m;
    const k = group.indexOf(s);
    // The same pass draws the whole group once, from its first series.
    if (k !== 0) return;
    const type = m.stock ?? "hlc";
    const ohlc = type.endsWith("ohlc");
    const [o, h, l, c] = ohlc ? [group[0], group[1], group[2], group[3]] : [undefined, group[0], group[1], group[2]];
    if (!h || !l) return;
    const n = Math.max(m.categories.length, ...group.map((x) => x.values.length));
    const g = el("g", { class: "chart-stock" });
    const bodyW = Math.max(3, band * 0.4);
    for (let i = 0; i < n; i++) {
      const hv = h.values[i];
      const lv = l.values[i];
      if (!finite(hv) || !finite(lv)) continue;
      const x = center(i);
      g.append(el("line", { class: "chart-hilow", x1: x, y1: valPx(hv), x2: x, y2: valPx(lv) }, tip(`${m.categories[i] ?? ""}: ${group.map((q) => `${q.name} ${formatNumber(q.values[i] ?? NaN, this.valueFormat())}`).join(", ")}`)));
      const ov = o?.values[i];
      const cv = c?.values[i];
      if (ohlc && finite(ov) && finite(cv)) {
        const y0 = valPx(Math.max(ov, cv));
        const y1 = valPx(Math.min(ov, cv));
        g.append(el("rect", { class: cv >= ov ? "chart-up" : "chart-down", x: x - bodyW / 2, y: y0, width: bodyW, height: Math.max(1, y1 - y0) }));
      } else if (!ohlc && finite(cv)) {
        g.append(el("line", { class: "chart-close", x1: x, y1: valPx(cv), x2: x + Math.min(8, band * 0.3), y2: valPx(cv) }));
      }
    }
    this.add(g);
    void si;
  }

  private comboScatter(s: ChartSeries, si: number, xPx: (x: number) => number, valPx: (v: number) => number, horizontal: boolean, plot: Box) {
    const m = this.m;
    const col = color(m, si, s);
    const g = el("g", { class: "chart-line" });
    const pts = s.values.map((y, i) => (finite(y) && finite(s.xValues?.[i]) ? { x: xPx(s.xValues![i]!), y: valPx(y), xv: s.xValues![i]!, yv: y } : null));
    const run = pts.filter((p): p is NonNullable<typeof p> => !!p);
    if (!s.noLine && run.length > 1) g.append(el("path", { class: "chart-stroke", d: s.smooth ? smoothPath(run) : "M" + run.map((p) => `${p.x} ${p.y}`).join("L"), stroke: col, fill: "none" }));
    for (const p of run) {
      g.append(el("circle", { class: s.marker || s.noLine ? "chart-marker" : "chart-hit", cx: horizontal ? p.y : p.x, cy: horizontal ? p.x : p.y, r: s.marker || s.noLine ? 4 : 6, fill: col }, tip(`${s.name}: (${general(p.xv)}, ${general(p.yv)})`)));
    }
    this.add(g);
    if (s.trendlines?.length) {
      const xs = run.map((p) => p.xv);
      const ys = run.map((p) => p.yv);
      this.trendlines(s, si, xs, ys, (x, y) => ({ x: xPx(x), y: valPx(y) }), plot);
    }
  }

  // -- scatter and bubble ------------------------------------------------------------------------

  private scatter() {
    const m = this.m;
    const bubble = m.kind === "bubble";
    const xa = axisOf(m, "x");
    const ya = axisOf(m, "y");
    const xs = m.series.flatMap((s) => (s.xValues ?? []).filter(finite));
    const pad = bubble ? 0.1 : 0;
    const xlo = Math.min(...xs, Infinity);
    const xhi = Math.max(...xs, -Infinity);
    const xSpan = Number.isFinite(xhi - xlo) ? xhi - xlo || 1 : 1;
    const xTicks = niceTicks(...autoRange(xlo - xSpan * pad, xhi + xSpan * pad, false), 6, tickOptions(xa));
    const [ylo, yhi] = this.extent(m.series, false);
    const ySpan = yhi - ylo || 1;
    const yTicks = niceTicks(...autoRange(ylo - ySpan * pad, yhi + ySpan * pad, false), 5, tickOptions(ya));
    const fmt = this.valueFormat(ya);
    const xfmt = xa.numberFormat;
    const valLabels = yTicks.values.map((v) => this.tickLabel(v, ya, fmt));
    let l = this.left + (ya.title ? 18 : 0);
    let b = this.bottom - (xa.title ? 18 : 0);
    const yHidden = !!ya.hidden || ya.labelPos === "none";
    const xHidden = !!xa.hidden || xa.labelPos === "none";
    if (!yHidden) l += Math.max(...valLabels.map((s) => textWidth(s))) + AXIS_LABEL_W;
    if (!xHidden) b -= FONT + 10;
    const plot: Box = { x: l, y: this.top + 6, w: Math.max(10, this.right - 10 - l), h: Math.max(10, b - this.top - 6) };
    this.add(...this.axisTitles(plot, false, false));
    const px = (v: number) => plot.x + (xa.reverse ? 1 - frac(xTicks, v) : frac(xTicks, v)) * plot.w;
    const py = (v: number) => plot.y + plot.h - (ya.reverse ? 1 - frac(yTicks, v) : frac(yTicks, v)) * plot.h;
    const grid = el("g", { class: "chart-gridlines" });
    if (ya.majorGrid || (ya.majorGrid === undefined && m.axes.grid)) for (const v of yTicks.values) grid.append(el("line", { class: "chart-grid", x1: plot.x, y1: py(v), x2: plot.x + plot.w, y2: py(v) }));
    if (ya.minorGrid) for (const v of yTicks.minor) grid.append(el("line", { class: "chart-grid minor", x1: plot.x, y1: py(v), x2: plot.x + plot.w, y2: py(v) }));
    if (xa.majorGrid) for (const v of xTicks.values) grid.append(el("line", { class: "chart-grid", x1: px(v), y1: plot.y, x2: px(v), y2: plot.y + plot.h }));
    if (xa.minorGrid) for (const v of xTicks.minor) grid.append(el("line", { class: "chart-grid minor", x1: px(v), y1: plot.y, x2: px(v), y2: plot.y + plot.h }));
    this.add(grid);
    const labels = el("g", { class: "chart-axes" });
    const tickLen = (k: string | undefined) => (k === "out" ? [5, 0] : k === "in" ? [0, 5] : k === "cross" ? [5, 5] : [0, 0]);
    if (!ya.hidden) {
      const [o, i] = tickLen(ya.majorTick);
      yTicks.values.forEach((v, k) => {
        if (!yHidden) labels.append(el("text", { class: "chart-t", x: plot.x - AXIS_LABEL_W, y: py(v) + FONT * 0.35, "text-anchor": "end" }, valLabels[k]!));
        if (o + i) labels.append(el("line", { class: "chart-tick", x1: plot.x - o, y1: py(v), x2: plot.x + i, y2: py(v) }));
      });
    }
    if (!xa.hidden) {
      const [o, i] = tickLen(xa.majorTick);
      for (const v of xTicks.values) {
        if (!xHidden) labels.append(el("text", { class: "chart-t", x: px(v), y: plot.y + plot.h + FONT + 6, "text-anchor": "middle" }, this.tickLabel(v, xa, xfmt)));
        if (o + i) labels.append(el("line", { class: "chart-tick", x1: px(v), y1: plot.y + plot.h - i, x2: px(v), y2: plot.y + plot.h + o }));
      }
    }
    this.add(labels);
    if (!xa.hidden) this.add(el("line", { class: "chart-axis", x1: plot.x, y1: plot.y + plot.h, x2: plot.x + plot.w, y2: plot.y + plot.h }));
    if (!ya.hidden) this.add(el("line", { class: "chart-axis", x1: plot.x, y1: plot.y, x2: plot.x, y2: plot.y + plot.h }));

    // Bubble sizes: the biggest is a quarter of the smaller side of the plot at 100%.
    const sizes = m.series.flatMap((s) => (s.sizes ?? []).filter((z): z is number => finite(z) && (m.showNegativeBubbles || z > 0)));
    const maxSize = Math.max(...sizes.map(Math.abs), 0) || 1;
    const rMax = (Math.min(plot.w, plot.h) * 0.25 * (m.bubbleScale ?? 100)) / 100 / 2;
    const radius = (z: number) => Math.max(2, rMax * (m.sizeRepresents === "width" ? Math.abs(z) / maxSize : Math.sqrt(Math.abs(z) / maxSize)));
    m.series.forEach((s, si) => {
      const col = color(m, si, s);
      const pts = s.values.map((y, i) => (finite(y) && finite(s.xValues?.[i]) ? { x: px(s.xValues![i]!), y: py(y), xv: s.xValues![i]!, yv: y, i, z: s.sizes?.[i] } : null));
      const g = el("g", { class: "chart-line" });
      const run = pts.filter((p): p is NonNullable<typeof p> => !!p);
      if (!bubble && !s.noLine && run.length > 1) g.append(el("path", { class: "chart-stroke", d: s.smooth ? smoothPath(run) : "M" + run.map((p) => `${p.x} ${p.y}`).join("L"), stroke: col, "stroke-width": s.lineWidth ? s.lineWidth * PT_PX : undefined, fill: "none" }));
      for (const p of run) {
        if (bubble) {
          if (!finite(p.z) || (!m.showNegativeBubbles && p.z <= 0)) continue;
          const attrs = s.fill ? fillAttrs(this.svg, s, col) : { fill: col, "fill-opacity": 0.5 };
          g.append(el("circle", { class: "chart-bubble", cx: p.x, cy: p.y, r: radius(p.z), stroke: hex(s.lineColor) ?? col, "stroke-width": (s.lineWidth ?? 2) * PT_PX, ...attrs }, tip(`${s.name}: (${general(p.xv)}, ${general(p.yv)}), ${general(p.z)}`)));
        } else {
          g.append(el("circle", { class: s.marker || s.noLine ? "chart-marker" : "chart-hit", cx: p.x, cy: p.y, r: s.marker || s.noLine ? 4 : 6, fill: col }, tip(`${s.name}: (${general(p.xv)}, ${general(p.yv)})`)));
        }
        const text = this.labelText(s, p.i, p.yv, { x: p.xv, size: p.z ?? undefined });
        if (text) g.append(...this.pointLabel(s, si, text, p.x, p.y, s.labels?.position ?? (bubble ? "ctr" : "r"), bubble && finite(p.z) ? radius(p.z) + 4 : 8));
      }
      this.add(g);
      for (const dir of ["x", "y"] as const) {
        const e = this.errBars(s, run.map((p) => ({ i: p.i, v: dir === "x" ? p.xv : p.yv, at: dir === "x" ? p.y : p.x })), dir === "x" ? px : py, dir === "x", dir);
        if (e) this.add(e);
      }
      this.trendlines(s, si, run.map((p) => p.xv), run.map((p) => p.yv), (x, y) => ({ x: px(x), y: py(y) }), plot);
    });
  }

  // -- radar --------------------------------------------------------------------------------------

  private radar() {
    const m = this.m;
    const n = Math.max(3, m.categories.length);
    const labelRoom = Math.max(0, ...m.categories.map((c) => textWidth(c))) + 12;
    const cx = (this.left + this.right) / 2;
    const cy = (this.top + this.bottom) / 2;
    const rad = Math.max(10, Math.min((this.right - this.left) / 2 - labelRoom, (this.bottom - this.top) / 2 - FONT - 8));
    const [lo, hi] = this.extent(m.series, false);
    const ya = axisOf(m, "y");
    const tk = niceTicks(...autoRange(lo, hi, true), 4, tickOptions(ya));
    const fmt = this.valueFormat(ya);
    const ang = (i: number) => -Math.PI / 2 + (i * 2 * Math.PI) / n;
    const at = (i: number, v: number) => {
      const f = frac(tk, v);
      return { x: cx + Math.cos(ang(i)) * rad * f, y: cy + Math.sin(ang(i)) * rad * f };
    };
    const g = el("g", {});
    for (const v of tk.values.slice(1)) {
      const ring = Array.from({ length: n }, (_, i) => at(i, v));
      g.append(el("path", { class: "chart-grid", d: `M${ring.map((p) => `${p.x} ${p.y}`).join("L")}Z`, fill: "none" }));
      if (!ya.hidden) g.append(el("text", { class: "chart-t", x: cx - 5, y: at(0, v).y + FONT * 0.35 + (v === tk.max ? 3 : 0), "text-anchor": "end" }, this.tickLabel(v, ya, fmt)));
    }
    for (let i = 0; i < n; i++) {
      const e = at(i, tk.max);
      g.append(el("line", { class: "chart-grid", x1: cx, y1: cy, x2: e.x, y2: e.y }));
      const lx = cx + Math.cos(ang(i)) * (rad + 12);
      const ly = cy + Math.sin(ang(i)) * (rad + 12);
      const c = Math.cos(ang(i));
      g.append(el("text", { class: "chart-t", x: lx, y: ly + FONT * 0.35 + Math.sin(ang(i)) * 6, "text-anchor": Math.abs(c) < 0.2 ? "middle" : c > 0 ? "start" : "end" }, m.categories[i] ?? ""));
    }
    this.add(g);
    m.series.forEach((s, si) => {
      const col = color(m, si, s);
      const pts = Array.from({ length: n }, (_, i) => at(i, finite(s.values[i]) ? s.values[i]! : tk.min));
      const d = `M${pts.map((p) => `${p.x} ${p.y}`).join("L")}Z`;
      if (m.filled) this.add(el("path", { class: "chart-area", d, ...fillAttrs(this.svg, s, col) }));
      this.add(el("path", { class: "chart-stroke", d, stroke: col, "stroke-width": s.lineWidth ? s.lineWidth * PT_PX : undefined, fill: "none" }));
      pts.forEach((p, i) => {
        if (s.marker) this.add(el("circle", { class: "chart-marker", cx: p.x, cy: p.y, r: 3.5, fill: col }, tip(`${s.name}, ${m.categories[i] ?? ""}: ${formatNumber(s.values[i] ?? NaN, fmt)}`)));
        const text = this.labelText(s, i, s.values[i] ?? NaN);
        if (text) this.add(this.labelEl(s, si, text, p.x, p.y - 7, "middle"));
      });
    });
  }

  // -- pie and doughnut ---------------------------------------------------------------------------

  private pie() {
    const m = this.m;
    const s = m.series[0]!;
    const vals = m.categories.map((_, i) => Math.max(0, finite(s.values[i]) ? s.values[i]! : 0));
    const total = vals.reduce((a, b) => a + b, 0);
    const cx = (this.left + this.right) / 2;
    const cy = (this.top + this.bottom) / 2;
    const labelled = !!s.labels;
    const outside = labelled && (s.labels!.position === "outEnd" || s.labels!.position === undefined || s.labels!.position === "bestFit") && m.kind === "pie";
    const room = outside ? 30 : 4;
    const rad = Math.max(10, Math.min((this.right - this.left) / 2, (this.bottom - this.top) / 2) - room);
    const inner = m.kind === "doughnut" ? rad * ((m.hole ?? 50) / 100) : 0;
    const g = el("g", { class: "chart-slices" });
    const labels = el("g", { class: "chart-labels" });
    if (total <= 0) {
      this.add(el("text", { class: "chart-t", x: cx, y: cy, "text-anchor": "middle" }, t("No data")));
      return;
    }
    let a0 = -Math.PI / 2;
    const fmt = this.valueFormat();
    vals.forEach((v, i) => {
      if (v <= 0) return;
      const a1 = a0 + (v / total) * Math.PI * 2;
      const fill = sliceColor(m, s, i);
      const full = v / total > 0.9999;
      const pt = (r: number, a: number) => `${cx + Math.cos(a) * r} ${cy + Math.sin(a) * r}`;
      const large = a1 - a0 > Math.PI ? 1 : 0;
      let d: string;
      if (full) d = inner ? `M${cx} ${cy - rad}A${rad} ${rad} 0 1 1 ${cx} ${cy + rad}A${rad} ${rad} 0 1 1 ${cx} ${cy - rad}ZM${cx} ${cy - inner}A${inner} ${inner} 0 1 0 ${cx} ${cy + inner}A${inner} ${inner} 0 1 0 ${cx} ${cy - inner}Z` : `M${cx} ${cy - rad}A${rad} ${rad} 0 1 1 ${cx} ${cy + rad}A${rad} ${rad} 0 1 1 ${cx} ${cy - rad}Z`;
      else if (inner) d = `M${pt(rad, a0)}A${rad} ${rad} 0 ${large} 1 ${pt(rad, a1)}L${pt(inner, a1)}A${inner} ${inner} 0 ${large} 0 ${pt(inner, a0)}Z`;
      else d = `M${cx} ${cy}L${pt(rad, a0)}A${rad} ${rad} 0 ${large} 1 ${pt(rad, a1)}Z`;
      g.append(el("path", { class: "chart-slice", d, fill, "fill-rule": "evenodd" }, tip(`${m.categories[i] ?? ""}: ${formatNumber(v, fmt)} (${Math.round((v / total) * 100)}%)`)));
      const text = this.labelText(s, i, v, { pct: v / total });
      if (text) {
        const mid = (a0 + a1) / 2;
        const c = Math.cos(mid);
        const pos = m.kind === "doughnut" ? "ring" : (s.labels?.position ?? "bestFit");
        const inside = pos === "ring" || pos === "ctr" || pos === "inEnd" || (pos === "bestFit" && v / total > 0.12);
        const lr = pos === "ring" ? (rad + inner) / 2 : pos === "ctr" ? rad * 0.5 : pos === "inEnd" ? rad * 0.78 : inside ? rad * 0.66 : rad + 12;
        const x = cx + Math.cos(mid) * lr;
        const y = cy + Math.sin(mid) * lr;
        labels.append(this.labelEl(s, 0, text, x, y + FONT * 0.35, inside ? "middle" : Math.abs(c) < 0.15 ? "middle" : c > 0 ? "start" : "end", inside && isDark(fill) ? "on-dark" : ""));
      }
      a0 = a1;
    });
    this.add(g, labels);
  }
}

function s0Color(m: ChartModel, i: number): string {
  return color(m, i);
}

/** What a trendline is called in the legend: its own name, else its type and the series. */
export function trendName(tl: Trendline, s: ChartSeries): string {
  if (tl.name) return tl.name;
  const kind: Record<Trendline["type"], () => string> = {
    linear: () => t("Linear"),
    exp: () => t("Exponential"),
    log: () => t("Logarithmic"),
    poly: () => t("Polynomial"),
    power: () => t("Power"),
    movingAvg: () => t("Moving average"),
  };
  return `${kind[tl.type]()} (${s.name})`;
}
