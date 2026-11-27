// Border styles as Word draws them. CSS has solid, double, dotted, dashed, ridge, groove, inset and outset; every other
// style (triple, the thin-thick pairs, dot-dash, wave) is painted as an image over a border-wide band: the band is
// reserved with a transparent CSS border, and `Edge.layer` is the background layer that fills it (see `paraCss`).
import type { Border } from "@docxtor/core";
import { hex } from "./color.ts";

export const PT_PX = 96 / 72;

export type Side = "top" | "bottom" | "left" | "right";

/** A background layer that paints one edge: it covers the border band of the element (origin and clip: border box). */
export interface Layer {
  image: string;
  size: string;
  position: string;
  repeat: string;
}

/** One edge of a box: its width in px, the CSS border that reserves it, and the layer that paints it when CSS can't. */
export interface Edge {
  width: number;
  css: string;
  layer?: Layer;
}

const hexColor = (c?: string) => hex(c) ?? "#000";

/** The base line width of a border in CSS px: Word's eighths of a point, never thinner than a hairline. */
export function lineWidth(b: Border): number {
  return Math.max(0.75, (b.sz / 8) * PT_PX);
}

/** Lines and gaps in units of the line width, from the outside in. Even places are lines, odd places gaps. */
const STRIPES: Record<string, number[]> = {
  triple: [1, 1, 1, 1, 1],
  thinThickSmallGap: [1, 1, 2],
  thickThinSmallGap: [2, 1, 1],
  thinThickThinSmallGap: [1, 1, 2, 1, 1],
  thinThickMediumGap: [1, 2, 2],
  thickThinMediumGap: [2, 2, 1],
  thinThickThinMediumGap: [1, 2, 2, 2, 1],
  thinThickLargeGap: [1, 3, 2],
  thickThinLargeGap: [2, 3, 1],
  thinThickThinLargeGap: [1, 3, 2, 3, 1],
};

/** Dashes and dots along the edge, in units of the line width: line, gap, line, gap, ... */
const DASHES: Record<string, number[]> = {
  dashSmallGap: [4, 1],
  dotDash: [1, 2, 4, 2],
  dotDotDash: [1, 2, 1, 2, 4, 2],
  dashDotStroked: [4, 1, 1, 1],
};

/** The CSS border style and width of the styles CSS has (null for those that need a layer). */
export function simpleBorder(b: Border): { style: string; width: number } | null {
  const u = lineWidth(b);
  switch (b.val) {
    case "double":
      return { style: "double", width: Math.max(3, u * 3) };
    case "thick":
      return { style: "solid", width: Math.max(3, u * 2) };
    case "dotted":
      return { style: "dotted", width: u };
    case "dashed":
      return { style: "dashed", width: u };
    case "threeDEmboss":
      return { style: "ridge", width: Math.max(3, u * 2) };
    case "threeDEngrave":
      return { style: "groove", width: Math.max(3, u * 2) };
    case "outset":
    case "inset":
      return { style: b.val, width: Math.max(2, u * 2) };
    case "wave":
    case "doubleWave":
      return null;
    default:
      return STRIPES[b.val] || DASHES[b.val] ? null : { style: "solid", width: u };
  }
}

const svg = (w: number, h: number, body: string) =>
  `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='${h}' viewBox='0 0 ${w} ${h}'>${body}</svg>`)}")`;

/** Wavy lines: one sine wave across a band, or two. The tile is drawn along the edge's direction. */
function wave(side: Side, b: Border, c: string): Edge {
  const u = lineWidth(b);
  const double = b.val === "doubleWave";
  const period = Math.max(8, u * 6);
  const height = double ? Math.max(9, u * 6) : Math.max(5, u * 4);
  const amp = double ? (height / 2 - u) / 2 : (height - u) / 2;
  const horizontal = side === "top" || side === "bottom";
  const path = (mid: number) => {
    const q = period / 4;
    const a = [`M0 ${mid}`, `Q${q} ${mid - amp} ${2 * q} ${mid}`, `T${period} ${mid}`].join("");
    // On a vertical edge the same wave is drawn across instead of down.
    return horizontal ? a : a.replace(/(-?[\d.]+) (-?[\d.]+)/g, (_, x, y) => `${y} ${x}`);
  };
  const mids = double ? [height / 4, (height * 3) / 4] : [height / 2];
  const body = mids.map((m) => `<path d='${path(m)}' fill='none' stroke='${c}' stroke-width='${u}'/>`).join("");
  const [w, h] = horizontal ? [period, height] : [height, period];
  return {
    width: height,
    css: `border-${side}:${height}px solid transparent`,
    layer: {
      image: svg(w, h, body),
      size: `${w}px ${h}px`,
      position: { top: "0 0", bottom: "0 100%", left: "0 0", right: "100% 0" }[side],
      repeat: horizontal ? "repeat-x" : "repeat-y",
    },
  };
}

/** One edge of a paragraph border (or any box edge) in the style Word names. */
export function borderEdge(side: Side, b: Border): Edge {
  const c = hexColor(b.color);
  const simple = simpleBorder(b);
  if (simple) return { width: simple.width, css: `border-${side}:${simple.width}px ${simple.style} ${c}` };
  const u = lineWidth(b);
  const horizontal = side === "top" || side === "bottom";
  const stripes = STRIPES[b.val];
  if (stripes) {
    const total = stripes.reduce((a, x) => a + x, 0) * u;
    const dir = { top: "to bottom", bottom: "to top", left: "to right", right: "to left" }[side];
    let at = 0;
    const stops = stripes.map((x, i) => {
      const from = at;
      at += x * u;
      return `${i % 2 === 0 ? c : "transparent"} ${from.toFixed(2)}px ${at.toFixed(2)}px`;
    });
    return {
      width: total,
      css: `border-${side}:${total}px solid transparent`,
      layer: {
        image: `linear-gradient(${dir},${stops.join(",")})`,
        size: horizontal ? `100% ${total}px` : `${total}px 100%`,
        position: { top: "0 0", bottom: "0 100%", left: "0 0", right: "100% 0" }[side],
        repeat: "no-repeat",
      },
    };
  }
  const dashes = DASHES[b.val];
  if (dashes) {
    const dir = horizontal ? "to right" : "to bottom";
    let at = 0;
    const stops = dashes.map((x, i) => {
      const from = at;
      at += x * u;
      return `${i % 2 === 0 ? c : "transparent"} ${from.toFixed(2)}px ${at.toFixed(2)}px`;
    });
    return {
      width: u,
      css: `border-${side}:${u}px solid transparent`,
      layer: {
        image: `repeating-linear-gradient(${dir},${stops.join(",")})`,
        size: horizontal ? `100% ${u}px` : `${u}px 100%`,
        position: { top: "0 0", bottom: "0 100%", left: "0 0", right: "100% 0" }[side],
        repeat: "no-repeat",
      },
    };
  }
  return wave(side, b, c);
}

/** A border as a single CSS shorthand value (`1px solid #000`) for places that can't paint layers, like table cells. */
export function borderShorthand(b: Border): string {
  const simple = simpleBorder(b);
  const c = hexColor(b.color);
  if (simple) return `${simple.width}px ${simple.style} ${c}`;
  // The painted styles show as a solid line as wide as their band.
  const edge = borderEdge("top", b);
  return `${edge.width}px solid ${c}`;
}
