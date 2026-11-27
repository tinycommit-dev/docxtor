// Frames (`w:framePr`): paragraphs set apart as positioned text blocks, and drop caps. A frame floats to the side of
// the text, which flows round it, like a picture with square wrapping. A drop cap is the first letter in a frame of
// its own, as many lines tall as the text beside it. A frame placed from the page or the margins (not from the text)
// is drawn where it says once the pages are laid out (`placeFrames`) and the text does not flow round it.
import type { FrameFmt, RPara } from "@docxtor/core";
import type { PageBox } from "./layout";
import { nextBlock } from "./far";

const tw = (v: number | undefined) => (v ?? 0) / 15;

/** Does a frame take its position from the page or the margins rather than from the text? Only then does it sit on the page itself. */
export function isAbsolute(f: FrameFmt): boolean {
  if (f.dropCap) return false;
  const v = f.vAnchor ?? "text";
  const hasY = f.y !== undefined || (f.yAlign !== undefined && f.yAlign !== "inline");
  return (v === "page" || v === "margin") && hasY;
}

/** Does text go round the frame (as opposed to above and below it)? */
function wraps(f: FrameFmt): boolean {
  return !(f.wrap === "notBeside" || f.wrap === "none");
}

/**
 * Turn a rendered paragraph into its frame. `gap` is the space the text paragraph after it has above it, which the frame
 * shares so that the two start level, and `pitch` the height (px) of one line of that paragraph. Returns the kind: `drop` for drop caps, `abs` for a frame placed on the page,
 * `float` for the rest.
 */
export function applyFrame(el: HTMLElement, p: RPara, gap: number, pitch = 0): "drop" | "abs" | "float" {
  const f = p.fmt.frame!;
  el.classList.add("pfloat");
  el.dataset.frame = "1";
  const s: string[] = [];
  const hs = tw(f.hSpace);
  if (f.dropCap) {
    // The letter floats at the start of the line, as tall as the lines it spans; a drop cap in the margin hangs out of the text.
    el.classList.add("dropcap");
    el.dataset.drop = f.dropCap;
    el.dataset.lines = String(f.lines ?? 3);
    el.dataset.gap = String(hs);
    // As tall as exactly that many lines, not a hair more, so that the line after them is not pushed aside.
    const height = pitch > 0 ? `;height:${Math.max(1, (f.lines ?? 3) * pitch - 1.5)}px` : "";
    s.push(`float:left;margin:${gap}px ${hs}px 0 0;padding:0;text-indent:0;width:max-content;white-space:nowrap${height}`);
    el.style.cssText += `;${s.join(";")}`;
    return "drop";
  }
  const vs = tw(f.vSpace);
  if (isAbsolute(f)) {
    el.classList.add("abs-frame");
    s.push(`position:absolute;margin:0;padding:0;${f.w ? `width:${tw(f.w)}px` : "width:max-content;max-width:100%"}`);
    for (const k of ["hAnchor", "vAnchor", "x", "y", "xAlign", "yAlign"] as const) if (f[k] !== undefined) el.dataset[`f${k}`] = String(f[k]);
    if (f.hRule === "exact" && f.h) s.push(`height:${tw(f.h)}px;overflow:hidden`);
    else if (f.h) s.push(`min-height:${tw(f.h)}px`);
    if (hs) el.dataset.fh = String(hs);
    el.style.cssText += `;${s.join(";")}`;
    return "abs";
  }
  const side = f.xAlign === "right" || f.xAlign === "outside" ? "right" : f.xAlign === "center" ? "center" : "left";
  const x = f.xAlign ? 0 : Math.max(0, tw(f.x));
  // From the page edge an offset counts the margin in; the editor's margins are what the text area starts at.
  s.push(f.w ? `width:${tw(f.w)}px` : "width:max-content;max-width:100%");
  if (f.hRule === "exact" && f.h) s.push(`height:${tw(f.h)}px;overflow:hidden`);
  else if (f.h) s.push(`min-height:${tw(f.h)}px`);
  const top = gap + vs + (f.vAnchor === "text" || !f.vAnchor ? tw(f.y) : 0);
  if (!wraps(f) || side === "center") {
    s.push(`float:none;margin:${top}px ${side === "right" ? `0 ${vs}px auto` : side === "center" ? `auto ${vs}px auto` : `auto ${vs}px ${x + hs}px`}`);
    if (side === "left") s.push(`margin-right:auto`);
  } else if (side === "right") s.push(`float:right;margin:${top}px ${hs}px ${vs}px ${hs}px`);
  else s.push(`float:left;margin:${top}px ${hs}px ${vs}px ${x + hs}px`);
  el.style.cssText += `;${s.join(";")}`;
  return "float";
}

/** What `placeFrames` needs to know about the pages. */
export interface FrameHost {
  flow: HTMLElement;
  pageAt(y: number): number;
  page(k: number): Readonly<PageBox>;
  /** The top margin of the section page k belongs to (px). */
  marginTop(k: number): number;
  bottomMargin(k: number): number;
  /** The sheets (pages) in order, for frames in a header or footer. */
  sheets(): HTMLElement[];
}

/** Where a frame that counts from the page or margins goes on page k, as the top left corner from the sheet's top left (px). */
function frameSpot(host: FrameHost, el: HTMLElement, k: number): { x: number; y: number } {
  const b = host.page(k);
  const d = el.dataset;
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  const mt = host.marginTop(k);
  const mb = host.bottomMargin(k);
  // Horizontal: from the page edge, the margins or the text column.
  const [l, r] = d.fhAnchor === "page" ? [0, b.w] : [b.ml, b.w - b.mr];
  const hs = parseFloat(d.fh ?? "0") || 0;
  let x = l + tw(Number(d.fx ?? 0)) + hs;
  if (d.fxAlign === "center") x = l + (r - l - w) / 2;
  else if (d.fxAlign === "right") x = r - w - hs;
  else if (d.fxAlign === "inside") x = b.num % 2 === 1 ? l + hs : r - w - hs;
  else if (d.fxAlign === "outside") x = b.num % 2 === 1 ? r - w - hs : l + hs;
  // Vertical: from the page edge, the margins or (for the rest) the paragraph.
  const [t, bt] = d.fvAnchor === "page" ? [0, b.h] : [mt, b.h - mb];
  let y = t + tw(Number(d.fy ?? 0));
  if (d.fyAlign === "center") y = t + (bt - t - h) / 2;
  else if (d.fyAlign === "bottom") y = bt - h;
  else if (d.fyAlign === "inside" || d.fyAlign === "outside") y = d.fyAlign === "inside" ? t : bt - h;
  return { x, y };
}

/** Put the frames that count from the page or margins where they belong, and pull hanging drop caps out into the margin. */
export function placeFrames(host: FrameHost) {
  const { flow } = host;
  for (const el of Array.from(flow.querySelectorAll<HTMLElement>(":scope > .dropcap[data-drop='margin'], :scope .cols > .dropcap[data-drop='margin']"))) {
    // A drop cap in the margin: it takes no room in the text. Measure it, then give it a negative margin as wide as itself.
    el.style.marginLeft = "0px";
    el.style.marginRight = "0px";
    const w = el.getBoundingClientRect().width / zoomOf(flow);
    const gap = parseFloat(el.dataset.gap ?? "") || 0;
    el.dataset.gap = String(gap);
    el.style.marginLeft = `${-(w + gap)}px`;
    el.style.marginRight = `${gap}px`;
  }
  for (const el of Array.from(flow.querySelectorAll<HTMLElement>(":scope > .abs-frame"))) {
    // The frame belongs to the paragraph after it: that decides the page and, for a position from the text, the top.
    let anchor = nextBlock(el);
    while (anchor && (anchor.classList.contains("pfloat") || anchor.classList.contains("colsp"))) anchor = nextBlock(anchor);
    const ref = anchor ?? el;
    const k = host.pageAt(ref.offsetTop);
    const b = host.page(k);
    const { x, y } = frameSpot(host, el, k);
    el.style.left = `${b.left + x}px`;
    el.style.top = `${b.top + y}px`;
    el.style.zIndex = "2";
  }
  // A frame in a header or footer (a page number in the margin) sits on its sheet: from the page, relative to the header's box.
  host.sheets().forEach((sheet, k) => {
    for (const el of Array.from(sheet.querySelectorAll<HTMLElement>(".hf .abs-frame"))) {
      const hf = el.closest<HTMLElement>(".hf")!;
      const { x, y } = frameSpot(host, el, k);
      el.style.left = `${x - hf.offsetLeft}px`;
      el.style.top = `${y - hf.offsetTop}px`;
      el.style.zIndex = "2";
    }
  });
}

const zoomOf = (flow: HTMLElement): number => (flow.offsetWidth ? flow.getBoundingClientRect().width / flow.offsetWidth : 1) || 1;
