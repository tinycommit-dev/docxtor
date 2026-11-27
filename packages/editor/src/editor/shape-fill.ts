// Fills of shapes as SVG paint: solid colours, linear and path gradients, pictures (stretched, cropped or tiled)
// and DrawingML's pattern presets. `paintFor` puts what a fill needs into a <defs> and says what to paint with, so
// other drawings (WordArt text fills) can use the same fills.
import type { RFill, RRect4 } from "@docxtor/core";
import { hexOr } from "./color.ts";

const NS = "http://www.w3.org/2000/svg";
let uid = 0;

/** A fresh id for something in a shape's SVG. The `fa` prefix is what printing renames (see `renameIds`). */
export function nextId(): string {
  return `fa${++uid}`;
}

function el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, ...kids: Node[]): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  e.append(...kids);
  return e;
}

/** A colour of the shape as CSS; a value that is not six hex digits is black (the engine hands out nothing else, this is the second check). */
const hex = (c: string) => hexOr(c);
const num = (v: number) => String(Math.round(v * 1000) / 1000);

/** What a fill paints with: a `fill` value for an SVG element and an opacity for it. */
export interface Paint {
  paint: string;
  opacity: number;
}

/** Where a fill is drawn and how to reach what it uses. */
export interface PaintEnv {
  /** The URL of a media part (`engine.mediaUrl`). */
  url?: (src: string) => string;
  /** The shape's rotation in degrees (a gradient that doesn't turn with its shape is turned back). */
  rot?: number;
}

/**
 * The paint for `fill` on a box of `w` by `h` points (the SVG's viewBox units). Definitions go into `defs`.
 * An unknown or unreadable fill paints nothing.
 */
export function paintFor(fill: RFill, defs: SVGElement, w: number, h: number, env: PaintEnv = {}): Paint {
  const alpha = fill.alpha ?? 1;
  switch (fill.kind) {
    case "gradient":
      return gradientPaint(fill, defs, w, h, env);
    case "pattern":
      return patternPaint(fill, defs);
    case "picture":
      return fill.src && env.url ? picturePaint(fill, defs, w, h, env.url(fill.src)) : { paint: "none", opacity: 1 };
    default:
      return { paint: hex(fill.color), opacity: alpha };
  }
}

// ---- gradients --------------------------------------------------------------------------------------------------

function stopsOf(f: RFill): SVGStopElement[] {
  const stops = f.stops?.length ? f.stops : [{ pos: 0, color: f.color, alpha: f.alpha }, { pos: 100, color: f.color2 ?? f.color, alpha: f.alpha }];
  return stops.map((s) => el("stop", { offset: num(s.pos / 100), "stop-color": hex(s.color), "stop-opacity": s.alpha ?? 1 }));
}

function gradientPaint(f: RFill, defs: SVGElement, w: number, h: number, env: PaintEnv): Paint {
  const id = nextId();
  if (f.path) {
    const to = f.fillTo ?? { l: 50, t: 50, r: 50, b: 50 };
    // The focus rectangle's centre, as a fraction of the box.
    const cx = (to.l + (100 - to.r)) / 200;
    const cy = (to.t + (100 - to.b)) / 200;
    if (f.path === "rect") return { paint: rectGradientPaint(f, defs, id, cx, cy), opacity: 1 };
    // `circle` (and `shape`, approximated): rings out to the farthest corner.
    const r = Math.hypot(Math.max(cx, 1 - cx), Math.max(cy, 1 - cy));
    const g = el("radialGradient", { id, cx: num(cx), cy: num(cy), fx: num(cx), fy: num(cy), r: num(r) }, ...stopsOf(f));
    defs.append(g);
    return { paint: `url(#${id})`, opacity: 1 };
  }
  let angle = f.angle ?? 90;
  // A gradient that doesn't turn with its shape is turned back by the shape's rotation.
  if (f.rotWithShape === false && env.rot) angle -= env.rot;
  const line = linearGradientLine(angle, w, h, !!f.scaled);
  const g = el("linearGradient", { id, ...(line.units === "user" ? { gradientUnits: "userSpaceOnUse" } : {}), x1: num(line.x1), y1: num(line.y1), x2: num(line.x2), y2: num(line.y2) }, ...stopsOf(f));
  defs.append(g);
  return { paint: `url(#${id})`, opacity: 1 };
}

/**
 * The line a linear gradient runs along. `box` units are fractions of the box (an angle of the unit square,
 * stretched with the box: 45 degrees runs corner to corner); `user` units are points (a true angle, the line
 * through the centre that just reaches the corners furthest along it). Along an axis the two are the same.
 */
export function linearGradientLine(angleDeg: number, w: number, h: number, scaled: boolean): { x1: number; y1: number; x2: number; y2: number; units: "box" | "user" } {
  const a = (angleDeg * Math.PI) / 180;
  const dx = Math.cos(a);
  const dy = Math.sin(a);
  if (scaled || Math.abs(dx * dy) < 1e-9) {
    const len = Math.abs(dx) + Math.abs(dy);
    return { x1: 0.5 - (dx * len) / 2, y1: 0.5 - (dy * len) / 2, x2: 0.5 + (dx * len) / 2, y2: 0.5 + (dy * len) / 2, units: "box" };
  }
  const len = Math.abs(w * dx) + Math.abs(h * dy);
  return { x1: w / 2 - (dx * len) / 2, y1: h / 2 - (dy * len) / 2, x2: w / 2 + (dx * len) / 2, y2: h / 2 + (dy * len) / 2, units: "user" };
}

function lerpColor(stops: { pos: number; color: string; alpha?: number }[], t: number): [number, number, number, number] {
  const rgb = (c: string) => [parseInt(c.slice(0, 2), 16), parseInt(c.slice(2, 4), 16), parseInt(c.slice(4, 6), 16)];
  const p = t * 100;
  let a = stops[0];
  let b = stops[stops.length - 1];
  for (let i = 0; i < stops.length - 1; i++) {
    if (p >= stops[i].pos && p <= stops[i + 1].pos) {
      a = stops[i];
      b = stops[i + 1];
      break;
    }
  }
  if (p <= stops[0].pos) b = a = stops[0];
  else if (p >= stops[stops.length - 1].pos) a = b = stops[stops.length - 1];
  const span = b.pos - a.pos;
  const u = span > 0 ? (p - a.pos) / span : 0;
  const ca = rgb(a.color);
  const cb = rgb(b.color);
  return [ca[0] + (cb[0] - ca[0]) * u, ca[1] + (cb[1] - ca[1]) * u, ca[2] + (cb[2] - ca[2]) * u, (a.alpha ?? 1) + ((b.alpha ?? 1) - (a.alpha ?? 1)) * u];
}

/**
 * A rectangular path gradient (concentric rectangles): SVG has no such paint, so it is drawn once into a small
 * canvas and used as a stretched picture. Colours run from the focus point out to the edges.
 */
function rectGradientPaint(f: RFill, defs: SVGElement, id: string, cx: number, cy: number): string {
  const size = 96;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext("2d");
  const stops = f.stops?.length ? f.stops : [{ pos: 0, color: f.color, alpha: f.alpha }, { pos: 100, color: f.color2 ?? f.color, alpha: f.alpha }];
  if (ctx) {
    const img = ctx.createImageData(size, size);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const u = (x + 0.5) / size;
        const v = (y + 0.5) / size;
        // How far from the focus towards the edge on each side, the greater of the two.
        const du = u < cx ? (cx - u) / (cx || 1) : (u - cx) / (1 - cx || 1);
        const dv = v < cy ? (cy - v) / (cy || 1) : (v - cy) / (1 - cy || 1);
        const [r, g, b, a] = lerpColor(stops, Math.min(1, Math.max(du, dv)));
        const i = (y * size + x) * 4;
        img.data.set([r, g, b, a * 255], i);
      }
    }
    ctx.putImageData(img, 0, 0);
  }
  const pat = el("pattern", { id, patternUnits: "objectBoundingBox", patternContentUnits: "objectBoundingBox", width: 1, height: 1 });
  const image = el("image", { x: 0, y: 0, width: 1, height: 1, preserveAspectRatio: "none" });
  image.setAttribute("href", canvas.toDataURL());
  pat.append(image);
  defs.append(pat);
  return `url(#${id})`;
}

// ---- pictures ---------------------------------------------------------------------------------------------------

/** Where a tile starts, by its alignment code: left, centre, right; top, middle, bottom. */
const ALIGN_H: Record<string, string> = { tl: "l", l: "l", bl: "l", t: "c", ctr: "c", b: "c", tr: "r", r: "r", br: "r" };
const ALIGN_V: Record<string, string> = { tl: "t", t: "t", tr: "t", l: "m", ctr: "m", r: "m", bl: "b", b: "b", br: "b" };

function picturePaint(f: RFill, defs: SVGElement, w: number, h: number, href: string): Paint {
  const id = nextId();
  const alpha = f.alpha ?? 1;
  if (!f.tile) {
    // The picture sits in the box inset by `stretch`; `crop` is the part of it that shows.
    const st: RRect4 = f.stretch ?? { l: 0, t: 0, r: 0, b: 0 };
    const cr: RRect4 = f.crop ?? { l: 0, t: 0, r: 0, b: 0 };
    const dx = (w * st.l) / 100;
    const dy = (h * st.t) / 100;
    const dw = w * (1 - (st.l + st.r) / 100);
    const dh = h * (1 - (st.t + st.b) / 100);
    const kx = Math.max(1 - (cr.l + cr.r) / 100, 0.001);
    const ky = Math.max(1 - (cr.t + cr.b) / 100, 0.001);
    const iw = dw / kx;
    const ih = dh / ky;
    const image = el("image", { x: num(dx - (iw * cr.l) / 100), y: num(dy - (ih * cr.t) / 100), width: num(iw), height: num(ih), preserveAspectRatio: "none" });
    image.setAttribute("href", href);
    // The tile is the whole box: what the picture leaves bare is clear.
    defs.append(el("pattern", { id, patternUnits: "userSpaceOnUse", x: 0, y: 0, width: num(w), height: num(h) }, image));
    return { paint: `url(#${id})`, opacity: alpha };
  }
  const t = f.tile;
  const pat = el("pattern", { id, patternUnits: "userSpaceOnUse", x: 0, y: 0, width: 16, height: 16 });
  defs.append(pat);
  // The size of a tile is the picture's own (at 96 dpi), scaled: known once the picture has loaded.
  const probe = new Image();
  const place = () => {
    const tw = ((probe.naturalWidth || 96) * 0.75 * t.sx) / 100;
    const th = ((probe.naturalHeight || 96) * 0.75 * t.sy) / 100;
    const fx = t.flip === "x" || t.flip === "xy";
    const fy = t.flip === "y" || t.flip === "xy";
    const cw = fx ? tw * 2 : tw;
    const chh = fy ? th * 2 : th;
    // Where the first tile starts: from the aligned point, plus the offset.
    const ah = ALIGN_H[t.align] ?? "l";
    const av = ALIGN_V[t.align] ?? "t";
    const x0 = (ah === "l" ? 0 : ah === "r" ? w - tw : (w - tw) / 2) + t.tx;
    const y0 = (av === "t" ? 0 : av === "b" ? h - th : (h - th) / 2) + t.ty;
    pat.setAttribute("x", num(x0));
    pat.setAttribute("y", num(y0));
    pat.setAttribute("width", num(cw));
    pat.setAttribute("height", num(chh));
    pat.replaceChildren();
    const put = (x: number, y: number, mx: boolean, my: boolean) => {
      const im = el("image", { x: 0, y: 0, width: num(tw), height: num(th), preserveAspectRatio: "none" });
      im.setAttribute("href", href);
      const tr = `translate(${num(x + (mx ? tw : 0))} ${num(y + (my ? th : 0))}) scale(${mx ? -1 : 1} ${my ? -1 : 1})`;
      im.setAttribute("transform", tr);
      pat.append(im);
    };
    put(0, 0, false, false);
    if (fx) put(tw, 0, true, false);
    if (fy) put(0, th, false, true);
    if (fx && fy) put(tw, th, true, true);
  };
  probe.onload = place;
  probe.src = href;
  place();
  return { paint: `url(#${id})`, opacity: alpha };
}

// ---- patterns ---------------------------------------------------------------------------------------------------

/** A pattern is an 8 by 8 bitmap of foreground and background, as in Office. `on(x, y)` says whether a pixel is foreground. */
type Bit = (x: number, y: number) => boolean;

const mod = (n: number, m: number) => ((n % m) + m) % m;

/** Bayer's 8 by 8 ordered dither: a pixel is on when its rank is below the coverage. */
const BAYER = (() => {
  const b = [[0]];
  let n = 1;
  let m = b;
  while (n < 8) {
    const next: number[][] = [];
    for (let y = 0; y < n * 2; y++) {
      next.push([]);
      for (let x = 0; x < n * 2; x++) {
        const q = m[y % n][x % n] * 4;
        next[y].push(q + (y < n ? (x < n ? 0 : 2) : x < n ? 3 : 1));
      }
    }
    m = next;
    n *= 2;
  }
  return m;
})();

const pct = (cover: number): Bit => (x, y) => BAYER[mod(y, 8)][mod(x, 8)] < Math.round((cover / 100) * 64);
const rows = (thick: number, period: number): Bit => (_x, y) => mod(y, period) < thick;
const cols = (thick: number, period: number): Bit => (x) => mod(x, period) < thick;
const dn = (thick: number, period: number): Bit => (x, y) => mod(x - y, period) < thick;
const up = (thick: number, period: number): Bit => (x, y) => mod(x + y, period) < thick;
const either = (a: Bit, b: Bit): Bit => (x, y) => a(x, y) || b(x, y);
const check = (cell: number): Bit => (x, y) => (Math.floor(mod(x, 8) / cell) + Math.floor(mod(y, 8) / cell)) % 2 === 0;
const CONFETTI = [0b10000100, 0b00100000, 0b01000010, 0b00010001, 0b10000100, 0b00100100, 0b01001000, 0b00010010];

const PATTERN_BITS: Record<string, Bit> = {
  pct5: pct(5),
  pct10: pct(10),
  pct20: pct(20),
  pct25: pct(25),
  pct30: pct(30),
  pct40: pct(40),
  pct50: pct(50),
  pct60: pct(60),
  pct70: pct(70),
  pct75: pct(75),
  pct80: pct(80),
  pct90: pct(90),
  horz: rows(2, 8),
  vert: cols(2, 8),
  ltHorz: rows(1, 4),
  ltVert: cols(1, 4),
  dkHorz: rows(2, 4),
  dkVert: cols(2, 4),
  narHorz: rows(1, 2),
  narVert: cols(1, 2),
  dashHorz: (x, y) => mod(y, 4) === 0 && mod(x + (mod(y, 8) < 4 ? 0 : 4), 8) < 4,
  dashVert: (x, y) => mod(x, 4) === 0 && mod(y + (mod(x, 8) < 4 ? 0 : 4), 8) < 4,
  cross: either(rows(1, 8), cols(1, 8)),
  dnDiag: dn(2, 8),
  upDiag: up(2, 8),
  ltDnDiag: dn(1, 4),
  ltUpDiag: up(1, 4),
  dkDnDiag: dn(2, 4),
  dkUpDiag: up(2, 4),
  wdDnDiag: dn(4, 8),
  wdUpDiag: up(4, 8),
  dashDnDiag: (x, y) => mod(x - y, 8) === 0 && mod(x + y, 8) < 4,
  dashUpDiag: (x, y) => mod(x + y, 8) === 0 && mod(x - y, 8) < 4,
  diagCross: either(dn(1, 8), up(1, 8)),
  smCheck: check(1),
  lgCheck: check(4),
  smGrid: either(rows(1, 4), cols(1, 4)),
  lgGrid: either(rows(1, 8), cols(1, 8)),
  dotGrid: (x, y) => mod(x, 4) === 0 && mod(y, 4) === 0,
  smConfetti: (x, y) => ((CONFETTI[mod(y, 8)] >> mod(x, 8)) & 1) === 1,
  lgConfetti: (x, y) => ((CONFETTI[mod(Math.floor(y / 2), 8)] >> mod(Math.floor(x / 2), 8)) & 1) === 1,
  horzBrick: (x, y) => mod(y, 4) === 0 || mod(x + (mod(y, 8) < 4 ? 0 : 4), 8) === 0,
  diagBrick: (x, y) => mod(x - y, 8) === 0 || mod(x + y, 8) === 4,
  solidDmnd: (x, y) => Math.abs(mod(x, 8) - 3.5) + Math.abs(mod(y, 8) - 3.5) < 3.6,
  openDmnd: (x, y) => {
    const d = Math.abs(mod(x, 8) - 3.5) + Math.abs(mod(y, 8) - 3.5);
    return d >= 2.6 && d < 4.1;
  },
  dotDmnd: (x, y) => (mod(x, 4) === 0 && mod(y, 4) === 0) || (mod(x, 4) === 2 && mod(y, 4) === 2),
  plaid: (x, y) => (mod(y, 8) < 4 ? mod(x + y, 2) === 0 : false) || mod(x, 8) < 2,
  sphere: (x, y) => {
    const dx = mod(x, 4) - 1.5;
    const dy = mod(y, 4) - 1.5;
    return dx * dx + dy * dy < 2.2;
  },
  weave: (x, y) => (mod(Math.floor(x / 2) + Math.floor(y / 2), 2) === 0 ? mod(y, 2) === 0 : mod(x, 2) === 0),
  divot: (x, y) => mod(x, 8) === mod(y, 8) || (mod(x, 4) === 1 && mod(y, 4) === 2),
  shingle: (x, y) => mod(x - Math.floor(y / 2) * 2, 8) < 2 || mod(y, 4) === 3,
  wave: (x, y) => mod(y - Math.round(1.5 * Math.sin((mod(x, 8) / 8) * Math.PI * 2)), 4) === 0,
  trellis: (x, y) => mod(x + y, 4) === 0 || mod(x - y, 4) === 0 ? mod(y, 8) < 4 : false,
  zigZag: (x, y) => mod(y - (mod(x, 8) < 4 ? mod(x, 8) : 7 - mod(x, 8)), 8) === 0,
};

/** The 8 by 8 bitmap of a preset pattern: true where a pixel is the foreground. */
export function patternPixels(name: string): boolean[][] {
  const bit = PATTERN_BITS[name] ?? PATTERN_BITS.pct50;
  return Array.from({ length: 8 }, (_, y) => Array.from({ length: 8 }, (_, x) => bit(x, y)));
}

/** The names of the patterns drawn. */
export const PATTERN_NAMES = Object.keys(PATTERN_BITS);

/** Whether pixel (x, y) of the 8 by 8 tile of a pattern is the foreground (the Format shape panel draws its tiles with this). */
export function patternBit(name: string, x: number, y: number): boolean {
  return (PATTERN_BITS[name] ?? PATTERN_BITS.pct50)(x, y);
}

/** One pixel of a pattern in the viewBox's units is 0.75 point (a pixel at 96 dpi). */
const PIXEL = 0.75;

function patternPaint(f: RFill, defs: SVGElement): Paint {
  const bit = PATTERN_BITS[f.pattern ?? "pct50"] ?? PATTERN_BITS.pct50;
  const id = nextId();
  let d = "";
  for (let y = 0; y < 8; y++) {
    let run = -1;
    for (let x = 0; x <= 8; x++) {
      const on = x < 8 && bit(x, y);
      if (on && run < 0) run = x;
      if (!on && run >= 0) {
        d += `M${run} ${y}h${x - run}v1h${-(x - run)}z`;
        run = -1;
      }
    }
  }
  const size = 8 * PIXEL;
  const pat = el(
    "pattern",
    { id, patternUnits: "userSpaceOnUse", width: num(size), height: num(size), patternContentUnits: "userSpaceOnUse", "data-pattern": f.pattern ?? "pct50" },
    el("rect", { width: num(size), height: num(size), fill: hex(f.color2 ?? "FFFFFF"), "fill-opacity": f.alpha ?? 1 }),
    el("path", { d, fill: hex(f.color), "fill-opacity": f.alpha ?? 1, transform: `scale(${PIXEL})`, "shape-rendering": "crispEdges" }),
  );
  defs.append(pat);
  return { paint: `url(#${id})`, opacity: 1 };
}
