// Pictures for the browser tests: PNG files made on the spot, and pictures written the way Word writes them, with
// their crop, turn, outline, adjustments, effects and place spelled out. Sizes and offsets are points.
import { deflateSync } from "node:zlib";

const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
const crc32 = (b: Uint8Array) => {
  let c = 0xffffffff;
  for (const x of b) c = CRC[(c ^ x) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const u32 = (v: number) => [(v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255];

/** A PNG of `w` x `h` pixels; `pixel` gives each one's red, green, blue and alpha. */
export function png(w: number, h: number, pixel: (x: number, y: number) => [number, number, number, number]): Uint8Array {
  const raw = new Uint8Array((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0;
    for (let x = 0; x < w; x++) raw.set(pixel(x, y), y * (w * 4 + 1) + 1 + x * 4);
  }
  const chunk = (type: string, data: Uint8Array) => {
    const body = new Uint8Array([...Buffer.from(type), ...data]);
    return [...u32(data.length), ...body, ...u32(crc32(body))];
  };
  const head = new Uint8Array([...u32(w), ...u32(h), 8, 6, 0, 0, 0]);
  return new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, ...chunk("IHDR", head), ...chunk("IDAT", deflateSync(raw)), ...chunk("IEND", new Uint8Array())]);
}

export const RED: [number, number, number, number] = [220, 30, 30, 255];
export const GREEN: [number, number, number, number] = [30, 170, 60, 255];
export const BLUE: [number, number, number, number] = [30, 60, 220, 255];
export const YELLOW: [number, number, number, number] = [240, 200, 20, 255];

/** A picture in four quarters: red top left, green top right, blue bottom left, yellow bottom right. */
export const quarters = (w = 40, h = 20): Uint8Array => png(w, h, (x, y) => (y < h / 2 ? (x < w / 2 ? RED : GREEN) : x < w / 2 ? BLUE : YELLOW));

const emu = (pt: number) => Math.round(pt * 12700);

export interface PictureSpec {
  /** The relationship id of the picture in the file (see `media` of the document builder). */
  rid?: string;
  id?: number;
  name?: string;
  alt?: string;
  w?: number;
  h?: number;
  /** left, top, right, bottom as fractions of the whole picture. */
  crop?: [number, number, number, number];
  rot?: number;
  flipH?: boolean;
  flipV?: boolean;
  geometry?: string;
  /** Outline: colour (hex), width in points, dash. */
  line?: { color: string; w?: number; dash?: string };
  /** Markup that goes inside `a:blip` (an `a:lum`, `a:grayscl`, `a:alphaModFix`...). */
  blip?: string;
  /** Markup that goes inside `a:effectLst`. */
  effects?: string;
  /** Floating: how it is anchored. Leave out for a picture in line with the text. */
  anchor?: {
    wrap?: "square" | "tight" | "through" | "topBottom" | "none";
    behind?: boolean;
    relH?: string;
    x?: number | string;
    relV?: string;
    y?: number | string;
    dist?: [number, number, number, number];
    locked?: boolean;
    overlap?: boolean;
    z?: number;
    wrapText?: string;
  };
  /** Space Word keeps around the picture (left, top, right, bottom). */
  effectExtent?: [number, number, number, number];
}

/** A run holding a picture, written the way Word writes it. */
export function pictureRun(o: PictureSpec): string {
  const w = o.w ?? 100;
  const h = o.h ?? 50;
  const id = o.id ?? 1;
  const [l, t, r, b] = o.crop ?? [0, 0, 0, 0];
  const src = o.crop ? `<a:srcRect${l ? ` l="${Math.round(l * 100000)}"` : ""}${t ? ` t="${Math.round(t * 100000)}"` : ""}${r ? ` r="${Math.round(r * 100000)}"` : ""}${b ? ` b="${Math.round(b * 100000)}"` : ""}/>` : "";
  const xfrm = `<a:xfrm${o.rot ? ` rot="${Math.round(o.rot * 60000)}"` : ""}${o.flipH ? ' flipH="1"' : ""}${o.flipV ? ' flipV="1"' : ""}><a:off x="0" y="0"/><a:ext cx="${emu(w)}" cy="${emu(h)}"/></a:xfrm>`;
  const ln = o.line
    ? `<a:ln w="${emu(o.line.w ?? 1)}"><a:solidFill><a:srgbClr val="${o.line.color}"/></a:solidFill>${o.line.dash ? `<a:prstDash val="${o.line.dash}"/>` : ""}</a:ln>`
    : "";
  const pic =
    `<pic:pic><pic:nvPicPr><pic:cNvPr id="${id}" name="${o.name ?? `Picture ${id}`}"/><pic:cNvPicPr/></pic:nvPicPr>` +
    `<pic:blipFill><a:blip r:embed="${o.rid ?? "rIdPic1"}">${o.blip ?? ""}</a:blip>${src}<a:stretch><a:fillRect/></a:stretch></pic:blipFill>` +
    `<pic:spPr>${xfrm}<a:prstGeom prst="${o.geometry ?? "rect"}"><a:avLst/></a:prstGeom>${ln}${o.effects ? `<a:effectLst>${o.effects}</a:effectLst>` : ""}</pic:spPr></pic:pic>`;
  const graphic = `<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">${pic}</a:graphicData></a:graphic>`;
  const docPr = `<wp:docPr id="${id}" name="${o.name ?? `Picture ${id}`}" descr="${o.alt ?? ""}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>`;
  const ee = o.effectExtent ?? [0, 0, 0, 0];
  const effectExtent = `<wp:effectExtent l="${emu(ee[0])}" t="${emu(ee[1])}" r="${emu(ee[2])}" b="${emu(ee[3])}"/>`;
  const extent = `<wp:extent cx="${emu(w)}" cy="${emu(h)}"/>`;
  const a = o.anchor;
  if (!a) return `<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">${extent}${effectExtent}${docPr}${graphic}</wp:inline></w:drawing></w:r>`;
  const pos = (axis: "H" | "V", rel: string, v: number | string) =>
    `<wp:position${axis} relativeFrom="${rel}">${typeof v === "number" ? `<wp:posOffset>${emu(v)}</wp:posOffset>` : `<wp:align>${v}</wp:align>`}</wp:position${axis}>`;
  const [dt, db, dl, dr] = a.dist ?? [0, 0, 9, 9];
  const wrapText = a.wrapText ?? "bothSides";
  const polygon = '<wp:wrapPolygon edited="0"><wp:start x="0" y="0"/><wp:lineTo x="0" y="21600"/><wp:lineTo x="21600" y="21600"/><wp:lineTo x="21600" y="0"/><wp:lineTo x="0" y="0"/></wp:wrapPolygon>';
  const wrap =
    a.wrap === "square"
      ? `<wp:wrapSquare wrapText="${wrapText}"/>`
      : a.wrap === "tight"
        ? `<wp:wrapTight wrapText="${wrapText}">${polygon}</wp:wrapTight>`
        : a.wrap === "through"
          ? `<wp:wrapThrough wrapText="${wrapText}">${polygon}</wp:wrapThrough>`
          : a.wrap === "topBottom"
            ? "<wp:wrapTopAndBottom/>"
            : "<wp:wrapNone/>";
  return (
    `<w:r><w:drawing><wp:anchor distT="${emu(dt)}" distB="${emu(db)}" distL="${emu(dl)}" distR="${emu(dr)}" simplePos="0" relativeHeight="${a.z ?? 251658240}" behindDoc="${a.behind ? 1 : 0}" locked="${a.locked ? 1 : 0}" layoutInCell="1" allowOverlap="${a.overlap === false ? 0 : 1}">` +
    `<wp:simplePos x="0" y="0"/>${pos("H", a.relH ?? "column", a.x ?? 0)}${pos("V", a.relV ?? "paragraph", a.y ?? 0)}${extent}${effectExtent}${wrap}${docPr}${graphic}</wp:anchor></w:drawing></w:r>`
  );
}
