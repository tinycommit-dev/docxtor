// What is drawn on a sheet apart from the text: the page background (a colour, gradient, pattern, texture or picture)
// and the page borders. Both are the document's own, so they use the colours the file gives, not tokens.
import type { Border, PageBorders, PageFill, SectionInfo } from "@docxtor/core";
import { borderEdge, PT_PX, type Layer, type Side } from "./borders";
import { hex } from "./color.ts";

const hex6 = (c?: string) => hex(c?.replace(/^#/, ""));

/** Where a radial gradient starts, as CSS. */
const FROM: Record<string, string> = {
  center: "50% 50%",
  topLeft: "0 0",
  topRight: "100% 0",
  bottomLeft: "0 100%",
  bottomRight: "100% 100%",
};

/**
 * CSS declarations for a page background. `url` turns an image part into a URL the browser can load. A solid colour
 * with nothing else is just `background`, as it always was.
 */
export function pageFillCss(fill: PageFill | undefined, color: string | undefined, url: (part: string) => string): string {
  const solid = hex6(fill?.color ?? color);
  if (!fill || fill.kind === "solid") return solid ? `background:${solid}` : "";
  const c1 = hex6(fill.color) ?? "#ffffff";
  const c2 = hex6(fill.color2) ?? "#ffffff";
  switch (fill.kind) {
    case "gradient":
      // VML counts degrees from a gradient that runs from top to bottom.
      return `background:linear-gradient(${(180 + (fill.angle ?? 0)).toFixed(1)}deg,${c1},${c2})`;
    case "radial":
      return `background:radial-gradient(circle at ${FROM[fill.from ?? "center"] ?? FROM.center},${c1},${c2} 75%)`;
    case "pattern":
    case "texture":
      if (!fill.media) return solid ? `background:${solid}` : "";
      // A pattern is a few pixels wide: keep them square. A texture tiles at its own size.
      return `background-color:${c2};background-image:url("${url(fill.media)}");background-repeat:repeat${fill.kind === "pattern" ? ";image-rendering:pixelated" : ""}`;
    case "picture":
      if (!fill.media) return solid ? `background:${solid}` : "";
      return `background-color:${c1};background-image:url("${url(fill.media)}");background-size:100% 100%;background-repeat:no-repeat`;
  }
  return "";
}

/** The word for a fill in the status line and the dialogs' sample (`gradient`, not `radial`). */
export const fillIsImage = (fill?: PageFill) => !!fill && (fill.kind === "pattern" || fill.kind === "texture" || fill.kind === "picture");

/** A page border as the border painter wants it: art borders are drawn as a plain line in the colour. */
function asLine(b: NonNullable<PageBorders["top"]>): Border {
  const art = b.val !== "nil" && !LINES.has(b.val);
  return art ? { val: "single", sz: 12, color: b.color, space: b.space } : { val: b.val, sz: b.sz, color: b.color, space: b.space };
}

/** Styles that draw as lines (the rest of Word's border names are art). */
const LINES = new Set([
  "single", "thick", "double", "dotted", "dashed", "dotDash", "dotDotDash", "triple", "thinThickSmallGap", "thickThinSmallGap",
  "thinThickThinSmallGap", "thinThickMediumGap", "thickThinMediumGap", "thinThickThinMediumGap", "thinThickLargeGap", "thickThinLargeGap",
  "thinThickThinLargeGap", "wave", "doubleWave", "dashSmallGap", "dashDotStroked", "threeDEmboss", "threeDEngrave", "outset", "inset",
]);

/** Does a section show its page borders on this page? `first` is the first page the section owns. */
export function bordersShow(b: PageBorders | undefined, first: boolean): b is PageBorders {
  if (!b) return false;
  if (b.display === "firstPage") return first;
  if (b.display === "notFirstPage") return !first;
  return true;
}

/** The sheet's geometry the borders are placed against, in CSS px. */
export interface BorderBox {
  w: number;
  h: number;
  /** The section's own margins (top, right, bottom, left), and where its header and footer start from the edge. */
  mt: number;
  mr: number;
  mb: number;
  ml: number;
  hdr: number;
  ftr: number;
  /** The page has a header or a footer: Word's border goes round them too when it counts from the text. */
  hasHeader: boolean;
  hasFooter: boolean;
}

/** The element that draws a section's page borders on one sheet. */
export function pageBorderEl(b: PageBorders, g: BorderBox): HTMLElement {
  const el = document.createElement("div");
  el.className = "pgb";
  el.setAttribute("aria-hidden", "true");
  const space = (side: Side) => (b[side]?.space ?? 0) * PT_PX;
  // From the page edge, the border sits `space` in from it; from the text, `space` outside the margin (and outside the header and footer).
  const fromText = b.offsetFrom === "text";
  const top = fromText ? (g.hasHeader ? Math.min(g.hdr, g.mt) : g.mt) - space("top") : space("top");
  const bottom = fromText ? (g.hasFooter ? Math.min(g.ftr, g.mb) : g.mb) - space("bottom") : space("bottom");
  const left = fromText ? g.ml - space("left") : space("left");
  const right = fromText ? g.mr - space("right") : space("right");
  const css: string[] = [`position:absolute;box-sizing:border-box;pointer-events:none;top:${Math.max(0, top)}px;bottom:${Math.max(0, bottom)}px;left:${Math.max(0, left)}px;right:${Math.max(0, right)}px`];
  const layers: Layer[] = [];
  for (const side of ["top", "left", "bottom", "right"] as const) {
    const s = b[side];
    if (!s) continue;
    const edge = borderEdge(side, asLine(s));
    css.push(edge.css);
    if (edge.layer) layers.push(edge.layer);
  }
  if (layers.length) {
    css.push(`background-image:${layers.map((l) => l.image).join(",")}`, `background-size:${layers.map((l) => l.size).join(",")}`, `background-position:${layers.map((l) => l.position).join(",")}`, `background-repeat:${layers.map((l) => l.repeat).join(",")}`, "background-origin:border-box");
  }
  // A shadow is a dark offset along the right and bottom sides.
  if ([b.top, b.left, b.bottom, b.right].some((s) => s?.shadow)) css.push("box-shadow:3px 3px 0 0 rgb(0 0 0 / 0.55)");
  el.setAttribute("style", css.join(";"));
  el.dataset.z = b.zOrder;
  return el;
}

/** The sides of a section's borders, for a test or the status line. */
export function borderSides(info: Pick<SectionInfo, "pageBorders">): string[] {
  const b = info.pageBorders;
  return b ? (["top", "left", "bottom", "right"] as const).filter((s) => b[s]) : [];
}
