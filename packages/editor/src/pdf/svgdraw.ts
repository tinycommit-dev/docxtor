// The geometry behind drawing SVG as PDF vectors: path data (with arcs turned into curves), the basic shapes as paths, matrices,
// and the colour gradients a PDF shading draws. No DOM: the page walker reads each element's attributes and style and calls these.
import { num } from "./writer.ts";

/** `[a, b, c, d, e, f]`: x' = a x + c y + e, y' = b x + d y + f. */
export type Matrix = [number, number, number, number, number, number];

export const IDENTITY: Matrix = [1, 0, 0, 1, 0, 0];

/** The matrix that does `a` after `b`. */
export function mul(a: Matrix, b: Matrix): Matrix {
  return [
    a[0] * b[0] + a[2] * b[1],
    a[1] * b[0] + a[3] * b[1],
    a[0] * b[2] + a[2] * b[3],
    a[1] * b[2] + a[3] * b[3],
    a[0] * b[4] + a[2] * b[5] + a[4],
    a[1] * b[4] + a[3] * b[5] + a[5],
  ];
}

export const apply = (m: Matrix, x: number, y: number): [number, number] => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];

/** The matrix that undoes `m`; null when it has none (it flattens the plane). */
export function invert(m: Matrix): Matrix | null {
  const det = m[0] * m[3] - m[1] * m[2];
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return null;
  return [m[3] / det, -m[1] / det, -m[2] / det, m[0] / det, (m[2] * m[5] - m[3] * m[4]) / det, (m[1] * m[4] - m[0] * m[5]) / det];
}

/** How much a matrix scales lengths, on average (the square root of its area factor). */
export const scaleOf = (m: Matrix): number => Math.sqrt(Math.abs(m[0] * m[3] - m[1] * m[2]));

/** Whether a matrix keeps squares square: a turn, an even scale and a move, with no skew, stretch or mirror. */
export function isSimilarity(m: Matrix): boolean {
  const tolerance = 1e-3 * scaleOf(m) + 1e-9;
  return Math.abs(m[0] - m[3]) < tolerance && Math.abs(m[1] + m[2]) < tolerance;
}

/** A path segment in absolute coordinates. */
export type Seg = ["M", number, number] | ["L", number, number] | ["C", number, number, number, number, number, number] | ["Z"];

const ARG_COUNT: Record<string, number> = { M: 2, L: 2, H: 1, V: 1, C: 6, S: 4, Q: 4, T: 2, A: 7, Z: 0 };

/** Split path data into commands with their numbers; null when it is not path data. */
function tokens(d: string): { cmd: string; args: number[] }[] | null {
  const out: { cmd: string; args: number[] }[] = [];
  const re = /([MmLlHhVvCcSsQqTtAaZz])|([+-]?(?:\d*\.\d+|\d+\.?)(?:[eE][+-]?\d+)?)|([\s,]+)/y;
  let cur: { cmd: string; args: number[] } | null = null;
  // The arc flags can be written with no space after them ("a1 1 0 00 1 1"): the numbers of an arc are read with that in mind.
  let i = 0;
  while (i < d.length) {
    re.lastIndex = i;
    const m = re.exec(d);
    if (!m) return null;
    i = re.lastIndex;
    if (m[3]) continue;
    if (m[1]) {
      cur = { cmd: m[1], args: [] };
      out.push(cur);
    } else if (cur) {
      if (cur.cmd.toUpperCase() === "A" && (cur.args.length % 7 === 3 || cur.args.length % 7 === 4)) {
        // A flag is one digit, even when the next number follows it directly.
        const text = m[2]!;
        if (/^[01]/.test(text) && text.length > 1 && !/^[01]\./.test(text)) {
          cur.args.push(Number(text[0]));
          i -= text.length - 1;
          continue;
        }
      }
      cur.args.push(Number(m[2]));
    } else return null;
  }
  return out;
}

/** An arc from (x0, y0) to (x, y) as cubic curves (SVG's endpoint form, SVG 1.1 appendix F.6). */
function arc(x0: number, y0: number, rxIn: number, ryIn: number, rotation: number, large: boolean, sweep: boolean, x: number, y: number): Seg[] {
  if (x0 === x && y0 === y) return [];
  let rx = Math.abs(rxIn);
  let ry = Math.abs(ryIn);
  if (!rx || !ry) return [["L", x, y]];
  const phi = (rotation * Math.PI) / 180;
  const cos = Math.cos(phi);
  const sin = Math.sin(phi);
  const dx = (x0 - x) / 2;
  const dy = (y0 - y) / 2;
  const x1 = cos * dx + sin * dy;
  const y1 = -sin * dx + cos * dy;
  const lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
  if (lambda > 1) {
    const s = Math.sqrt(lambda);
    rx *= s;
    ry *= s;
  }
  const numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
  const denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
  const k = (large === sweep ? -1 : 1) * Math.sqrt(Math.max(0, numerator / denominator));
  const cxp = (k * rx * y1) / ry;
  const cyp = (-k * ry * x1) / rx;
  const cx = cos * cxp - sin * cyp + (x0 + x) / 2;
  const cy = sin * cxp + cos * cyp + (y0 + y) / 2;
  const angle = (ux: number, uy: number, vx: number, vy: number) => Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
  const theta = angle(1, 0, (x1 - cxp) / rx, (y1 - cyp) / ry);
  let delta = angle((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
  if (!sweep && delta > 0) delta -= 2 * Math.PI;
  else if (sweep && delta < 0) delta += 2 * Math.PI;
  const parts = Math.max(1, Math.ceil(Math.abs(delta) / (Math.PI / 2) - 1e-9));
  const step = delta / parts;
  const t = (4 / 3) * Math.tan(step / 4);
  const out: Seg[] = [];
  for (let i = 0; i < parts; i++) {
    const a0 = theta + i * step;
    const a1 = a0 + step;
    const point = (a: number, ox: number, oy: number): [number, number] => {
      const px = rx * (Math.cos(a) + ox);
      const py = ry * (Math.sin(a) + oy);
      return [cos * px - sin * py + cx, sin * px + cos * py + cy];
    };
    const c1 = point(a0, -t * Math.sin(a0), t * Math.cos(a0));
    const c2 = point(a1, t * Math.sin(a1), -t * Math.cos(a1));
    const end = i === parts - 1 ? ([x, y] as [number, number]) : point(a1, 0, 0);
    out.push(["C", c1[0], c1[1], c2[0], c2[1], end[0], end[1]]);
  }
  return out;
}

/** Path data as absolute moves, lines and cubic curves (quadratic curves and arcs are converted); null when it can't be read. */
export function parsePathData(d: string): Seg[] | null {
  const toks = tokens(d);
  if (!toks || !toks.length) return toks ? [] : null;
  const out: Seg[] = [];
  let x = 0;
  let y = 0;
  let sx = 0;
  let sy = 0;
  // The last control point, for smooth curves.
  let cx = 0;
  let cy = 0;
  let last = "";
  for (const { cmd, args } of toks) {
    const up = cmd.toUpperCase();
    const rel = cmd !== up;
    const n = ARG_COUNT[up];
    if (n === undefined) return null;
    if (up === "Z") {
      out.push(["Z"]);
      x = sx;
      y = sy;
      last = "Z";
      continue;
    }
    if (args.length === 0 || args.length % n !== 0) return null;
    // After the first pair, more pairs of a move are lines.
    for (let i = 0, k = 0; i < args.length; i += n, k++) {
      const a = args.slice(i, i + n);
      const ox = rel ? x : 0;
      const oy = rel ? y : 0;
      let kind = up;
      if (up === "M" && k > 0) kind = "L";
      switch (kind) {
        case "M":
          x = a[0]! + ox;
          y = a[1]! + oy;
          sx = x;
          sy = y;
          out.push(["M", x, y]);
          break;
        case "L":
          x = a[0]! + ox;
          y = a[1]! + oy;
          out.push(["L", x, y]);
          break;
        case "H":
          x = a[0]! + (rel ? x : 0);
          out.push(["L", x, y]);
          break;
        case "V":
          y = a[0]! + (rel ? y : 0);
          out.push(["L", x, y]);
          break;
        case "C": {
          const [x1, y1, x2, y2, ex, ey] = [a[0]! + ox, a[1]! + oy, a[2]! + ox, a[3]! + oy, a[4]! + ox, a[5]! + oy];
          out.push(["C", x1, y1, x2, y2, ex, ey]);
          cx = x2;
          cy = y2;
          x = ex;
          y = ey;
          break;
        }
        case "S": {
          const [x1, y1] = last === "C" || last === "S" ? [2 * x - cx, 2 * y - cy] : [x, y];
          const [x2, y2, ex, ey] = [a[0]! + ox, a[1]! + oy, a[2]! + ox, a[3]! + oy];
          out.push(["C", x1, y1, x2, y2, ex, ey]);
          cx = x2;
          cy = y2;
          x = ex;
          y = ey;
          break;
        }
        case "Q":
        case "T": {
          let qx: number;
          let qy: number;
          let ex: number;
          let ey: number;
          if (kind === "Q") {
            qx = a[0]! + ox;
            qy = a[1]! + oy;
            ex = a[2]! + ox;
            ey = a[3]! + oy;
          } else {
            [qx, qy] = last === "Q" || last === "T" ? [2 * x - cx, 2 * y - cy] : [x, y];
            ex = a[0]! + ox;
            ey = a[1]! + oy;
          }
          // A quadratic curve as the cubic with the same shape.
          out.push(["C", x + (2 / 3) * (qx - x), y + (2 / 3) * (qy - y), ex + (2 / 3) * (qx - ex), ey + (2 / 3) * (qy - ey), ex, ey]);
          cx = qx;
          cy = qy;
          x = ex;
          y = ey;
          break;
        }
        case "A": {
          const ex = a[5]! + ox;
          const ey = a[6]! + oy;
          out.push(...arc(x, y, a[0]!, a[1]!, a[2]!, a[3] !== 0, a[4] !== 0, ex, ey));
          x = ex;
          y = ey;
          break;
        }
      }
      last = kind;
    }
  }
  // A path has to begin with a move.
  return out.length && out[0]![0] !== "M" ? null : out;
}

const KAPPA = 0.5522847498307936;

/** A rectangle, with rounded corners when `rx` or `ry` is more than zero. */
export function rectSegments(x: number, y: number, w: number, h: number, rx = 0, ry = 0): Seg[] {
  if (w <= 0 || h <= 0) return [];
  rx = Math.min(rx, w / 2);
  ry = Math.min(ry, h / 2);
  if (rx <= 0 || ry <= 0) return [["M", x, y], ["L", x + w, y], ["L", x + w, y + h], ["L", x, y + h], ["Z"]];
  const kx = rx * KAPPA;
  const ky = ry * KAPPA;
  return [
    ["M", x + rx, y],
    ["L", x + w - rx, y],
    ["C", x + w - rx + kx, y, x + w, y + ry - ky, x + w, y + ry],
    ["L", x + w, y + h - ry],
    ["C", x + w, y + h - ry + ky, x + w - rx + kx, y + h, x + w - rx, y + h],
    ["L", x + rx, y + h],
    ["C", x + rx - kx, y + h, x, y + h - ry + ky, x, y + h - ry],
    ["L", x, y + ry],
    ["C", x, y + ry - ky, x + rx - kx, y, x + rx, y],
    ["Z"],
  ];
}

/** An ellipse (a circle when the radii are equal) as four curves. */
export function ellipseSegments(cx: number, cy: number, rx: number, ry: number): Seg[] {
  if (rx <= 0 || ry <= 0) return [];
  const kx = rx * KAPPA;
  const ky = ry * KAPPA;
  return [
    ["M", cx + rx, cy],
    ["C", cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry],
    ["C", cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy],
    ["C", cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry],
    ["C", cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy],
    ["Z"],
  ];
}

/** A line, or the lines through a list of points (`polyline`), closed for a `polygon`. */
export function polySegments(points: number[], close: boolean): Seg[] {
  if (points.length < 4) return [];
  const out: Seg[] = [["M", points[0]!, points[1]!]];
  for (let i = 2; i + 1 < points.length; i += 2) out.push(["L", points[i]!, points[i + 1]!]);
  if (close) out.push(["Z"]);
  return out;
}

/** The numbers of a `points` attribute. */
export function pointList(s: string): number[] {
  return (s.match(/[+-]?(?:\d*\.\d+|\d+\.?)(?:[eE][+-]?\d+)?/g) ?? []).map(Number);
}

export function transformSegments(segs: Seg[], m: Matrix): Seg[] {
  return segs.map((s): Seg => {
    if (s[0] === "Z") return s;
    if (s[0] === "C") {
      const a = apply(m, s[1], s[2]);
      const b = apply(m, s[3], s[4]);
      const c = apply(m, s[5], s[6]);
      return ["C", a[0], a[1], b[0], b[1], c[0], c[1]];
    }
    const p = apply(m, s[1], s[2]);
    return [s[0], p[0], p[1]];
  });
}

/** Path operators for segments (`x y m`, `x y l`, `… c`, `h`). */
export function pathOps(segs: Seg[]): string {
  return segs
    .map((s) => {
      switch (s[0]) {
        case "M":
          return `${num(s[1])} ${num(s[2])} m`;
        case "L":
          return `${num(s[1])} ${num(s[2])} l`;
        case "C":
          return `${num(s[1])} ${num(s[2])} ${num(s[3])} ${num(s[4])} ${num(s[5])} ${num(s[6])} c`;
        default:
          return "h";
      }
    })
    .join(" ");
}

// -- gradients ----------------------------------------------------------------------------------------------------------

export interface Stop {
  /** 0 to 1 along the gradient. */
  offset: number;
  rgb: [number, number, number];
}

/** The colour function of a gradient: one exponential function between two stops, or a stitching of them for more. */
export function gradientFunction(stops: Stop[]): string {
  // Offsets never go backwards, and the first and last stops are at the ends (the colour is held beyond them).
  const clean: Stop[] = [];
  for (const s of stops) clean.push({ offset: Math.min(1, Math.max(clean.length ? clean[clean.length - 1]!.offset : 0, s.offset)), rgb: s.rgb });
  if (clean.length === 1) clean.push({ ...clean[0]!, offset: 1 });
  const color = (c: [number, number, number]) => c.map(num).join(" ");
  const piece = (a: Stop, b: Stop) => `<< /FunctionType 2 /Domain [0 1] /C0 [${color(a.rgb)}] /C1 [${color(b.rgb)}] /N 1 >>`;
  if (clean.length === 2 && clean[0]!.offset <= 0 && clean[1]!.offset >= 1) return piece(clean[0]!, clean[1]!);
  // Stitch: hold the first colour up to the first stop and the last colour after the last.
  const full: Stop[] = [...clean];
  if (full[0]!.offset > 0) full.unshift({ offset: 0, rgb: full[0]!.rgb });
  if (full[full.length - 1]!.offset < 1) full.push({ offset: 1, rgb: full[full.length - 1]!.rgb });
  const functions: string[] = [];
  const bounds: number[] = [];
  for (let i = 0; i + 1 < full.length; i++) {
    functions.push(piece(full[i]!, full[i + 1]!));
    if (i + 2 < full.length) bounds.push(full[i + 1]!.offset);
  }
  return `<< /FunctionType 3 /Domain [0 1] /Functions [${functions.join(" ")}] /Bounds [${bounds.map(num).join(" ")}] /Encode [${functions.map(() => "0 1").join(" ")}] >>`;
}

/** An axial (linear) shading from (x0, y0) to (x1, y1), colours held beyond both ends. */
export function axialShading(x0: number, y0: number, x1: number, y1: number, stops: Stop[]): string {
  return `<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [${[x0, y0, x1, y1].map(num).join(" ")}] /Function ${gradientFunction(stops)} /Extend [true true] >>`;
}

/** A radial shading from a focal point (fx, fy) to the circle at (cx, cy) with radius r. */
export function radialShading(fx: number, fy: number, cx: number, cy: number, r: number, stops: Stop[]): string {
  return `<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [${[fx, fy, 0, cx, cy, r].map(num).join(" ")}] /Function ${gradientFunction(stops)} /Extend [true true] >>`;
}
