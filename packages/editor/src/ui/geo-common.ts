// What the curve, freeform, edit points and connector tools share: finding the shape under the pointer, where its box and
// connection sites are on screen, and turning a point on screen into the offsets an op takes.
import type { MPos, RShape, RSite } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { frameOf, offsetsAt } from "../editor/float-place";

export const PT_PX = 96 / 72;
export type Pt = [number, number];

/** A shape's box on screen: its centre and size in pixels (zoom included), its turn and mirroring. */
export interface Box {
  cx: number;
  cy: number;
  w: number;
  h: number;
  rot: number;
  flipH: boolean;
  flipV: boolean;
}

/** The atom of the shape at a position. */
export function atomAt(ed: Editor, pos: MPos): HTMLElement | null {
  return ed.paraElement(pos.p)?.querySelector<HTMLElement>(`:scope > .atom.shape[data-o="${pos.o}"]`) ?? null;
}

/** The position of the object an atom stands for. */
export function posOf(atom: HTMLElement): MPos | null {
  const para = atom.closest<HTMLElement>("[data-p]");
  return para ? { p: Number(para.dataset.p), o: Number(atom.dataset.o) } : null;
}

/** Where the box of a shape's atom is on screen. */
export function boxOf(ed: Editor, atom: HTMLElement, s: Pick<RShape, "flipH" | "flipV">): Box {
  const r = atom.getBoundingClientRect();
  const z = ed.zoom || 1;
  return {
    cx: r.left + r.width / 2,
    cy: r.top + r.height / 2,
    w: Number(atom.dataset.w) * PT_PX * z,
    h: Number(atom.dataset.h) * PT_PX * z,
    rot: Number(atom.dataset.rot) || 0,
    flipH: !!s.flipH,
    flipV: !!s.flipV,
  };
}

/** A point of the box (pixels from its unturned top-left corner) as it is on screen. */
export function toScreen(b: Box, lx: number, ly: number): Pt {
  const x = b.flipH ? b.w - lx : lx;
  const y = b.flipV ? b.h - ly : ly;
  const [dx, dy] = [x - b.w / 2, y - b.h / 2];
  const [sin, cos] = [Math.sin((b.rot * Math.PI) / 180), Math.cos((b.rot * Math.PI) / 180)];
  return [b.cx + dx * cos - dy * sin, b.cy + dx * sin + dy * cos];
}

/** The reverse of `toScreen`: a point on screen as pixels from the box's unturned top-left corner. */
export function fromScreen(b: Box, sx: number, sy: number): Pt {
  const [dx, dy] = [sx - b.cx, sy - b.cy];
  const [sin, cos] = [Math.sin((-b.rot * Math.PI) / 180), Math.cos((-b.rot * Math.PI) / 180)];
  const [x, y] = [dx * cos - dy * sin + b.w / 2, dx * sin + dy * cos + b.h / 2];
  return [b.flipH ? b.w - x : x, b.flipV ? b.h - y : y];
}

/** A direction in a box (degrees) as it points on screen, snapped to an axis: a unit step along x or y. */
export function dirOnScreen(b: Box, ang: number): Pt {
  let [vx, vy] = [Math.cos((ang * Math.PI) / 180), Math.sin((ang * Math.PI) / 180)];
  if (b.flipH) vx = -vx;
  if (b.flipV) vy = -vy;
  const [sin, cos] = [Math.sin((b.rot * Math.PI) / 180), Math.cos((b.rot * Math.PI) / 180)];
  [vx, vy] = [vx * cos - vy * sin, vx * sin + vy * cos];
  return Math.abs(vx) >= Math.abs(vy) ? [vx >= 0 ? 1 : -1, 0] : [0, vy >= 0 ? 1 : -1];
}

/** A connection site on screen: where it is and which way a connector leaves it. */
export interface ScreenSite {
  idx: number;
  x: number;
  y: number;
  dir: Pt;
}

/** The sites of the shape at `pos`, on screen. */
export function sitesOnScreen(ed: Editor, atom: HTMLElement, pos: MPos, s: RShape): ScreenSite[] {
  let sites: RSite[] = [];
  try {
    sites = ed.engine.shapeSites(pos);
  } catch {
    return [];
  }
  const b = boxOf(ed, atom, s);
  const z = ed.zoom || 1;
  return sites.map((site, idx) => {
    const [x, y] = toScreen(b, site.x * PT_PX * z, site.y * PT_PX * z);
    return { idx, x, y, dir: dirOnScreen(b, site.ang) };
  });
}

/** Whether a shape is a connector the tools can glue: a line or connector with its own connection record. */
export function isConnector(s: RShape | null | undefined): boolean {
  return !!s && s.kind === "shape" && s.connect !== undefined && /^(line|lineInv|straightConnector1|bentConnector[2-5]|curvedConnector[2-5])$/.test(s.geometry);
}

/**
 * The paragraph a drawing started at a point on screen belongs to: the one under it, else the nearest in the text.
 * Objects are anchored to paragraphs and counted from them.
 */
export function paragraphAt(ed: Editor, x: number, y: number): HTMLElement | null {
  const under = document.elementsFromPoint(x, y).map((el) => el.closest<HTMLElement>("[data-p]")).find((p) => p && ed.flow.contains(p) && !p.closest(".atom"));
  if (under) return under;
  let best: { el: HTMLElement; d: number } | null = null;
  for (const el of Array.from(ed.flow.querySelectorAll<HTMLElement>("[data-p]"))) {
    if (el.closest(".atom")) continue;
    const b = el.getBoundingClientRect();
    const d = y < b.top ? b.top - y : y > b.bottom ? y - b.bottom : 0;
    if (!best || d < best.d) best = { el, d };
  }
  return best?.el ?? null;
}

/** A point on screen as points from the column and the paragraph (what `x` and `y` of an op count from). */
export function offsetsIn(ed: Editor, para: HTMLElement, x: number, y: number): Pt {
  const frame = frameOf(ed.floatHost, para, undefined, undefined);
  const o = offsetsAt(ed.floatHost, frame, { left: x, top: y });
  return [Math.round(o.x * 100) / 100, Math.round(o.y * 100) / 100];
}

/** The shape atom under a point on screen (not a connector, not a member of a group), if any. */
export function shapeUnder(ed: Editor, x: number, y: number, skip?: HTMLElement | null): HTMLElement | null {
  for (const el of document.elementsFromPoint(x, y)) {
    const atom = el.closest<HTMLElement>(".atom.shape");
    if (atom && ed.flow.contains(atom) && atom !== skip && !atom.classList.contains("line")) return atom;
  }
  return null;
}
