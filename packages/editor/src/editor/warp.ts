// WordArt's text warps (`a:prstTxWarp`, VML shape types 136 to 175) as geometry, without a DOM.
//
// Two kinds of warp:
//  - a mesh: the block of text is stretched over the shape's box and bent, so a point of the text (u across, v down,
//    both 0 to 1) lands at `mesh(u, v)` in the box. Most presets are a band between a top and a bottom curve; the
//    pour presets are rings; the fades are trapezoids. The drawing code cuts the text into thin slices and gives
//    each slice the affine map that fits the mesh there (`slices`).
//  - a path: the text sits on an arc of an ellipse (arch up and down, circle, button) with its letters upright to
//    the path and not distorted (`followPath`); the drawing code uses an SVG <textPath>.
//
// Sizes are in the box's own units (points). Adjust values are the file's (`adj`: 100000ths, angles 60000ths of
// a degree), read from `RWarp.adj`.

export type Pt = [number, number];
export type Mesh = (u: number, v: number) => Pt;
type Adj = Record<string, number> | undefined;

export type WarpKind = "plain" | "mesh" | "path";

export interface WarpDef {
  kind: WarpKind;
  mesh?: Mesh;
  /** The preset is drawn as a stand-in: its overall shape is right, its exact curves are not Word's. */
  approx: boolean;
}

/** Presets whose bending is exact (a plain box) or true text on a path. */
export const EXACT = new Set(["textPlain", "textArchUp", "textArchDown", "textCircle"]);

/** Presets drawn as plain text because their mapping (a different bend for each line) is not drawn. */
export const FLAT = new Set(["textDeflateInflate", "textDeflateInflateDeflate"]);

/** Presets that put the text on a path. */
export const PATHS = new Set(["textArchUp", "textArchDown", "textCircle", "textButton"]);

const TAU = Math.PI * 2;
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** An adjust value as a fraction (`adj` 50000 is 0.5), or the preset's default. */
function frac(adj: Adj, name: string, def: number): number {
  const v = adj?.[name];
  return (Number.isFinite(v) ? (v as number) : def) / 100000;
}

/** An angle adjust in degrees. */
function degrees(adj: Adj, name: string, def: number): number {
  const v = adj?.[name];
  return (Number.isFinite(v) ? (v as number) : def * 60000) / 60000;
}

/** A band between two curves: across the box, and down from the top curve to the bottom one. */
function band(w: number, top: (u: number) => number, bottom: (u: number) => number): Mesh {
  return (u, v) => {
    const t = top(u);
    return [u * w, t + v * (bottom(u) - t)];
  };
}

// ---- rings --------------------------------------------------------------------------------------------------

interface Ring {
  /** The point of the arc at angle `a` (degrees, clockwise from the right) and radius `r` (1 is the outer edge), in the box. */
  at(a: number, r: number): Pt;
  /** Radii of the outer edge in the box. */
  rx: number;
  ry: number;
  /** The middle of the circle in the box. */
  cx: number;
  cy: number;
}

/** A unit circle's arc from `a0` through `sweep` degrees, stretched so its bounding box is the whole box. */
function ring(a0: number, sweep: number, w: number, h: number): Ring {
  const rad = (d: number) => (d * Math.PI) / 180;
  let x0 = Infinity;
  let x1 = -Infinity;
  let y0 = Infinity;
  let y1 = -Infinity;
  // The ends of the arc, and the points at 0, 90, 180 and 270 degrees it passes through.
  const angles = [a0, a0 + sweep];
  const lo = Math.min(a0, a0 + sweep);
  const hi = Math.max(a0, a0 + sweep);
  for (let q = Math.ceil(lo / 90) * 90; q <= hi; q += 90) angles.push(q);
  for (const d of angles) {
    const a = rad(d);
    x0 = Math.min(x0, Math.cos(a));
    x1 = Math.max(x1, Math.cos(a));
    y0 = Math.min(y0, Math.sin(a));
    y1 = Math.max(y1, Math.sin(a));
  }
  const sx = w / Math.max(x1 - x0, 1e-6);
  const sy = h / Math.max(y1 - y0, 1e-6);
  return {
    at: (a, r) => [(r * Math.cos(rad(a)) - x0) * sx, (r * Math.sin(rad(a)) - y0) * sy],
    rx: sx,
    ry: sy,
    cx: -x0 * sx,
    cy: -y0 * sy,
  };
}

/**
 * Text poured into a ring segment: across the text is the angle, down the text is the radius. `topOutside` puts the
 * tops of the letters on the outer edge (arch up, circle); otherwise they face the middle (arch down).
 */
function poured(a0: number, sweep: number, inner: number, topOutside: boolean, w: number, h: number): Mesh {
  const r = ring(a0, sweep, w, h);
  return (u, v) => {
    const radius = topOutside ? 1 - v * (1 - inner) : inner + v * (1 - inner);
    return r.at(a0 + sweep * u, radius);
  };
}

// ---- the presets ---------------------------------------------------------------------------------------------

/** The warp of a preset in a box of `w` by `h`. */
export function warpDef(preset: string, adj: Adj, w: number, h: number): WarpDef {
  const plain: WarpDef = { kind: "plain", mesh: (u, v) => [u * w, v * h], approx: false };
  if (PATHS.has(preset)) return { kind: "path", approx: !EXACT.has(preset) };
  if (FLAT.has(preset)) return { ...plain, approx: true };
  const mesh = (m: Mesh, approx = true): WarpDef => ({ kind: "mesh", mesh: m, approx });
  switch (preset) {
    case "textPlain":
      return plain;
    case "textStop": {
      const dy = clamp(Math.min(w, h) * frac(adj, "adj", 25000), 0, Math.min(w, h) / 2);
      const top = (u: number) => {
        const x = u * w;
        return dy <= 0 ? 0 : x < dy ? dy - x : x > w - dy ? x - (w - dy) : 0;
      };
      return mesh(band(w, top, (u) => h - top(u)), false);
    }
    case "textTriangle":
    case "textTriangleInverted": {
      const apex = clamp(frac(adj, "adj", 50000), 0.001, 0.999);
      // The text is half as tall at its ends as at the apex.
      const tri = (u: number) => (h / 2) * (u <= apex ? 1 - u / apex : (u - apex) / (1 - apex));
      return preset === "textTriangle" ? mesh(band(w, tri, () => h)) : mesh(band(w, () => 0, (u) => h - tri(u)));
    }
    case "textChevron":
    case "textChevronInverted": {
      const dy = clamp(frac(adj, "adj", 25000), 0, 0.9) * h;
      const v = (u: number) => Math.abs(2 * u - 1);
      const top = preset === "textChevron" ? (u: number) => dy * v(u) : (u: number) => dy * (1 - v(u));
      return mesh(band(w, top, (u) => top(u) + (h - dy)), false);
    }
    case "textRingOutside":
      return mesh(poured(degrees(adj, "adj", 180), 360, clamp(frac(adj, "adj2", 50000), 0.05, 0.95), true, w, h));
    case "textRingInside":
      return mesh(poured(degrees(adj, "adj", 180), 360, clamp(frac(adj, "adj2", 50000), 0.05, 0.95), false, w, h));
    case "textArchUpPour":
    case "textButtonPour": {
      const a0 = clamp(degrees(adj, "adj", 180), 91, 269);
      return mesh(poured(a0, 540 - 2 * a0, clamp(frac(adj, "adj2", 50000), 0.05, 0.95), true, w, h));
    }
    case "textArchDownPour": {
      const a0 = clamp(degrees(adj, "adj", 180), 91, 269);
      return mesh(poured(a0, -(540 - 2 * a0), clamp(frac(adj, "adj2", 50000), 0.05, 0.95), false, w, h));
    }
    case "textCirclePour":
      return mesh(poured(degrees(adj, "adj", 180), 360, clamp(frac(adj, "adj2", 50000), 0.05, 0.95), true, w, h));
    case "textCurveUp":
    case "textCurveDown": {
      const a = clamp(frac(adj, "adj", 45977), 0.05, 0.95);
      const top = (u: number) => a * h * Math.cos((Math.PI * u) / 2);
      const bottom = (u: number) => h - (1 - a) * h * Math.sin((Math.PI * u) / 2);
      return preset === "textCurveUp" ? mesh(band(w, top, bottom)) : mesh(band(w, (u) => h - bottom(u), (u) => h - top(u)));
    }
    case "textCascadeUp": {
      const a = clamp(frac(adj, "adj", 44444), 0.05, 0.95);
      return mesh(band(w, (u) => (1 - a) * h * (1 - u), () => h));
    }
    case "textCascadeDown": {
      const a = clamp(frac(adj, "adj", 44444), 0.05, 0.95);
      return mesh(band(w, () => 0, (u) => h - (1 - a) * h * u));
    }
    case "textWave1":
    case "textWave2":
    case "textDoubleWave1":
    case "textWave4": {
      const double = preset === "textDoubleWave1" || preset === "textWave4";
      const a = clamp(2 * frac(adj, "adj1", double && preset === "textDoubleWave1" ? 6250 : 12500) * h, 0, h * 0.45);
      const phase = frac(adj, "adj2", 0) * TAU;
      const periods = double ? 2 : 1;
      const sign = preset === "textWave1" || preset === "textDoubleWave1" ? -1 : 1;
      const top = (u: number) => a + sign * a * Math.sin(TAU * periods * u + phase);
      return mesh(band(w, top, (u) => top(u) + (h - 2 * a)));
    }
    case "textInflate": {
      const d = clamp(frac(adj, "adj", 18000), 0, 0.5) * h;
      const top = (u: number) => d * (1 - Math.sin(Math.PI * u));
      return mesh(band(w, top, (u) => h - top(u)));
    }
    case "textDeflate": {
      const d = clamp(frac(adj, "adj", 18750), 0, 0.5) * h;
      const top = (u: number) => d * Math.sin(Math.PI * u);
      return mesh(band(w, top, (u) => h - top(u)));
    }
    case "textInflateBottom": {
      const a = clamp(frac(adj, "adj", 60000), 0.05, 1);
      return mesh(band(w, () => 0, (u) => h * (a + (1 - a) * Math.sin(Math.PI * u))));
    }
    case "textDeflateBottom": {
      const a = clamp(frac(adj, "adj", 50000), 0.05, 1);
      return mesh(band(w, () => 0, (u) => h * (1 - (1 - a) * Math.sin(Math.PI * u))));
    }
    case "textInflateTop": {
      const a = clamp(frac(adj, "adj", 60000), 0.05, 1);
      return mesh(band(w, (u) => h * (1 - a) * (1 - Math.sin(Math.PI * u)), () => h));
    }
    case "textDeflateTop": {
      const a = clamp(frac(adj, "adj", 50000), 0.05, 1);
      return mesh(band(w, (u) => h * (1 - a) * Math.sin(Math.PI * u), () => h));
    }
    case "textFadeRight":
    case "textFadeLeft": {
      const dy = clamp(frac(adj, "adj", 33333), 0, 0.49) * h;
      const t = preset === "textFadeRight" ? (u: number) => dy * u : (u: number) => dy * (1 - u);
      return mesh(band(w, t, (u) => h - t(u)), false);
    }
    case "textFadeUp":
    case "textFadeDown": {
      const dx = clamp(frac(adj, "adj", 33333), 0, 0.49) * w;
      return mesh((u, v) => {
        const inset = dx * (preset === "textFadeUp" ? 1 - v : v);
        return [inset + u * (w - 2 * inset), v * h];
      }, false);
    }
    case "textSlantUp": {
      const a = clamp(frac(adj, "adj", 55556), 0.05, 0.95);
      return mesh(band(w, (u) => a * h * (1 - u), (u) => h - (1 - a) * h * u), false);
    }
    case "textSlantDown": {
      // The up slant turned over, with the adjust turned over too.
      const a = clamp(1 - frac(adj, "adj", 44445), 0.05, 0.95);
      return mesh(band(w, (u) => (1 - a) * h * u, (u) => h - a * h * (1 - u)), false);
    }
    case "textCanUp":
    case "textCanDown": {
      const d = clamp(2 * frac(adj, "adj", 14286), 0, 0.45) * h;
      const top = preset === "textCanUp" ? (u: number) => d * (1 - Math.sin(Math.PI * u)) : (u: number) => d * Math.sin(Math.PI * u);
      return mesh(band(w, top, (u) => top(u) + (h - d)));
    }
    default:
      return plain;
  }
}

// ---- slices -------------------------------------------------------------------------------------------------

/** A SVG matrix `[a, b, c, d, e, f]`. */
export type Matrix = [number, number, number, number, number, number];

export interface Slice {
  /** The part of the text this slice shows, in the text's own units (0 to the text's width). */
  x0: number;
  x1: number;
  /** Where that part goes: maps the text's units to the box. */
  matrix: Matrix;
}

/**
 * Cut a block of text `tw` wide and `th` tall into `count` slices, each with the affine map that fits `mesh`
 * at its middle. Text is laid out from (0, 0) at the top left.
 */
export function slices(mesh: Mesh, tw: number, th: number, count: number): Slice[] {
  const out: Slice[] = [];
  const e = 1 / (count * 16);
  for (let i = 0; i < count; i++) {
    const u = (i + 0.5) / count;
    const c = mesh(u, 0.5);
    const [ax, ay] = mesh(clamp(u + e, 0, 1), 0.5);
    const [bx, by] = mesh(clamp(u - e, 0, 1), 0.5);
    const [dx, dy] = mesh(u, 0.5 + e);
    const [ex, ey] = mesh(u, 0.5 - e);
    const du = 2 * e;
    const ux: Pt = [(ax - bx) / du / tw, (ay - by) / du / tw];
    const vx: Pt = [(dx - ex) / (2 * e) / th, (dy - ey) / (2 * e) / th];
    // The slice's own middle (u * tw across, half the height down) is where the mesh is `c`.
    const px = u * tw;
    const py = th / 2;
    out.push({ x0: (i * tw) / count, x1: ((i + 1) * tw) / count, matrix: [ux[0], ux[1], vx[0], vx[1], c[0] - ux[0] * px - vx[0] * py, c[1] - ux[1] * px - vx[1] * py] });
  }
  return out;
}

/** How many slices a warp over `widthPx` pixels needs: about one for every four pixels. */
export function sliceCount(widthPx: number): number {
  return clamp(Math.round(widthPx / 4), 24, 160);
}

/** The bounds of the warped block: the box that holds the mesh's edges. */
export function meshBounds(mesh: Mesh): { x0: number; y0: number; x1: number; y1: number } {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  for (let i = 0; i <= 24; i++) {
    for (const v of [0, 1]) {
      const [x, y] = mesh(i / 24, v);
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
    }
  }
  return { x0, y0, x1, y1 };
}

// ---- text on a path -----------------------------------------------------------------------------------------

export interface FollowLayout {
  /** The path the text's baseline follows, SVG path data. */
  d: string;
  /** Its length. */
  length: number;
  /** The font size that makes the text as long as the path (limited so the letters stay inside the box). */
  fontSize: number;
  /** The letters' tops face away from the middle. */
  outward: boolean;
}

/** An ellipse arc's length. */
function arcLength(rx: number, ry: number, a0: number, sweep: number): number {
  const n = 240;
  const rad = Math.PI / 180;
  let len = 0;
  for (let i = 0; i < n; i++) {
    const a = (a0 + (sweep * (i + 0.5)) / n) * rad;
    len += Math.hypot(rx * Math.sin(a), ry * Math.cos(a)) * Math.abs(sweep * rad) / n;
  }
  return len;
}

/**
 * The path for text that follows an arc: `unit` is the text's width at font size 1. The font size is chosen so
 * the text is as long as the arc, but no taller than the arc's room allows (capped letters take 0.72 of the size).
 */
export function followPath(preset: string, adj: Adj, w: number, h: number, unit: number): FollowLayout {
  const circle = preset === "textCircle";
  const down = preset === "textArchDown";
  const a0 = clamp(degrees(adj, "adj", 180), circle ? -360 : 91, circle ? 360 : 269);
  const sweep = circle ? 360 : down ? -(540 - 2 * a0) : 540 - 2 * a0;
  const r = ring(a0, sweep, w, h);
  const room = Math.min(r.rx, r.ry);
  const cap = 0.72;
  // Letters on top of the path (arch up, circle) reach out from the baseline; below (arch down) they hang in.
  let fontSize = 1;
  let rho = 1;
  for (let i = 0; i < 3; i++) {
    const len = arcLength(r.rx * rho, r.ry * rho, a0, sweep);
    fontSize = clamp((len * 0.94) / Math.max(unit, 1e-6), 1, (room * 0.42) / cap);
    rho = down ? 1 : clamp(1 - (fontSize * cap) / room, 0.2, 1);
  }
  const length = arcLength(r.rx * rho, r.ry * rho, a0, sweep);
  const p = (a: number): Pt => r.at(a, rho);
  const rx = r.rx * rho;
  const ry = r.ry * rho;
  const f = (n: number) => String(Math.round(n * 100) / 100);
  const flag = sweep > 0 ? 1 : 0;
  let d: string;
  if (Math.abs(sweep) >= 359.99) {
    const mid = p(a0 + sweep / 2);
    const start = p(a0);
    d = `M${f(start[0])} ${f(start[1])}A${f(rx)} ${f(ry)} 0 1 ${flag} ${f(mid[0])} ${f(mid[1])}A${f(rx)} ${f(ry)} 0 1 ${flag} ${f(start[0])} ${f(start[1])}`;
  } else {
    const s = p(a0);
    const e = p(a0 + sweep);
    d = `M${f(s[0])} ${f(s[1])}A${f(rx)} ${f(ry)} 0 ${Math.abs(sweep) > 180 ? 1 : 0} ${flag} ${f(e[0])} ${f(e[1])}`;
  }
  return { d, length, fontSize, outward: !down };
}
