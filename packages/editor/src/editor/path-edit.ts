// Editing the points of an outline: the vertices and Bézier handles of a custom shape, as pure functions on lists of path
// commands (`M`, `L`, `C`, `Z`, the form the engine reports in `RPath.cmds` and takes in `set_shape_path`). Nothing here
// touches the page. It also turns the SVG path data of a preset shape into commands, so any shape can be edited.

export type Pt = [number, number];

export interface Cmd {
  t: "M" | "L" | "C" | "Z";
  pts: Pt[];
}

/** A point of a command: the command's index and the point's index in it. */
export interface Ref {
  cmd: number;
  pt: number;
}

export type PointType = "corner" | "smooth" | "straight";

const EPS = 1e-6;
const same = (a: Pt, b: Pt) => Math.abs(a[0] - b[0]) < EPS && Math.abs(a[1] - b[1]) < EPS;
const add = (a: Pt, b: Pt): Pt => [a[0] + b[0], a[1] + b[1]];
const sub = (a: Pt, b: Pt): Pt => [a[0] - b[0], a[1] - b[1]];
const mid = (a: Pt, b: Pt, t = 0.5): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
const len = (a: Pt) => Math.hypot(a[0], a[1]);

export const clone = (cmds: Cmd[]): Cmd[] => cmds.map((c) => ({ t: c.t, pts: c.pts.map((p) => [p[0], p[1]] as Pt) }));

// ---------------------------------------------------------------------------
// SVG path data to commands
// ---------------------------------------------------------------------------

/** An SVG elliptical arc (endpoint form) as cubic Bézier segments: `[handle, handle, end]` each. */
export function arcToCubics(from: Pt, rx: number, ry: number, rotDeg: number, large: boolean, sweep: boolean, to: Pt): Pt[][] {
  if (rx === 0 || ry === 0 || same(from, to)) return [[from, to, to]];
  rx = Math.abs(rx);
  ry = Math.abs(ry);
  const phi = (rotDeg * Math.PI) / 180;
  const [cos, sin] = [Math.cos(phi), Math.sin(phi)];
  const dx2 = (from[0] - to[0]) / 2;
  const dy2 = (from[1] - to[1]) / 2;
  const x1p = cos * dx2 + sin * dy2;
  const y1p = -sin * dx2 + cos * dy2;
  const lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
  if (lam > 1) {
    const s = Math.sqrt(lam);
    rx *= s;
    ry *= s;
  }
  const num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
  const den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
  const coef = (large === sweep ? -1 : 1) * Math.sqrt(Math.max(0, num / den));
  const cxp = coef * ((rx * y1p) / ry);
  const cyp = coef * ((-ry * x1p) / rx);
  const cx = cos * cxp - sin * cyp + (from[0] + to[0]) / 2;
  const cy = sin * cxp + cos * cyp + (from[1] + to[1]) / 2;
  const angle = (ux: number, uy: number, vx: number, vy: number) => Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
  const theta = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry);
  let delta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
  if (!sweep && delta > 0) delta -= 2 * Math.PI;
  else if (sweep && delta < 0) delta += 2 * Math.PI;
  const n = Math.max(1, Math.ceil(Math.abs(delta) / (Math.PI / 2) - 1e-9));
  const step = delta / n;
  const k = (4 / 3) * Math.tan(step / 4);
  const at = (a: number): Pt => [cx + rx * Math.cos(a) * cos - ry * Math.sin(a) * sin, cy + rx * Math.cos(a) * sin + ry * Math.sin(a) * cos];
  const tangent = (a: number): Pt => [-rx * Math.sin(a) * cos - ry * Math.cos(a) * sin, -rx * Math.sin(a) * sin + ry * Math.cos(a) * cos];
  const out: Pt[][] = [];
  for (let i = 0; i < n; i++) {
    const a = theta + step * i;
    const b = theta + step * (i + 1);
    const [pa, pb, da, db] = [at(a), at(b), tangent(a), tangent(b)];
    out.push([add(pa, [k * da[0], k * da[1]]), sub(pb, [k * db[0], k * db[1]]), i === n - 1 ? to : pb]);
  }
  return out;
}

/**
 * The commands of SVG path data as the presets write it: absolute `M L C Q A Z` (and relative or `H V` forms are not
 * needed). Arcs and quadratic curves become cubic curves.
 */
export function svgToCmds(d: string): Cmd[] {
  const tokens = d.match(/[MLCQAZHVmlcqazhv]|-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/g) ?? [];
  const out: Cmd[] = [];
  let i = 0;
  let cur: Pt = [0, 0];
  let start: Pt = [0, 0];
  const num = () => Number(tokens[i++]);
  const pt = (): Pt => [num(), num()];
  let op = "";
  while (i < tokens.length) {
    if (/^[A-Za-z]$/.test(tokens[i])) op = tokens[i++];
    switch (op) {
      case "M":
        cur = start = pt();
        out.push({ t: "M", pts: [cur] });
        op = "L";
        break;
      case "L":
        cur = pt();
        out.push({ t: "L", pts: [cur] });
        break;
      case "H":
        cur = [num(), cur[1]];
        out.push({ t: "L", pts: [cur] });
        break;
      case "V":
        cur = [cur[0], num()];
        out.push({ t: "L", pts: [cur] });
        break;
      case "C": {
        const [a, b, c] = [pt(), pt(), pt()];
        out.push({ t: "C", pts: [a, b, c] });
        cur = c;
        break;
      }
      case "Q": {
        const [q, e] = [pt(), pt()];
        out.push({ t: "C", pts: [mid(cur, q, 2 / 3), mid(e, q, 2 / 3), e] });
        cur = e;
        break;
      }
      case "A": {
        const [rx, ry, rot, large, sweep] = [num(), num(), num(), num(), num()];
        const e = pt();
        for (const seg of arcToCubics(cur, rx, ry, rot, large !== 0, sweep !== 0, e)) out.push({ t: "C", pts: seg });
        cur = e;
        break;
      }
      case "Z":
      case "z":
        out.push({ t: "Z", pts: [] });
        cur = start;
        break;
      default:
        i++;
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Looking at an outline
// ---------------------------------------------------------------------------

export const point = (cmds: Cmd[], r: Ref): Pt => cmds[r.cmd].pts[r.pt];

/** Where a subpath starts and ends (`end` is exclusive) for the command at `i`. */
export function subpath(cmds: Cmd[], i: number): { start: number; end: number } {
  let start = i;
  while (start > 0 && cmds[start].t !== "M") start--;
  let end = i + 1;
  while (end < cmds.length && cmds[end].t !== "M") end++;
  return { start, end };
}

/** The point each move, line and curve ends on, in order. */
export function anchors(cmds: Cmd[]): Ref[] {
  const out: Ref[] = [];
  cmds.forEach((c, cmd) => {
    if (c.t === "M" || c.t === "L") out.push({ cmd, pt: 0 });
    else if (c.t === "C") out.push({ cmd, pt: 2 });
  });
  return out;
}

const isAnchorCmd = (c: Cmd | undefined) => !!c && (c.t === "M" || c.t === "L" || c.t === "C");
const endOf = (c: Cmd): Pt => c.pts[c.pts.length - 1];

/** The last anchor of the subpath that starts at `start`, and whether it is closed. */
function lastAnchor(cmds: Cmd[], start: number, end: number): { idx: number; closed: boolean } {
  let closed = false;
  let idx = start;
  for (let i = start + 1; i < end; i++) {
    if (cmds[i].t === "Z") closed = true;
    else if (isAnchorCmd(cmds[i])) idx = i;
  }
  return { idx, closed };
}

/** The other command that stands for the same vertex when a closed subpath ends where it started. */
export function twin(cmds: Cmd[], r: Ref): Ref | null {
  const { start, end } = subpath(cmds, r.cmd);
  const { idx, closed } = lastAnchor(cmds, start, end);
  if (!closed || idx === start || !same(endOf(cmds[idx]), cmds[start].pts[0])) return null;
  if (r.cmd === start) return { cmd: idx, pt: cmds[idx].t === "C" ? 2 : 0 };
  if (r.cmd === idx) return { cmd: start, pt: 0 };
  return null;
}

/** The anchor before the command at `i` (the point it starts from). */
function previousAnchor(cmds: Cmd[], i: number): Ref | null {
  const { start } = subpath(cmds, i);
  for (let j = i - 1; j >= start; j--) if (isAnchorCmd(cmds[j])) return { cmd: j, pt: cmds[j].t === "C" ? 2 : 0 };
  return null;
}

/** The anchor after the one at `r`, going round to the start when the subpath is closed. */
function nextAnchor(cmds: Cmd[], r: Ref): Ref | null {
  const { start, end } = subpath(cmds, r.cmd);
  for (let j = r.cmd + 1; j < end; j++) if (isAnchorCmd(cmds[j])) return { cmd: j, pt: cmds[j].t === "C" ? 2 : 0 };
  const { closed } = lastAnchor(cmds, start, end);
  if (closed) {
    const first = nextAnchorFromStart(cmds, start, end);
    if (first && !(first.cmd === r.cmd)) return first;
  }
  return null;
}

function nextAnchorFromStart(cmds: Cmd[], start: number, end: number): Ref | null {
  for (let j = start + 1; j < end; j++) if (isAnchorCmd(cmds[j])) return { cmd: j, pt: cmds[j].t === "C" ? 2 : 0 };
  return null;
}

/** The Bézier handles of an anchor: the one on the segment that arrives, and the one on the segment that leaves. */
export function handlesOf(cmds: Cmd[], r: Ref): { in: Ref | null; out: Ref | null } {
  let inH: Ref | null = null;
  let outH: Ref | null = null;
  if (cmds[r.cmd].t === "C" && r.pt === 2) inH = { cmd: r.cmd, pt: 1 };
  const next = cmds[r.cmd + 1];
  if (next?.t === "C") outH = { cmd: r.cmd + 1, pt: 0 };
  // A vertex where a closed outline ends and starts again carries the handles of both ends.
  const t = twin(cmds, r);
  if (t) {
    const other = handlesOf(cmds, t);
    inH ??= other.in;
    outH ??= other.out;
  }
  return { in: inH, out: outH };
}

/** The anchor a handle belongs to. */
export function anchorOf(cmds: Cmd[], h: Ref): Ref {
  if (h.pt === 1) return { cmd: h.cmd, pt: 2 };
  return previousAnchor(cmds, h.cmd) ?? { cmd: h.cmd, pt: 0 };
}

/** How an anchor behaves: straight (no curve touches it), smooth (its handles line up) or corner. */
export function pointType(cmds: Cmd[], r: Ref): PointType {
  const h = handlesOf(cmds, r);
  if (!h.in && !h.out) return "straight";
  const a = point(cmds, r);
  if (h.in && h.out) {
    const [i, o] = [sub(point(cmds, h.in), a), sub(point(cmds, h.out), a)];
    const [li, lo] = [len(i), len(o)];
    if (li < EPS || lo < EPS) return "corner";
    // Opposite directions: the cross product is zero and the dot product negative.
    const cross = Math.abs(i[0] * o[1] - i[1] * o[0]) / (li * lo);
    const dot = (i[0] * o[0] + i[1] * o[1]) / (li * lo);
    return cross < 1e-3 && dot < 0 ? "smooth" : "corner";
  }
  return "corner";
}

// ---------------------------------------------------------------------------
// Changing an outline
// ---------------------------------------------------------------------------

/** Move an anchor, taking its handles (and its twin's) along, or move a handle on its own. */
export function movePoint(cmds: Cmd[], r: Ref, to: Pt, opts: { handle?: boolean; keepSmooth?: boolean } = {}): Cmd[] {
  const out = clone(cmds);
  if (opts.handle) {
    const owner = anchorOf(out, r);
    const wasSmooth = opts.keepSmooth !== false && pointType(out, owner) === "smooth";
    const hs = handlesOf(out, owner);
    const other = hs.in && hs.in.cmd === r.cmd && hs.in.pt === r.pt ? hs.out : hs.in;
    const a = point(out, owner);
    const lenOther = other ? len(sub(point(out, other), a)) : 0;
    out[r.cmd].pts[r.pt] = to;
    // A smooth point keeps its two handles in line: the other one turns to face away, keeping its length.
    if (wasSmooth && other) {
      const d = sub(to, a);
      const l = len(d);
      if (l > EPS) out[other.cmd].pts[other.pt] = [a[0] - (d[0] / l) * lenOther, a[1] - (d[1] / l) * lenOther];
    }
    return out;
  }
  const from = point(out, r);
  const delta = sub(to, from);
  const move = (ref: Ref) => {
    const c = out[ref.cmd];
    c.pts[ref.pt] = add(c.pts[ref.pt], delta);
  };
  const t = twin(out, r);
  for (const a of t ? [r, t] : [r]) {
    // The handles on both sides of the anchor go with it.
    if (out[a.cmd].t === "C" && a.pt === 2) move({ cmd: a.cmd, pt: 1 });
    if (out[a.cmd + 1]?.t === "C") move({ cmd: a.cmd + 1, pt: 0 });
    move(a);
  }
  return out;
}

/** The point on a segment, and which segment, nearest to `p`. */
export interface Hit {
  /** The command index of the segment (a line, a curve, or the close). */
  cmd: number;
  t: number;
  dist: number;
  pt: Pt;
}

function cubicAt(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: number): Pt {
  const u = 1 - t;
  return [u * u * u * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t * t * t * p3[0], u * u * u * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t * t * t * p3[1]];
}

/** The segments of an outline as `{cmd, from, ...}`: where each starts, and (for a close) the line back to the start. */
function segments(cmds: Cmd[]): { cmd: number; from: Pt; pts: Pt[]; close?: boolean }[] {
  const out: { cmd: number; from: Pt; pts: Pt[]; close?: boolean }[] = [];
  let cur: Pt = [0, 0];
  let start: Pt = [0, 0];
  cmds.forEach((c, cmd) => {
    if (c.t === "M") cur = start = c.pts[0];
    else if (c.t === "L") {
      out.push({ cmd, from: cur, pts: c.pts });
      cur = c.pts[0];
    } else if (c.t === "C") {
      out.push({ cmd, from: cur, pts: c.pts });
      cur = c.pts[2];
    } else if (!same(cur, start)) {
      out.push({ cmd, from: cur, pts: [start], close: true });
      cur = start;
    }
  });
  return out;
}

/** The nearest point of the outline to `p` within `tol` (in the outline's own units), or null. */
export function nearest(cmds: Cmd[], p: Pt, tol: number): Hit | null {
  let best: Hit | null = null;
  for (const s of segments(cmds)) {
    const curve = s.pts.length === 3;
    const steps = curve ? 40 : 1;
    for (let k = 0; k < steps; k++) {
      // A short piece of the segment: closest point on it to p.
      const [t0, t1] = [k / steps, (k + 1) / steps];
      const a = curve ? cubicAt(s.from, s.pts[0], s.pts[1], s.pts[2], t0) : s.from;
      const b = curve ? cubicAt(s.from, s.pts[0], s.pts[1], s.pts[2], t1) : s.pts[0];
      const ab = sub(b, a);
      const l2 = ab[0] * ab[0] + ab[1] * ab[1];
      const u = l2 < EPS ? 0 : Math.max(0, Math.min(1, ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2));
      const q = add(a, [ab[0] * u, ab[1] * u]);
      const dist = len(sub(p, q));
      if (dist <= tol && (!best || dist < best.dist)) best = { cmd: s.cmd, t: t0 + (t1 - t0) * u, dist, pt: q };
    }
  }
  return best;
}

/** Put a new anchor on a segment at parameter `t`, keeping the outline's shape. The new anchor is at the returned ref. */
export function splitAt(cmds: Cmd[], cmd: number, t: number): { cmds: Cmd[]; added: Ref } {
  const out = clone(cmds);
  const seg = segments(out).find((s) => s.cmd === cmd);
  if (!seg) return { cmds: out, added: { cmd, pt: 0 } };
  if (seg.pts.length === 3) {
    const [p0, [c1, c2, p3]] = [seg.from, [seg.pts[0], seg.pts[1], seg.pts[2]]];
    const [a, b, c] = [mid(p0, c1, t), mid(c1, c2, t), mid(c2, p3, t)];
    const [d, e] = [mid(a, b, t), mid(b, c, t)];
    const q = mid(d, e, t);
    out.splice(cmd, 1, { t: "C", pts: [a, d, q] }, { t: "C", pts: [e, c, p3] });
    return { cmds: out, added: { cmd, pt: 2 } };
  }
  const q = mid(seg.from, seg.pts[0], t);
  if (seg.close) out.splice(cmd, 0, { t: "L", pts: [q] });
  else out.splice(cmd, 1, { t: "L", pts: [q] }, { t: "L", pts: [seg.pts[0]] });
  return { cmds: out, added: { cmd, pt: 0 } };
}

/** How many anchors the subpath of `r` has (a closing vertex counted once). */
function anchorCount(cmds: Cmd[], r: Ref): number {
  const { start, end } = subpath(cmds, r.cmd);
  let n = 0;
  for (let i = start; i < end; i++) if (isAnchorCmd(cmds[i])) n++;
  return twin(cmds, { cmd: start, pt: 0 }) ? n - 1 : n;
}

/** Take an anchor out, joining its two segments into one. Null when the subpath would be left with too few points. */
export function deleteAnchor(cmds: Cmd[], r: Ref): Cmd[] | null {
  const { start, end } = subpath(cmds, r.cmd);
  const closed = lastAnchor(cmds, start, end).closed;
  if (anchorCount(cmds, r) <= (closed ? 3 : 2)) return null;
  const out = clone(cmds);
  const tw = twin(out, r);
  if (r.cmd === start || (tw && tw.cmd === start)) {
    // The start of the outline: the next vertex becomes the start.
    const last = tw ? (r.cmd === start ? tw.cmd : r.cmd) : -1;
    const firstSeg = start + 1;
    const first = out[firstSeg];
    const newStart = endOf(first);
    if (tw && last > firstSeg) {
      // Closed where it started: the segment that arrived at the start now arrives at the new start.
      const prev = previousAnchor(out, last)!;
      const p = point(out, prev);
      const lastCmd = out[last];
      const bothLines = lastCmd.t === "L" && first.t === "L";
      out[last] = bothLines ? { t: "L", pts: [newStart] } : { t: "C", pts: [lastCmd.t === "C" ? lastCmd.pts[0] : p, first.t === "C" ? first.pts[1] : newStart, newStart] };
    }
    out.splice(firstSeg, 1);
    out[start] = { t: "M", pts: [newStart] };
    return out;
  }
  const i = r.cmd;
  const next = out[i + 1];
  if (!next || !isAnchorCmd(next)) {
    // The last anchor: the outline just ends one point sooner.
    out.splice(i, 1);
    return out;
  }
  const prev = previousAnchor(out, i)!;
  const p = point(out, prev);
  const n = endOf(next);
  const cur = out[i];
  const merged: Cmd = cur.t === "L" && next.t === "L" ? { t: "L", pts: [n] } : { t: "C", pts: [cur.t === "C" ? cur.pts[0] : p, next.t === "C" ? next.pts[1] : n, n] };
  out.splice(i, 2, merged);
  return out;
}

/** Make the segments around an anchor lines (straight), curves with free handles (corner) or curves that run through it in line (smooth). */
export function setPointType(cmds: Cmd[], r: Ref, type: PointType): Cmd[] {
  const out = clone(cmds);
  const a = point(out, r);
  const tw = twin(out, r);
  const refs = tw ? [r, tw] : [r];
  if (type === "straight") {
    for (const ref of refs) {
      const c = out[ref.cmd];
      if (c.t === "C" && ref.pt === 2) out[ref.cmd] = { t: "L", pts: [c.pts[2]] };
      const n = out[ref.cmd + 1];
      if (n?.t === "C") out[ref.cmd + 1] = { t: "L", pts: [n.pts[2]] };
    }
    return out;
  }
  // Curves on both sides first (a line becomes a curve with handles a third of the way along).
  const curve = (idx: number) => {
    const c = out[idx];
    if (c?.t !== "L") return;
    const from = previousAnchor(out, idx);
    if (!from) return;
    const p0 = point(out, from);
    out[idx] = { t: "C", pts: [mid(p0, c.pts[0], 1 / 3), mid(p0, c.pts[0], 2 / 3), c.pts[0]] };
  };
  for (const ref of refs) {
    curve(ref.cmd);
    curve(ref.cmd + 1);
  }
  if (type === "smooth") {
    const prevRef = previousAnchor(out, r.cmd) ?? (tw ? previousAnchor(out, tw.cmd) : null);
    const nextRef = nextAnchor(out, r) ?? (tw ? nextAnchor(out, tw) : null);
    const hs = handlesOf(out, r);
    const dir = prevRef && nextRef ? sub(point(out, nextRef), point(out, prevRef)) : hs.out ? sub(point(out, hs.out), a) : hs.in ? sub(a, point(out, hs.in)) : null;
    if (dir && len(dir) > EPS) {
      const u: Pt = [dir[0] / len(dir), dir[1] / len(dir)];
      const set = (h: Ref | null, sign: number, other: Ref | null) => {
        if (!h) return;
        const cur = len(sub(point(out, h), a));
        const l = cur > EPS ? cur : other ? (len(sub(point(out, other), a)) || 1) : 1;
        out[h.cmd].pts[h.pt] = [a[0] + u[0] * l * sign, a[1] + u[1] * l * sign];
      };
      // Both handles, whichever vertex of a closed pair carries them.
      const all = handlesOf(out, r);
      set(all.in, -1, null);
      set(all.out, 1, null);
    }
  }
  return out;
}

/** The bounds of the outline (its anchors and handles: a loose box, used to size a hit area). */
export function pointBounds(cmds: Cmd[]): { x0: number; y0: number; x1: number; y1: number } {
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const c of cmds) {
    for (const p of c.pts) {
      x0 = Math.min(x0, p[0]);
      y0 = Math.min(y0, p[1]);
      x1 = Math.max(x1, p[0]);
      y1 = Math.max(y1, p[1]);
    }
  }
  return { x0, y0, x1, y1 };
}

/** SVG path data for commands (the form `svgToCmds` reads). */
export function cmdsToD(cmds: Cmd[]): string {
  const n = (v: number) => String(Math.round(v * 100) / 100);
  return cmds
    .map((c) => (c.t === "Z" ? "Z" : `${c.t}${c.pts.map((p) => `${n(p[0])},${n(p[1])}`).join(" ")}`))
    .join(" ");
}

// ---------------------------------------------------------------------------
// Drawing by hand: the preview of what the engine makes of the points (it simplifies, then smooths, the same way)
// ---------------------------------------------------------------------------

/** Leave out points that add nothing to the line (Ramer, Douglas and Peucker), to a tolerance. */
export function simplify(points: Pt[], tol: number): Pt[] {
  if (points.length < 3) return points.slice();
  const keep = new Array<boolean>(points.length).fill(false);
  keep[0] = keep[points.length - 1] = true;
  const stack: [number, number][] = [[0, points.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop()!;
    const [pa, pb] = [points[a], points[b]];
    const [dx, dy] = [pb[0] - pa[0], pb[1] - pa[1]];
    const l = Math.hypot(dx, dy);
    let far: [number, number] = [0, a];
    for (let i = a + 1; i < b; i++) {
      const p = points[i];
      const d = l === 0 ? Math.hypot(p[0] - pa[0], p[1] - pa[1]) : Math.abs((p[0] - pa[0]) * dy - (p[1] - pa[1]) * dx) / l;
      if (d > far[0]) far = [d, i];
    }
    if (far[0] > tol) {
      keep[far[1]] = true;
      stack.push([a, far[1]], [far[1], b]);
    }
  }
  return points.filter((_, i) => keep[i]);
}

function catmull(p0: Pt, p1: Pt, p2: Pt, p3: Pt): Cmd {
  return { t: "C", pts: [[p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6], [p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6], p2] };
}

/** A smooth open curve through the points: the commands after the move to the first point. */
export function smoothOpen(points: Pt[], tol = 0.35): Cmd[] {
  const pts = simplify(points, tol);
  if (pts.length < 2) return [];
  if (pts.length === 2) return [{ t: "L", pts: [pts[1]] }];
  const at = (i: number) => pts[Math.max(0, Math.min(pts.length - 1, i))];
  const out: Cmd[] = [];
  for (let i = 0; i < pts.length - 1; i++) out.push(catmull(at(i - 1), at(i), at(i + 1), at(i + 2)));
  return out;
}

/** A smooth closed curve through the points, wrapping round to the first: a move, the curves and a close. */
export function smoothClosed(points: Pt[]): Cmd[] {
  const n = points.length;
  const out: Cmd[] = [{ t: "M", pts: [points[0]] }];
  if (n < 3) {
    for (const p of points.slice(1)) out.push({ t: "L", pts: [p] });
    return [...out, { t: "Z", pts: [] }];
  }
  for (let i = 0; i < n; i++) out.push(catmull(points[(i + n - 1) % n], points[i], points[(i + 1) % n], points[(i + 2) % n]));
  return [...out, { t: "Z", pts: [] }];
}
