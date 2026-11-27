// WordArt on the page: text bent to a preset, drawn as SVG over the shape's box (see warp.ts for the geometry).
//
// A mesh warp (wave, inflate, slant, ...) cuts the text into thin vertical slices, each drawn through the affine
// map that fits the warp there; a path warp (arch, circle) uses <textPath>. Fill, outline, shadow, glow and
// reflection are the text's own (`RWarp`), so they are applied to the whole warped block, not to the slices.
import type { Engine, RBlock, RShape, RWarp } from "@docxtor/core";
import { fontStack } from "./render";
import { nextId, paintFor } from "./shape-fill";
import { followPath, meshBounds, sliceCount, slices, warpDef, type Matrix } from "./warp";
import { hexOr } from "./color.ts";

const NS = "http://www.w3.org/2000/svg";
const PT_PX = 96 / 72;

function el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, ...kids: Node[]): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  e.append(...kids);
  return e;
}

const num = (v: number) => String(Math.round(v * 1000) / 1000);
const matrix = (m: Matrix) => `matrix(${m.map(num).join(" ")})`;

/** One line of the text and how it looks. */
export interface WarpLine {
  text: string;
  font?: string;
  bold?: boolean;
  italic?: boolean;
  /** Hex, when the text has a colour of its own. */
  color?: string;
}

/** The lines of a WordArt shape: its story's paragraphs, or for VML WordArt its `string`. */
export function warpLines(s: RShape): WarpLine[] {
  const w = s.warp;
  if (!w) return [];
  if (w.source === "vml" || w.text !== undefined) return (w.text ?? "").split("\n").map((text) => ({ text, font: w.font, bold: w.bold, italic: w.italic }));
  const out: WarpLine[] = [];
  const walk = (blocks: RBlock[]) => {
    for (const b of blocks) {
      if (b.t !== "p") continue;
      const texts = b.segs.filter((g) => g.k === "t" && g.x);
      const f = texts[0]?.f;
      out.push({ text: texts.map((g) => g.x).join(""), font: f?.font, bold: f?.bold, italic: f?.italic, color: f?.color });
    }
  };
  walk(s.text ?? []);
  return out;
}

let canvas: CanvasRenderingContext2D | null | undefined;

/** The width of `text` at font size `size` (any unit) in the font. A rough guess where there is no canvas. */
export function measure(text: string, line: WarpLine, size: number): number {
  if (canvas === undefined) canvas = typeof document !== "undefined" ? document.createElement("canvas").getContext("2d") : null;
  if (!canvas) return text.length * size * 0.55;
  canvas.font = `${line.italic ? "italic " : ""}${line.bold ? "700 " : "400 "}${size}px ${fontStack(line.font) ?? "sans-serif"}`;
  return canvas.measureText(text).width;
}

/** The size the text is laid out at before it is stretched to the box (only its proportions matter). */
const BASE = 100;
const LEADING = 1.2;
const ASCENT = 0.93;

/** The colour of text with no fill of its own. */
function plainFill(lines: WarpLine[]): string {
  return hexOr(lines.find((l) => l.color && l.color !== "auto")?.color);
}

/**
 * The SVG for a warped shape, or null when there is no text. It fills the shape's box (`preserveAspectRatio="none"`,
 * the viewBox is the box in points) and never takes a click.
 */
export function warpSvg(s: RShape, env: { engine?: Engine } = {}): SVGSVGElement | null {
  const warp = s.warp;
  const lines = warpLines(s);
  while (lines.length && !lines[lines.length - 1].text) lines.pop();
  if (!warp || !lines.some((l) => l.text.trim())) return null;
  const w = Math.max(s.w, 1);
  const h = Math.max(s.h, 1);
  const defs = el("defs");
  const svg = el("svg", { class: "shape-svg warp-svg", viewBox: `0 0 ${num(w)} ${num(h)}`, preserveAspectRatio: "none", "aria-hidden": "true", focusable: "false" }, defs);
  const def = warpDef(warp.preset, warp.adj, w, h);
  // The text, laid out from (0, 0), one line under another and centred across.
  const widths = lines.map((l) => measure(l.text, l, BASE));
  const tw = Math.max(...widths, BASE * 0.5);
  const th = lines.length * BASE * LEADING;
  // How much smaller the finished drawing is than the text as laid out: an outline's width is in the drawing's points.
  const paint = textPaint(warp, defs, tw, th, def.kind === "path" ? 1 : (tw / w + th / h) / 2, env, lines);
  const place = (g: SVGElement) => {
    for (const [k, v] of Object.entries(paint)) g.setAttribute(k, String(v));
  };
  const group = el("g", { id: nextId() });

  if (def.kind === "path") {
    const unit = Math.max(...widths, 1) / BASE;
    const layout = followPath(warp.preset, warp.adj, w, h, unit);
    const id = nextId();
    defs.append(el("path", { id, d: layout.d }));
    const t = el("text", { "font-size": num(layout.fontSize), "text-anchor": "middle" });
    applyFont(t, lines[0]);
    place(t);
    // Lines of text follow the path one after another.
    const path = el("textPath", { startOffset: "50%" });
    path.setAttribute("href", `#${id}`);
    path.textContent = lines.map((l) => l.text).join(" ");
    t.append(path);
    group.append(t);
  } else {
    const text = el("text", { id: nextId() });
    applyFont(text, lines[0]);
    place(text);
    text.setAttribute("font-size", String(BASE));
    text.setAttribute("text-anchor", "middle");
    lines.forEach((l, i) => {
      const tspan = el("tspan", { x: num(tw / 2), y: num(i * BASE * LEADING + BASE * ASCENT) });
      if (l.font !== lines[0].font) tspan.setAttribute("font-family", fontStack(l.font) ?? "sans-serif");
      if (!!l.bold !== !!lines[0].bold) tspan.setAttribute("font-weight", l.bold ? "700" : "400");
      if (!!l.italic !== !!lines[0].italic) tspan.setAttribute("font-style", l.italic ? "italic" : "normal");
      tspan.textContent = l.text;
      text.append(tspan);
    });
    const mesh = def.mesh!;
    if (def.kind === "plain") {
      group.append(el("g", { transform: matrix([w / tw, 0, 0, h / th, 0, 0]) }, text));
    } else {
      defs.append(text);
      const count = sliceCount(w * PT_PX);
      const overlap = tw / count / 3;
      for (const sl of slices(mesh, tw, th, count)) {
        const clipId = nextId();
        // Neighbouring slices overlap a little so no seam shows; the first and last reach past the text.
        const left = sl.x0 <= 0 ? -BASE * 2 : sl.x0 - overlap;
        const right = sl.x1 >= tw - 0.01 ? tw + BASE * 2 : sl.x1 + overlap;
        defs.append(el("clipPath", { id: clipId }, el("rect", { x: num(left), y: num(-BASE), width: num(right - left), height: num(th + BASE * 2) })));
        const use = el("use", {});
        use.setAttribute("href", `#${text.id}`);
        group.append(el("g", { transform: matrix(sl.matrix), "clip-path": `url(#${clipId})` }, use));
      }
    }
    effects(svg, defs, group, warp, w, h, meshBounds(mesh));
    return svg;
  }
  effects(svg, defs, group, warp, w, h, { x0: 0, y0: 0, x1: w, y1: h });
  return svg;
}

function applyFont(t: SVGElement, line: WarpLine) {
  t.setAttribute("font-family", fontStack(line.font) ?? "sans-serif");
  if (line.bold) t.setAttribute("font-weight", "700");
  if (line.italic) t.setAttribute("font-style", "italic");
}

/** Fill and outline attributes of the text. */
function textPaint(warp: RWarp, defs: SVGElement, tw: number, th: number, scale: number, env: { engine?: Engine }, lines: WarpLine[]): Record<string, string | number> {
  const out: Record<string, string | number> = {};
  if (warp.fill) {
    const p = paintFor(warp.fill, defs, tw, th, { url: env.engine ? (src) => env.engine!.mediaUrl(src) : undefined });
    out.fill = p.paint;
    if (p.opacity < 1) out["fill-opacity"] = p.opacity;
  } else out.fill = plainFill(lines);
  if (warp.line) {
    out.stroke = hexOr(warp.line.color);
    out["stroke-width"] = num(warp.line.widthPt * scale);
    out["stroke-linejoin"] = warp.line.join ?? "miter";
    if (warp.line.alpha !== undefined && warp.line.alpha < 1) out["stroke-opacity"] = warp.line.alpha;
    out["paint-order"] = "normal";
  }
  return out;
}

/** Shadow, glow and reflection around the warped block, which go on the finished drawing (box units). */
function effects(svg: SVGSVGElement, defs: SVGElement, group: SVGElement, warp: RWarp, w: number, h: number, b: { x0: number; y0: number; x1: number; y1: number }) {
  const pad = Math.max(w, h);
  const area = { filterUnits: "userSpaceOnUse", x: num(-pad), y: num(-pad), width: num(w + 2 * pad), height: num(h + 2 * pad), "color-interpolation-filters": "sRGB" };
  let target: SVGElement = group;
  if (warp.glow || warp.shadow) {
    const id = nextId();
    const f = el("filter", { id, ...area });
    let last = "SourceGraphic";
    if (warp.glow) {
      f.append(
        el("feGaussianBlur", { in: "SourceAlpha", stdDeviation: num(warp.glow.radiusPt / 2), result: "gb" }),
        el("feFlood", { "flood-color": hexOr(warp.glow.color), "flood-opacity": warp.glow.alpha, result: "gc" }),
        el("feComposite", { in: "gc", in2: "gb", operator: "in", result: "glow" }),
        el("feMerge", { result: "withGlow" }, el("feMergeNode", { in: "glow" }), el("feMergeNode", { in: "SourceGraphic" })),
      );
      last = "withGlow";
    }
    if (warp.shadow) {
      const sd = warp.shadow;
      f.append(
        el("feGaussianBlur", { in: "SourceAlpha", stdDeviation: num(sd.blurPt / 2), result: "sb" }),
        el("feOffset", { in: "sb", dx: num(sd.dx), dy: num(sd.dy), result: "so" }),
        el("feFlood", { "flood-color": hexOr(sd.color), "flood-opacity": sd.alpha, result: "sc" }),
        el("feComposite", { in: "sc", in2: "so", operator: "in", result: "shadow" }),
        el("feMerge", {}, el("feMergeNode", { in: "shadow" }), el("feMergeNode", { in: last })),
      );
    }
    defs.append(f);
    target = el("g", { filter: `url(#${id})` }, group);
  }
  svg.append(target);
  if (warp.reflection) {
    const r = warp.reflection;
    const top = b.y1 + r.distPt;
    const depth = Math.max(1, ((b.y1 - b.y0) * r.size) / 100);
    const gid = nextId();
    const mid = nextId();
    defs.append(
      el(
        "linearGradient",
        { id: gid, gradientUnits: "userSpaceOnUse", x1: 0, y1: num(top), x2: 0, y2: num(top + depth) },
        el("stop", { offset: 0, "stop-color": "#fff", "stop-opacity": r.startAlpha }),
        el("stop", { offset: 1, "stop-color": "#fff", "stop-opacity": r.endAlpha }),
      ),
      el("mask", { id: mid, maskUnits: "userSpaceOnUse", x: num(-pad), y: num(top), width: num(w + 2 * pad), height: num(depth) }, el("rect", { x: num(-pad), y: num(top), width: num(w + 2 * pad), height: num(depth), fill: `url(#${gid})` })),
    );
    const use = el("use", { transform: `translate(0 ${num(2 * b.y1 + r.distPt)}) scale(1 -1)`, mask: `url(#${mid})` });
    use.setAttribute("href", `#${group.id}`);
    svg.append(use);
  }
}
