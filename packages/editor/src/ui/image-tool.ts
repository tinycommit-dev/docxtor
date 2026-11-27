// Image selection frame: eight resize handles and a turn handle, a control strip for wrapping, alignment, crop, turn,
// format and position, and moving a floating picture by dragging it. Edits go through the engine's `set_image` op.
import type { Op, RImage } from "@docxtor/core";
import { msg, t, type ChartPaletteContribution } from "@docxtor/extensions";
import { chartOf, redrawChart } from "../chart/registry";
import type { Editor } from "../editor/editor";
import { horizontalRef, mediaOf, offsetsFor, screenBox, verticalRef } from "../editor/object-move";
import { sizePicture } from "../editor/picture";
import { openChartDialog } from "./chart-dialog";
import { behindAt, openArrangeMenu } from "./arrange";
import { ARRANGE_ICONS } from "./arrange-icons";
import { CropTool, type CropResult } from "./crop-tool";
import { h, overlayLayer, popover, promptDialog, svg } from "./dom";
import { I } from "./icons";
import { ObjectFrame, track } from "./object-frame";
import { openPictureFormat } from "./picture-format";
import { PICTURE_ICONS } from "./picture-icons";
import { openAspectMenu, openShapeMenu } from "./picture-menus";
import { openPicturePosition } from "./picture-position";

const PT_PX = 96 / 72;
const RAD = Math.PI / 180;
type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

const WRAPS: [string, string, string][] = [
  ["inline", msg("In line"), msg("Moves with the text, like a character")],
  ["square", msg("Wrap"), msg("Text flows around the image")],
  ["topBottom", msg("Break"), msg("Text above and below only")],
  ["behind", msg("Behind"), msg("Behind the text")],
  ["front", msg("Front"), msg("In front of the text")],
];

export class ImageTool {
  private frame: ObjectFrame;
  private strip: HTMLElement;
  private sizeReadout: HTMLElement;
  private target: { p: number; o: number } | null = null;
  /** A drag, resize or turn is under way. */
  private busy = false;
  private cropper: CropTool;
  /** The colour palettes extensions offer the Edit chart dialog. */
  chartPalettes: () => ChartPaletteContribution[] = () => [];

  constructor(private ed: Editor) {
    this.frame = new ObjectFrame((e, k) => (k === "rot" ? this.turn(e) : this.resize(e, k)), { rotate: true, onMove: (e) => this.dragEdge(e) });
    // The names the first image tool had, so the page and its tests keep finding them.
    this.frame.el.classList.replace("obj-frame", "img-frame");
    this.frame.el.querySelectorAll(".obj-handle").forEach((hd) => hd.classList.add("img-handle"));
    this.sizeReadout = h("span", { class: "readout img-size" });
    this.strip = h("div", { class: "img-strip", style: "display:none", role: "toolbar", "aria-label": t("Image") });
    overlayLayer().append(ed.life.own(this.frame.el), ed.life.own(this.strip));
    this.cropper = new CropTool(ed, (r) => this.cropDone(r));
    // On the stage, not the flow: the pictures of a header being edited are not in the flow.
    ed.stageElement.addEventListener("mousedown", (e) => this.onDown(e), true);
    // A chart opens its dialog on a double-click, as a table or a link opens its own tool; a picture opens its format.
    ed.flow.addEventListener("dblclick", (e) => {
      const hit = (e.target as HTMLElement).closest(".atom.img");
      if (!hit) return;
      e.preventDefault();
      if (hit.classList.contains("chart")) this.editChart();
      else if (this.target && !ed.readOnly) this.format();
    });
    ed.canvas.addEventListener("scroll", () => this.place(), { passive: true });
    ed.life.on(window, "resize", () => this.place());
    ed.life.on(document, "selectionchange", () => {
      if (!this.target || this.busy || this.cropper.active) return;
      const s = ed.sel;
      if (!s || s.anchor.p !== this.target.p || Math.min(s.anchor.o, s.focus.o) !== this.target.o || Math.max(s.anchor.o, s.focus.o) !== this.target.o + 1) this.hide();
    });
  }

  private atom(): HTMLElement | null {
    if (!this.target) return null;
    return this.ed.root.querySelector<HTMLElement>(`[data-p="${this.target.p}"] .atom.img[data-o="${this.target.o}"]`);
  }

  private onDown(e: MouseEvent) {
    if (this.cropper.active) return;
    const target = e.target as HTMLElement;
    if (!this.ed.root.contains(target)) return;
    let img: Element | null = target.closest(".atom.img");
    if (!img && !target.closest(".atom")) {
      const behind = behindAt(this.ed.root, e);
      img = behind?.classList.contains("img") ? behind : null;
    }
    if (!img) return;
    const para = img.closest<HTMLElement>("[data-p]");
    if (!para) return;
    e.preventDefault();
    this.target = { p: Number(para.dataset.p), o: Number((img as HTMLElement).dataset.o) };
    this.ed.focusRoot();
    this.ed.select({ anchor: { p: this.target.p, o: this.target.o }, focus: { p: this.target.p, o: this.target.o + 1 } });
    this.show();
    // A floating picture follows the pointer once it is dragged.
    if (e.button === 0 && !e.shiftKey && !this.ed.readOnly && this.movable()) this.dragBody(e);
  }

  /** Select the image at a position, as a click on it does; false if there is none. */
  selectAt(p: number, o: number): boolean {
    const atom = this.ed.root.querySelector<HTMLElement>(`[data-p="${p}"] .atom.img[data-o="${o}"]`);
    if (!atom) return false;
    this.ed.ensureDrawn(atom);
    atom.scrollIntoView({ block: "center", inline: "nearest" });
    this.target = { p, o };
    this.ed.focusRoot();
    this.ed.select({ anchor: { p, o }, focus: { p, o: o + 1 } });
    this.show();
    return true;
  }

  /** Re-attach after the document re-renders (the image element is replaced). */
  refresh() {
    if (this.target) {
      if (!this.atom()) {
        this.cropper.cancel();
        this.hide();
      } else if (!this.cropper.active) this.show();
    }
  }

  private show() {
    const a = this.atom();
    if (!a) return this.hide();
    this.frame.el.classList.toggle("no-rot", a.classList.contains("chart"));
    this.strip.style.display = "";
    this.buildStrip(a);
    this.place();
  }

  hide() {
    this.target = null;
    this.frame.hide();
    this.strip.style.display = "none";
  }

  private place() {
    const a = this.atom();
    if (!a || this.cropper.active) return;
    const b = screenBox(a, this.ed.zoom);
    const m = mediaOf(a) ?? a;
    const r = m.getBoundingClientRect();
    const c = this.ed.canvas.getBoundingClientRect();
    const visible = r.bottom > c.top && r.top < c.bottom;
    this.frame.place(b, visible);
    const sw = this.strip.offsetWidth || 360;
    const sh = this.strip.offsetHeight || 36;
    // Above the picture and its turn handle; below it when there is no room.
    const above = a.classList.contains("chart") ? r.top : Math.min(r.top, b.top) - 26;
    const top = above - sh - 10 < c.top + 4 ? r.bottom + 10 : above - sh - 10;
    this.strip.style.left = `${Math.max(8, Math.min(window.innerWidth - sw - 8, r.left + r.width / 2 - sw / 2))}px`;
    this.strip.style.top = `${Math.max(4, top)}px`;
    this.strip.style.display = visible && !this.busy ? "" : "none";
    this.sizeReadout.textContent = `${Math.round(Number(a.dataset.w))} × ${Math.round(Number(a.dataset.h))} pt`;
  }

  /** An image is selected (clicked or right-clicked). */
  get selected(): boolean {
    return !!this.atom();
  }

  /** The wrapping of the selected image or chart (`inline`, `square`, ...). */
  get wrap(): string | null {
    return this.atom()?.dataset.wrap ?? null;
  }

  /** The position of the selected object. */
  get targetPos(): { p: number; o: number } | null {
    return this.atom() ? this.target : null;
  }

  /** The selected object floats over the text, so it has a place in the stacking order. */
  get floating(): boolean {
    return (this.wrap ?? "inline") !== "inline";
  }

  /** The selected object is a picture (not a chart). */
  get isPicture(): boolean {
    const a = this.atom();
    return !!a && !a.classList.contains("chart");
  }

  /** A picture is being cropped. */
  get cropping(): boolean {
    return this.cropper.active;
  }

  /** The selected picture as the engine reads it (size, place and formatting). */
  info(): RImage | null {
    const tg = this.target;
    if (!tg || !this.isPicture) return null;
    try {
      return this.ed.engine.pictureAt(tg);
    } catch {
      return null;
    }
  }

  /** The selected picture can be dragged: it floats, and nothing else is going on. */
  private movable(): boolean {
    return this.floating && !this.cropper.active;
  }

  /** Move the selected object in the stacking order. */
  order(how: "front" | "back" | "forward" | "backward") {
    const tg = this.target;
    if (!tg) return;
    this.ed.exec({ op: "set_object_order", at: tg, order: how }, { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } } });
    this.show();
  }

  /** Put the selected object behind the text or in front of it. */
  setBehind(behind: boolean) {
    const tg = this.target;
    if (!tg) return;
    this.ed.exec({ op: "set_behind_text", at: tg, behind }, { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } } });
    this.show();
  }

  /** The selected object is a chart. */
  get isChart(): boolean {
    return !!this.atom()?.classList.contains("chart");
  }

  /** Open the selected chart's dialog. */
  editChart() {
    const tg = this.target;
    if (!tg || !this.isChart) return;
    openChartDialog(this.ed, tg, this.chartPalettes());
  }

  /** Ask for the selected image's description for screen readers. */
  async editAlt() {
    const tg = this.target;
    const a = this.atom();
    if (!tg || !a) return;
    const chart = a.classList.contains("chart");
    const v = await promptDialog({
      title: t("Alt text"),
      label: chart ? t("Describe the chart for screen readers") : t("Describe the image for screen readers"),
      value: chart ? a.dataset.alt ?? "" : a.querySelector("img")?.alt ?? "",
      ok: t("Save"),
    });
    if (v !== null) {
      this.target = tg;
      this.apply({ alt: v });
    }
  }

  /** Pick a file to put in the selected image's place. */
  replace() {
    const tg = this.target;
    if (!tg) return;
    const picker = h("input", { type: "file", accept: "image/*,.emf,.wmf,.tif,.tiff" });
    picker.addEventListener("change", () => {
      const f = picker.files?.[0];
      if (!f) return;
      this.hide();
      this.ed.select({ anchor: tg, focus: { p: tg.p, o: tg.o + 1 } });
      void this.ed.insertImage(f);
    });
    picker.click();
  }

  /** Change the selected picture (any field of the `set_image` op), keeping it selected. */
  apply(patch: Record<string, unknown>, opts: { onError?: (m: string) => void; merge?: boolean } = {}): boolean {
    if (!this.target) return false;
    const tg = this.target;
    const res = this.ed.exec({ op: "set_image", at: tg, ...patch } as Op, { select: { anchor: { p: tg.p, o: tg.o }, focus: { p: tg.p, o: tg.o + 1 } }, onError: opts.onError, merge: opts.merge });
    this.target = tg;
    this.show();
    return !!res;
  }

  // -- crop, turn, flip, shape -----------------------------------------------------------------

  /** Start cropping the selected picture on the picture itself. */
  crop() {
    const a = this.atom();
    const im = this.info();
    if (!a || !im || this.ed.readOnly) return;
    this.frame.hide();
    this.strip.style.display = "none";
    this.cropper.start(a, im);
  }

  private cropDone(r: CropResult | null) {
    if (r) this.apply({ crop: r.crop, ...(r.geometry ? { geometry: r.geometry } : {}) });
    else this.show();
  }

  /** Crop the selected picture to an aspect ratio (width over height), keeping the middle of what is shown. */
  cropAspect(aw: number, ah: number) {
    const im = this.info();
    if (!im || this.ed.readOnly) return;
    const [l, tp, r, b] = im.pic?.crop ?? [0, 0, 0, 0];
    const fw = im.w / Math.max(0.02, 1 - l - r);
    const fh = im.h / Math.max(0.02, 1 - tp - b);
    const ratio = aw / ah;
    // The largest frame of that shape inside the whole picture, centred where the frame is now.
    let w = fw;
    let hh = w / ratio;
    if (hh > fh) {
      hh = fh;
      w = hh * ratio;
    }
    const cx = (l + (1 - r)) / 2;
    const cy = (tp + (1 - b)) / 2;
    const x0 = Math.min(Math.max(cx - w / fw / 2, 0), 1 - w / fw);
    const y0 = Math.min(Math.max(cy - hh / fh / 2, 0), 1 - hh / fh);
    this.apply({ crop: [x0, y0, 1 - x0 - w / fw, 1 - y0 - hh / fh].map((v) => Math.round(v * 100000) / 100000) });
  }

  /** Crop the selected picture to a shape (`rect` takes the shape off). */
  setShape(geometry: string) {
    if (this.isPicture && !this.ed.readOnly) this.apply({ geometry });
  }

  /** Turn the selected picture by a number of degrees (clockwise when positive). */
  rotateBy(deg: number) {
    const im = this.info();
    if (!im || this.ed.readOnly) return;
    this.apply({ rot: (((im.pic?.rot ?? 0) + deg) % 360 + 360) % 360 });
  }

  /** Mirror the selected picture. */
  flip(axis: "h" | "v") {
    const im = this.info();
    if (!im || this.ed.readOnly) return;
    if (axis === "h") this.apply({ flip_h: !im.pic?.flipH });
    else this.apply({ flip_v: !im.pic?.flipV });
  }

  /** Take every adjustment, effect and the crop off the selected picture. */
  reset() {
    if (this.isPicture && !this.ed.readOnly) this.apply({ reset: true });
  }

  /** Open Format picture for the selected picture. */
  format() {
    const tg = this.target;
    if (tg && this.isPicture && !this.ed.readOnly) openPictureFormat(this.ed, this, tg);
  }

  /** Open Position and layout for the selected picture. */
  position() {
    const tg = this.target;
    if (tg && this.isPicture && !this.ed.readOnly) openPicturePosition(this.ed, this, tg);
  }

  /** Where the selected picture's top-left corner is, in points from what a reference counts from. */
  offsetsIn(relX: string, relY: string): { x: number; y: number } | null {
    const a = this.atom();
    return a ? offsetsFor(this.ed, a, relX, relY) : null;
  }

  // -- the strip -----------------------------------------------------------------------------

  private buildStrip(a: HTMLElement) {
    const wrap = a.dataset.wrap ?? "inline";
    const align = a.dataset.align ?? "left";
    const wraps = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Wrapping") });
    for (const [id, label, title] of WRAPS) {
      const b = h("button", { type: "button", class: id === wrap || (id === "square" && wrap === "tight") ? "on" : "", title: t(title) }, t(label));
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => this.apply({ wrap: id }));
      wraps.append(b);
    }
    const aligns = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Alignment") });
    const icons: [string, string, string][] = [
      ["left", I.alignLeft, t("Align left")],
      ["center", I.alignCenter, t("Center")],
      ["right", I.alignRight, t("Align right")],
    ];
    for (const [id, icon, name] of icons) {
      const on = wrap === "inline" ? false : id === align;
      const b = h("button", { type: "button", class: on ? "on" : "", title: name, "aria-label": name }, svg(icon));
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => {
        if (wrap === "inline") this.ed.setPara({ align: id });
        else this.apply({ align: id });
      });
      aligns.append(b);
    }
    const chart = a.classList.contains("chart");
    const tool = (label: string, icon: string, run: (b: HTMLElement) => void) => {
      const b = h("button", { class: "tb", type: "button", title: label, "aria-label": label }, svg(icon));
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => run(b));
      return b;
    };
    const picture: HTMLElement[] = [];
    if (!chart) {
      const info = this.info();
      picture.push(
        tool(t("Crop"), PICTURE_ICONS.crop, () => this.crop()),
        tool(t("Crop to shape"), PICTURE_ICONS.cropShape, (b) => openShapeMenu(b, info?.pic?.geom, (g) => this.setShape(g))),
        tool(t("Rotate and flip"), PICTURE_ICONS.rotateRight, (b) => this.openTurnMenu(b)),
        tool(t("Format picture"), PICTURE_ICONS.pictureFormat, () => this.format()),
        tool(t("Position and layout"), PICTURE_ICONS.picturePosition, () => this.position()),
      );
    }
    const alt = h("button", { class: "tb", type: "button", title: t("Alt text") }, t("Alt"));
    alt.addEventListener("click", () => void this.editAlt());
    const edit = chart ? h("button", { class: "tb", type: "button", title: t("Edit chart"), "aria-label": t("Edit chart") }, t("Edit")) : null;
    edit?.addEventListener("click", () => this.editChart());
    const del = h("button", { class: "tb", type: "button", title: chart ? t("Delete chart") : t("Delete image"), "aria-label": chart ? t("Delete chart") : t("Delete image") }, svg(I.close));
    del.addEventListener("click", () => {
      const tg = this.target;
      if (!tg) return;
      this.hide();
      this.ed.exec({ op: "delete", from: tg, to: { p: tg.p, o: tg.o + 1 } }, { select: tg });
    });
    const arrange = h("button", { class: "tb", type: "button", title: t("Arrange"), "aria-label": t("Arrange"), "aria-haspopup": "menu" });
    arrange.innerHTML = ARRANGE_ICONS.bringToFront;
    arrange.addEventListener("mousedown", (e) => e.preventDefault());
    arrange.addEventListener("click", () => openArrangeMenu(arrange, { floating: this.floating, behind: this.wrap === "behind", order: (how) => this.order(how), setBehind: (on) => this.setBehind(on) }));
    this.strip.replaceChildren(wraps, aligns, ...picture, arrange, this.sizeReadout, ...(edit ? [edit] : []), alt, del);
  }

  private openTurnMenu(anchor: HTMLElement) {
    const list = h("div", { class: "arrange-menu" });
    let close = () => {};
    const item = (label: string, icon: string, run: () => void) => {
      const b = h("button", { class: "menu-item", type: "button", role: "menuitem" }, h("span", { class: "menu-icon", "aria-hidden": "true" }, svg(icon)), h("span", { class: "menu-label" }, label));
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => {
        close();
        run();
      });
      list.append(b);
    };
    item(t("Rotate right 90°"), PICTURE_ICONS.rotateRight, () => this.rotateBy(90));
    item(t("Rotate left 90°"), PICTURE_ICONS.rotateLeft, () => this.rotateBy(-90));
    item(t("Flip horizontal"), PICTURE_ICONS.flipHorizontal, () => this.flip("h"));
    item(t("Flip vertical"), PICTURE_ICONS.flipVertical, () => this.flip("v"));
    const aspect = h("button", { class: "menu-item", type: "button", role: "menuitem" }, h("span", { class: "menu-icon", "aria-hidden": "true" }, svg(PICTURE_ICONS.cropAspect)), h("span", { class: "menu-label" }, t("Crop to aspect ratio")));
    aspect.addEventListener("mousedown", (e) => e.preventDefault());
    aspect.addEventListener("click", () => {
      close();
      openAspectMenu(anchor, (aw, ah) => this.cropAspect(aw, ah));
    });
    list.append(h("div", { class: "menu-sep", role: "separator" }), aspect);
    close = popover(anchor, list);
  }

  // -- moving, resizing, turning --------------------------------------------------------------

  /** Drag the picture by its frame's edge: the same move as dragging the picture. */
  private dragEdge(e: PointerEvent) {
    if (this.ed.readOnly || !this.movable()) return;
    e.preventDefault();
    e.stopPropagation();
    this.dragBody(e);
  }

  /** Drag a floating picture: the frame follows the pointer and the picture lands where it is let go. */
  private dragBody(e: MouseEvent | PointerEvent) {
    const a = this.atom();
    const im = this.info();
    const tg = this.target;
    if (!a || !im || !tg) return;
    const start = screenBox(a, this.ed.zoom);
    const relX = horizontalRef(im.relX);
    const relY = verticalRef(im.relY);
    this.busy = true;
    track(
      e,
      (dx, dy) => {
        this.frame.place({ ...start, left: start.left + dx, top: start.top + dy });
        this.strip.style.display = "none";
      },
      (moved, ev) => {
        this.busy = false;
        if (!moved) return this.show();
        const o = offsetsFor(this.ed, a, relX, relY, { left: start.left + ev.clientX - (e as MouseEvent).clientX, top: start.top + ev.clientY - (e as MouseEvent).clientY });
        if (!o) return this.show();
        this.apply({ x: o.x, y: o.y, rel_x: relX, rel_y: relY });
      },
    );
  }

  private resize(e: PointerEvent, k: Handle | "rot") {
    if (k === "rot") return;
    const a = this.atom();
    const media = a ? mediaOf(a) : null;
    if (!a || !media || !this.target) return;
    const chart = chartOf(a);
    e.preventDefault();
    e.stopPropagation();
    this.busy = true;
    const z = this.ed.zoom;
    const w0 = Number(a.dataset.w);
    const h0 = Number(a.dataset.h);
    const ratio = w0 / h0;
    const rot = (Number(media.closest<HTMLElement>(".pic")?.dataset.rot) || 0) * RAD;
    const cos = Math.cos(rot);
    const sin = Math.sin(rot);
    const corner = k.length === 2;
    let w = w0;
    let hh = h0;
    const move = (dx0: number, dy0: number, ev: PointerEvent) => {
      // The pointer's travel along the picture's own axes (the same as the screen's when it is not turned).
      const dxl = dx0 * cos + dy0 * sin;
      const dyl = -dx0 * sin + dy0 * cos;
      const dx = (dxl / z / PT_PX) * (k.includes("w") ? -1 : 1);
      const dy = (dyl / z / PT_PX) * (k.includes("n") ? -1 : 1);
      w = k === "n" || k === "s" ? w0 : Math.max(8, w0 + dx);
      hh = k === "e" || k === "w" ? h0 : Math.max(8, h0 + dy);
      if (corner && !ev.shiftKey) {
        // Keep proportions from the dominant axis.
        if (Math.abs(dx) * (1 / ratio) > Math.abs(dy)) hh = w / ratio;
        else w = hh * ratio;
      }
      // A chart is drawn again at the new size, so its text keeps its size and its labels fit.
      if (chart) redrawChart(a, w, hh);
      else if (media.classList.contains("pic")) sizePicture(media, w * PT_PX, hh * PT_PX);
      else {
        media.style.width = `${w * PT_PX}px`;
        media.style.height = `${hh * PT_PX}px`;
      }
      a.dataset.w = String(w);
      a.dataset.h = String(hh);
      this.sizeReadout.textContent = `${Math.round(w)} × ${Math.round(hh)} pt`;
      this.frame.place(screenBox(a, z));
    };
    track(e, move, (moved) => {
      this.busy = false;
      if (!moved || (Math.abs(w - w0) < 0.5 && Math.abs(hh - h0) < 0.5)) {
        a.dataset.w = String(w0);
        a.dataset.h = String(h0);
        return this.show();
      }
      this.apply({ width: Math.round(w * 10) / 10, height: Math.round(hh * 10) / 10 });
    });
  }

  /** Turn the picture with the handle above it. */
  private turn(e: PointerEvent) {
    const a = this.atom();
    const media = a ? mediaOf(a) : null;
    const im = this.info();
    if (!a || !media || !im || !this.target) return;
    e.preventDefault();
    e.stopPropagation();
    this.busy = true;
    const r = media.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    const rot0 = im.pic?.rot ?? 0;
    const from = Math.atan2(e.clientY - cy, e.clientX - cx);
    const flip = `${im.pic?.flipH ? "scaleX(-1) " : ""}${im.pic?.flipV ? "scaleY(-1)" : ""}`;
    let rot = rot0;
    track(
      e,
      (_dx, _dy, ev) => {
        rot = rot0 + (Math.atan2(ev.clientY - cy, ev.clientX - cx) - from) / RAD;
        if (ev.shiftKey) rot = Math.round(rot / 15) * 15;
        rot = ((Math.round(rot * 10) / 10) % 360 + 360) % 360;
        media.style.transform = `rotate(${rot}deg) ${flip}`;
        const holder = media.closest<HTMLElement>(".pic");
        if (holder) holder.dataset.rot = String(rot);
        this.frame.place(screenBoxAt(a, media, this.ed.zoom, rot, cx, cy));
      },
      (moved) => {
        this.busy = false;
        if (!moved) return this.show();
        this.apply({ rot });
      },
    );
  }
}

/** The frame's box for a picture turned to `rot` degrees about the point `(cx, cy)`. */
function screenBoxAt(a: HTMLElement, _media: HTMLElement, zoom: number, rot: number, cx: number, cy: number) {
  const w = Number(a.dataset.w) * PT_PX * zoom;
  const hh = Number(a.dataset.h) * PT_PX * zoom;
  return { left: cx - w / 2, top: cy - hh / 2, width: w, height: hh, rot };
}
