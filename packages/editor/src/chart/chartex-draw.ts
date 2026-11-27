// Draws the new chart types (waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst) from the
// engine's model as SVG, in the same look as the classic charts in draw.ts: the document's font, Word's greys,
// the series palette tokens, a title, a legend, axes, gridlines and data labels, every size in points. The layout
// maths is in chartex-math.ts. Docxtor does not edit these charts: Word does.
import type { BoxStat, ChartExKind, ChartExModel, HistogramBin } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import { autoRange, formatNumber, general, niceTicks, type Ticks } from "./format";
import {
  accentColor,
  accentHex,
  arcLabel,
  arcPath,
  barSlot,
  binLabel,
  countTicks,
  sunburstLayout,
  textWidth,
  tint,
  tintHex,
  treemapLayout,
  wantsLightText,
  waterfallRange,
  type Rect,
} from "./chartex-math";
import { hex } from "../editor/color.ts";

const NS = "http://www.w3.org/2000/svg";
const PT_PX = 96 / 72;
const FONT = 12; // 9 pt: Word's axis and legend text
const TITLE_FONT = 18.67; // 14 pt
const AXIS_TITLE_FONT = 12;
const PAD = 10;
const LEGEND_ROW = 18;

type Attrs = Record<string, string | number | undefined>;

function el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Attrs = {}, ...kids: (Node | string | null | undefined | false)[]): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v !== undefined) e.setAttribute(k, typeof v === "number" ? String(Math.round(v * 100) / 100) : v);
  for (const c of kids) if (c) e.append(c);
  return e;
}

function tip(label: string): SVGTitleElement {
  const e = document.createElementNS(NS, "title");
  e.textContent = label;
  return e;
}

/** The text cut to fit `width` px, with an ellipsis; empty when not even a letter fits. */
function fit(text: string, width: number, size = FONT): string {
  if (textWidth(text, size) <= width) return text;
  for (let n = text.length - 1; n > 0; n--) if (textWidth(text.slice(0, n), size) + textWidth("…", size) <= width) return `${text.slice(0, n)}…`;
  return "";
}

const KIND_TITLE: Record<ChartExKind, () => string> = {
  waterfall: () => t("Waterfall chart"),
  histogram: () => t("Histogram"),
  pareto: () => t("Pareto chart"),
  boxWhisker: () => t("Box and whisker chart"),
  funnel: () => t("Funnel chart"),
  treemap: () => t("Treemap chart"),
  sunburst: () => t("Sunburst chart"),
  regionMap: () => t("Region map"),
  other: () => t("Chart"),
};

/** Whether the model holds something to draw. */
export function hasData(m: ChartExModel): boolean {
  switch (m.kind) {
    case "waterfall":
      return !!m.waterfall?.points.length;
    case "histogram":
    case "pareto":
      return !!m.histogram?.bins.length;
    case "boxWhisker":
      return !!m.boxes?.series.some((s) => s.boxes.some(Boolean));
    case "funnel":
      return !!m.funnel?.length;
    case "treemap":
    case "sunburst":
      return !!m.tree && m.tree.value > 0;
    default:
      return false;
  }
}

/** The text a screen reader gets: what kind of chart, its title and a first look at the data. */
export function chartExLabel(m: ChartExModel): string {
  const kind = KIND_TITLE[m.kind]();
  const head = m.title ? `${kind}: ${m.title}` : kind;
  switch (m.kind) {
    case "waterfall":
      return `${head}. ${tn(m.waterfall?.points.length ?? 0, "{n} point", "{n} points")}.`;
    case "histogram":
    case "pareto":
      return `${head}. ${tn(m.histogram?.bins.length ?? 0, "{n} bar", "{n} bars")}.`;
    case "boxWhisker":
      return `${head}. ${tn(m.boxes?.categories.length ?? 0, "{n} box", "{n} boxes")}.`;
    case "funnel":
      return `${head}. ${tn(m.funnel?.length ?? 0, "{n} stage", "{n} stages")}.`;
    case "treemap":
    case "sunburst":
      return `${head}. ${tn(m.tree?.children?.length ?? 0, "{n} group", "{n} groups")}.`;
    default:
      return head;
  }
}

function cell(row: HTMLTableRowElement, text: string, th = false) {
  const c = document.createElement(th ? "th" : "td");
  c.textContent = text;
  if (th) c.scope = "row";
  row.append(c);
}

/** The chart's data as a table, for people who can't see the drawing. */
export function chartExTable(m: ChartExModel): HTMLElement {
  const wrap = document.createElement("span");
  wrap.className = "chart-sr";
  const table = document.createElement("table");
  table.className = "chart-data";
  wrap.append(table);
  if (m.title) table.createCaption().textContent = m.title;
  const head = (...labels: string[]) => {
    const r = table.createTHead().insertRow();
    for (const l of labels) r.append(Object.assign(document.createElement("th"), { textContent: l, scope: "col" }));
  };
  const fmt = m.numberFormat;
  const num = (v: number) => formatNumber(v, fmt);
  if (m.waterfall) {
    head("", t("Value"), t("Running total"));
    for (const p of m.waterfall.points) {
      const r = table.insertRow();
      cell(r, p.label, true);
      cell(r, num(p.value));
      cell(r, num(p.end));
    }
  } else if (m.histogram) {
    head("", t("Count"));
    for (const b of m.histogram.bins) {
      const r = table.insertRow();
      cell(r, binLabel(b, fmt), true);
      cell(r, general(b.count));
    }
  } else if (m.boxes) {
    head("", t("Series"), t("Minimum"), t("First quartile"), t("Median"), t("Third quartile"), t("Maximum"), t("Mean"));
    m.boxes.categories.forEach((c, i) => {
      for (const s of m.boxes!.series) {
        const b = s.boxes[i];
        if (!b) continue;
        const r = table.insertRow();
        cell(r, c, true);
        cell(r, s.name);
        for (const v of [b.whiskerLo, b.q1, b.median, b.q3, b.whiskerHi, b.mean]) cell(r, num(v));
      }
    });
  } else if (m.funnel) {
    head("", t("Value"));
    for (const p of m.funnel) {
      const r = table.insertRow();
      cell(r, p.label, true);
      cell(r, num(p.value));
    }
  } else if (m.tree) {
    head("", t("Value"));
    const walk = (n: NonNullable<ChartExModel["tree"]>, path: string[]) => {
      for (const c of n.children ?? []) {
        const p = [...path, c.name];
        if (!c.children?.length) {
          const r = table.insertRow();
          cell(r, p.join(" / "), true);
          cell(r, num(c.value));
        } else walk(c, p);
      }
    };
    walk(m.tree, []);
  }
  return wrap;
}

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Draw a chart `w` × `h` points. */
export function drawChartEx(m: ChartExModel, wPt: number, hPt: number): SVGSVGElement {
  const W = Math.max(40, wPt * PT_PX);
  const H = Math.max(40, hPt * PT_PX);
  const root = el("svg", { class: "chart-svg chartex-svg", viewBox: `0 0 ${W} ${H}`, width: W, height: H, role: "img", "aria-label": chartExLabel(m), "data-kind": m.kind, "data-chartex": m.kind });
  if (!hasData(m)) {
    root.append(
      el("rect", { class: "chart-ph", x: 0.5, y: 0.5, width: W - 1, height: H - 1, rx: 2 }),
      el("text", { class: "chart-t chart-title", x: W / 2, y: H / 2 - 4, "text-anchor": "middle" }, m.title || KIND_TITLE[m.kind]()),
      el("text", { class: "chart-t", x: W / 2, y: H / 2 + 16, "text-anchor": "middle" }, t("Docxtor keeps this chart as it is.")),
    );
    return root;
  }
  new ExDrawer(m, root, W, H).run();
  return root;
}

interface LegendEntry {
  name: string;
  color: string;
  line?: boolean;
}

interface Frame {
  plot: Box;
  band: number;
  /** Pixel position of a value on the main value axis. */
  y: (v: number) => number;
  /** Start of slot `i` along the category axis. */
  x0: (i: number) => number;
  ticks: Ticks;
}

class ExDrawer {
  private left = PAD;
  private top = PAD;
  private right: number;
  private bottom: number;
  private fmt: string | undefined;

  constructor(
    private m: ChartExModel,
    private svg: SVGSVGElement,
    private W: number,
    private H: number,
  ) {
    this.right = W - PAD;
    this.bottom = H - PAD;
    this.fmt = m.numberFormat;
  }

  private add(...n: (Node | null | undefined | false)[]) {
    for (const x of n) if (x) this.svg.append(x);
  }

  run() {
    const m = this.m;
    this.add(el("rect", { class: "chart-bg", x: 0, y: 0, width: this.W, height: this.H }));
    this.title();
    this.legend();
    switch (m.kind) {
      case "waterfall":
        return this.waterfall();
      case "histogram":
      case "pareto":
        return this.histogram();
      case "boxWhisker":
        return this.boxes();
      case "funnel":
        return this.funnel();
      case "treemap":
        return this.treemap();
      case "sunburst":
        return this.sunburst();
    }
  }

  private color(i: number): string {
    return accentColor(this.m, i);
  }

  /** The colour of the whole series: the file's, else the first accent. */
  private seriesColor(i = 0): string {
    return (i === 0 ? hex(this.m.color) : undefined) ?? this.color(i);
  }

  // -- title and legend --------------------------------------------------------------------------

  private title() {
    if (!this.m.title) return;
    const lines = this.m.title.split("\n");
    lines.forEach((line, i) => this.add(el("text", { class: "chart-t chart-title", x: this.W / 2, y: this.top + TITLE_FONT * 0.85 + i * TITLE_FONT * 1.2, "text-anchor": "middle" }, line)));
    this.top += lines.length * TITLE_FONT * 1.2 + 8;
  }

  private legendEntries(): LegendEntry[] {
    const m = this.m;
    switch (m.kind) {
      case "waterfall": {
        const pts = m.waterfall?.points ?? [];
        const pick = (kind: string, i: number) => {
          const at = pts.findIndex((p) => p.kind === kind);
          const own = at >= 0 ? hex(m.pointColors?.[at]) : undefined;
          return own ?? this.color(i);
        };
        return [
          { name: t("Increase"), color: pick("increase", 0) },
          { name: t("Decrease"), color: pick("decrease", 1) },
          { name: t("Total"), color: pick("total", 2) },
        ];
      }
      case "histogram":
        return [{ name: m.name || t("Frequency"), color: this.seriesColor() }];
      case "pareto":
        return [
          { name: m.name || t("Frequency"), color: this.seriesColor() },
          { name: m.histogram?.lineName || t("Cumulative %"), color: hex(m.histogram?.lineColor) ?? this.color(1), line: true },
        ];
      case "boxWhisker":
        return (m.boxes?.series ?? []).map((s, i) => ({ name: s.name, color: hex(s.color) ?? this.color(i) }));
      case "funnel":
        return [{ name: m.name || t("Series"), color: this.seriesColor() }];
      case "treemap":
      case "sunburst":
        return (m.tree?.children ?? []).map((c, i) => ({ name: c.name, color: this.color(i) }));
      default:
        return [];
    }
  }

  private legend() {
    const lg = this.m.legend;
    const entries = this.legendEntries();
    if (!lg || entries.length === 0) return;
    const sw = 10;
    const gap = 6;
    const widths = entries.map((e) => sw + gap + textWidth(e.name, FONT) + 14);
    const draw = (e: LegendEntry, x: number, y: number) => {
      const mark = e.line
        ? el("g", {}, el("line", { x1: x, y1: y, x2: x + sw + 4, y2: y, stroke: e.color, "stroke-width": 2.25, "stroke-linecap": "round" }), el("circle", { cx: x + (sw + 4) / 2, cy: y, r: 3, fill: e.color }))
        : el("rect", { x, y: y - sw / 2, width: sw, height: sw, fill: e.color });
      this.add(mark, el("text", { class: "chart-t", x: x + sw + gap + (e.line ? 4 : 0), y: y + FONT * 0.35 }, e.name));
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
      if (pos === "t") this.top += h + 4;
      else this.bottom -= h + 4;
    } else {
      const maxW = Math.max(...widths) - 14;
      const w = maxW + 8;
      const h = entries.length * LEGEND_ROW;
      const x0 = pos === "r" ? this.right - maxW : this.left;
      const y0 = (this.top + this.bottom) / 2 - h / 2;
      entries.forEach((e, i) => draw(e, x0, y0 + i * LEGEND_ROW + LEGEND_ROW / 2));
      if (pos === "r") this.right -= w + 8;
      else this.left += w + 8;
    }
  }

  // -- axes ----------------------------------------------------------------------------------------

  /**
   * The plot area with a value axis on the left (and a second one on the right when `secondary` is given),
   * gridlines, and a label under each slot. Returns how to place things in it.
   */
  private frame(ticks: Ticks, cats: string[], secondary?: Ticks, secFmt?: string, valFmt: string | undefined = this.fmt): Frame {
    const ax = this.m.axes;
    let l = this.left;
    let r = this.right;
    let tp = this.top;
    let b = this.bottom;
    if (ax.valTitle) l += 18;
    if (ax.catTitle) b -= 18;
    const valLabels = ticks.values.map((v) => formatNumber(v, valFmt));
    const valW = Math.max(...valLabels.map((s) => textWidth(s, FONT))) + 8;
    const secLabels = secondary ? secondary.values.map((v) => formatNumber(v, secFmt)) : [];
    const n = Math.max(1, cats.length);
    const catW = Math.max(0, ...cats.map((c) => textWidth(c, FONT)));
    const showCats = !ax.catHidden && cats.some((c) => c !== "");
    if (!ax.valHidden) l += valW;
    if (secondary) r -= Math.max(...secLabels.map((s) => textWidth(s, FONT))) + 8;
    let rotate = false;
    let skip = 1;
    const slot = (r - l) / n;
    if (showCats) {
      if (catW + 6 > slot) {
        // Turned sideways when that fits below the bars, else every other (third...) label.
        if (catW * 0.72 + 14 <= this.H * 0.32) rotate = true;
        else skip = Math.ceil((catW + 10) / slot);
      }
      b -= rotate ? catW * 0.72 + 14 : FONT + 10;
    }
    tp += 6;
    const plot: Box = { x: l, y: tp, w: Math.max(10, r - l), h: Math.max(10, b - tp) };
    const y = (v: number) => plot.y + plot.h - ((v - ticks.min) / (ticks.max - ticks.min || 1)) * plot.h;
    const band = plot.w / n;

    // Axis titles.
    if (ax.catTitle) this.add(el("text", { class: "chart-t chart-axis-title", x: plot.x + plot.w / 2, y: this.bottom - 4, "text-anchor": "middle" }, ax.catTitle));
    if (ax.valTitle) {
      const x = this.left + AXIS_TITLE_FONT * 0.8;
      const yy = plot.y + plot.h / 2;
      this.add(el("text", { class: "chart-t chart-axis-title", x, y: yy, "text-anchor": "middle", transform: `rotate(-90 ${x} ${yy})` }, ax.valTitle));
    }
    // Gridlines and value labels.
    const grid = el("g", { class: "chart-gridlines" });
    const labels = el("g", {});
    for (const v of ticks.values) {
      const p = y(v);
      if (ax.grid) grid.append(el("line", { class: "chart-grid", x1: plot.x, y1: p, x2: plot.x + plot.w, y2: p }));
      if (!ax.valHidden) labels.append(el("text", { class: "chart-t", x: plot.x - 8, y: p + FONT * 0.35, "text-anchor": "end" }, formatNumber(v, valFmt)));
    }
    if (secondary) for (const v of secondary.values) labels.append(el("text", { class: "chart-t", x: plot.x + plot.w + 8, y: plot.y + plot.h - ((v - secondary.min) / (secondary.max - secondary.min || 1)) * plot.h + FONT * 0.35, "text-anchor": "start" }, formatNumber(v, secFmt)));
    this.add(grid, labels);
    // The category axis runs along zero, or the bottom edge when zero is off the scale.
    const zero = y(Math.min(Math.max(0, ticks.min), ticks.max));
    this.add(el("line", { class: "chart-axis", x1: plot.x, y1: zero, x2: plot.x + plot.w, y2: zero }));
    // Category labels.
    if (showCats) {
      const lab = el("g", { class: "chartex-cats" });
      for (let i = 0; i < cats.length; i += skip) {
        const c = plot.x + (i + 0.5) * band;
        const ly = plot.y + plot.h;
        if (rotate) lab.append(el("text", { class: "chart-t", x: c, y: ly + 10, "text-anchor": "end", transform: `rotate(-45 ${c} ${ly + 10})` }, cats[i]));
        else lab.append(el("text", { class: "chart-t", x: c, y: ly + FONT + 6, "text-anchor": "middle" }, cats[i]));
      }
      this.add(lab);
    }
    return { plot, band, y, x0: (i) => plot.x + i * band, ticks };
  }

  private valueTicks(lo: number, hi: number, includeZero: boolean): Ticks {
    const ax = this.m.axes;
    const [a, b] = autoRange(lo, hi, includeZero);
    return niceTicks(a, b, 5, { min: ax.valMin, max: ax.valMax, step: ax.valMajor });
  }

  /** A data label above (or below) a bar. */
  private valueLabel(text: string, x: number, y: number, cls = ""): SVGTextElement {
    return el("text", { class: `chart-t chart-label${cls}`, x, y, "text-anchor": "middle" }, text);
  }

  // -- waterfall -----------------------------------------------------------------------------------

  private waterfall() {
    const m = this.m;
    const pts = m.waterfall!.points;
    const [lo, hi] = waterfallRange(pts);
    const f = this.frame(this.valueTicks(lo, hi, true), pts.map((p) => p.label));
    const slot = barSlot(f.band, m.axes.gapWidth ?? 0.5);
    const kindIndex = { increase: 0, decrease: 1, total: 2 } as const;
    const bars = el("g", { class: "chartex-bars" });
    const conn = el("g", { class: "chartex-connectors" });
    const labels = el("g", { class: "chart-labels" });
    pts.forEach((p, i) => {
      const x = f.x0(i) + slot.offset;
      const y0 = f.y(p.start);
      const y1 = f.y(p.end);
      const fill = hex(m.pointColors?.[i]) ?? this.color(kindIndex[p.kind]);
      bars.append(
        el("rect", { class: "chartex-bar", x, y: Math.min(y0, y1), width: slot.width, height: Math.abs(y1 - y0), fill, "data-i": i, "data-kind": p.kind }, tip(`${p.label}: ${formatNumber(p.value, this.fmt)}`)),
      );
      if (m.waterfall!.connectors && i < pts.length - 1) {
        conn.append(el("line", { class: "chartex-connector", x1: x + slot.width, y1: y1, x2: f.x0(i + 1) + slot.offset, y2: y1 }));
      }
      if (m.dataLabels && m.labelValues) {
        const up = p.kind !== "decrease";
        labels.append(this.valueLabel(formatNumber(p.value, this.fmt), x + slot.width / 2, up ? Math.min(y0, y1) - 4 : Math.max(y0, y1) + FONT + 2));
      }
    });
    this.add(conn, bars, labels);
  }

  // -- histogram and Pareto ----------------------------------------------------------------------

  private histogram() {
    const m = this.m;
    const h = m.histogram!;
    const bins: HistogramBin[] = h.bins;
    const cats = bins.map((b) => binLabel(b, this.fmt));
    const ticks = countTicks(Math.max(0, ...bins.map((b) => b.count)), { min: m.axes.valMin, max: m.axes.valMax, step: m.axes.valMajor });
    const pareto = h.cumulative && m.kind === "pareto" ? niceTicks(0, 1, 5, { min: 0, max: 1, step: 0.2 }) : undefined;
    // The axis counts bars; only a sum of the values (by category) is in the data's number format.
    const countFmt = h.byCategory ? this.fmt : "General";
    const f = this.frame(ticks, cats, pareto, "0%", countFmt);
    const slot = barSlot(f.band, m.axes.gapWidth ?? 0);
    const fill = this.seriesColor();
    const bars = el("g", { class: "chartex-bars" });
    const labels = el("g", { class: "chart-labels" });
    bins.forEach((b, i) => {
      const x = f.x0(i) + slot.offset;
      const top = f.y(b.count);
      const base = f.y(Math.max(0, ticks.min));
      bars.append(el("rect", { class: "chartex-bar", x, y: top, width: slot.width, height: Math.max(0, base - top), fill, "data-i": i, "data-kind": b.kind }, tip(`${cats[i]}: ${formatNumber(b.count, countFmt)}`)));
      if (m.dataLabels && m.labelValues) labels.append(this.valueLabel(formatNumber(b.count, countFmt), x + slot.width / 2, top - 4));
    });
    this.add(bars, labels);
    if (pareto && h.cumulative) {
      const col = hex(h.lineColor) ?? this.color(1);
      const py = (v: number) => f.plot.y + f.plot.h - ((v - pareto.min) / (pareto.max - pareto.min || 1)) * f.plot.h;
      const pts = h.cumulative.map((v, i) => ({ x: f.x0(i) + f.band / 2, y: py(v), v }));
      const g = el("g", { class: "chartex-pareto" });
      g.append(el("path", { class: "chart-stroke", d: "M" + pts.map((p) => `${p.x} ${p.y}`).join("L"), stroke: col, fill: "none" }));
      pts.forEach((p, i) => g.append(el("circle", { class: "chart-marker", cx: p.x, cy: p.y, r: 3.5, fill: col, "data-i": i }, tip(`${cats[i]}: ${formatNumber(p.v, "0.0%")}`))));
      this.add(g);
    }
  }

  // -- box and whisker -----------------------------------------------------------------------------

  private boxes() {
    const m = this.m;
    const bx = m.boxes!;
    const stats = bx.series.flatMap((s) => s.boxes.filter((b): b is BoxStat => !!b));
    const vals = stats.flatMap((b) => [b.whiskerLo, b.whiskerHi, b.q1, b.q3, ...(bx.outliers ? b.outliers : [])]);
    const f = this.frame(this.valueTicks(Math.min(...vals), Math.max(...vals), false), bx.categories);
    const k = bx.series.length;
    const slot = barSlot(f.band, m.axes.gapWidth ?? 1, k);
    const g = el("g", { class: "chartex-boxes" });
    bx.series.forEach((s, si) => {
      const fill = hex(s.color) ?? this.color(si);
      const means: { x: number; y: number }[] = [];
      s.boxes.forEach((b, ci) => {
        if (!b) return;
        const x = f.x0(ci) + slot.offset + si * slot.width;
        const cx = x + slot.width / 2;
        const label = `${bx.categories[ci] || s.name}`;
        const yq1 = f.y(b.q1);
        const yq3 = f.y(b.q3);
        const cap = slot.width * 0.25;
        const box = el("g", { class: "chartex-box", "data-c": ci, "data-s": si });
        // Whiskers, then the box over them.
        box.append(
          el("line", { class: "chartex-whisker", x1: cx, y1: yq3, x2: cx, y2: f.y(b.whiskerHi) }),
          el("line", { class: "chartex-whisker", x1: cx - cap, y1: f.y(b.whiskerHi), x2: cx + cap, y2: f.y(b.whiskerHi) }),
          el("line", { class: "chartex-whisker", x1: cx, y1: yq1, x2: cx, y2: f.y(b.whiskerLo) }),
          el("line", { class: "chartex-whisker", x1: cx - cap, y1: f.y(b.whiskerLo), x2: cx + cap, y2: f.y(b.whiskerLo) }),
          el(
            "rect",
            { class: "chartex-box-body", x, y: Math.min(yq1, yq3), width: slot.width, height: Math.abs(yq1 - yq3), fill },
            tip(`${label}: ${t("median {median}, quartiles {q1} to {q3}", { median: formatNumber(b.median, this.fmt), q1: formatNumber(b.q1, this.fmt), q3: formatNumber(b.q3, this.fmt) })}`),
          ),
          el("line", { class: "chartex-median", x1: x, y1: f.y(b.median), x2: x + slot.width, y2: f.y(b.median) }),
        );
        if (bx.points) for (const v of b.points) box.append(el("circle", { class: "chartex-point", cx, cy: f.y(v), r: 2.5 }));
        if (bx.outliers) for (const v of b.outliers) box.append(el("circle", { class: "chartex-outlier", cx, cy: f.y(v), r: 3, stroke: fill }, tip(`${general(v)}`)));
        if (bx.meanMarker) {
          const my = f.y(b.mean);
          box.append(el("path", { class: "chartex-mean", d: `M${cx - 4} ${my - 4}L${cx + 4} ${my + 4}M${cx - 4} ${my + 4}L${cx + 4} ${my - 4}` }));
        }
        means.push({ x: cx, y: f.y(b.mean) });
        g.append(box);
      });
      if (bx.meanLine && means.length > 1) g.append(el("path", { class: "chartex-mean-line", d: "M" + means.map((p) => `${p.x} ${p.y}`).join("L"), stroke: fill }));
    });
    this.add(g);
  }

  // -- funnel ----------------------------------------------------------------------------------------

  private funnel() {
    const m = this.m;
    const pts = m.funnel!;
    const max = Math.max(...pts.map((p) => p.value), 1e-9);
    const labelW = Math.min((this.right - this.left) * 0.4, Math.max(...pts.map((p) => textWidth(p.label, FONT))) + 12);
    const plot: Box = { x: this.left + labelW, y: this.top + 4, w: Math.max(10, this.right - this.left - labelW), h: Math.max(10, this.bottom - this.top - 8) };
    const band = plot.h / pts.length;
    const gap = m.axes.gapWidth ?? 0.06;
    const barH = band / (1 + gap);
    const fill = this.seriesColor();
    const hex = m.color ?? accentHex(m, 0);
    const g = el("g", { class: "chartex-bars" });
    const labels = el("g", { class: "chart-labels" });
    pts.forEach((p, i) => {
      const w = Math.max(0, (p.value / max) * plot.w);
      const y = plot.y + i * band + (band - barH) / 2;
      g.append(el("rect", { class: "chartex-bar", x: plot.x + (plot.w - w) / 2, y, width: w, height: barH, fill, "data-i": i }, tip(`${p.label}: ${formatNumber(p.value, this.fmt)}`)));
      if (!m.axes.catHidden) labels.append(el("text", { class: "chart-t", x: plot.x - 8, y: y + barH / 2 + FONT * 0.35, "text-anchor": "end" }, fit(p.label, labelW - 12)));
      if (m.dataLabels && m.labelValues) {
        const text = formatNumber(p.value, this.fmt);
        const inside = textWidth(text, FONT) + 6 <= w;
        labels.append(el("text", { class: inside ? `chart-t chartex-on-fill${wantsLightText(hex) ? " light" : ""}` : "chart-t chart-label", x: plot.x + plot.w / 2, y: y + barH / 2 + FONT * 0.35, "text-anchor": "middle" }, text));
      }
    });
    this.add(g, labels);
  }

  // -- treemap ---------------------------------------------------------------------------------------

  private labelText(name: string, value: number): string[] {
    const m = this.m;
    const out: string[] = [];
    if (m.labelNames) out.push(name);
    if (m.labelValues && m.dataLabels) out.push(formatNumber(value, this.fmt));
    return out;
  }

  private treemap() {
    const m = this.m;
    const box: Rect = { x: this.left, y: this.top + 2, w: Math.max(10, this.right - this.left), h: Math.max(10, this.bottom - this.top - 2) };
    const mode = m.parentLabels ?? "banner";
    const tiles = treemapLayout(m.tree!, box, { parentLabels: mode, banner: 18, gap: 1.5 });
    const g = el("g", { class: "chartex-tiles" });
    const labels = el("g", { class: "chart-labels" });
    const lone = m.tree!.children?.every((c) => !c.children?.length);
    for (const tile of tiles) {
      const level = lone ? 0 : tile.depth - 1;
      const base = this.color(tile.branch);
      const fill = tint(base, level);
      const hex = tintHex(accentHex(m, tile.branch), level);
      const r = tile.rect;
      g.append(
        el("rect", { class: `chart-slice chartex-tile${tile.leaf ? "" : " parent"}`, x: r.x, y: r.y, width: r.w, height: r.h, fill, "data-depth": tile.depth, "data-name": tile.node.name, "data-value": tile.node.value }, tip(`${tile.path.join(" / ")}: ${formatNumber(tile.node.value, this.fmt)}`)),
      );
      const lines = this.labelText(tile.node.name, tile.node.value);
      const lab = tile.label;
      if (!lines.length || !lab) continue;
      const dark = wantsLightText(hex);
      if (tile.leaf) {
        // Name on the first line, the value under it, when they fit.
        lines.slice(0, Math.max(0, Math.floor((lab.h - 2) / (FONT + 2)))).forEach((line, i) => {
          const text = fit(line, lab.w - 8);
          if (text) labels.append(el("text", { class: `chart-t chartex-on-fill${dark ? " light" : ""}`, x: lab.x + 4, y: lab.y + FONT + 1 + i * (FONT + 2), "text-anchor": "start" }, text));
        });
      } else if (lab.h >= FONT + 4 && m.labelNames) {
        const text = fit(tile.node.name, lab.w - 8);
        if (text) labels.append(el("text", { class: `chart-t chartex-on-fill chartex-parent-name${dark ? " light" : ""}`, x: lab.x + 4, y: lab.y + FONT + 1, "text-anchor": "start" }, text));
      }
    }
    this.add(g, labels);
  }

  // -- sunburst --------------------------------------------------------------------------------------

  private sunburst() {
    const m = this.m;
    const cx = (this.left + this.right) / 2;
    const cy = (this.top + this.bottom) / 2;
    const rad = Math.max(10, Math.min((this.right - this.left) / 2, (this.bottom - this.top) / 2) - 4);
    const arcs = sunburstLayout(m.tree!, rad);
    const g = el("g", { class: "chartex-arcs" });
    const labels = el("g", { class: "chart-labels" });
    for (const a of arcs) {
      const level = a.depth - 1;
      const fill = tint(this.color(a.branch), level);
      const hex = tintHex(accentHex(m, a.branch), level);
      g.append(
        el("path", { class: "chart-slice chartex-arc", d: arcPath(cx, cy, a.r0, a.r1, a.a0, a.a1), fill, "fill-rule": "evenodd", "data-depth": a.depth, "data-name": a.node.name, "data-value": a.node.value, "data-a0": String(a.a0), "data-a1": String(a.a1) }, tip(`${a.path.join(" / ")}: ${formatNumber(a.node.value, this.fmt)}`)),
      );
      if (!m.dataLabels || !m.labelNames) continue;
      const lab = arcLabel(a, cx, cy, FONT, a.node.name);
      if (lab) labels.append(el("text", { class: `chart-t chartex-on-fill${wantsLightText(hex) ? " light" : ""}`, x: lab.x, y: lab.y + FONT * 0.35, "text-anchor": "middle", transform: `rotate(${lab.rotate} ${lab.x} ${lab.y})` }, lab.text));
    }
    this.add(g, labels);
  }
}
