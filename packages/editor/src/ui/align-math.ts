// The arithmetic of aligning and distributing objects, on boxes in any one unit. No DOM: the boxes come from the
// pages and the answers are distances to move each box (see align.ts for how they become ops).

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** What to line the boxes up on. */
export type Edge = "left" | "center" | "right" | "top" | "middle" | "bottom";
export type Axis = "h" | "v";

/** The axis an edge belongs to: left, center and right move boxes across; top, middle and bottom move them down. */
export function axisOf(edge: Edge): Axis {
  return edge === "left" || edge === "center" || edge === "right" ? "h" : "v";
}

/** The smallest box around all of them. */
export function bounds(rects: Rect[]): Rect {
  return {
    left: Math.min(...rects.map((r) => r.left)),
    top: Math.min(...rects.map((r) => r.top)),
    right: Math.max(...rects.map((r) => r.right)),
    bottom: Math.max(...rects.map((r) => r.bottom)),
  };
}

/** How far each box moves (across for h, down for v) so that its `edge` meets the reference's. */
export function alignMoves(rects: Rect[], edge: Edge, ref: Rect): number[] {
  return rects.map((r) => {
    switch (edge) {
      case "left":
        return ref.left - r.left;
      case "center":
        return (ref.left + ref.right) / 2 - (r.left + r.right) / 2;
      case "right":
        return ref.right - r.right;
      case "top":
        return ref.top - r.top;
      case "middle":
        return (ref.top + ref.bottom) / 2 - (r.top + r.bottom) / 2;
      default:
        return ref.bottom - r.bottom;
    }
  });
}

/**
 * How far each box moves along `axis` so the gaps between neighbours are equal. The boxes keep their order. Without
 * a reference the outermost two stay where they are and the others spread between them (Word's "align to selected
 * objects"); with one (the margins or the page) the first lands on its start and the last on its end.
 */
export function distributeMoves(rects: Rect[], axis: Axis, ref?: Rect): number[] {
  const n = rects.length;
  const moves = new Array<number>(n).fill(0);
  if (n < 2) return moves;
  const start = (r: Rect) => (axis === "h" ? r.left : r.top);
  const end = (r: Rect) => (axis === "h" ? r.right : r.bottom);
  const order = rects.map((_, i) => i).sort((a, b) => start(rects[a]) - start(rects[b]) || a - b);
  const from = ref ? start(ref) : Math.min(...rects.map(start));
  const to = ref ? end(ref) : Math.max(...rects.map(end));
  const sizes = order.map((i) => end(rects[i]) - start(rects[i]));
  const gap = (to - from - sizes.reduce((a, b) => a + b, 0)) / (n - 1);
  let at = from;
  order.forEach((i, k) => {
    moves[i] = at - start(rects[i]);
    at += sizes[k] + gap;
  });
  return moves;
}
