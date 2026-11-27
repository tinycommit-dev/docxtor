// What the Edit chart dialog works on: the data as typed (strings, like a spreadsheet) and a copy of the chart's
// model for everything about how it looks. The preview is drawn from the pair; Apply turns the difference from
// the chart as it was into one `set_chart` op, so one undo takes it back.
import type { ChartGrouping, ChartKind, ChartModel, ChartOptions, ChartSeries, StockType, SurfaceType } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { resolveColor } from "./colors.ts";

export type Kind = Exclude<ChartKind, "other">;

export interface DSeries {
  name: string;
  /** Scatter and bubble charts: the X value of each point, as typed. */
  x: string[];
  /** The values as typed. */
  y: string[];
  /** Bubble charts: the sizes as typed. */
  size: string[];
}

export interface Draft {
  kind: Kind;
  grouping: ChartGrouping;
  title: string;
  categories: string[];
  series: DSeries[];
  /** Everything else: legend, axes, labels, colours, trendlines ... (series line up with `series`). */
  f: ChartModel;
}

export const isXY = (k: ChartKind): boolean => k === "scatter" || k === "bubble";
export const isStackable = (k: ChartKind): boolean => k === "column" || k === "bar" || k === "line" || k === "area";
export const COMBINABLE: Kind[] = ["column", "bar", "line", "area", "scatter"];

/** A number as people type it: `1,234.5`, `3,5`, `12 %` (blank is a gap). NaN when it isn't one. */
export function parseNumber(s: string): number | null {
  let v = s.trim();
  if (!v) return null;
  v = v.replace(/\s/g, "").replace(/%$/, "");
  if (/^-?\d{1,3}(,\d{3})+(\.\d+)?$/.test(v)) v = v.replace(/,/g, "");
  else if (/^-?\d+,\d+$/.test(v)) v = v.replace(",", ".");
  const n = Number(v);
  return Number.isFinite(n) && /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(v) ? n : Number.NaN;
}

const fmt = (v: number | null | undefined): string => (v === null || v === undefined ? "" : String(v));

export function draftOf(m: ChartModel): Draft {
  const xy = isXY(m.kind);
  const rows = xy ? Math.max(0, ...m.series.map((s) => s.values.length)) : m.categories.length;
  const kind: Kind = m.kind === "other" ? "column" : m.kind;
  return {
    kind,
    grouping: m.grouping,
    title: m.title ?? "",
    categories: xy ? [] : Array.from({ length: rows }, (_, i) => m.categories[i] ?? ""),
    series: m.series.map((s) => ({
      name: s.name,
      x: xy ? Array.from({ length: rows }, (_, i) => fmt(s.xValues?.[i])) : [],
      y: Array.from({ length: rows }, (_, i) => fmt(s.values[i])),
      size: m.kind === "bubble" ? Array.from({ length: rows }, (_, i) => fmt(s.sizes?.[i])) : [],
    })),
    f: structuredClone(m),
  };
}

export const rowsOf = (d: Draft): number => (isXY(d.kind) ? Math.max(0, ...d.series.map((s) => s.y.length)) : d.categories.length);

/** The roles of the series of a stock chart, in order. */
const STOCK_NAMES = () => ({ v: t("Volume"), o: t("Open"), h: t("High"), l: t("Low"), c: t("Close") });

export function stockRoles(type: StockType): string[] {
  const names = STOCK_NAMES();
  return [...(type.startsWith("v") ? [names.v] : []), ...(type.endsWith("ohlc") ? [names.o] : []), names.h, names.l, names.c];
}

export const STOCK_TYPES: { type: StockType; label: () => string }[] = [
  { type: "hlc", label: () => t("High-low-close") },
  { type: "ohlc", label: () => t("Open-high-low-close") },
  { type: "vhlc", label: () => t("Volume-high-low-close") },
  { type: "vohlc", label: () => t("Volume-open-high-low-close") },
];

export const SURFACE_TYPES: { type: SurfaceType; label: () => string }[] = [
  { type: "surface", label: () => t("3-D surface") },
  { type: "wireframe", label: () => t("Wireframe 3-D surface") },
  { type: "contour", label: () => t("Contour") },
  { type: "wireframe_contour", label: () => t("Wireframe contour") },
];

const blankSeries = (name: string, rows: number, xy: boolean): DSeries => ({ name, x: xy ? Array.from({ length: rows }, (_, i) => String(i + 1)) : [], y: Array.from({ length: rows }, () => ""), size: [] });

/** A new series for the draft, formatted like the last one without what would repeat (its colour, lines). */
export function addSeries(d: Draft, name: string): DSeries {
  const rows = rowsOf(d);
  const s = blankSeries(name, rows, isXY(d.kind));
  if (d.kind === "bubble") s.size = Array.from({ length: rows }, () => "1");
  if (isXY(d.kind) && d.series[0]) s.x = [...d.series[0].x];
  d.series.push(s);
  const last = d.f.series[d.f.series.length - 1];
  const copy: ChartSeries = last ? { ...structuredClone(last), name, values: [], color: undefined, pointColors: undefined, trendlines: undefined, errorBars: undefined, nameRef: undefined, catRef: undefined, valRef: undefined, sizeRef: undefined } : { name, values: [], kind: d.kind };
  d.f.series.push(copy);
  return s;
}

export function removeSeries(d: Draft, i: number) {
  d.series.splice(i, 1);
  d.f.series.splice(i, 1);
}

/** Make the series of a draft the ones a stock type has, keeping the ones whose names match. */
export function setStockType(d: Draft, type: StockType) {
  const roles = stockRoles(type);
  const names = STOCK_NAMES();
  const rows = Math.max(1, rowsOf(d));
  const old = d.series.map((s, i) => ({ s, f: d.f.series[i]! }));
  const taken = new Set<number>();
  const next: { s: DSeries; f: ChartSeries }[] = roles.map((role, k) => {
    let at = old.findIndex((o, i) => !taken.has(i) && o.s.name.trim().toLowerCase() === role.toLowerCase());
    // Names that don't match: take the series in order.
    if (at < 0 && old[k] && !taken.has(k) && old.length === roles.length) at = k;
    if (at >= 0) taken.add(at);
    const found = at >= 0 ? old[at]! : null;
    // A role the chart didn't have starts from the closing prices, so there is something to look at.
    const seed = old.find((o) => o.s.name.trim().toLowerCase() === t("Close").toLowerCase())?.s ?? old[old.length - 1]?.s;
    const fresh = blankSeries(role, rows, false);
    if (seed && role !== names.v) fresh.y = Array.from({ length: rows }, (_, r) => seed.y[r] ?? "");
    const s: DSeries = found ? { ...found.s, name: role } : fresh;
    const f: ChartSeries = found ? { ...found.f, name: role } : { name: role, values: [], kind: "stock" };
    return { s, f };
  });
  d.series = next.map((n) => n.s);
  d.f.series = next.map((n, i) => ({ ...n.f, kind: type.startsWith("v") && i === 0 ? "column" : "stock", secondary: type.startsWith("v") && i > 0 }));
  d.f.stock = type;
}

/** Change the type in the draft. Scatter-like charts' X columns and a category chart's labels stand in for each other. */
export function switchKind(d: Draft, next: Kind) {
  if (next === d.kind && !isCombo(d)) return;
  const rows = Math.max(rowsOf(d), 1);
  const wasKind = d.kind;
  const wasXY = isXY(d.kind);
  const toXY = isXY(next);
  if (toXY && !wasXY) {
    for (const s of d.series) s.x = Array.from({ length: rows }, (_, i) => String(i + 1));
    d.categories = [];
  } else if (!toXY && wasXY) {
    d.categories = Array.from({ length: rows }, (_, i) => d.series[0]?.x[i] ?? "");
    for (const s of d.series) s.x = [];
  }
  for (const s of d.series) s.size = next === "bubble" ? (s.size.length ? s.size : Array.from({ length: rows }, () => "1")) : [];
  d.kind = next;
  d.f.kind = next;
  d.f.combo = false;
  for (const s of d.f.series) {
    s.kind = next;
    s.secondary = false;
    s.noLine = undefined;
    s.marker = next === "line" || next === "scatter" || next === "radar" ? (wasKind === next ? s.marker : true) : undefined;
  }
  if (!isStackable(next)) d.grouping = "clustered";
  if ((next === "pie" || next === "doughnut") && !d.f.legend) d.f.legend = { position: "r" };
  if (next === "stock") {
    d.f.stock = undefined;
    setStockType(d, d.series.length >= 5 ? "vohlc" : d.series.length === 4 ? "ohlc" : "hlc");
    if (d.series.length > 5) d.series.length = 5;
  } else {
    d.f.stock = undefined;
  }
  d.f.surface = next === "surface" ? (d.f.surface ?? "surface") : undefined;
  if (next === "bubble") d.f.bubbleScale ??= 100;
}

/** Whether the series of the draft have different types (a combination chart). */
export const isCombo = (d: Draft): boolean => d.kind !== "stock" && d.f.series.some((s) => s.kind !== d.f.series[0]?.kind || !!s.secondary !== !!d.f.series[0]?.secondary);

/** The chart as the draft would make it, for the preview: colours are resolved against the theme. */
export function previewModel(d: Draft): ChartModel {
  const xy = isXY(d.kind);
  const numbers = (list: string[]) =>
    list.map((s) => {
      const n = parseNumber(s);
      return n === null || Number.isNaN(n) ? null : n;
    });
  const theme = d.f.theme;
  const rc = (c: string | undefined) => (c ? resolveColor(c, theme) : undefined);
  const series: ChartSeries[] = d.series.map((s, i) => {
    const f = d.f.series[i] ?? ({ name: "", values: [], kind: d.kind } as ChartSeries);
    return {
      ...f,
      name: s.name,
      values: numbers(s.y),
      xValues: xy ? numbers(s.x) : undefined,
      sizes: d.kind === "bubble" ? numbers(s.size) : undefined,
      color: rc(f.color),
      lineColor: rc(f.lineColor),
      pointColors: f.pointColors?.map((c) => (c ? (rc(c) ?? null) : null)) as (string | null)[] | undefined,
    };
  });
  return {
    ...d.f,
    kind: d.kind,
    combo: isCombo(d),
    grouping: isStackable(d.kind) ? d.grouping : "clustered",
    title: d.title || undefined,
    categories: xy ? [] : d.categories,
    series,
    dataLabels: series.some((s) => s.labels),
  };
}

// ---------------------------------------------------------------------------
// The options a draft asks for
// ---------------------------------------------------------------------------

type Json = unknown;

const isObj = (v: Json): v is Record<string, Json> => typeof v === "object" && v !== null && !Array.isArray(v);

function same(a: Json, b: Json): boolean {
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((x, i) => same(x, b[i]));
  if (isObj(a) && isObj(b)) {
    const ka = Object.keys(a);
    return ka.length === Object.keys(b).length && ka.every((k) => k in b && same(a[k], b[k]));
  }
  return false;
}

/** Drop what means nothing (false, empty, undefined), so a missing setting and an off one compare equal. */
function strip(v: Json): Json {
  if (Array.isArray(v)) return v.map(strip);
  if (isObj(v)) {
    const out: Record<string, Json> = {};
    for (const [k, x] of Object.entries(v)) {
      const s = strip(x);
      if (s === undefined || s === null || s === false || (Array.isArray(s) && s.length === 0 && k !== "trendlines" && k !== "errorBars") || (isObj(s) && Object.keys(s).length === 0)) continue;
      out[k] = s;
    }
    return out;
  }
  return v;
}

/** The merge patch that turns `a` into `b` (null removes a setting); undefined when they are the same. */
export function jsonDiff(a: Json, b: Json): Json | undefined {
  if (same(a, b)) return undefined;
  if (isObj(a) && isObj(b)) {
    const out: Record<string, Json> = {};
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
      if (!(k in b)) out[k] = null;
      else {
        const d = jsonDiff(a[k], b[k]);
        if (d !== undefined) out[k] = d;
      }
    }
    return out;
  }
  return b === undefined ? null : b;
}

const SERIES_KEYS = ["color", "pointColors", "fill", "lineWidth", "lineColor", "labels", "trendlines", "errorBars", "marker", "smooth", "noLine", "kind", "secondary"] as const;
const TOP_KEYS = ["legendOverlay", "titleOverlay", "gapWidth", "overlap", "bubbleScale", "sizeRepresents", "showNegativeBubbles", "stock", "surface", "hole", "varyColors", "style"] as const;

/**
 * The options that turn the chart as it was (`base`) into the draft's formatting. Nothing for what didn't change.
 * `palette` is sent as the palette's name too, so the engine writes the palette part.
 */
export function diffOptions(base: ChartModel, d: Draft): ChartOptions | undefined {
  const f = d.f;
  const o: Record<string, Json> = {};
  for (const k of TOP_KEYS) {
    const dv = jsonDiff(strip((base as unknown as Record<string, Json>)[k]), strip((f as unknown as Record<string, Json>)[k]));
    if (dv !== undefined) o[k] = (f as unknown as Record<string, Json>)[k] ?? null;
  }
  if (!same(base.legend ?? null, f.legend ?? null)) o.legend = f.legend ?? null;
  for (const k of ["xAxis", "yAxis", "y2Axis"] as const) {
    const dv = jsonDiff(strip(base[k] ?? {}), strip(f[k] ?? {}));
    if (dv !== undefined && isObj(dv) && Object.keys(dv).length) o[k] = dv;
  }
  const series: Record<string, Json>[] = [];
  const last = base.series[base.series.length - 1] as unknown as Record<string, Json> | undefined;
  // A series added in the dialog starts as a copy of the last one without its colour, outline and lines.
  const INHERITED = ["labels", "marker", "smooth", "kind", "secondary"];
  f.series.forEach((fs, i) => {
    const bs = base.series[i] as unknown as Record<string, Json> | undefined;
    const patch: Record<string, Json> = {};
    for (const k of SERIES_KEYS) {
      const ref = bs ? bs[k] : last && INHERITED.includes(k) ? last[k] : undefined;
      const mine = (fs as unknown as Record<string, Json>)[k];
      if (same(strip(ref) ?? null, strip(mine) ?? null)) continue;
      patch[k] = mine ?? null;
    }
    if (Object.keys(patch).length) series.push({ index: i, ...patch });
  });
  if (series.length) o.series = series;
  if (f.palette && f.palette !== base.palette) o.palette = f.palette;
  return Object.keys(o).length ? (o as ChartOptions) : undefined;
}
