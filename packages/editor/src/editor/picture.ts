// Drawing a picture the way Word shows it: cropped, turned and flipped, in its shape, with an outline, brightness,
// contrast, recolouring, transparency, and shadow, glow, reflection and soft edges. A plain picture is just an
// <img>; one with formatting is a `.pic` box holding a clipped copy of the image, an outline, and the SVG filter
// its adjustments need. The engine reads the formatting from the file (`RImage.pic`); this file only draws.
import type { PicFmt, RImage } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { hexOr } from "./color.ts";

const PT_PX = 96 / 72;
const SVG_NS = "http://www.w3.org/2000/svg";
let uid = 0;

/** The shapes a picture can be cropped to, with the labels the Crop menu shows. */
export const PICTURE_SHAPES: [string, string][] = [
  ["rect", msg("Rectangle")],
  ["roundRect", msg("Rounded rectangle")],
  ["round1Rect", msg("Round one corner")],
  ["round2SameRect", msg("Round top corners")],
  ["round2DiagRect", msg("Round opposite corners")],
  ["snip1Rect", msg("Snip one corner")],
  ["snip2SameRect", msg("Snip top corners")],
  ["snip2DiagRect", msg("Snip opposite corners")],
  ["ellipse", msg("Oval")],
  ["triangle", msg("Triangle")],
  ["rtTriangle", msg("Right triangle")],
  ["diamond", msg("Diamond")],
  ["parallelogram", msg("Parallelogram")],
  ["trapezoid", msg("Trapezoid")],
  ["pentagon", msg("Pentagon")],
  ["hexagon", msg("Hexagon")],
  ["octagon", msg("Octagon")],
  ["star5", msg("Star")],
  ["plus", msg("Cross")],
  ["rightArrow", msg("Arrow right")],
  ["leftArrow", msg("Arrow left")],
  ["upArrow", msg("Arrow up")],
  ["downArrow", msg("Arrow down")],
  ["heart", msg("Heart")],
  ["chevron", msg("Chevron")],
  ["homePlate", msg("Pentagon arrow")],
  ["flowChartTerminator", msg("Pill")],
  ["teardrop", msg("Teardrop")],
  ["plaque", msg("Plaque")],
];

const poly = (pts: [number, number][]) => `M${pts.map(([x, y]) => `${r2(x)},${r2(y)}`).join("L")}Z`;
const r2 = (n: number) => Math.round(n * 100) / 100;

/** SVG path data for a preset shape fitted to a w x h box, in pixels. */
export function shapePath(geom: string | undefined, w: number, h: number, adj?: number): string {
  const ss = Math.min(w, h);
  const arc = (r: number, x: number, y: number) => `A${r2(r)},${r2(r)} 0 0 1 ${r2(x)},${r2(y)}`;
  switch (geom) {
    case "roundRect": {
      const r = Math.min(ss * (adj ?? 0.16667), ss / 2);
      return `M${r2(r)},0H${r2(w - r)}${arc(r, w, r)}V${r2(h - r)}${arc(r, w - r, h)}H${r2(r)}${arc(r, 0, h - r)}V${r2(r)}${arc(r, r, 0)}Z`;
    }
    case "round1Rect": {
      const r = ss * 0.16667;
      return `M0,0H${r2(w - r)}${arc(r, w, r)}V${r2(h)}H0Z`;
    }
    case "round2SameRect": {
      const r = ss * 0.16667;
      return `M${r2(r)},0H${r2(w - r)}${arc(r, w, r)}V${r2(h)}H0V${r2(r)}${arc(r, r, 0)}Z`;
    }
    case "round2DiagRect": {
      const r = ss * 0.16667;
      return `M${r2(r)},0H${r2(w)}V${r2(h - r)}${arc(r, w - r, h)}H0V${r2(r)}${arc(r, r, 0)}Z`;
    }
    case "snip1Rect": {
      const s = ss * 0.16667;
      return poly([[0, 0], [w - s, 0], [w, s], [w, h], [0, h]]);
    }
    case "snip2SameRect": {
      const s = ss * 0.16667;
      return poly([[s, 0], [w - s, 0], [w, s], [w, h], [0, h], [0, s]]);
    }
    case "snip2DiagRect": {
      const s = ss * 0.16667;
      return poly([[s, 0], [w, 0], [w, h - s], [w - s, h], [0, h], [0, s]]);
    }
    case "ellipse":
      return `M0,${r2(h / 2)}A${r2(w / 2)},${r2(h / 2)} 0 1 1 ${r2(w)},${r2(h / 2)}A${r2(w / 2)},${r2(h / 2)} 0 1 1 0,${r2(h / 2)}Z`;
    case "triangle":
      return poly([[w / 2, 0], [w, h], [0, h]]);
    case "rtTriangle":
      return poly([[0, 0], [w, h], [0, h]]);
    case "diamond":
      return poly([[w / 2, 0], [w, h / 2], [w / 2, h], [0, h / 2]]);
    case "parallelogram": {
      const o = Math.min(ss * 0.25, w / 2);
      return poly([[o, 0], [w, 0], [w - o, h], [0, h]]);
    }
    case "trapezoid": {
      const o = Math.min(ss * 0.25, w / 2);
      return poly([[o, 0], [w - o, 0], [w, h], [0, h]]);
    }
    case "pentagon":
      return poly([[w * 0.5, 0], [w, h * 0.3819], [w * 0.809, h], [w * 0.191, h], [0, h * 0.3819]]);
    case "hexagon": {
      const o = Math.min(ss * 0.25, w / 2);
      return poly([[0, h / 2], [o, 0], [w - o, 0], [w, h / 2], [w - o, h], [o, h]]);
    }
    case "octagon": {
      const o = ss * 0.29289;
      return poly([[o, 0], [w - o, 0], [w, o], [w, h - o], [w - o, h], [o, h], [0, h - o], [0, o]]);
    }
    case "star5": {
      const pts: [number, number][] = [];
      for (let i = 0; i < 10; i++) {
        const rad = i % 2 ? 0.382 : 1;
        const a = -Math.PI / 2 + (i * Math.PI) / 5;
        pts.push([Math.cos(a) * rad, Math.sin(a) * rad]);
      }
      const minX = Math.min(...pts.map((p) => p[0]));
      const maxX = Math.max(...pts.map((p) => p[0]));
      const minY = Math.min(...pts.map((p) => p[1]));
      const maxY = Math.max(...pts.map((p) => p[1]));
      return poly(pts.map(([x, y]) => [((x - minX) / (maxX - minX)) * w, ((y - minY) / (maxY - minY)) * h]));
    }
    case "plus": {
      const o = ss * 0.25;
      return poly([[o, 0], [w - o, 0], [w - o, o], [w, o], [w, h - o], [w - o, h - o], [w - o, h], [o, h], [o, h - o], [0, h - o], [0, o], [o, o]]);
    }
    case "rightArrow": {
      const hd = Math.min(ss * 0.5, w);
      return poly([[0, h * 0.25], [w - hd, h * 0.25], [w - hd, 0], [w, h / 2], [w - hd, h], [w - hd, h * 0.75], [0, h * 0.75]]);
    }
    case "leftArrow": {
      const hd = Math.min(ss * 0.5, w);
      return poly([[w, h * 0.25], [hd, h * 0.25], [hd, 0], [0, h / 2], [hd, h], [hd, h * 0.75], [w, h * 0.75]]);
    }
    case "downArrow": {
      const hd = Math.min(ss * 0.5, h);
      return poly([[w * 0.25, 0], [w * 0.75, 0], [w * 0.75, h - hd], [w, h - hd], [w / 2, h], [0, h - hd], [w * 0.25, h - hd]]);
    }
    case "upArrow": {
      const hd = Math.min(ss * 0.5, h);
      return poly([[w * 0.25, h], [w * 0.75, h], [w * 0.75, hd], [w, hd], [w / 2, 0], [0, hd], [w * 0.25, hd]]);
    }
    case "heart": {
      const x = (v: number) => r2(v * w);
      const y = (v: number) => r2(v * h);
      return `M${x(0.5)},${y(0.25)}C${x(0.5)},${y(0.1)} ${x(0.35)},0 ${x(0.22)},0C${x(0.08)},0 0,${y(0.12)} 0,${y(0.3)}C0,${y(0.55)} ${x(0.3)},${y(0.72)} ${x(0.5)},${y(1)}C${x(0.7)},${y(0.72)} ${x(1)},${y(0.55)} ${x(1)},${y(0.3)}C${x(1)},${y(0.12)} ${x(0.92)},0 ${x(0.78)},0C${x(0.65)},0 ${x(0.5)},${y(0.1)} ${x(0.5)},${y(0.25)}Z`;
    }
    case "chevron": {
      const o = Math.min(ss * 0.5, w);
      return poly([[0, 0], [w - o, 0], [w, h / 2], [w - o, h], [0, h], [o, h / 2]]);
    }
    case "homePlate": {
      const o = Math.min(ss * 0.5, w);
      return poly([[0, 0], [w - o, 0], [w, h / 2], [w - o, h], [0, h]]);
    }
    case "flowChartTerminator": {
      const r = Math.min(h / 2, w / 2);
      return `M${r2(r)},0H${r2(w - r)}${arc(r, w - r, h)}H${r2(r)}${arc(r, r, 0)}Z`;
    }
    case "teardrop":
      return `M0,${r2(h / 2)}A${r2(w / 2)},${r2(h / 2)} 0 0 1 ${r2(w / 2)},0H${r2(w)}V${r2(h / 2)}A${r2(w / 2)},${r2(h / 2)} 0 0 1 ${r2(w / 2)},${r2(h)}A${r2(w / 2)},${r2(h / 2)} 0 0 1 0,${r2(h / 2)}Z`;
    case "plaque": {
      const o = ss * 0.16667;
      const sweepOut = (x: number, y: number) => `A${r2(o)},${r2(o)} 0 0 0 ${r2(x)},${r2(y)}`;
      return `M${r2(o)},0H${r2(w - o)}${sweepOut(w, o)}V${r2(h - o)}${sweepOut(w - o, h)}H${r2(o)}${sweepOut(0, h - o)}V${r2(o)}${sweepOut(o, 0)}Z`;
    }
    default:
      return `M0,0H${r2(w)}V${r2(h)}H0Z`;
  }
}

/**
 * The file extension a picture's bytes call for (`png`, `jpg`, `gif`, `webp`, `bmp`, `tiff`, `svg`, `emf`, `wmf`), or
 * null for a format Word cannot show either. The name a picture comes with is not trusted: addresses and local
 * files often have no extension, or the wrong one.
 */
export function pictureExtension(b: Uint8Array): string | null {
  const is = (at: number, ...sig: number[]) => sig.every((v, i) => b[at + i] === v);
  const ascii = (at: number, s: string) => [...s].every((c, i) => b[at + i] === c.charCodeAt(0));
  if (is(0, 0x89, 0x50, 0x4e, 0x47)) return "png";
  if (is(0, 0xff, 0xd8, 0xff)) return "jpg";
  if (ascii(0, "GIF8")) return "gif";
  if (ascii(0, "RIFF") && ascii(8, "WEBP")) return "webp";
  if (ascii(0, "BM")) return "bmp";
  if (is(0, 1, 0, 0, 0) && ascii(40, " EMF")) return "emf";
  if (is(0, 0x49, 0x49, 0x2a, 0) || is(0, 0x4d, 0x4d, 0, 0x2a)) return "tiff";
  if (is(0, 0xd7, 0xcd, 0xc6, 0x9a)) return "wmf";
  // A WMF without the placeable header (as Word writes its own): record type 1 or 2, header size 9 words, version 0x0100 or 0x0300.
  if ((is(0, 1, 0) || is(0, 2, 0)) && is(2, 9, 0) && (is(4, 0, 1) || is(4, 0, 3))) return "wmf";
  const head = new TextDecoder().decode(b.subarray(0, 512)).toLowerCase();
  return head.includes("<svg") ? "svg" : null;
}

/** Whether the picture needs the `.pic` structure (anything beyond plain). */
export function isFormatted(f: PicFmt | undefined): f is PicFmt {
  return !!f && !!(f.crop || f.rot || f.flipH || f.flipV || f.geom || f.line || f.opacity !== undefined || f.bright || f.contrast || f.saturation || f.recolor || f.clrChange || f.shadow || f.glow || f.softEdge || f.reflection || f.art);
}

const DASHES: Record<string, number[]> = {
  dot: [1, 3],
  dash: [4, 3],
  lgDash: [8, 3],
  dashDot: [4, 3, 1, 3],
  lgDashDot: [8, 3, 1, 3],
  lgDashDotDot: [8, 3, 1, 3, 1, 3],
  sysDash: [3, 1],
  sysDot: [1, 1],
  sysDashDot: [3, 1, 1, 1],
  sysDashDotDot: [3, 1, 1, 1, 1, 1],
};

const rgba = (hex: string, a: number) => {
  const n = parseInt(hex.padEnd(6, "0").slice(0, 6), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${Math.round(a * 1000) / 1000})`;
};

/** The SVG filter for brightness, contrast, saturation and recolouring, or null when none is needed. */
function adjustments(f: PicFmt, id: string): string | null {
  const parts: string[] = [];
  const b = f.bright ?? 0;
  const c = f.contrast ?? 0;
  if (b || c) {
    const m = c >= 0 ? Math.min(1 / (1 - c * 0.995), 100) : 1 + c;
    // Brightness moves the middle by half its value, so a washed-out picture (70%, -70%) stays readable.
    const icpt = 0.5 - 0.5 * m + 0.5 * b;
    const fn = (ch: string) => `<feFunc${ch} type="linear" slope="${r2(m * 1000) / 1000}" intercept="${Math.round(icpt * 1000) / 1000}"/>`;
    parts.push(`<feComponentTransfer>${fn("R")}${fn("G")}${fn("B")}</feComponentTransfer>`);
  }
  if (f.saturation !== undefined && f.saturation !== 1) parts.push(`<feColorMatrix type="saturate" values="${f.saturation}"/>`);
  const rc = f.recolor;
  if (rc?.kind === "grayscale") parts.push('<feColorMatrix type="saturate" values="0"/>');
  else if (rc?.kind === "blackWhite") {
    const n = 100;
    const cut = Math.round((rc.threshold ?? 0.5) * n);
    const vals = Array.from({ length: n }, (_, i) => (i < cut ? 0 : 1)).join(" ");
    const fn = (ch: string) => `<feFunc${ch} type="discrete" tableValues="${vals}"/>`;
    parts.push('<feColorMatrix type="matrix" values="0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0 0 0 1 0"/>', `<feComponentTransfer>${fn("R")}${fn("G")}${fn("B")}</feComponentTransfer>`);
  } else if (rc?.kind === "duotone") {
    const hex = (s: string | undefined, d: string) => parseInt((s ?? d).padEnd(6, "0").slice(0, 6), 16);
    const a = hex(rc.color1, "000000");
    const z = hex(rc.color2, "FFFFFF");
    const ch = (v: number, shift: number) => ((v >> shift) & 255) / 255;
    const fn = (name: string, shift: number) => `<feFunc${name} type="table" tableValues="${ch(a, shift)} ${ch(z, shift)}"/>`;
    parts.push(
      '<feColorMatrix type="matrix" values="0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0 0 0 1 0"/>',
      `<feComponentTransfer>${fn("R", 16)}${fn("G", 8)}${fn("B", 0)}</feComponentTransfer>`,
    );
  }
  if (!parts.length) return null;
  return `<svg class="pic-defs" width="0" height="0" aria-hidden="true" focusable="false"><filter id="${id}" color-interpolation-filters="sRGB" x="0" y="0" width="100%" height="100%">${parts.join("")}</filter></svg>`;
}

interface PicState {
  fmt: PicFmt;
  clip: HTMLElement;
  img: HTMLImageElement;
  ln: SVGSVGElement | null;
}
const states = new WeakMap<HTMLElement, PicState>();

/** The file name a linked picture points to, for the placeholder. */
export function linkName(link: string): string {
  const tail = link.split(/[\\/]/).filter(Boolean).pop() ?? link;
  try {
    return decodeURIComponent(tail.split("?")[0]);
  } catch {
    return tail;
  }
}

/** What stands in for a picture that is only linked to the document: its size, and the name of the file. */
export function placeholder(im: RImage, link: string, unsupported = false, show?: () => void): HTMLElement {
  const name = linkName(link);
  const el = document.createElement("span");
  el.className = "pic-missing";
  el.setAttribute("role", "img");
  el.setAttribute("aria-label", `${unsupported ? t("Picture") : t("Linked picture")}: ${name}`);
  el.title = link;
  el.style.width = `${Math.max(1, im.w * PT_PX)}px`;
  el.style.height = `${Math.max(1, im.h * PT_PX)}px`;
  const icon = document.createElementNS(SVG_NS, "svg");
  icon.setAttribute("viewBox", "0 0 24 24");
  icon.setAttribute("aria-hidden", "true");
  icon.innerHTML = '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="m4 17 5-4.5 3.5 3L16 12l4 4.5"/><path d="M4 20 20 4"/>';
  const label = document.createElement("span");
  label.className = "pic-missing-name";
  label.textContent = name;
  const note = document.createElement("span");
  note.className = "pic-missing-note";
  note.textContent = unsupported ? t("This picture can't be shown here") : t("Linked picture, not in this file");
  el.append(icon, label, note);
  // A picture at a web address is fetched only when the person asks: the address learns who opened the document.
  if (show) {
    const ask = document.createElement("button");
    ask.type = "button";
    ask.className = "pic-missing-action";
    ask.textContent = t("Show linked pictures");
    ask.title = t("Pictures linked in this document are fetched from their web addresses, so those sites can see you opened it.");
    ask.addEventListener("mousedown", (e) => e.stopPropagation());
    ask.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      show();
    });
    el.append(ask);
  }
  return el;
}

/** The pictures already recoloured, by source, colour and replacement, so a render neither decodes the picture again nor makes another blob URL. */
const recoloured = new Map<string, string>();

/** Turn the picture's colour into see-through (or another colour), since no CSS filter picks one colour. */
function changeColor(img: HTMLImageElement, from: string, to?: string) {
  const key = `${img.src}|${from}|${to ?? ""}`;
  const done = recoloured.get(key);
  if (done) {
    img.src = done;
    return;
  }
  const run = async () => {
    try {
      await img.decode();
      const c = document.createElement("canvas");
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      const g = c.getContext("2d");
      if (!g || !c.width || !c.height) return;
      g.drawImage(img, 0, 0);
      const data = g.getImageData(0, 0, c.width, c.height);
      const f = parseInt(from, 16);
      const [fr, fg, fb] = [(f >> 16) & 255, (f >> 8) & 255, f & 255];
      const tt = to ? parseInt(to, 16) : null;
      const near = 28;
      const px = data.data;
      for (let i = 0; i < px.length; i += 4) {
        if (Math.abs(px[i] - fr) <= near && Math.abs(px[i + 1] - fg) <= near && Math.abs(px[i + 2] - fb) <= near) {
          if (tt === null) px[i + 3] = 0;
          else {
            px[i] = (tt >> 16) & 255;
            px[i + 1] = (tt >> 8) & 255;
            px[i + 2] = tt & 255;
          }
        }
      }
      g.putImageData(data, 0, 0);
      c.toBlob((blob) => {
        if (!blob) return;
        // One blob URL per picture and colour, shared by every render (two renders that raced end up on the same one).
        let url = recoloured.get(key);
        if (!url) {
          url = URL.createObjectURL(blob);
          recoloured.set(key, url);
        }
        img.src = url;
      });
    } catch {
      // A picture from another address can't be read back; it stays as it is.
    }
  };
  void run();
}

/**
 * The element that shows a picture. `url` is where the browser gets the image (the SVG version when the picture has
 * one, else the picture itself). Returns an <img> for a plain picture, or a `.pic` box for a formatted one.
 */
export function pictureNode(im: RImage, url: string): HTMLElement {
  const f = im.pic;
  const w = Math.max(1, im.w * PT_PX);
  const h = Math.max(1, im.h * PT_PX);
  const img = document.createElement("img");
  // A picture at a web address is asked for with no referrer, so the site does not learn which document it is in. It is
  // a plain image request (not a cross-origin one), so it shows whenever the browser can show it, as on any web page.
  if (/^https?:/i.test(url)) img.referrerPolicy = "no-referrer";
  img.src = url;
  img.alt = im.alt ?? "";
  img.draggable = false;
  if (!isFormatted(f)) {
    img.width = w;
    img.height = h;
    return img;
  }
  const el = document.createElement("span");
  el.className = "pic";
  const clip = document.createElement("span");
  clip.className = "pic-clip";
  img.className = "pic-img";
  clip.append(img);
  el.append(clip);
  states.set(el, { fmt: f, clip, img, ln: null });
  if (f.rot) el.dataset.rot = String(f.rot);
  if (f.flipH) el.dataset.fliph = "1";
  if (f.flipV) el.dataset.flipv = "1";
  const id = `pf${++uid}`;
  const defs = adjustments(f, id);
  const filters: string[] = [];
  if (defs) {
    el.insertAdjacentHTML("beforeend", defs);
    filters.push(`url(#${id})`);
  }
  if (f.art) {
    const [name, radius] = f.art;
    if (name === "artisticBlur") filters.push(`blur(${Math.max(0.5, radius * 0.4)}px)`);
    else if (/Pencil|LineDrawing|Cutout|Cement|ChalkSketch|Photocopy|Marker|Crisscross|Glass/.test(name)) filters.push("grayscale(1) contrast(1.3)");
  }
  if (filters.length) img.style.filter = filters.join(" ");
  if (f.opacity !== undefined) clip.style.opacity = String(f.opacity);
  if (f.clrChange) changeColor(img, f.clrChange.from, f.clrChange.to);
  sizePicture(el, w, h);
  return el;
}

/** Fit a `.pic` to a size in pixels: the box, the crop, the shape's clip, the outline and the effects that depend on size. */
export function sizePicture(el: HTMLElement, w: number, h: number) {
  const st = states.get(el);
  if (!st) return;
  const { fmt: f, clip, img } = st;
  el.style.width = `${w}px`;
  el.style.height = `${h}px`;
  const [cl, ct, cr, cb] = f.crop ?? [0, 0, 0, 0];
  const fw = w / Math.max(0.02, 1 - cl - cr);
  const fh = h / Math.max(0.02, 1 - ct - cb);
  img.style.width = `${fw}px`;
  img.style.height = `${fh}px`;
  img.style.left = `${-cl * fw}px`;
  img.style.top = `${-ct * fh}px`;
  // The shape clips the picture; its outline is drawn over the clip so the stroke is not cut in half.
  const path = f.geom && f.geom !== "rect" ? shapePath(f.geom, w, h, f.adj) : null;
  clip.style.clipPath = path ? `path('${path}')` : "";
  const ts: string[] = [];
  if (f.rot) ts.push(`rotate(${f.rot}deg)`);
  if (f.flipH || f.flipV) ts.push(`scale(${f.flipH ? -1 : 1},${f.flipV ? -1 : 1})`);
  el.style.transform = ts.join(" ");
  // Effects.
  const fx: string[] = [];
  const rad = ((f.rot ?? 0) * Math.PI) / 180;
  if (f.shadow) {
    const s = f.shadow;
    const a = (s.angle * Math.PI) / 180;
    let dx = Math.cos(a) * s.distPt * PT_PX;
    let dy = Math.sin(a) * s.distPt * PT_PX;
    // The shadow keeps its direction on the page when the picture is turned (Word's presets do not turn with the shape).
    [dx, dy] = [dx * Math.cos(-rad) - dy * Math.sin(-rad), dx * Math.sin(-rad) + dy * Math.cos(-rad)];
    if (f.flipH) dx = -dx;
    if (f.flipV) dy = -dy;
    fx.push(`drop-shadow(${r2(dx)}px ${r2(dy)}px ${r2(s.blurPt * PT_PX * 0.6)}px ${rgba(s.color, s.alpha)})`);
  }
  if (f.glow) fx.push(`drop-shadow(0 0 ${r2(f.glow.radiusPt * PT_PX * 0.5)}px ${rgba(f.glow.color, Math.min(1, f.glow.alpha * 1.4))})`, `drop-shadow(0 0 ${r2(f.glow.radiusPt * PT_PX * 0.5)}px ${rgba(f.glow.color, Math.min(1, f.glow.alpha * 1.4))})`);
  el.style.filter = fx.join(" ");
  if (f.softEdge) {
    const r = f.softEdge * PT_PX;
    const grad = (dir: string) => `linear-gradient(${dir},transparent,#000 ${r2(r)}px,#000 calc(100% - ${r2(r)}px),transparent)`;
    clip.style.maskImage = `${grad("to right")},${grad("to bottom")}`;
    clip.style.setProperty("-webkit-mask-image", `${grad("to right")},${grad("to bottom")}`);
    clip.style.maskComposite = "intersect";
    clip.style.setProperty("-webkit-mask-composite", "source-in");
  }
  if (f.reflection) {
    const r = f.reflection;
    el.style.setProperty("-webkit-box-reflect", `below ${r2(r.distPt * PT_PX)}px linear-gradient(to bottom, rgba(255,255,255,${r.alpha}), transparent ${Math.round(r.size * 100)}%)`);
  }
  // The outline.
  st.ln?.remove();
  st.ln = null;
  if (f.line) {
    const lw = Math.max(0.5, f.line.widthPt * PT_PX);
    const s = document.createElementNS(SVG_NS, "svg");
    s.setAttribute("class", "pic-ln");
    s.setAttribute("viewBox", `0 0 ${r2(w)} ${r2(h)}`);
    s.setAttribute("preserveAspectRatio", "none");
    s.setAttribute("aria-hidden", "true");
    const p = document.createElementNS(SVG_NS, "path");
    p.setAttribute("d", path ?? shapePath("rect", w, h));
    p.setAttribute("fill", "none");
    p.setAttribute("stroke", hexOr(f.line.color));
    p.setAttribute("stroke-width", String(r2(lw)));
    p.setAttribute("vector-effect", "non-scaling-stroke");
    const dash = DASHES[f.line.dash];
    if (dash) p.setAttribute("stroke-dasharray", dash.map((d) => r2(d * lw)).join(" "));
    s.append(p);
    el.append(s);
    st.ln = s;
  }
}

/** The formatting a `.pic` element was drawn from. */
export function picFormat(el: Element | null): PicFmt | null {
  const pic = el?.closest<HTMLElement>(".pic");
  return (pic && states.get(pic)?.fmt) ?? null;
}
