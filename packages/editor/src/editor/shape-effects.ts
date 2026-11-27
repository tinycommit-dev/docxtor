// Shape effects drawn in the shape's SVG: outer and inner shadow, glow, soft edges, reflection, bevel and
// extrusion. Distances are points in the SVG's viewBox (which is the shape's box in points), so a blur of 4 points
// stays 4 points however the SVG is stretched. The look follows Word's at ordinary sizes; the 3-D parts are
// approximations (a bevel is lit relief, an extrusion is stacked copies), not a renderer.
import type { REffects, RShadow } from "@docxtor/core";

import { nextId } from "./shape-fill";
import { hexOr } from "./color.ts";

const NS = "http://www.w3.org/2000/svg";

function el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, ...kids: Node[]): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  e.append(...kids);
  return e;
}

const num = (v: number) => String(Math.round(v * 1000) / 1000);
/** A colour of the shape as CSS; a value that is not six hex digits is black (the engine hands out nothing else, this is the second check). */
const hex = (c: string) => hexOr(c);
const rad = (deg: number) => (deg * Math.PI) / 180;

/** What the effects are drawn for. */
export interface FxContext {
  w: number;
  h: number;
  flipH: boolean;
  flipV: boolean;
  /** The shape's rotation in degrees (the shape is turned by CSS, so what stays put on the page is turned back). */
  rot: number;
  /** The colour of the shape's fill, for an extrusion that has none of its own. */
  fillColor?: string;
}

function filter(defs: SVGElement, box: { x: number; y: number; w: number; h: number }, ...kids: Node[]): string {
  const id = nextId();
  defs.append(el("filter", { id, filterUnits: "userSpaceOnUse", x: num(box.x), y: num(box.y), width: num(box.w), height: num(box.h), "color-interpolation-filters": "sRGB" }, ...kids));
  return id;
}

/** The box and a margin all round it. */
const around = (w: number, h: number, pad: number) => ({ x: -pad, y: -pad, w: w + 2 * pad, h: h + 2 * pad });

/**
 * A direction on the page in degrees clockwise from the right, as the SVG has to say it so that it shows that way
 * once the SVG is flipped by CSS (and, for what stays put on the page, once the shape is turned too).
 */
function unflip(angle: number, c: FxContext, fixedOnPage = false): number {
  let a = fixedOnPage ? angle - c.rot : angle;
  if (c.flipH) a = 180 - a;
  if (c.flipV) a = -a;
  return ((a % 360) + 360) % 360;
}

/** Where a scaled shadow stays anchored: a point of the box from its `algn` code. */
function anchorPoint(algn: string | undefined, w: number, h: number): [number, number] {
  const a = algn ?? "b";
  const x = a === "tl" || a === "l" || a === "bl" ? 0 : a === "tr" || a === "r" || a === "br" ? w : w / 2;
  const y = a === "tl" || a === "t" || a === "tr" ? 0 : a === "bl" || a === "b" || a === "br" ? h : h / 2;
  return [x, y];
}

function use(href: string, transform?: string): SVGUseElement {
  const u = el("use");
  u.setAttribute("href", `#${href}`);
  if (transform) u.setAttribute("transform", transform);
  return u;
}

// ---- outer shadow -----------------------------------------------------------------------------------------------

function outerShadow(defs: SVGElement, body: string, s: RShadow, c: FxContext): SVGGElement {
  // Word writes shadows that keep their direction on the page when the shape turns.
  const dir = rad(unflip(s.angle, c, s.rotWithShape === false));
  const dx = s.distPt * Math.cos(dir);
  const dy = s.distPt * Math.sin(dir);
  const sx = (s.sx ?? 100) / 100;
  const sy = (s.sy ?? 100) / 100;
  const [ax, ay] = anchorPoint(s.align, c.w, c.h);
  // Scaled about the anchor, then moved.
  const t = `translate(${num(dx)} ${num(dy)}) translate(${num(ax)} ${num(ay)}) scale(${num(sx)} ${num(sy)}) translate(${num(-ax)} ${num(-ay)})`;
  const sigma = s.blurPt / 2;
  const reach = sigma * 3 + 2;
  // The moved, scaled box and the blur around it.
  const xs = [0, c.w].map((x) => ax + (x - ax) * sx + dx);
  const ys = [0, c.h].map((y) => ay + (y - ay) * sy + dy);
  const box = { x: Math.min(...xs, 0) - reach, y: Math.min(...ys, 0) - reach, w: Math.max(...xs, c.w) - Math.min(...xs, 0) + reach * 2, h: Math.max(...ys, c.h) - Math.min(...ys, 0) + reach * 2 };
  const f = filter(
    defs,
    box,
    el("feGaussianBlur", { in: "SourceAlpha", stdDeviation: num(sigma), result: "b" }),
    el("feFlood", { "flood-color": hex(s.color), "flood-opacity": s.alpha ?? 1, result: "c" }),
    el("feComposite", { in: "c", in2: "b", operator: "in" }),
  );
  return el("g", { class: "fx-shadow", filter: `url(#${f})` }, use(body, t));
}

// ---- inner shadow -----------------------------------------------------------------------------------------------

function innerShadowFilter(s: RShadow, c: FxContext): { primitives: Node[]; reach: number } {
  const dir = rad(unflip(s.angle, c));
  const sigma = s.blurPt / 2;
  const inv = el("feComponentTransfer", { in: "SourceAlpha", result: "inv" }, el("feFuncA", { type: "table", tableValues: "1 0" }));
  return {
    reach: sigma * 3 + s.distPt + 2,
    primitives: [
      inv,
      el("feOffset", { in: "inv", dx: num(s.distPt * Math.cos(dir)), dy: num(s.distPt * Math.sin(dir)), result: "o" }),
      el("feGaussianBlur", { in: "o", stdDeviation: num(sigma), result: "b" }),
      el("feFlood", { "flood-color": hex(s.color), "flood-opacity": s.alpha ?? 1, result: "c" }),
      el("feComposite", { in: "c", in2: "b", operator: "in", result: "sh" }),
      el("feComposite", { in: "sh", in2: "SourceAlpha", operator: "in", result: "clip" }),
      el("feMerge", {}, el("feMergeNode", { in: "SourceGraphic" }), el("feMergeNode", { in: "clip" })),
    ],
  };
}

// ---- glow and soft edges ----------------------------------------------------------------------------------------

function glow(defs: SVGElement, body: string, g: { color: string; alpha?: number; radiusPt: number }, c: FxContext): SVGGElement {
  const r = g.radiusPt;
  const f = filter(
    defs,
    around(c.w, c.h, r * 1.6 + 2),
    el("feMorphology", { in: "SourceAlpha", operator: "dilate", radius: num(Math.max(r / 3, 0.1)), result: "d" }),
    el("feGaussianBlur", { in: "d", stdDeviation: num(Math.max(r / 3, 0.1)), result: "b" }),
    el("feFlood", { "flood-color": hex(g.color), "flood-opacity": g.alpha ?? 1, result: "c" }),
    el("feComposite", { in: "c", in2: "b", operator: "in" }),
  );
  return el("g", { class: "fx-glow", filter: `url(#${f})` }, use(body));
}

function softEdges(defs: SVGElement, r: number, c: FxContext): string {
  return filter(
    defs,
    around(c.w, c.h, 2),
    el("feMorphology", { in: "SourceAlpha", operator: "erode", radius: num(Math.max(r / 2, 0.1)), result: "e" }),
    el("feGaussianBlur", { in: "e", stdDeviation: num(Math.max(r / 3, 0.1)), result: "b" }),
    el("feComposite", { in: "SourceGraphic", in2: "b", operator: "in" }),
  );
}

// ---- reflection -------------------------------------------------------------------------------------------------

function reflection(defs: SVGElement, body: string, r: NonNullable<REffects["reflection"]>, c: FxContext): SVGGElement {
  const dirDeg = unflip(r.direction, c);
  // The side the reflection falls on: below (the usual), above, right or left.
  const side = dirDeg >= 45 && dirDeg < 135 ? "b" : dirDeg >= 225 && dirDeg < 315 ? "t" : dirDeg < 45 || dirDeg >= 315 ? "r" : "l";
  const k = Math.abs((r.sy ?? 100) / 100) || 1;
  const kx = Math.abs((r.sx ?? 100) / 100) || 1;
  const extent = side === "b" || side === "t" ? c.h : c.w;
  let t: string;
  let from: [number, number];
  let u: [number, number];
  if (side === "b") {
    t = `translate(${num(c.w / 2)} ${num(c.h + r.distPt)}) scale(${num(kx)} ${num(-k)}) translate(${num(-c.w / 2)} ${num(-c.h)})`;
    from = [c.w / 2, c.h + r.distPt];
    u = [0, 1];
  } else if (side === "t") {
    t = `translate(${num(c.w / 2)} ${num(-r.distPt)}) scale(${num(kx)} ${num(-k)}) translate(${num(-c.w / 2)} 0)`;
    from = [c.w / 2, -r.distPt];
    u = [0, -1];
  } else if (side === "r") {
    t = `translate(${num(c.w + r.distPt)} ${num(c.h / 2)}) scale(${num(-k)} ${num(kx)}) translate(${num(-c.w)} ${num(-c.h / 2)})`;
    from = [c.w + r.distPt, c.h / 2];
    u = [1, 0];
  } else {
    t = `translate(${num(-r.distPt)} ${num(c.h / 2)}) scale(${num(-k)} ${num(kx)}) translate(0 ${num(-c.h / 2)})`;
    from = [-r.distPt, c.h / 2];
    u = [-1, 0];
  }
  const st = (r.startPos ?? 0) / 100;
  const en = (r.endPos ?? 100) / 100;
  const at = (p: number): [number, number] => [from[0] + u[0] * p * extent * k, from[1] + u[1] * p * extent * k];
  const a = at(st);
  const b = at(en);
  const gid = nextId();
  const mid = nextId();
  const reach = Math.max(c.w, c.h) * 2 + r.distPt + 4;
  defs.append(
    el(
      "linearGradient",
      { id: gid, gradientUnits: "userSpaceOnUse", x1: num(a[0]), y1: num(a[1]), x2: num(b[0]), y2: num(b[1]) },
      el("stop", { offset: 0, "stop-color": "#fff", "stop-opacity": r.startAlpha }),
      el("stop", { offset: 1, "stop-color": "#fff", "stop-opacity": r.endAlpha }),
    ),
    el("mask", { id: mid, maskUnits: "userSpaceOnUse", x: num(-reach), y: num(-reach), width: num(c.w + reach * 2), height: num(c.h + reach * 2) }, el("rect", { x: num(-reach), y: num(-reach), width: num(c.w + reach * 2), height: num(c.h + reach * 2), fill: `url(#${gid})` })),
  );
  const g = el("g", { class: "fx-reflection", mask: `url(#${mid})` }, el("g", { transform: t }, use(body)));
  if (r.blurPt > 0) {
    const f = filter(defs, { x: -reach, y: -reach, w: c.w + reach * 2, h: c.h + reach * 2 }, el("feGaussianBlur", { stdDeviation: num(r.blurPt / 2) }));
    // A blur and a mask on one element: the blur first.
    const outer = el("g", { class: "fx-reflection", filter: `url(#${f})` }, g);
    g.removeAttribute("class");
    return outer;
  }
  return g;
}

// ---- bevel ------------------------------------------------------------------------------------------------------

interface BevelLook {
  sigma: number;
  scale: number;
  inset?: boolean;
}

function bevelLook(preset: string, wPt: number, hPt: number): BevelLook {
  const w = Math.max(wPt, 0.5);
  const h = Math.max(hPt, 0.5);
  switch (preset) {
    case "hardEdge":
      return { sigma: w * 0.25, scale: h * 1.4 };
    case "angle":
    case "slope":
    case "coolSlant":
    case "artDeco":
    case "cross":
    case "riblet":
      return { sigma: w * 0.35, scale: h * 1.2 };
    case "relaxedInset":
    case "divot":
      return { sigma: w * 0.5, scale: h * 1.1, inset: true };
    case "convex":
      return { sigma: w * 0.6, scale: h * 1.3 };
    default:
      return { sigma: w * 0.5, scale: h * 1.2 };
  }
}

function bevelPrimitives(b: NonNullable<REffects["bevel"]>, c: FxContext): Node[] {
  const look = bevelLook(b.preset, b.wPt, b.hPt);
  // The light comes from the top left (an inset bevel from the bottom right), seen through the flips.
  let az = look.inset ? 45 : 225;
  az = unflip(az, c, true);
  const sigma = Math.max(look.sigma, 0.2);
  // The lit surface darkens the side turned from the light, never to black, and brightens the side that faces it.
  const soften = () => ["feFuncR", "feFuncG", "feFuncB"].map((f) => el(f as "feFuncR", { type: "linear", slope: 0.45, intercept: 0.55 }));
  return [
    el("feGaussianBlur", { in: "SourceAlpha", stdDeviation: num(sigma), result: "bump" }),
    el("feDiffuseLighting", { in: "bump", surfaceScale: num(look.scale * 0.5), diffuseConstant: 1.22, "lighting-color": "#fff", result: "diff" }, el("feDistantLight", { azimuth: num(az), elevation: 55 })),
    el("feComponentTransfer", { in: "diff", result: "diffSoft" }, ...soften()),
    el("feSpecularLighting", { in: "bump", surfaceScale: num(look.scale * 0.5), specularConstant: 0.6, specularExponent: 34, "lighting-color": "#fff", result: "spec" }, el("feDistantLight", { azimuth: num(az), elevation: 50 })),
    el("feComposite", { in: "diffSoft", in2: "SourceAlpha", operator: "in", result: "diffIn" }),
    el("feBlend", { in: "SourceGraphic", in2: "diffIn", mode: "multiply", result: "shaded" }),
    el("feComposite", { in: "spec", in2: "SourceAlpha", operator: "in", result: "specIn" }),
    el("feComposite", { in: "shaded", in2: "specIn", operator: "arithmetic", k1: 0, k2: 1, k3: 1, k4: 0 }),
  ];
}

// ---- extrusion --------------------------------------------------------------------------------------------------

function darker(hexColor: string, f: number): string {
  const p = (i: number) => Math.round(parseInt(hexColor.slice(i, i + 2), 16) * f);
  const h = (v: number) => v.toString(16).padStart(2, "0");
  return `${h(p(0))}${h(p(2))}${h(p(4))}`.toUpperCase();
}

function extrusion(defs: SVGElement, body: string, x: NonNullable<REffects["extrusion"]>, c: FxContext): SVGGElement | null {
  const dir = x.dir;
  if (!dir) return null;
  const color = x.color ?? darker(c.fillColor ?? "808080", 0.55);
  const depth = x.heightPt;
  const steps = Math.min(80, Math.max(2, Math.ceil(depth / 0.6)));
  const copies: Node[] = [];
  // Far copies first, so the near ones lie over them.
  for (let i = steps; i >= 1; i--) {
    const d = (depth * i) / steps;
    copies.push(use(body, `translate(${num(dir[0] * d)} ${num(dir[1] * d)})`));
  }
  const reach = depth + 2;
  const f = filter(defs, around(c.w, c.h, reach), el("feFlood", { "flood-color": hex(color), result: "c" }), el("feComposite", { in: "c", in2: "SourceAlpha", operator: "in" }));
  return el("g", { class: "fx-extrusion", filter: `url(#${f})` }, ...copies);
}

// ---- putting it together ----------------------------------------------------------------------------------------

/**
 * Draw the effects of a shape. `body` is the group that holds the shape's paths, already in `svg` (it needs an
 * id). The layers behind the shape are inserted before it. Filters that work on the shape itself (relief, inner
 * shadow, soft edges) go on groups wrapped around it.
 */
export function applyEffects(svg: SVGSVGElement, defs: SVGElement, body: SVGGElement, fx: REffects, c: FxContext): void {
  const bid = body.id;
  const back: Node[] = [];
  if (fx.reflection) back.push(reflection(defs, bid, fx.reflection, c));
  if (fx.shadow) back.push(outerShadow(defs, bid, fx.shadow, c));
  if (fx.extrusion) {
    const x = extrusion(defs, bid, fx.extrusion, c);
    if (x) back.push(x);
  }
  if (fx.glow) back.push(glow(defs, bid, fx.glow, c));
  let top: SVGGElement = body;
  const around_ = (filterId: string) => {
    const g = el("g", { filter: `url(#${filterId})` });
    svg.insertBefore(g, top);
    g.append(top);
    top = g;
  };
  if (fx.bevel) around_(filter(defs, around(c.w, c.h, fx.bevel.wPt * 2 + 4), ...bevelPrimitives(fx.bevel, c)));
  if (fx.innerShadow) {
    const inner = innerShadowFilter(fx.innerShadow, c);
    around_(filter(defs, around(c.w, c.h, inner.reach), ...inner.primitives));
  }
  if (fx.softEdgePt) around_(softEdges(defs, fx.softEdgePt, c));
  for (const n of back) svg.insertBefore(n, top);
}
