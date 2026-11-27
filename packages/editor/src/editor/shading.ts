// Shading with a pattern, as Word has it: a fill, and over it a pattern (`pct25`, `diagStripe`, ...) drawn in a colour.
// CSS has no such thing, so the pattern is a small SVG tile repeated over the fill. `shdStyle` is the whole answer for
// a paragraph, a table cell or a run: pass what the engine reports (`shd`, with theme colours already resolved) and
// the flat colour (`shading`) it gives beside it.
import type { Shd } from "@docxtor/core";
import { hex as hexOf } from "./color.ts";

/** The pieces of a shading as CSS: the colour under the pattern, and the pattern (an image to repeat at `size`). */
export interface ShdPaint {
  color?: string;
  image?: string;
  size?: string;
}

/** 4 x 4 ordered-dither thresholds: a percentage pattern fills the cells under `round(percent * 16 / 100)`. */
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

const hex = (c?: string, fallback?: string) => hexOf(c) ?? fallback;

const url = (svg: string) => `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;

function tile(size: number, body: string): string {
  return url(`<svg xmlns='http://www.w3.org/2000/svg' width='${size}' height='${size}' viewBox='0 0 ${size} ${size}' shape-rendering='crispEdges'>${body}</svg>`);
}

/** Lines of `w` px every `period` px, horizontal, vertical or diagonal, in `c`. */
function lines(kind: "h" | "v" | "d" | "r", period: number, w: number, c: string): string {
  const s = period;
  const stroke = `stroke='${c}' stroke-width='${w}' fill='none'`;
  switch (kind) {
    case "h":
      return `<path d='M0 ${w / 2}H${s}' ${stroke}/>`;
    case "v":
      return `<path d='M${w / 2} 0V${s}' ${stroke}/>`;
    case "d":
      return `<path d='M0 0L${s} ${s}M${-s} 0L${s} ${s * 2}M0 ${-s}L${s * 2} ${s}' ${stroke} shape-rendering='geometricPrecision'/>`;
    default:
      return `<path d='M${s} 0L0 ${s}M${s * 2} 0L${-s} ${s * 2}M${s * 2} ${-s}L0 ${s}' ${stroke} shape-rendering='geometricPrecision'/>`;
  }
}

/** The pattern of a shading style over nothing: an image and its tile size, or null for a plain fill. */
function pattern(val: string, c: string): { image: string; size: number } | null {
  const pct = /^pct(\d+)$/.exec(val);
  if (pct) {
    const on = Math.round((Number(pct[1]) * 16) / 100);
    const cells = BAYER.map((t, i) => (t < on ? `<rect x='${i % 4}' y='${Math.floor(i / 4)}' width='1' height='1'/>` : "")).join("");
    return { image: tile(4, `<g fill='${c}'>${cells}</g>`), size: 4 };
  }
  switch (val) {
    case "horzStripe":
      return { image: tile(4, lines("h", 4, 2, c)), size: 4 };
    case "thinHorzStripe":
      return { image: tile(4, lines("h", 4, 1, c)), size: 4 };
    case "vertStripe":
      return { image: tile(4, lines("v", 4, 2, c)), size: 4 };
    case "thinVertStripe":
      return { image: tile(4, lines("v", 4, 1, c)), size: 4 };
    case "diagStripe":
      return { image: tile(8, lines("d", 8, 2.5, c)), size: 8 };
    case "thinDiagStripe":
      return { image: tile(4, lines("d", 4, 1, c)), size: 4 };
    case "reverseDiagStripe":
      return { image: tile(8, lines("r", 8, 2.5, c)), size: 8 };
    case "thinReverseDiagStripe":
      return { image: tile(4, lines("r", 4, 1, c)), size: 4 };
    case "horzCross":
      return { image: tile(8, lines("h", 8, 2, c) + lines("v", 8, 2, c)), size: 8 };
    case "thinHorzCross":
      return { image: tile(4, lines("h", 4, 1, c) + lines("v", 4, 1, c)), size: 4 };
    case "diagCross":
      return { image: tile(8, lines("d", 8, 2, c) + lines("r", 8, 2, c)), size: 8 };
    case "thinDiagCross":
      return { image: tile(4, lines("d", 4, 1, c) + lines("r", 4, 1, c)), size: 4 };
    default:
      return null;
  }
}

/** How a shading looks: `flat` is the plain colour the engine reports when it can't say more. */
export function shdPaint(shd?: Shd, flat?: string): ShdPaint {
  if (!shd) return { color: hex(flat) };
  const val = shd.val;
  if (val === "nil" || val === "none") return {};
  const fill = hex(shd.fill);
  if (val === "clear" || !val) return { color: fill ?? hex(flat) };
  const color = hex(shd.color, "#000000")!;
  if (val === "solid") return { color };
  const p = pattern(val, color);
  if (!p) return { color: fill ?? hex(flat) };
  return { color: fill, image: p.image, size: `${p.size}px ${p.size}px` };
}

/** A shading as CSS declarations (background-color, and the pattern over it), or "" for none. */
export function shdStyle(shd?: Shd, flat?: string): string {
  const p = shdPaint(shd, flat);
  const s: string[] = [];
  if (p.color) s.push(`background-color:${p.color}`);
  if (p.image) s.push(`background-image:${p.image}`, `background-size:${p.size}`);
  return s.join(";");
}
