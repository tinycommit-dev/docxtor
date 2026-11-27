// Where a picture, shape or other object sits on the page, as the CSS an inline-or-floating object needs.
// Images and shapes share it: wrap modes `inline`, `square`/`tight` (text flows around: a float), `topBottom`
// (text above and below only), `behind` and `front` (positioned over the paragraph, out of the text flow).
const PT_PX = 96 / 72;

/** The text area of the page the editor shows, in points (floating shapes placed from the page edge need it). */
export const pageMetrics = { textW: 468, marginL: 72 };

export function setPageMetrics(textWpt: number, marginLpt: number) {
  pageMetrics.textW = Math.max(36, textWpt);
  pageMetrics.marginL = marginLpt;
}

export interface Place {
  wrap?: string;
  align?: string;
  /** Points from `relX` / `relY`. */
  x?: number;
  y?: number;
  relX?: string;
  relY?: string;
  /** The object's width in points; given for shapes, which are positioned from their offsets, not only their alignment. */
  w?: number;
  /** Stacking order (`relativeHeight`) of an object behind or in front of the text. */
  z?: number;
  alignY?: string;
  /** Distance of the text from a floating object: top, bottom, left, right in points (Word's wrap distances). */
  dist?: [number, number, number, number];
}

/**
 * The CSS z-index of an object that is drawn out of the text flow. The higher a value, the nearer the viewer; objects
 * behind the text get negative values (below every line of text) in the order of the same numbers. The page
 * keeps these inside the flow's own stacking context.
 */
export function zIndexFor(wrap: string | undefined, z: number | undefined): number {
  if (z === undefined) return wrap === "behind" ? -1 : 2;
  const v = Math.min(Math.max(0, Math.round(z)), 2147483000);
  return wrap === "behind" ? v - 2147483000 - 1 : Math.max(2, v);
}

/**
 * Mark an object that is drawn out of the text flow and placed from the page (see float-place.ts) with what its
 * position counts from, for the pass that places it exactly once the pages are laid out.
 */
export function floatData(el: HTMLElement, o: Place & { w: number; h: number }) {
  if (o.wrap !== "behind" && o.wrap !== "front") return;
  el.dataset.abs = "1";
  if (o.relX) el.dataset.relx = o.relX;
  if (o.relY) el.dataset.rely = o.relY;
  if (o.alignY) el.dataset.aligny = o.alignY;
  if (o.x !== undefined) el.dataset.fx = String(o.x);
  if (o.y !== undefined) el.dataset.fy = String(o.y);
  if (o.align) el.dataset.align = o.align;
  if (o.z !== undefined) el.dataset.z = String(o.z);
}

/** CSS declarations (without the object's own size) for the object's wrap mode and position. */
export function placeCss(o: Place): string {
  const place: string[] = [];
  const align = o.align ?? "left";
  const shape = o.w !== undefined;
  // Offsets count from the paragraph or column when they can (the page edge is turned into one).
  const fromX = (x: number) => (o.relX === "page" ? x - pageMetrics.marginL : x);
  const flowY = !o.relY || o.relY === "paragraph" || o.relY === "line";
  // Word's distances between the text and the object (top, bottom, left, right), where the file gives them.
  const [dT, dB, dL, dR] = o.dist ?? [3, 6, 9, 9];
  if (o.wrap === "square" || o.wrap === "tight") {
    if (align === "center") place.push(`display:block;margin:${o.dist ? dT : 6}pt auto ${o.dist ? dB : 6}pt;width:max-content`);
    else if (shape && !o.align && o.x !== undefined) {
      // A shape at an offset floats to the side it is nearer to, keeping its distance from that edge.
      const x = Math.max(0, fromX(o.x));
      const right = x + o.w! / 2 > pageMetrics.textW / 2;
      const top = flowY ? Math.max(0, o.y ?? 0) : 0;
      if (right) place.push(`float:right;margin:${top}pt ${Math.max(0, pageMetrics.textW - x - o.w!)}pt ${o.dist ? dB : 6}pt ${o.dist ? dL : 9}pt`);
      else place.push(`float:left;margin:${top}pt ${o.dist ? dR : 9}pt ${o.dist ? dB : 6}pt ${x}pt`);
    } else place.push(`float:${align === "right" ? "right" : "left"};margin:${align === "right" ? `${dT}pt 0 ${dB}pt ${dL}pt` : `${dT}pt ${dR}pt ${dB}pt 0`}`);
  } else if (o.wrap === "topBottom") {
    const [tt, bb] = o.dist ? [dT, dB] : [6, 6];
    if (shape && !o.align && o.x !== undefined) {
      // At an offset: that far in from the edge, and that far down from the top of the paragraph.
      place.push(`display:block;width:max-content;margin:${tt + (flowY ? Math.max(0, o.y ?? 0) : 0)}pt auto ${bb}pt ${Math.max(0, fromX(o.x))}pt`);
    } else place.push(`display:block;width:max-content;margin:${align === "left" ? `${tt}pt auto ${bb}pt 0` : align === "right" ? `${tt}pt 0 ${bb}pt auto` : `${tt}pt auto ${bb}pt`}`);
  } else if (o.wrap === "behind" || o.wrap === "front") {
    place.push(`position:absolute;top:${(flowY ? o.y ?? 0 : 0) * PT_PX}px;z-index:${zIndexFor(o.wrap, o.z)}`);
    if (o.x !== undefined && !o.align) place.push(`left:${fromX(o.x) * PT_PX}px`);
    else if (align === "center") place.push(shape ? `left:calc(50% - ${(o.w! * PT_PX) / 2}px)` : "left:50%;transform:translateX(-50%)");
    else if (align === "right") place.push("right:0");
    else place.push("left:0");
  }
  return place.join(";");
}
