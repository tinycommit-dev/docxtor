// What every chart drawing shares: SVG element helper, sizes, colours and text measuring.
import type { ChartModel, ChartSeries } from "@docxtor/core";
import { hex } from "../editor/color.ts";

export const NS = "http://www.w3.org/2000/svg";
export const PT_PX = 96 / 72;
export const FONT = 12; // 9 pt: Word's axis and legend text
export const TITLE_FONT = 18.67; // 14 pt
export const AXIS_TITLE_FONT = 12;
export const PAD = 10;
export const LEGEND_ROW = 18;

export type Attrs = Record<string, string | number | undefined>;

export function el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Attrs = {}, ...kids: (Node | string | null | undefined | false)[]): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v !== undefined) e.setAttribute(k, typeof v === "number" ? String(Math.round(v * 100) / 100) : v);
  for (const c of kids) if (c) e.append(c);
  return e;
}

/** Text width in px, close enough to lay out labels before the browser has measured them. */
export function textWidth(s: string, size = FONT): number {
  let w = 0;
  for (const ch of s) w += /[A-Z0-9%$€£MW]/.test(ch) ? 0.62 : /[ilI.,:;'!|1 ]/.test(ch) ? 0.3 : /[⺀-鿿가-힯]/.test(ch) ? 1 : 0.52;
  return w * size;
}

export const finite = (v: number | null | undefined): v is number => typeof v === "number" && Number.isFinite(v);

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function tip(label: string): SVGTitleElement {
  const t = document.createElementNS(NS, "title");
  t.textContent = label;
  return t;
}

/** A series' colour: its own, else the theme's accent for its place (with Word's variations after six), as a CSS colour. */
export function color(m: ChartModel, i: number, s?: ChartSeries): string {
  const own = hex(s?.color);
  if (own) return own;
  const accent = (m.accents && m.accents.length >= 6 ? hex(m.accents[i % 6]) : undefined) ?? `var(--chart-${(i % 6) + 1})`;
  if (i < 6) return accent;
  return i < 12 ? `color-mix(in srgb, ${accent} 60%, black)` : `color-mix(in srgb, ${accent} 70%, white)`;
}

export function sliceColor(m: ChartModel, s: ChartSeries, i: number): string {
  return hex(s.pointColors?.[i]) ?? color(m, i);
}

/** Whether a fill given as `#rrggbb` is dark enough to want light text on it (colours from the theme's tokens aren't known here). */
export function isDark(fill: string): boolean {
  const m = /^#([0-9a-f]{6})$/i.exec(fill);
  if (!m) return false;
  const n = parseInt(m[1]!, 16);
  const lin = (c: number) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const lum = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
  // Light text (white) beats the chart's dark grey on a fill when the white contrast is higher.
  return 1.05 / (lum + 0.05) > (lum + 0.05) / 0.15;
}

let gradients = 0;

/** What a bar, area or bubble of a series is painted with: its colour, or an outline, or a see-through fill, or a gradient. */
export function fillAttrs(svg: SVGSVGElement, s: ChartSeries, fill: string): Attrs {
  const width = s.lineWidth ? s.lineWidth * PT_PX : undefined;
  const line = hex(s.lineColor) ?? fill;
  switch (s.fill) {
    case "none":
      return { fill: "none", stroke: line, "stroke-width": width ?? 2.25 * PT_PX };
    case "soft":
      return { fill, "fill-opacity": 0.7, stroke: line, "stroke-width": width ?? 1 };
    case "gradient": {
      const id = `chart-grad-${++gradients}`;
      let defs = svg.querySelector("defs");
      if (!defs) {
        defs = el("defs");
        svg.prepend(defs);
      }
      defs.append(
        el(
          "linearGradient",
          { id, x1: 0, y1: 0, x2: 0, y2: 1 },
          el("stop", { offset: "0%", "stop-color": `color-mix(in srgb, ${fill} 60%, white)` }),
          el("stop", { offset: "100%", "stop-color": fill }),
        ),
      );
      return { fill: `url(#${id})`, ...(width ? { stroke: line, "stroke-width": width } : {}) };
    }
    default:
      return width ? { fill, stroke: line, "stroke-width": width } : { fill };
  }
}

/** A smooth curve through points (Catmull-Rom as Bézier segments). */
export function smoothPath(p: { x: number; y: number }[]): string {
  if (p.length < 3) return "M" + p.map((q) => `${q.x} ${q.y}`).join("L");
  let d = `M${p[0]!.x} ${p[0]!.y}`;
  for (let i = 0; i < p.length - 1; i++) {
    const p0 = p[i - 1] ?? p[i]!;
    const p1 = p[i]!;
    const p2 = p[i + 1]!;
    const p3 = p[i + 2] ?? p2;
    d += `C${p1.x + (p2.x - p0.x) / 6} ${p1.y + (p2.y - p0.y) / 6} ${p2.x - (p3.x - p1.x) / 6} ${p2.y - (p3.y - p1.y) / 6} ${p2.x} ${p2.y}`;
  }
  return d;
}
