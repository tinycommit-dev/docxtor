// Drawings the PDF doesn't write as vectors yet (charts, shapes, equations, characters no font file covers) are
// drawn by the browser to a canvas and stored as pictures. SVG goes through an <img> (copying the styles it
// gets from CSS into the markup first, since an SVG picture can't see the page's style sheets); MathML goes
// through a foreignObject.

/** Room kept around a drawing for what sticks out of its box (arrow heads, wide strokes). */
export const MARGIN = 8;
const MAX_SIDE = 4096;

const SVG_PROPS = [
  "fill", "fill-opacity", "fill-rule", "stroke", "stroke-width", "stroke-opacity", "stroke-dasharray", "stroke-dashoffset", "stroke-linecap", "stroke-linejoin",
  "stroke-miterlimit", "opacity", "color", "font-family", "font-size", "font-weight", "font-style", "letter-spacing", "text-anchor", "dominant-baseline",
  "visibility", "display", "paint-order", "stop-color", "stop-opacity",
];

function toDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
}

/** Pictures inside a drawing can't be fetched once it is an image, so put their data in the markup. */
async function inlinePictures(root: Element): Promise<void> {
  const images = Array.from(root.querySelectorAll("image, img"));
  await Promise.all(
    images.map(async (el) => {
      const attr = el.hasAttribute("href") ? "href" : el.hasAttribute("xlink:href") ? "xlink:href" : "src";
      const url = el.getAttribute(attr);
      if (!url || url.startsWith("data:")) return;
      try {
        const blob = await (await fetch(url)).blob();
        el.setAttribute(attr, await toDataUrl(blob));
      } catch {
        el.removeAttribute(attr);
      }
    }),
  );
}

async function render(markup: string, w: number, h: number, scale: number): Promise<HTMLCanvasElement | null> {
  const k = Math.min(scale, MAX_SIDE / Math.max(w, h, 1));
  // A data address, not a blob one: the browser lets a canvas keep drawings from those (with MathML in them) readable.
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(markup)}`;
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.ceil(w * k));
    canvas.height = Math.max(1, Math.ceil(h * k));
    canvas.getContext("2d", { willReadFrequently: true })!.drawImage(img, 0, 0, canvas.width, canvas.height);
    return canvas;
  } catch {
    return null;
  }
}

/** An SVG element as a canvas of (w + 2 MARGIN) by (h + 2 MARGIN) CSS pixels at `scale`, the SVG in the middle. */
export async function rasterSvg(svg: SVGElement, w: number, h: number, scale: number): Promise<HTMLCanvasElement | null> {
  const clone = svg.cloneNode(true) as SVGElement;
  const from = [svg, ...Array.from(svg.querySelectorAll("*"))];
  const to = [clone, ...Array.from(clone.querySelectorAll("*"))];
  from.forEach((el, i) => {
    const cs = getComputedStyle(el);
    const target = to[i] as SVGElement;
    for (const p of SVG_PROPS) {
      const v = cs.getPropertyValue(p);
      if (v) target.style.setProperty(p, v);
    }
  });
  clone.style.transform = "none";
  clone.style.overflow = "visible";
  clone.setAttribute("x", "0");
  clone.setAttribute("y", "0");
  clone.setAttribute("width", String(w));
  clone.setAttribute("height", String(h));
  await inlinePictures(clone);
  const inner = new XMLSerializer().serializeToString(clone);
  const m = MARGIN;
  const markup = `<svg xmlns="http://www.w3.org/2000/svg" width="${w + 2 * m}" height="${h + 2 * m}" viewBox="${-m} ${-m} ${w + 2 * m} ${h + 2 * m}">${inner}</svg>`;
  return render(markup, w + 2 * m, h + 2 * m, scale);
}

/** An equation (a MathML element) as a canvas of (w + 2 MARGIN) by (h + 2 MARGIN) CSS pixels. */
export async function rasterMath(math: Element, w: number, h: number, scale: number): Promise<HTMLCanvasElement | null> {
  const cs = getComputedStyle(math);
  const SVG = "http://www.w3.org/2000/svg";
  const m = MARGIN;
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("width", String(w + 2 * m));
  svg.setAttribute("height", String(h + 2 * m));
  const object = document.createElementNS(SVG, "foreignObject");
  for (const [k, v] of [["x", m], ["y", m], ["width", w], ["height", h]] as const) object.setAttribute(k, String(v));
  const box = document.createElement("div");
  box.style.cssText = "margin:0;padding:0";
  for (const p of ["font-family", "font-size", "font-weight", "font-style", "color"]) box.style.setProperty(p, cs.getPropertyValue(p));
  const clone = math.cloneNode(true) as HTMLElement;
  clone.style.display = "block math";
  clone.style.margin = "0";
  box.append(clone);
  object.append(box);
  svg.append(object);
  const markup = new XMLSerializer().serializeToString(svg);
  return render(markup, w + 2 * m, h + 2 * m, scale);
}

/** A run of text as a canvas of w by h CSS pixels, drawn in the font and colour the page shows it in. */
export function rasterChar(text: string, font: string, color: string, direction: string, w: number, h: number, scale: number): HTMLCanvasElement {
  const k = Math.min(scale, MAX_SIDE / Math.max(w, h, 1));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.ceil(w * k));
  canvas.height = Math.max(1, Math.ceil(h * k));
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  ctx.scale(k, k);
  ctx.font = font;
  ctx.fillStyle = color;
  ctx.textBaseline = "alphabetic";
  ctx.textAlign = "left";
  ctx.direction = direction === "rtl" ? "rtl" : "ltr";
  const m = ctx.measureText(text);
  ctx.fillText(text, 0, m.fontBoundingBoxAscent);
  return canvas;
}

/**
 * A character drawn so its ink fills a canvas of w by h CSS pixels: an operator an equation stretched or enlarged
 * (a big sum, a tall integral) that no embeddable font has. Null when the character has no ink.
 */
export function rasterInk(text: string, font: string, color: string, w: number, h: number, scale: number): HTMLCanvasElement | null {
  const k = Math.min(scale, MAX_SIDE / Math.max(w, h, 1));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.ceil(w * k));
  canvas.height = Math.max(1, Math.ceil(h * k));
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  ctx.font = font;
  const m = ctx.measureText(text);
  const inkW = m.actualBoundingBoxLeft + m.actualBoundingBoxRight;
  const inkH = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent;
  if (!(inkW > 0) || !(inkH > 0)) return null;
  ctx.scale((w * k) / inkW, (h * k) / inkH);
  ctx.fillStyle = color;
  ctx.textBaseline = "alphabetic";
  ctx.textAlign = "left";
  ctx.fillText(text, m.actualBoundingBoxLeft, m.actualBoundingBoxAscent);
  return canvas;
}
