// Pictures in the PDF. A JPEG goes in as it is; anything else is decoded by the browser, split into colour
// and transparency and compressed. Drawings made by the browser itself (a chart, a shape, an equation) come
// in as canvases.
import { timeLimit } from "@docxtor/extensions";
import { readJpeg } from "./jpeg.ts";
import { ref, type PdfWriter } from "./writer.ts";

export interface ImageRef {
  /** The name to paint it by (`/Im3 Do`). */
  name: string;
}

/** The largest number of pixels kept in one picture; bigger ones are scaled down. */
const MAX_PIXELS = 24_000_000;

interface Entry {
  name: string;
  obj: number;
}

/** A plain box the size of a picture that could not be read, drawn in the colours of the document's own placeholder. */
function unreadablePlaceholder(img: HTMLImageElement, cssW: number, cssH: number, scale: number): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(cssW * scale));
  canvas.height = Math.max(1, Math.round(cssH * scale));
  const g = canvas.getContext("2d")!;
  const css = getComputedStyle(img);
  g.fillStyle = css.getPropertyValue("--paper").trim() || "white";
  g.fillRect(0, 0, canvas.width, canvas.height);
  g.strokeStyle = css.getPropertyValue("--paper-chart-axis").trim() || "gray";
  g.lineWidth = scale;
  g.setLineDash([4 * scale, 3 * scale]);
  g.strokeRect(scale / 2, scale / 2, canvas.width - scale, canvas.height - scale);
  return canvas;
}

export class ImageStore {
  private w: PdfWriter;
  private entries: Entry[] = [];
  private byKey = new Map<string, ImageRef>();
  private jobs: Promise<void>[] = [];
  /** Pictures the browser shows but would not hand over (a web address that sends no cross-origin permission): drawn as a placeholder. */
  unreadable = 0;

  constructor(writer: PdfWriter) {
    this.w = writer;
  }

  private make(key: string | null, fill: (obj: number) => Promise<void>): ImageRef {
    if (key) {
      const known = this.byKey.get(key);
      if (known) return known;
    }
    const obj = this.w.alloc();
    const ref: ImageRef = { name: `Im${this.entries.length + 1}` };
    this.entries.push({ name: ref.name, obj });
    if (key) this.byKey.set(key, ref);
    this.jobs.push(fill(obj).catch(() => this.blank(obj)));
    return ref;
  }

  /** A picture that can't be read becomes a transparent dot, so the file stays valid. */
  private async blank(obj: number): Promise<void> {
    const dict = "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8";
    const mask = await this.w.addStream(dict, new Uint8Array([0]), false);
    await this.w.putStream(obj, `${dict} /SMask ${ref(mask)}`, new Uint8Array([255]), false);
  }

  /** A picture shown by an <img> element. `scale` is how many pixels per CSS pixel to keep for vector pictures. */
  fromImage(img: HTMLImageElement, cssW: number, cssH: number, scale = 2): ImageRef {
    const src = img.currentSrc || img.src;
    return this.make(`img:${src}`, async (obj) => {
      let blob: Blob;
      try {
        const res = await fetch(src, { referrerPolicy: "no-referrer", credentials: "omit", signal: timeLimit(20000) });
        if (!res.ok) throw new Error(String(res.status));
        blob = await res.blob();
      } catch {
        // A picture linked at a web address shows in the editor without the site's permission, but can only be read
        // into the file with it. The file gets a placeholder in its place; the export still finishes.
        this.unreadable++;
        await this.putCanvas(obj, unreadablePlaceholder(img, cssW, cssH, scale));
        return;
      }
      const bytes = new Uint8Array(await blob.arrayBuffer());
      const jpeg = blob.type === "image/svg+xml" ? null : readJpeg(bytes);
      if (jpeg && jpeg.orientation === 1 && jpeg.bits === 8) {
        const decode = jpeg.inverted ? " /Decode [1 0 1 0 1 0 1 0]" : "";
        const space = jpeg.components === 1 ? "/DeviceGray" : jpeg.components === 3 ? "/DeviceRGB" : "/DeviceCMYK";
        await this.w.putStream(obj, `/Type /XObject /Subtype /Image /Width ${jpeg.width} /Height ${jpeg.height} /ColorSpace ${space} /BitsPerComponent 8 /Filter /DCTDecode${decode}`, bytes, false);
        return;
      }
      const vector = blob.type === "image/svg+xml" || /^\s*(<\?xml|<svg)/.test(new TextDecoder().decode(bytes.subarray(0, 64)));
      const canvas = vector ? await drawVector(img, cssW, cssH, scale) : await decodeBitmap(blob, img);
      await this.putCanvas(obj, canvas);
    });
  }

  /** A picture made from a canvas. `key` names it, so the same drawing is stored once. */
  fromCanvas(make: () => Promise<HTMLCanvasElement | null>, key: string | null = null): ImageRef {
    return this.make(key, async (obj) => {
      const canvas = await make();
      if (!canvas) throw new Error("nothing to draw");
      await this.putCanvas(obj, canvas);
    });
  }

  private async putCanvas(obj: number, canvas: HTMLCanvasElement | OffscreenCanvas): Promise<void> {
    let { width, height } = canvas;
    let source: HTMLCanvasElement | OffscreenCanvas = canvas;
    if (width * height > MAX_PIXELS) {
      const k = Math.sqrt(MAX_PIXELS / (width * height));
      const small = new OffscreenCanvas(Math.max(1, Math.floor(width * k)), Math.max(1, Math.floor(height * k)));
      small.getContext("2d")!.drawImage(canvas, 0, 0, small.width, small.height);
      source = small;
      width = small.width;
      height = small.height;
    }
    const data = (source.getContext("2d") as CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D).getImageData(0, 0, width, height).data;
    const rgb = new Uint8Array(width * height * 3);
    const alpha = new Uint8Array(width * height);
    let opaque = true;
    for (let i = 0, p = 0, q = 0; i < data.length; i += 4, p += 3, q++) {
      rgb[p] = data[i];
      rgb[p + 1] = data[i + 1];
      rgb[p + 2] = data[i + 2];
      const a = data[i + 3];
      alpha[q] = a;
      if (a !== 255) opaque = false;
    }
    // Fully transparent pixels take the colour of their neighbours: viewers blend the colour of a pixel into
    // the edge of what is drawn over it, and black would show as a fringe.
    if (!opaque) bleed(rgb, alpha, width, height);
    let mask = "";
    if (!opaque) {
      const m = await this.w.addStream(`/Type /XObject /Subtype /Image /Width ${width} /Height ${height} /ColorSpace /DeviceGray /BitsPerComponent 8`, alpha);
      mask = ` /SMask ${ref(m)}`;
    }
    await this.w.putStream(obj, `/Type /XObject /Subtype /Image /Width ${width} /Height ${height} /ColorSpace /DeviceRGB /BitsPerComponent 8${mask}`, rgb);
  }

  /** Wait for every picture to be stored; returns the XObject entries of the resource dictionary. */
  async finish(): Promise<string> {
    await Promise.all(this.jobs);
    return this.entries.map((e) => `/${e.name} ${ref(e.obj)}`).join(" ");
  }
}

async function decodeBitmap(blob: Blob, img: HTMLImageElement): Promise<HTMLCanvasElement> {
  const canvas = document.createElement("canvas");
  try {
    const bmp = await createImageBitmap(blob);
    canvas.width = bmp.width;
    canvas.height = bmp.height;
    canvas.getContext("2d", { willReadFrequently: true })!.drawImage(bmp, 0, 0);
    bmp.close();
  } catch {
    // Formats createImageBitmap doesn't take: draw what the page shows.
    canvas.width = img.naturalWidth || 1;
    canvas.height = img.naturalHeight || 1;
    canvas.getContext("2d", { willReadFrequently: true })!.drawImage(img, 0, 0);
  }
  return canvas;
}

async function drawVector(img: HTMLImageElement, cssW: number, cssH: number, scale: number): Promise<HTMLCanvasElement> {
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.min(4096, Math.round(cssW * scale)));
  canvas.height = Math.max(1, Math.min(4096, Math.round(cssH * scale)));
  canvas.getContext("2d", { willReadFrequently: true })!.drawImage(img, 0, 0, canvas.width, canvas.height);
  return canvas;
}

/** Give the transparent pixels of each row the colour of the nearest opaque pixel in it (white when the row has none). */
function bleed(rgb: Uint8Array, alpha: Uint8Array, width: number, height: number) {
  for (let y = 0; y < height; y++) {
    const row = y * width;
    let seen = -1;
    for (let x = 0; x < width; x++) {
      const i = row + x;
      if (alpha[i] !== 0) seen = i;
      else if (seen >= 0) {
        rgb[i * 3] = rgb[seen * 3];
        rgb[i * 3 + 1] = rgb[seen * 3 + 1];
        rgb[i * 3 + 2] = rgb[seen * 3 + 2];
      }
    }
    seen = -1;
    for (let x = width - 1; x >= 0; x--) {
      const i = row + x;
      if (alpha[i] !== 0) seen = i;
      else if (seen >= 0 && !(rgb[i * 3] | rgb[i * 3 + 1] | rgb[i * 3 + 2])) {
        // Only the pixels before the row's first opaque one are still unset (black).
        rgb[i * 3] = rgb[seen * 3];
        rgb[i * 3 + 1] = rgb[seen * 3 + 1];
        rgb[i * 3 + 2] = rgb[seen * 3 + 2];
      } else if (seen < 0) rgb.fill(255, i * 3, i * 3 + 3);
    }
  }
}
