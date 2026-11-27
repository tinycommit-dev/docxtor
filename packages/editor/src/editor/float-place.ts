// Where a floating object (a shape, a picture, a chart) that is placed by the page rather than by the text sits,
// worked out from the page it is on. Word counts a position from something: the page edge, the margins, the column,
// the paragraph. The object's offset (or its alignment, `center` and so on) is measured from the edge of that area,
// so the same math both places the object after layout and turns a drag back into the offset to write.
//
// Only objects drawn out of the text flow (behind or in front of the text) are placed here; a wrapped float keeps
// the position its CSS float gives it.
import type { PageBox } from "./layout";

const PT_PX = 96 / 72;

/** What the math needs from the editor. */
export interface FloatHost {
  /** The element holding the sheets and the text. */
  stage: HTMLElement;
  zoom: number;
  pages: number;
  pageBox(k: number): Readonly<PageBox>;
  /** The margins of the section page k belongs to, in points: top, right, bottom, left. */
  margins(k: number): [number, number, number, number];
}

/** Where a position counts from, as a range on each axis in stage pixels (before zoom). */
export interface Frame {
  h: [number, number];
  v: [number, number];
  /** The page it is on. */
  page: number;
  /** The top-left corner of the paragraph's padding box, which `left` and `top` count from. */
  origin: { x: number; y: number };
}

/** The sheet (page) a paragraph in a header or footer is drawn on, or -1 for a paragraph in the body. */
function sheetOf(stage: HTMLElement, para: HTMLElement): number {
  const sheet = para.closest<HTMLElement>(".sheet");
  if (!sheet) return -1;
  return Array.from(stage.querySelectorAll(".sheets > .sheet")).indexOf(sheet);
}

/** The point on screen as stage pixels (unzoomed). */
function stagePoint(host: FloatHost, clientX: number, clientY: number) {
  const r = host.stage.getBoundingClientRect();
  const z = host.zoom || 1;
  return { x: (clientX - r.left) / z, y: (clientY - r.top) / z };
}

/** Which page a stage y is on (a page owns the gap below it). */
export function pageAtY(host: FloatHost, y: number): number {
  let k = 0;
  while (k < host.pages - 1 && y >= host.pageBox(k + 1).top) k++;
  return k;
}

/** The frame a paragraph's objects count their position from. */
export function frameOf(host: FloatHost, para: HTMLElement, relX: string | undefined, relY: string | undefined): Frame {
  const pr = para.getBoundingClientRect();
  const topLeft = stagePoint(host, pr.left, pr.top);
  const bottomRight = stagePoint(host, pr.right, pr.bottom);
  const sheet = sheetOf(host.stage, para);
  const page = sheet >= 0 ? sheet : pageAtY(host, topLeft.y);
  const box = host.pageBox(page);
  const [mt, , mb] = host.margins(page);
  const pageL = box.left;
  const pageR = box.left + box.w;
  const textL = pageL + box.ml;
  const textR = pageR - box.mr;
  // Facing pages mirror the margins: the inside is the side by the spine, the left one on a right-hand (odd) page.
  const oddPage = box.num % 2 === 1;
  const inside = (side: "l" | "r") => (side === "l") === oddPage;
  const hRange = (): [number, number] => {
    switch (relX) {
      case "page":
        return [pageL, pageR];
      case "leftMargin":
        return [pageL, textL];
      case "rightMargin":
        return [textR, pageR];
      case "insideMargin":
        return inside("l") ? [pageL, textL] : [textR, pageR];
      case "outsideMargin":
        return inside("l") ? [textR, pageR] : [pageL, textL];
      default:
        // The column. Text in columns sits between the paragraph's own edges.
        return para.closest(".cols") ? [topLeft.x, bottomRight.x] : [textL, textR];
    }
  };
  const pageT = box.top;
  const pageB = box.top + box.h;
  const textT = pageT + mt * PT_PX;
  const textB = pageB - mb * PT_PX;
  const vRange = (): [number, number] => {
    switch (relY) {
      case "page":
        return [pageT, pageB];
      case "margin":
        return [textT, textB];
      case "topMargin":
      case "insideMargin":
        return [pageT, textT];
      case "bottomMargin":
      case "outsideMargin":
        return [textB, pageB];
      default:
        // The paragraph (or the line the object is anchored in, which is near enough its top).
        return [topLeft.y, bottomRight.y];
    }
  };
  return { h: hRange(), v: vRange(), page, origin: { x: topLeft.x + para.clientLeft, y: topLeft.y + para.clientTop } };
}

export interface FloatSpec {
  /** Width and height in points. */
  w: number;
  h: number;
  align?: string;
  alignY?: string;
  x?: number;
  y?: number;
  relX?: string;
  relY?: string;
}

/** The top-left corner of the object's unrotated box in stage pixels, from where it says it counts from. */
export function resolve(frame: Frame, s: FloatSpec): { x: number; y: number } {
  const [l, r] = frame.h;
  const [t, b] = frame.v;
  const w = s.w * PT_PX;
  const h = s.h * PT_PX;
  const x = s.align === "center" ? l + (r - l - w) / 2 : s.align === "right" ? r - w : s.align === "left" ? l : l + (s.x ?? 0) * PT_PX;
  const y = s.alignY === "center" ? t + (b - t - h) / 2 : s.alignY === "bottom" ? b - h : s.alignY === "top" ? t : t + (s.y ?? 0) * PT_PX;
  return { x, y };
}

/** The offsets (points) that put the object's unrotated box at a place on screen, counted from its frame. */
export function offsetsAt(host: FloatHost, frame: Frame, client: { left: number; top: number }): { x: number; y: number } {
  const p = stagePoint(host, client.left, client.top);
  return { x: (p.x - frame.h[0]) / PT_PX, y: (p.y - frame.v[0]) / PT_PX };
}

/** Place every object drawn out of the text flow under `root` exactly, for a page-relative position. */
export function placeFloats(host: FloatHost, root: ParentNode) {
  for (const atom of Array.from(root.querySelectorAll<HTMLElement>(".atom[data-abs]"))) {
    const para = atom.parentElement?.closest<HTMLElement>("[data-p], .fp");
    if (!para) continue;
    const w = Number(atom.dataset.w);
    const h = Number(atom.dataset.h);
    if (!Number.isFinite(w) || !Number.isFinite(h)) continue;
    const num = (v: string | undefined) => (v === undefined || v === "" ? undefined : Number(v));
    const frame = frameOf(host, para, atom.dataset.relx, atom.dataset.rely);
    const at = resolve(frame, { w, h, align: atom.dataset.align, alignY: atom.dataset.aligny, x: num(atom.dataset.fx), y: num(atom.dataset.fy), relX: atom.dataset.relx, relY: atom.dataset.rely });
    atom.style.left = `${Math.round((at.x - frame.origin.x) * 100) / 100}px`;
    atom.style.top = `${Math.round((at.y - frame.origin.y) * 100) / 100}px`;
    atom.style.right = "auto";
    atom.style.transform = atom.dataset.rot ? `rotate(${atom.dataset.rot}deg)` : "";
  }
}
