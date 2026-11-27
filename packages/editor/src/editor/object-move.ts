// Where an object on the page is, and how to move it: the box it fills on screen before any turn, and its
// offsets from what Word counts a position from (the page edge, the margins, the column or the paragraph).
// Pictures use it to be dragged and to show their position as numbers; shapes have the same math in their tool.
import type { Editor } from "./editor";
import { frameOf, offsetsAt } from "./float-place";

const PT_PX = 96 / 72;

/** The element that draws an object: the picture, its box when it has formatting, a chart or a placeholder. */
export function mediaOf(a: HTMLElement): HTMLElement | null {
  return a.querySelector<HTMLElement>(".pic, .pic-missing, img, .chart-svg");
}

export interface ScreenBox {
  left: number;
  top: number;
  width: number;
  height: number;
  rot: number;
}

/** The object's own box on screen in viewport pixels, before it is turned, and how far it is turned. */
export function screenBox(a: HTMLElement, zoom: number): ScreenBox {
  const m = mediaOf(a) ?? a;
  const r = m.getBoundingClientRect();
  const rot = Number((m.closest(".pic") as HTMLElement | null)?.dataset.rot) || 0;
  if (!rot) return { left: r.left, top: r.top, width: r.width, height: r.height, rot: 0 };
  const w = Number(a.dataset.w) * PT_PX * zoom;
  const h = Number(a.dataset.h) * PT_PX * zoom;
  const cx = r.left + r.width / 2;
  const cy = r.top + r.height / 2;
  return { left: cx - w / 2, top: cy - h / 2, width: w, height: h, rot };
}

/** A horizontal reference the page layout understands (a character counts as the column it is in). */
export const horizontalRef = (rel?: string): string => (!rel || rel === "character" ? "column" : rel);
/** A vertical reference the page layout understands (a line counts as the paragraph it is in). */
export const verticalRef = (rel?: string): string => (!rel || rel === "line" ? "paragraph" : rel);

/**
 * The object's top-left corner as points from what `relX` and `relY` count from. `at` is where the corner is on
 * screen (the object's own place by default). Null when the object is not on a page.
 */
export function offsetsFor(ed: Editor, a: HTMLElement, relX: string | undefined, relY: string | undefined, at?: { left: number; top: number }): { x: number; y: number } | null {
  const para = a.closest<HTMLElement>("[data-p]");
  if (!para) return null;
  const frame = frameOf(ed.floatHost, para, horizontalRef(relX), verticalRef(relY));
  const b = at ?? screenBox(a, ed.zoom);
  const o = offsetsAt(ed.floatHost, frame, b);
  return { x: Math.round(o.x * 10) / 10, y: Math.round(o.y * 10) / 10 };
}
