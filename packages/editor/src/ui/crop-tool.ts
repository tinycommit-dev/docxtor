// Cropping a picture on the picture itself. The whole picture is drawn dimmed where it is cut off and plain inside
// the crop frame; the frame's handles trim it and a drag inside the frame moves the picture under it. The result is
// the four crop fractions the engine writes as `a:srcRect`.
import type { RImage } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, overlayLayer, svg } from "./dom";
import { track } from "./object-frame";
import { PICTURE_ICONS } from "./picture-icons";
import { openAspectMenu, openShapeMenu } from "./picture-menus";

const PT_PX = 96 / 72;
const RAD = Math.PI / 180;
const SIDES = ["nw", "n", "ne", "e", "se", "s", "sw", "w"] as const;
type Side = (typeof SIDES)[number];
type Crop = [number, number, number, number];

export interface CropResult {
  /** Left, top, right and bottom, as the file stores them. */
  crop: Crop;
  /** Set when the person picked a shape to crop to. */
  geometry?: string;
}

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export class CropTool {
  private layer: HTMLElement | null = null;
  private strip: HTMLElement | null = null;
  private win!: HTMLElement;
  private keep!: HTMLImageElement;
  private dim!: HTMLImageElement;
  private handles = new Map<Side, HTMLElement>();
  private atom: HTMLElement | null = null;
  private media: HTMLElement | null = null;
  private off: Array<() => void> = [];
  private rot = 0;
  private flipH = false;
  private flipV = false;
  /** The size of the whole picture and where it sits, in pixels inside the layer (which is the picture's box as it was). */
  private full = { x: 0, y: 0, w: 1, h: 1 };
  private box: Box = { x: 0, y: 0, w: 1, h: 1 };
  private crop0: Crop = [0, 0, 0, 0];
  private size = { w: 1, h: 1 };

  constructor(
    private ed: Editor,
    private done: (r: CropResult | null) => void,
  ) {}

  get active(): boolean {
    return !!this.layer;
  }

  /** Start cropping the picture drawn in `atom`. */
  start(atom: HTMLElement, im: RImage) {
    if (this.layer) this.close();
    const f = im.pic ?? {};
    const media = atom.querySelector<HTMLElement>(".pic, img");
    const source = atom.querySelector<HTMLImageElement>("img");
    if (!media || !source) return;
    this.atom = atom;
    this.media = media;
    this.rot = f.rot ?? 0;
    this.flipH = !!f.flipH;
    this.flipV = !!f.flipV;
    const z = this.ed.zoom;
    const w0 = im.w * PT_PX * z;
    const h0 = im.h * PT_PX * z;
    this.size = { w: w0, h: h0 };
    // The crop as it looks on screen: a flip turns the file's left edge into the right one.
    const [l, tp, r, b] = f.crop ?? [0, 0, 0, 0];
    const [vl, vt, vr, vb] = [this.flipH ? r : l, this.flipV ? b : tp, this.flipH ? l : r, this.flipV ? tp : b];
    const fw = w0 / Math.max(0.02, 1 - vl - vr);
    const fh = h0 / Math.max(0.02, 1 - vt - vb);
    this.full = { x: -vl * fw, y: -vt * fh, w: fw, h: fh };
    this.box = { x: 0, y: 0, w: w0, h: h0 };
    this.crop0 = this.fractions();

    const flip = `scale(${this.flipH ? -1 : 1},${this.flipV ? -1 : 1})`;
    const picture = (cls: string) => {
      const img = h("img", { class: cls, src: source.currentSrc || source.src, alt: "", draggable: "false" });
      img.style.filter = source.style.filter;
      img.style.transform = flip;
      return img;
    };
    this.dim = picture("crop-dim");
    this.keep = picture("crop-keep");
    this.win = h("div", { class: "crop-win" }, this.keep);
    this.layer = h("div", { class: "crop-layer" }, this.dim, this.win);
    for (const k of SIDES) {
      const hd = h("span", { class: `crop-handle ${k}`, "data-h": k });
      hd.addEventListener("pointerdown", (e) => this.resize(e, k));
      this.handles.set(k, hd);
      this.layer.append(hd);
    }
    this.win.addEventListener("pointerdown", (e) => this.pan(e));
    this.strip = this.buildStrip();
    overlayLayer().append(this.layer, this.strip);
    atom.style.visibility = "hidden";
    this.paint();

    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        this.cancel();
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        this.finish();
      }
    };
    const down = (e: MouseEvent) => {
      const el = e.target as HTMLElement;
      if (this.layer?.contains(el) || this.strip?.contains(el) || el.closest(".pop")) return;
      this.finish();
    };
    const move = () => this.paint();
    document.addEventListener("keydown", key, true);
    document.addEventListener("mousedown", down, true);
    this.ed.canvas.addEventListener("scroll", move, { passive: true });
    window.addEventListener("resize", move);
    this.off = [
      () => document.removeEventListener("keydown", key, true),
      () => document.removeEventListener("mousedown", down, true),
      () => this.ed.canvas.removeEventListener("scroll", move),
      () => window.removeEventListener("resize", move),
    ];
  }

  private buildStrip(): HTMLElement {
    const button = (label: string, icon: string | null, run: (b: HTMLElement) => void, cls = "tb") => {
      const b = h("button", { class: cls, type: "button", title: label, "aria-label": label }, icon ? svg(icon) : null, icon ? null : label);
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => run(b));
      return b;
    };
    const aspect = button(t("Aspect ratio"), PICTURE_ICONS.cropAspect, (b) => openAspectMenu(b, (aw, ah) => this.aspect(aw, ah)));
    const shape = button(t("Crop to shape"), PICTURE_ICONS.cropShape, (b) => openShapeMenu(b, undefined, (geom) => this.finish(geom)));
    const reset = button(t("Show the whole picture"), PICTURE_ICONS.pictureReset, () => {
      this.box = { x: this.full.x, y: this.full.y, w: this.full.w, h: this.full.h };
      this.paint();
    });
    const ok = button(t("Done"), null, () => this.finish(), "btn primary crop-done");
    const cancel = button(t("Cancel"), null, () => this.cancel(), "btn ghost crop-cancel");
    return h("div", { class: "obj-strip crop-strip", role: "toolbar", "aria-label": t("Crop") }, aspect, shape, reset, ok, cancel);
  }

  /** The crop frame as the four fractions of the whole picture (as shown), then as the file stores them. */
  private fractions(): Crop {
    const { box: b, full: f } = this;
    const vl = (b.x - f.x) / f.w;
    const vt = (b.y - f.y) / f.h;
    const vr = 1 - (b.x + b.w - f.x) / f.w;
    const vb = 1 - (b.y + b.h - f.y) / f.h;
    const q = (n: number) => Math.round(n * 100000) / 100000;
    return [q(this.flipH ? vr : vl), q(this.flipV ? vb : vt), q(this.flipH ? vl : vr), q(this.flipV ? vt : vb)];
  }

  private paint() {
    if (!this.layer || !this.media || !this.strip) return;
    const m = this.media.getBoundingClientRect();
    const cx = m.left + m.width / 2;
    const cy = m.top + m.height / 2;
    const { w, h: hh } = this.size;
    const c = this.ed.canvas.getBoundingClientRect();
    const visible = m.bottom > c.top && m.top < c.bottom;
    this.layer.style.cssText = `left:${cx - w / 2}px;top:${cy - hh / 2}px;width:${w}px;height:${hh}px;transform:${this.rot ? `rotate(${this.rot}deg)` : "none"};display:${visible ? "" : "none"}`;
    const f = this.full;
    for (const img of [this.dim]) {
      img.style.left = `${f.x}px`;
      img.style.top = `${f.y}px`;
      img.style.width = `${f.w}px`;
      img.style.height = `${f.h}px`;
    }
    const b = this.box;
    this.win.style.cssText = `left:${b.x}px;top:${b.y}px;width:${b.w}px;height:${b.h}px`;
    this.keep.style.left = `${f.x - b.x}px`;
    this.keep.style.top = `${f.y - b.y}px`;
    this.keep.style.width = `${f.w}px`;
    this.keep.style.height = `${f.h}px`;
    const at: Record<Side, [number, number]> = {
      nw: [b.x, b.y],
      n: [b.x + b.w / 2, b.y],
      ne: [b.x + b.w, b.y],
      e: [b.x + b.w, b.y + b.h / 2],
      se: [b.x + b.w, b.y + b.h],
      s: [b.x + b.w / 2, b.y + b.h],
      sw: [b.x, b.y + b.h],
      w: [b.x, b.y + b.h / 2],
    };
    for (const [k, el] of this.handles) {
      el.style.left = `${at[k][0]}px`;
      el.style.top = `${at[k][1]}px`;
    }
    // The strip sits above the frame, or below it when there is no room.
    const wr = this.win.getBoundingClientRect();
    const sw = this.strip.offsetWidth || 320;
    const sh = this.strip.offsetHeight || 36;
    const top = wr.top - sh - 12 < c.top + 4 ? wr.bottom + 12 : wr.top - sh - 12;
    this.strip.style.left = `${Math.max(8, Math.min(window.innerWidth - sw - 8, wr.left + wr.width / 2 - sw / 2))}px`;
    this.strip.style.top = `${Math.max(4, top)}px`;
    this.strip.style.display = visible ? "" : "none";
  }

  /** The pointer's travel along the picture's own axes. */
  private local(dx: number, dy: number): [number, number] {
    const a = this.rot * RAD;
    return [dx * Math.cos(a) + dy * Math.sin(a), -dx * Math.sin(a) + dy * Math.cos(a)];
  }

  private resize(e: PointerEvent, k: Side) {
    e.preventDefault();
    e.stopPropagation();
    const start = { ...this.box };
    const f = this.full;
    const min = Math.max(12, Math.min(f.w, f.h) * 0.04);
    // Padding may go as far as the picture's own size on each side.
    const [loX, hiX, loY, hiY] = [f.x - f.w, f.x + 2 * f.w, f.y - f.h, f.y + 2 * f.h];
    track(
      e,
      (dx, dy) => {
        const [lx, ly] = this.local(dx, dy);
        let { x, y, w, h: hh } = start;
        if (k.includes("w")) {
          const nx = Math.min(Math.max(start.x + lx, loX), start.x + start.w - min);
          w = start.x + start.w - nx;
          x = nx;
        }
        if (k.includes("e")) w = Math.min(Math.max(start.w + lx, min), hiX - start.x);
        if (k.includes("n")) {
          const ny = Math.min(Math.max(start.y + ly, loY), start.y + start.h - min);
          hh = start.y + start.h - ny;
          y = ny;
        }
        if (k.includes("s")) hh = Math.min(Math.max(start.h + ly, min), hiY - start.y);
        this.box = { x, y, w, h: hh };
        this.paint();
      },
      () => undefined,
    );
  }

  /** Drag inside the frame: the picture moves under it. */
  private pan(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const start = { ...this.full };
    const b = this.box;
    track(
      e,
      (dx, dy) => {
        const [lx, ly] = this.local(dx, dy);
        let nx = start.x + lx;
        let ny = start.y + ly;
        // While the frame lies inside the picture, the picture keeps covering it.
        if (b.x >= start.x && b.x + b.w <= start.x + start.w) nx = Math.min(b.x, Math.max(b.x + b.w - start.w, nx));
        if (b.y >= start.y && b.y + b.h <= start.y + start.h) ny = Math.min(b.y, Math.max(b.y + b.h - start.h, ny));
        this.full = { ...start, x: nx, y: ny };
        this.paint();
      },
      () => undefined,
    );
  }

  /** Crop to an aspect ratio: the largest frame of that shape inside the picture, centred on where the frame is. */
  aspect(aw: number, ah: number) {
    const r = aw / ah;
    const f = this.full;
    let w = f.w;
    let hh = w / r;
    if (hh > f.h) {
      hh = f.h;
      w = hh * r;
    }
    const cx = this.box.x + this.box.w / 2;
    const cy = this.box.y + this.box.h / 2;
    const x = Math.min(Math.max(cx - w / 2, f.x), f.x + f.w - w);
    const y = Math.min(Math.max(cy - hh / 2, f.y), f.y + f.h - hh);
    this.box = { x, y, w, h: hh };
    this.paint();
  }

  /** Whether the crop is what it was when the tool started. */
  private unchanged(): boolean {
    const now = this.fractions();
    return now.every((v, i) => Math.abs(v - this.crop0[i]) < 0.0005);
  }

  /** Keep the crop (and a shape, if one was picked). */
  finish(geometry?: string) {
    if (!this.layer) return;
    const crop = this.fractions();
    const same = this.unchanged() && !geometry;
    this.close();
    this.done(same ? null : { crop, geometry });
  }

  cancel() {
    if (!this.layer) return;
    this.close();
    this.done(null);
  }

  private close() {
    for (const o of this.off) o();
    this.off = [];
    this.layer?.remove();
    this.strip?.remove();
    this.layer = null;
    this.strip = null;
    this.handles.clear();
    if (this.atom) this.atom.style.visibility = "";
    this.atom = null;
    this.media = null;
  }
}
