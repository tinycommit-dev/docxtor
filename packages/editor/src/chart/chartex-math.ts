// The layout maths of the new chart types (waterfall, histogram, Pareto, box and whisker, funnel, treemap,
// sunburst): where bars, boxes, tiles and arcs go. Pure functions on numbers, no DOM, so they are tested in Node.
// The engine has already worked out the data (bins, quartiles, running totals, the tree).
import type { ChartExModel, HistogramBin, TreeNode, WaterfallPoint } from "@docxtor/core";
import { formatNumber, niceTicks, type Ticks } from "./format.ts";
import { hex } from "../editor/color.ts";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const finite = (v: number | null | undefined): v is number => typeof v === "number" && Number.isFinite(v);

// -- value axes ---------------------------------------------------------------------------------

/** The ends of the value axis of a waterfall: every bar start and end, and zero. */
export function waterfallRange(points: WaterfallPoint[]): [number, number] {
  let lo = 0;
  let hi = 0;
  for (const p of points) {
    lo = Math.min(lo, p.start, p.end);
    hi = Math.max(hi, p.start, p.end);
  }
  return [lo, hi];
}

/** Ticks for a count axis: whole numbers when the counts are small. */
export function countTicks(max: number, fixed: { min?: number; max?: number; step?: number } = {}): Ticks {
  const top = Math.max(1, max);
  return niceTicks(0, top, 5, { step: top <= 5 ? 1 : undefined, ...Object.fromEntries(Object.entries(fixed).filter(([, v]) => v !== undefined)) });
}

/** Where the bars of one slot go: the slot's `band` width is shared by the bar and the gap on either side (`gap` is
 * a fraction of the bar's width, as in `cx:catScaling gapWidth`). Returns the offset of the bar from the slot's start and its width. */
export function barSlot(band: number, gap: number, groups = 1): { offset: number; width: number } {
  const group = band / (1 + gap);
  return { offset: (band - group) / 2, width: group / Math.max(1, groups) };
}

// -- histogram labels ---------------------------------------------------------------------------

/** The label Excel puts under a bar of a histogram: `[1, 6]`, `(6, 11]`, `≤3`, `>12`, or the category. */
export function binLabel(b: HistogramBin, fmt?: string): string {
  if (b.kind === "category") return b.label ?? "";
  const f = (v: number | undefined) => (finite(v) ? formatNumber(v, fmt) : "");
  if (b.kind === "underflow") return `≤${f(b.hi)}`;
  if (b.kind === "overflow") return `>${f(b.lo)}`;
  return `${b.closedLo ? "[" : "("}${f(b.lo)}, ${f(b.hi)}${b.closedHi ? "]" : ")"}`;
}

// -- treemap ------------------------------------------------------------------------------------

function worst(row: number[], side: number): number {
  const sum = row.reduce((a, b) => a + b, 0);
  if (sum <= 0 || side <= 0) return Infinity;
  const max = Math.max(...row);
  const min = Math.min(...row);
  return Math.max((side * side * max) / (sum * sum), (sum * sum) / (side * side * min));
}

/**
 * Squarified treemap (Bruls, Huizing, van Wijk): rectangles for `values` (largest first) that fill `box`, each
 * with an area in proportion to its value and as close to a square as the order allows. Values that are zero or
 * less get an empty rectangle.
 */
export function squarify(values: number[], box: Rect): Rect[] {
  const out: Rect[] = values.map(() => ({ x: box.x, y: box.y, w: 0, h: 0 }));
  const items = values.map((v, i) => ({ v: finite(v) && v > 0 ? v : 0, i })).filter((it) => it.v > 0);
  const total = items.reduce((a, it) => a + it.v, 0);
  if (!total || box.w <= 0 || box.h <= 0) return out;
  const scale = (box.w * box.h) / total;
  let { x, y, w, h } = box;
  let at = 0;
  while (at < items.length) {
    const side = Math.min(w, h);
    const row = [items[at]!.v * scale];
    let end = at + 1;
    while (end < items.length) {
      const next = items[end]!.v * scale;
      if (worst([...row, next], side) > worst(row, side)) break;
      row.push(next);
      end++;
    }
    const sum = row.reduce((a, b) => a + b, 0);
    const last = end >= items.length;
    if (w >= h) {
      // A column down the left side; the last row takes what is left so the pieces fill the box exactly.
      const t = last ? w : Math.min(w, sum / h);
      let yy = y;
      row.forEach((a, k) => {
        const hh = last || k === row.length - 1 ? y + h - yy : a / t;
        out[items[at + k]!.i] = { x, y: yy, w: t, h: hh };
        yy += hh;
      });
      x += t;
      w -= t;
    } else {
      const t = last ? h : Math.min(h, sum / w);
      let xx = x;
      row.forEach((a, k) => {
        const ww = last || k === row.length - 1 ? x + w - xx : a / t;
        out[items[at + k]!.i] = { x: xx, y, w: ww, h: t };
        xx += ww;
      });
      y += t;
      h -= t;
    }
    at = end;
  }
  return out;
}

export interface TreeTile {
  node: TreeNode;
  /** 1 for the first level of names. */
  depth: number;
  /** The tile, including a parent's banner and its children. */
  rect: Rect;
  /** Where the name goes: a parent's banner, or the whole tile of a leaf. Null when a parent has no room for one. */
  label: Rect | null;
  leaf: boolean;
  /** Which first-level branch the tile belongs to (a leaf directly under the root counts as its own branch). */
  branch: number;
  path: string[];
}

export interface TreemapOptions {
  /** Where the names of parents go. */
  parentLabels: "banner" | "overlapping" | "none";
  /** Height of a parent's banner, in px. */
  banner: number;
  /** Space between tiles, in px. */
  gap: number;
}

const inset = (r: Rect, d: number): Rect => ({ x: r.x + d, y: r.y + d, w: Math.max(0, r.w - 2 * d), h: Math.max(0, r.h - 2 * d) });

/** Every node of a hierarchy as a tile in `box`: children share their parent's tile (below its banner, if it has one), largest first. */
export function treemapLayout(root: TreeNode, box: Rect, opts: TreemapOptions): TreeTile[] {
  const tiles: TreeTile[] = [];
  const place = (node: TreeNode, rect: Rect, depth: number, branch: number | null, path: string[]) => {
    const kids = (node.children ?? []).filter((c) => c.value > 0).sort((a, b) => b.value - a.value);
    if (!kids.length) return;
    const rects = squarify(
      kids.map((k) => k.value),
      rect,
    );
    kids.forEach((k, i) => {
      const r = rects[i]!;
      if (r.w <= 0 || r.h <= 0) return;
      const b = branch ?? root.children!.indexOf(k);
      const p = [...path, k.name];
      const leaf = !(k.children ?? []).some((c) => c.value > 0);
      if (leaf) {
        tiles.push({ node: k, depth, rect: r, label: inset(r, opts.gap), leaf, branch: b, path: p });
        return;
      }
      const body = inset(r, opts.gap);
      const head = opts.parentLabels === "banner" && body.h > opts.banner * 1.6 ? opts.banner : 0;
      tiles.push({ node: k, depth, rect: r, label: opts.parentLabels === "none" ? null : { x: body.x, y: body.y, w: body.w, h: head || opts.banner }, leaf, branch: b, path: p });
      place(k, { x: body.x, y: body.y + head, w: body.w, h: body.h - head }, depth + 1, b, p);
    });
  };
  place(root, box, 1, null, []);
  return tiles;
}

// -- sunburst -----------------------------------------------------------------------------------

export interface Arc {
  node: TreeNode;
  depth: number;
  /** Angles in radians, clockwise from `start`. */
  a0: number;
  a1: number;
  /** Radii of the ring. */
  r0: number;
  r1: number;
  branch: number;
  path: string[];
}

/** How many levels of names a tree has below its root. */
export function treeDepth(n: TreeNode): number {
  return (n.children ?? []).reduce((a, c) => Math.max(a, treeDepth(c) + 1), 0);
}

/** Rings for every node of a hierarchy: each level one ring thick, each node an arc in proportion to its value inside its parent's. */
export function sunburstLayout(root: TreeNode, radius: number, start = -Math.PI / 2): Arc[] {
  const levels = Math.max(1, treeDepth(root));
  const ring = radius / levels;
  const arcs: Arc[] = [];
  const place = (node: TreeNode, a0: number, a1: number, depth: number, branch: number | null, path: string[]) => {
    const kids = (node.children ?? []).filter((c) => c.value > 0);
    const sum = kids.reduce((a, c) => a + c.value, 0);
    if (!sum) return;
    // A parent that also holds a value of its own leaves that share of its arc empty.
    const span = (a1 - a0) * Math.min(1, sum / Math.max(node.value, sum));
    let at = a0;
    kids.forEach((k) => {
      const a = at + (span * k.value) / sum;
      const b = branch ?? root.children!.indexOf(k);
      const p = [...path, k.name];
      arcs.push({ node: k, depth, a0: at, a1: a, r0: (depth - 1) * ring, r1: depth * ring, branch: b, path: p });
      place(k, at, a, depth + 1, b, p);
      at = a;
    });
  };
  place(root, start, start + Math.PI * 2, 1, null, []);
  return arcs;
}

/** The path of an annular wedge (a pie slice when `r0` is zero). */
export function arcPath(cx: number, cy: number, r0: number, r1: number, a0: number, a1: number): string {
  const pt = (r: number, a: number) => `${cx + Math.cos(a) * r} ${cy + Math.sin(a) * r}`;
  if (a1 - a0 >= Math.PI * 2 - 1e-6) {
    const ring = (r: number, sweep: number) => `M${cx} ${cy - r}A${r} ${r} 0 1 ${sweep} ${cx} ${cy + r}A${r} ${r} 0 1 ${sweep} ${cx} ${cy - r}Z`;
    return r0 > 0.5 ? ring(r1, 1) + ring(r0, 0) : ring(r1, 1);
  }
  const large = a1 - a0 > Math.PI ? 1 : 0;
  if (r0 <= 0.5) return `M${cx} ${cy}L${pt(r1, a0)}A${r1} ${r1} 0 ${large} 1 ${pt(r1, a1)}Z`;
  return `M${pt(r1, a0)}A${r1} ${r1} 0 ${large} 1 ${pt(r1, a1)}L${pt(r0, a1)}A${r0} ${r0} 0 ${large} 0 ${pt(r0, a0)}Z`;
}

/** Text width in px, close enough to decide whether a label fits before the browser has measured it. */
export function textWidth(s: string, size: number): number {
  let w = 0;
  for (const ch of s) w += /[A-Z0-9%$€£MW]/.test(ch) ? 0.62 : /[ilI.,:;'!|1 ]/.test(ch) ? 0.3 : /[⺀-鿿가-힯]/.test(ch) ? 1 : 0.52;
  return w * size;
}

export interface ArcLabel {
  x: number;
  y: number;
  /** Rotation in degrees about (x, y). */
  rotate: number;
  text: string;
}

/**
 * Where an arc's name goes, or null when it does not fit: along the ring when the arc is long enough, along the
 * radius when the ring is thick enough, turned so that it reads left to right.
 */
export function arcLabel(a: Arc, cx: number, cy: number, size: number, text: string): ArcLabel | null {
  const mid = (a.a0 + a.a1) / 2;
  const rm = (a.r0 + a.r1) / 2;
  const thick = a.r1 - a.r0;
  const arcLen = (a.a1 - a.a0) * rm;
  const w = textWidth(text, size);
  const x = cx + Math.cos(mid) * rm;
  const y = cy + Math.sin(mid) * rm;
  const deg = (mid * 180) / Math.PI;
  const fits = (along: number, across: number) => along >= w + 6 && across >= size + 2;
  // Along the radius: reads outward; flipped on the left half so it is never upside down.
  if (fits(thick, arcLen)) return { x, y, rotate: Math.cos(mid) < 0 ? deg + 180 : deg, text };
  // Along the ring: tangent to the circle.
  if (fits(arcLen, thick)) {
    const tangent = deg + 90;
    return { x, y, rotate: Math.sin(mid) > 0 ? tangent + 180 : tangent, text };
  }
  return null;
}

// -- colours ------------------------------------------------------------------------------------

/** Word's Office palette, which the chart tokens `--chart-1` to `--chart-6` carry; used only to choose light or dark text. */
export const OFFICE_HEX = ["4472c4", "ed7d31", "a5a5a5", "ffc000", "5b9bd5", "70ad47"];

/** The colour of the i-th thing: the theme's accent, else the chart token; past six, darker then lighter. */
export function accentColor(m: Pick<ChartExModel, "accents">, i: number): string {
  const base = (m.accents && m.accents.length >= 6 ? hex(m.accents[i % 6]) : undefined) ?? `var(--chart-${(i % 6) + 1})`;
  if (i < 6) return base;
  return i < 12 ? `color-mix(in srgb, ${base} 60%, black)` : `color-mix(in srgb, ${base} 70%, white)`;
}

/** The hex of the i-th accent (to choose text colour); the chart tokens are Word's palette. */
export function accentHex(m: Pick<ChartExModel, "accents">, i: number): string {
  return m.accents && m.accents.length >= 6 ? m.accents[i % 6]! : OFFICE_HEX[i % 6]!;
}

/** A lighter shade of a colour for the levels inside a branch: the deeper, the lighter. */
export function tint(color: string, level: number): string {
  if (level <= 0) return color;
  const keep = Math.max(35, 100 - level * 22);
  return `color-mix(in srgb, ${color} ${keep}%, white)`;
}

/** Relative luminance of the page's ink (#201515), plus the 0.05 of the contrast ratio. */
const INK_LUMINANCE = 0.009 + 0.05;

/** Whether text on this hex colour should be white: its contrast beats that of the page's ink (`--paper-ink`). */
export function wantsLightText(hex: string): boolean {
  const m = /^([0-9a-f]{6})$/i.exec(hex);
  if (!m) return false;
  const n = parseInt(m[1]!, 16);
  const lin = (c: number) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const lum = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
  return 1.05 / (lum + 0.05) > (lum + 0.05) / INK_LUMINANCE;
}

/** The hex behind a tile's fill after `tint` mixed it with white, for the same choice. */
export function tintHex(hex: string, level: number): string {
  if (level <= 0) return hex;
  const keep = Math.max(35, 100 - level * 22) / 100;
  const n = parseInt(hex, 16);
  const mix = (c: number) => Math.round(c * keep + 255 * (1 - keep));
  const r = mix((n >> 16) & 255);
  const g = mix((n >> 8) & 255);
  const b = mix(n & 255);
  return [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
}
