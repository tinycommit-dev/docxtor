// Watermarks: text (or a picture) behind the text of a page, centred in its margin box. Word keeps them
// as a shape in each header; the engine reads them into `PageSetup.watermark` and hides the shape from
// the header's text, so they are drawn here, on the sheet under the editable flow. The same drawing
// serves the page and the preview in the Watermark dialog.
import type { Watermark } from "@docxtor/core";
import { fontStack } from "./render";
import { hex as hexOf } from "./color.ts";

const PX = 96 / 72;
/** Word's washout for a picture: lighter than the file, so text on top stays readable. */
const WASHOUT = 0.3;

let ruler: CanvasRenderingContext2D | null | undefined;

/** The width of `text` at 100px in a font stack, or 0 where nothing can measure. */
function widthAt100(text: string, stack: string): number {
  if (ruler === undefined) ruler = typeof document === "undefined" ? null : document.createElement("canvas").getContext("2d");
  if (!ruler) return 0;
  ruler.font = `100px ${stack}`;
  return ruler.measureText(text).width;
}

const hex = (c?: string) => hexOf(c?.replace(/^#/, "")) ?? "";

/**
 * The watermark as an element that fills a box of `w` by `h` px (the margin box of a page; the caller
 * places it). The text is centred, turned as the file says (315 degrees is Word's diagonal) and sized to
 * the shape Word made for it, or to about 70% of the box's diagonal (80% of its width when horizontal).
 */
export function watermarkElement(wm: Watermark, w: number, h: number, mediaUrl: (part: string) => string): HTMLElement {
  const box = document.createElement("div");
  box.className = "wm";
  box.dataset.kind = wm.kind;
  box.dataset.layout = wm.layout;
  box.setAttribute("aria-hidden", "true");
  box.style.cssText = `width:${w}px;height:${h}px`;
  const diagonal = wm.layout === "diagonal";
  const angle = ((wm.angle % 360) + 360) % 360;
  const turn = angle ? `rotate(${angle}deg)` : "";
  // The longest line the mark may take: the shape's own width, held inside what the box can show.
  const reach = diagonal ? Math.hypot(w, h) : w;
  const target = Math.max(8, Math.min(wm.width ? wm.width * PX : reach * (diagonal ? 0.7 : 0.8), reach * (diagonal ? 0.95 : 1)));
  if (wm.kind === "picture") {
    if (!wm.media) return box;
    const img = document.createElement("img");
    img.className = "wm-mark";
    img.alt = "";
    img.draggable = false;
    img.src = mediaUrl(wm.media);
    img.style.cssText = `width:${wm.width ? wm.width * PX : Math.min(w * 0.8, target)}px;${wm.height ? `height:${wm.height * PX}px;` : ""}transform:translate(-50%,-50%) ${turn};opacity:${wm.washout === false ? 1 : WASHOUT}`;
    box.append(img);
    return box;
  }
  const text = wm.text ?? "";
  if (!text) return box;
  const stack = fontStack(wm.font) ?? "Calibri, Carlito, sans-serif";
  const natural = widthAt100(text, stack);
  const size = natural > 0 ? (100 * target) / natural : (wm.size ?? 72) * PX;
  const mark = document.createElement("div");
  mark.className = "wm-mark";
  mark.textContent = text;
  const color = hex(wm.color);
  mark.style.cssText = `font-family:${stack};font-size:${size.toFixed(2)}px;${color ? `color:${color};` : ""}opacity:${wm.opacity ?? 1};transform:translate(-50%,-50%) ${turn}`;
  box.append(mark);
  return box;
}
