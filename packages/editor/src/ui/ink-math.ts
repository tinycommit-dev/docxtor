// The geometry of the drawing tools, on points in any one unit and no DOM: how near a stroke is to a point (the stroke
// eraser), cutting out the part of a stroke under a circle (the point eraser), and which strokes a lasso loop catches.

export interface Pt {
  x: number;
  y: number;
}

/** The distance from a point to a segment. */
export function distToSegment(p: Pt, a: Pt, b: Pt): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/** The distance from a point to a stroke (to its only point, for a dot). */
export function distToStroke(p: Pt, pts: Pt[]): number {
  if (pts.length === 1) return Math.hypot(p.x - pts[0].x, p.y - pts[0].y);
  let best = Infinity;
  for (let i = 1; i < pts.length; i++) best = Math.min(best, distToSegment(p, pts[i - 1], pts[i]));
  return best;
}

/** The same line with a point at least every `step`, so a cut can fall between two far-apart points. */
export function densify(pts: Pt[], step: number): Pt[] {
  const out: Pt[] = [pts[0]];
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1];
    const b = pts[i];
    const n = Math.ceil(Math.hypot(b.x - a.x, b.y - a.y) / step);
    for (let k = 1; k < n; k++) out.push({ x: a.x + ((b.x - a.x) * k) / n, y: a.y + ((b.y - a.y) * k) / n });
    out.push(b);
  }
  return out;
}

/**
 * What is left of a stroke when the circle at `c` with radius `r` is cut out of it: the pieces on either side, each at
 * least two points. Returns null when the circle doesn't touch the stroke. A cut falls within a quarter of the radius.
 */
export function cutOut(pts: Pt[], c: Pt, r: number): Pt[][] | null {
  if (distToStroke(c, pts) > r) return null;
  const dense = densify(pts, Math.max(r / 4, 0.5));
  const pieces: Pt[][] = [];
  let run: Pt[] = [];
  for (const p of dense) {
    if (Math.hypot(p.x - c.x, p.y - c.y) <= r) {
      if (run.length) pieces.push(run);
      run = [];
    } else run.push(p);
  }
  if (run.length) pieces.push(run);
  return pieces.filter((p) => p.length >= 2);
}

/** Whether a point is inside a closed loop of points (even-odd). */
export function inLoop(p: Pt, loop: Pt[]): boolean {
  let inside = false;
  for (let i = 0, j = loop.length - 1; i < loop.length; j = i++) {
    const a = loop[i];
    const b = loop[j];
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
}

function crosses(a: Pt, b: Pt, c: Pt, d: Pt): boolean {
  const o = (p: Pt, q: Pt, r: Pt) => (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
  const d1 = o(c, d, a);
  const d2 = o(c, d, b);
  const d3 = o(a, b, c);
  const d4 = o(a, b, d);
  return d1 * d2 < 0 && d3 * d4 < 0;
}

/** Whether a stroke is caught by a lasso loop: it lies inside, or the loop cuts across it. */
export function caughtBy(loop: Pt[], pts: Pt[]): boolean {
  if (loop.length < 3) return false;
  if (pts.some((p) => inLoop(p, loop))) return true;
  const closed = [...loop, loop[0]];
  for (let i = 1; i < pts.length; i++) {
    for (let j = 1; j < closed.length; j++) if (crosses(pts[i - 1], pts[i], closed[j - 1], closed[j])) return true;
  }
  return false;
}

/** The box round some strokes' points. */
export function boundsOf(strokes: Pt[][]): { left: number; top: number; right: number; bottom: number } | null {
  const all = strokes.flat();
  if (!all.length) return null;
  return {
    left: Math.min(...all.map((p) => p.x)),
    top: Math.min(...all.map((p) => p.y)),
    right: Math.max(...all.map((p) => p.x)),
    bottom: Math.max(...all.map((p) => p.y)),
  };
}
