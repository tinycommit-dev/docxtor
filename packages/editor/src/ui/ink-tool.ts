// Drawing by hand: Insert, Drawing, Draw. A layer over the pages takes the pen strokes (mouse, pen or finger) with a
// strip for the pen, the eraser and the lasso. A pen is a kind (pen, pencil, marker, highlighter) with a colour and a
// width; each stroke keeps the pen it was drawn with. The eraser takes whole strokes or just the part under its circle;
// the lasso picks strokes to move, recolour or delete. Done puts the strokes in the text, one drawing for each pen (a
// shape with a smooth path for each stroke), in one undo step. A drawing is then an ordinary shape: moved, resized,
// recoloured, deleted. Which pen was last used is remembered for the session.
import { msg, t, tn } from "@docxtor/extensions";
import type { Op } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { frameOf } from "../editor/float-place";
import { ALIGN_ICONS } from "./align-icons";
import { h, overlayLayer, svg } from "./dom";
import { inkFromShape } from "./ink-import";
import { boundsOf, caughtBy, cutOut, distToStroke, type Pt } from "./ink-math";
import { pickColor, type ShapeTool } from "./shape-tool";
import { hexOr } from "../editor/color.ts";

const PT_PX = 96 / 72;
const NS = "http://www.w3.org/2000/svg";

export type PenKind = "pen" | "pencil" | "marker" | "highlighter";
type Tool = "draw" | "erase" | "lasso";
type EraserMode = "stroke" | "point";

/** The pen a stroke was drawn with. */
export interface Pen {
  kind: PenKind;
  /** Hex without `#`. */
  color: string;
  /** Points. */
  width: number;
  /** 0 to 1. */
  alpha: number;
  cap: "round" | "square";
}

export interface Stroke {
  pts: Pt[];
  pen: Pen;
}

interface Preset {
  label: string;
  icon: keyof typeof ALIGN_ICONS;
  color: string;
  width: number;
  widths: number[];
  alpha: number;
  cap: "round" | "square";
}

/** What each kind of pen starts as: a highlighter is wide, translucent and square-ended, like a marker pen. */
export const PRESETS: Record<PenKind, Preset> = {
  pen: { label: msg("Pen"), icon: "pen", color: "000000", width: 2.25, widths: [0.75, 1.5, 2.25, 4.5, 6, 9], alpha: 1, cap: "round" },
  pencil: { label: msg("Pencil"), icon: "pencil", color: "404040", width: 1, widths: [0.5, 0.75, 1, 1.5, 2.25], alpha: 0.8, cap: "round" },
  marker: { label: msg("Marker"), icon: "marker", color: "1A73E8", width: 4.5, widths: [3, 4.5, 6, 9, 12], alpha: 1, cap: "round" },
  highlighter: { label: msg("Highlighter"), icon: "highlighter", color: "FFEB3B", width: 15, widths: [9, 12, 15, 18, 24], alpha: 0.5, cap: "square" },
};
const KINDS = Object.keys(PRESETS) as PenKind[];

/** Quick colours when there are no recent ones to show. */
const QUICK = ["000000", "D93025", "1A73E8", "188038", "F9AB00", "FFEB3B"];

/** The eraser's radius in points, by size. */
const ERASER_SIZES: [string, number][] = [
  [msg("Small eraser"), 4],
  [msg("Medium eraser"), 9],
  [msg("Large eraser"), 18],
];

/** What the tool remembers while the page stays open. */
const session = {
  tool: "draw" as Tool,
  kind: "pen" as PenKind,
  colors: Object.fromEntries(KINDS.map((k) => [k, PRESETS[k].color])) as Record<PenKind, string>,
  widths: Object.fromEntries(KINDS.map((k) => [k, PRESETS[k].width])) as Record<PenKind, number>,
  recent: [] as string[],
  eraser: "stroke" as EraserMode,
  eraserSize: 9,
};

export class InkTool {
  private layer: HTMLElement | null = null;
  private svg: SVGSVGElement | null = null;
  private strip: HTMLElement | null = null;
  private count: HTMLElement | null = null;
  private strokes: Stroke[] = [];
  /** Earlier states of `strokes`, for Undo stroke. */
  private history: Stroke[][] = [];
  /** The stroke being drawn. */
  private live: { pts: Pt[]; path: SVGPathElement } | null = null;
  private off: (() => void) | null = null;
  /** The strokes the lasso holds (indexes into `strokes`). */
  private picked = new Set<number>();
  /** The lasso being drawn, or a move of the held strokes. */
  private loop: { pts: Pt[]; path: SVGPathElement } | null = null;
  private move: { from: Pt; dx: number; dy: number } | null = null;
  /** The eraser is down (one undo step for everything it takes in one go). */
  private erasing: { changed: boolean } | null = null;
  private cursor: HTMLElement | null = null;
  private refresh: (() => void) | null = null;
  /** A drawing taken back into the tool: where it is in the text, and its element (hidden while it is drawn on the layer). */
  private editing: { at: { p: number; o: number }; atom: HTMLElement } | null = null;

  constructor(
    private ed: Editor,
    private shapes: ShapeTool,
  ) {}

  /** The pen is down on the pages. */
  get active(): boolean {
    return !!this.layer;
  }

  /** Strokes drawn so far. */
  get length(): number {
    return this.strokes.length;
  }

  /** The pen now in hand. */
  private get pen(): Pen {
    const k = session.kind;
    const p = PRESETS[k];
    return { kind: k, color: session.colors[k], width: session.widths[k], alpha: p.alpha, cap: p.cap };
  }

  /** Where a pointer is on the stage, in pixels before zoom. */
  private point(e: PointerEvent): Pt {
    const r = this.ed.stageElement.getBoundingClientRect();
    const z = this.ed.zoom || 1;
    return { x: (e.clientX - r.left) / z, y: (e.clientY - r.top) / z };
  }

  start() {
    if (this.layer || this.ed.readOnly || this.ed.story) return;
    // A drawing that is selected is taken back into the tool: its strokes can be added to, erased, moved and recoloured.
    const taken = this.takeSelected();
    // A shape that was selected is let go (its frame would otherwise come back with the selection).
    const r = this.ed.range();
    this.shapes.hide();
    if (r && !taken) this.ed.select(r[0]);
    const layer = h("div", { class: "ink-layer", "aria-label": t("Drawing"), role: "application" });
    const svgEl = document.createElementNS(NS, "svg");
    svgEl.setAttribute("aria-hidden", "true");
    layer.append(svgEl);
    this.ed.stageElement.append(layer);
    this.layer = layer;
    this.svg = svgEl;
    this.strokes = taken?.strokes ?? [];
    this.history = [];
    this.picked.clear();
    this.buildStrip();
    this.syncLayer();
    layer.addEventListener("pointerdown", (e) => this.down(e));
    layer.addEventListener("pointermove", (e) => this.moved(e));
    layer.addEventListener("pointerup", (e) => this.up(e));
    layer.addEventListener("pointercancel", (e) => this.up(e));
    layer.addEventListener("pointerleave", () => this.cursor?.style.setProperty("display", "none"));
    const key = (e: KeyboardEvent) => this.onKey(e);
    document.addEventListener("keydown", key, true);
    this.off = () => document.removeEventListener("keydown", key, true);
    layer.tabIndex = -1;
    layer.focus({ preventScroll: true });
  }

  /** The selected shape, if the drawing tool made it (or could have): its strokes, and the shape hidden while they are on the layer. */
  private takeSelected(): { strokes: Stroke[] } | null {
    const at = this.shapes.targetPos;
    if (!at || this.shapes.multiple) return null;
    const atom = this.ed.paraElement(at.p)?.querySelector<HTMLElement>(`:scope > .atom.shape[data-o="${at.o}"]`);
    let model = null;
    try {
      model = this.ed.engine.shapeAt(at);
    } catch {
      model = null;
    }
    if (!atom || !model) return null;
    const stage = this.ed.stageElement.getBoundingClientRect();
    const box = atom.getBoundingClientRect();
    const z = this.ed.zoom || 1;
    const ink = inkFromShape(model, (box.left - stage.left) / z, (box.top - stage.top) / z);
    if (!ink) return null;
    this.editing = { at: { p: at.p, o: at.o }, atom };
    atom.style.visibility = "hidden";
    return { strokes: ink.strokes.map((pts) => ({ pts, pen: ink.pen })) };
  }

  private onKey(e: KeyboardEvent) {
    const inField = !!(e.target as HTMLElement).closest("select,input,textarea");
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      // The first Escape lets go of the lasso's strokes; the next one puts the pen down.
      if (this.picked.size) this.pick([]);
      else this.cancel();
    } else if (e.key === "Enter" && !(e.target as HTMLElement).closest("select,button")) {
      e.preventDefault();
      e.stopPropagation();
      this.finish();
    } else if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === "z") {
      e.preventDefault();
      e.stopPropagation();
      this.undoStroke();
    } else if ((e.key === "Delete" || e.key === "Backspace") && this.picked.size && !inField) {
      e.preventDefault();
      e.stopPropagation();
      this.deletePicked();
    }
  }

  // -- the strip -------------------------------------------------------------------------------------

  private buildStrip() {
    const press = (b: HTMLElement, on: boolean) => {
      b.classList.toggle("on", on);
      b.setAttribute("aria-pressed", String(on));
    };
    const toolButton = (label: string, icon: keyof typeof ALIGN_ICONS, title: string) => {
      const b = h("button", { type: "button", class: "ink-tool", title, "aria-label": label, "aria-pressed": "false" }, h("span", { class: "ink-tool-icon", "aria-hidden": "true" }, svg(ALIGN_ICONS[icon])), h("span", {}, label));
      b.addEventListener("mousedown", (e) => e.preventDefault());
      return b;
    };

    const pens = h("div", { class: "seg", role: "group", "aria-label": t("Pen") });
    const penButtons = new Map<PenKind, HTMLButtonElement>();
    for (const k of KINDS) {
      const b = toolButton(t(PRESETS[k].label), PRESETS[k].icon, t(PRESETS[k].label));
      b.addEventListener("click", () => {
        session.kind = k;
        session.tool = "draw";
        this.pick([]);
        this.syncStrip();
      });
      penButtons.set(k, b);
      pens.append(b);
    }
    const erasers = h("div", { class: "seg", role: "group", "aria-label": t("Eraser") });
    const eraserButtons = new Map<EraserMode, HTMLButtonElement>();
    for (const [mode, label, title] of [
      ["stroke", t("Stroke eraser"), t("Erase a whole stroke where you touch it")],
      ["point", t("Point eraser"), t("Erase only the part of a stroke you rub out")],
    ] as const) {
      const b = toolButton(label, "eraser", title);
      b.addEventListener("click", () => {
        session.tool = "erase";
        session.eraser = mode;
        this.pick([]);
        this.syncStrip();
      });
      eraserButtons.set(mode, b);
      erasers.append(b);
    }
    const lasso = toolButton(t("Lasso select"), "lasso", t("Draw a loop round strokes to move, recolor or delete them"));
    lasso.addEventListener("click", () => {
      session.tool = "lasso";
      this.syncStrip();
    });

    // The colour in hand, and a few to pick from: the ones used lately, then the usual.
    const chip = h("span", { class: "chip" });
    const color = h("button", { type: "button", class: "tb color-chip", title: t("Pen color"), "aria-label": t("Pen color") }, chip, h("span", {}, t("Color")));
    color.addEventListener("mousedown", (e) => e.preventDefault());
    color.addEventListener("click", () => pickColor(color, t("Pen color"), this.currentColor(), null, (c) => c && this.setColor(c)));
    const swatches = h("div", { class: "ink-swatches", role: "group", "aria-label": t("Colors") });
    const width = h("select", { class: "tb-select", "aria-label": t("Pen width"), title: t("Pen width") });
    width.addEventListener("change", () => {
      if (session.tool === "erase") session.eraserSize = Number(width.value);
      else session.widths[session.kind] = Number(width.value);
      this.syncStrip();
    });
    this.count = h("span", { class: "readout obj-size", "aria-live": "polite" });
    const del = h("button", { type: "button", class: "tb", title: t("Delete the selected strokes") }, t("Delete"));
    del.addEventListener("click", () => this.deletePicked());
    const undo = h("button", { type: "button", class: "tb", title: t("Undo last stroke") }, t("Undo stroke"));
    undo.addEventListener("click", () => this.undoStroke());
    const cancel = h("button", { type: "button", class: "tb", title: t("Cancel") }, t("Cancel"));
    cancel.addEventListener("click", () => this.cancel());
    const done = h("button", { type: "button", class: "btn primary", title: t("Put the drawing in the document") }, t("Done"));
    done.addEventListener("click", () => this.finish());

    this.strip = h(
      "div",
      { class: "obj-strip ink-strip", role: "toolbar", "aria-label": t("Drawing") },
      h("div", { class: "ink-row" }, pens, erasers, h("div", { class: "seg", role: "group", "aria-label": t("Select") }, lasso)),
      h("div", { class: "ink-row" }, color, swatches, width, this.count, del, undo, cancel, done),
    );
    overlayLayer().append(this.strip);

    this.refresh = () => {
      for (const [k, b] of penButtons) press(b, session.tool === "draw" && session.kind === k);
      for (const [m, b] of eraserButtons) press(b, session.tool === "erase" && session.eraser === m);
      press(lasso, session.tool === "lasso");
      const c = this.currentColor();
      chip.style.background = `#${c}`;
      // Colours: the last few used, then the quick ones, up to six.
      const list = [...session.recent, ...QUICK.filter((q) => !session.recent.includes(q))].slice(0, 6);
      swatches.replaceChildren(
        ...list.map((hex) => {
          const on = hex === c;
          const s = h("button", { type: "button", class: on ? "swatch on" : "swatch", title: `#${hex}`, "aria-label": `#${hex}`, "aria-pressed": String(on), style: `background:${hexOr(hex)}` });
          s.addEventListener("mousedown", (e) => e.preventDefault());
          s.addEventListener("click", () => this.setColor(hex));
          return s;
        }),
      );
      // The width select is the eraser's size while the eraser is in hand.
      const erasing = session.tool === "erase";
      width.setAttribute("aria-label", erasing ? t("Eraser size") : t("Pen width"));
      width.title = erasing ? t("Eraser size") : t("Pen width");
      width.replaceChildren(
        ...(erasing
          ? ERASER_SIZES.map(([label, r]) => h("option", { value: String(r), selected: r === session.eraserSize }, t(label)))
          : (PRESETS[session.kind].widths.includes(session.widths[session.kind]) ? PRESETS[session.kind].widths : [...PRESETS[session.kind].widths, session.widths[session.kind]].sort((a, b) => a - b)).map((v) => h("option", { value: String(v), selected: v === session.widths[session.kind] }, `${v} pt`))),
      );
      color.disabled = erasing;
      width.disabled = session.tool === "lasso";
      for (const s of Array.from(swatches.children) as HTMLButtonElement[]) s.disabled = erasing;
      del.hidden = !this.picked.size;
      this.count!.textContent = this.picked.size ? tn(this.picked.size, "{n} stroke selected", "{n} strokes selected") : tn(this.strokes.length, "{n} stroke", "{n} strokes");
      undo.disabled = !this.history.length;
      this.placeStrip();
    };
    this.refresh();
  }

  private syncStrip() {
    this.refresh?.();
    this.syncLayer();
  }

  /** The colour the strip shows: the held strokes' when they all share one, else the pen's. */
  private currentColor(): string {
    if (session.tool === "lasso" && this.picked.size) {
      const colors = new Set(Array.from(this.picked).map((i) => this.strokes[i]?.pen.color));
      if (colors.size === 1) return [...colors][0]!;
    }
    return session.colors[session.kind];
  }

  /** Choose a colour for the pen, or for the strokes the lasso holds. */
  private setColor(hex: string) {
    const c = hex.toUpperCase();
    session.recent = [c, ...session.recent.filter((r) => r !== c)].slice(0, 6);
    if (session.tool === "lasso" && this.picked.size) {
      this.snapshot();
      for (const i of this.picked) this.strokes[i] = { ...this.strokes[i], pen: { ...this.strokes[i].pen, color: c } };
      this.redraw();
    } else if (session.tool === "draw") session.colors[session.kind] = c;
    this.syncStrip();
  }

  private placeStrip() {
    if (!this.strip) return;
    const c = this.ed.canvas.getBoundingClientRect();
    const w = this.strip.offsetWidth || 560;
    this.strip.style.left = `${Math.max(8, c.left + c.width / 2 - w / 2)}px`;
    // Never above the window, even when the page itself has been scrolled.
    this.strip.style.top = `${Math.max(8, c.top + 12)}px`;
  }

  /** What the layer does with the pointer, for the tool in hand. */
  private syncLayer() {
    if (!this.layer) return;
    this.layer.dataset.tool = session.tool === "erase" ? `erase-${session.eraser}` : session.tool === "lasso" ? "lasso" : session.kind;
    if (session.tool !== "erase") this.cursor?.remove(), (this.cursor = null);
    this.redraw();
  }

  // -- strokes on the layer ---------------------------------------------------------------------------

  private path(points: Pt[]): string {
    return points.map((p, i) => `${i ? "L" : "M"}${p.x.toFixed(1)} ${p.y.toFixed(1)}`).join(" ") + (points.length === 1 ? ` L${(points[0].x + 0.01).toFixed(2)} ${points[0].y.toFixed(1)}` : "");
  }

  private strokePath(pts: Pt[], pen: Pen): SVGPathElement {
    const path = document.createElementNS(NS, "path");
    path.setAttribute("class", "ink-stroke");
    path.dataset.kind = pen.kind;
    path.setAttribute("stroke", hexOr(pen.color));
    path.setAttribute("stroke-width", String(pen.width * PT_PX));
    if (pen.alpha < 1) path.setAttribute("stroke-opacity", String(pen.alpha));
    path.style.strokeLinecap = pen.cap;
    path.setAttribute("d", this.path(pts));
    return path;
  }

  /** Draw every stroke again from the list (and the lasso's selection over them). */
  private redraw() {
    const svgEl = this.svg;
    if (!svgEl) return;
    svgEl.replaceChildren();
    const picked = Array.from(this.picked).filter((i) => this.strokes[i]);
    // The strokes the lasso holds sit on a wash.
    for (const i of picked) {
      const s = this.strokes[i];
      const under = document.createElementNS(NS, "path");
      under.setAttribute("class", "ink-sel");
      under.setAttribute("stroke-width", String(s.pen.width * PT_PX + 7));
      under.setAttribute("d", this.path(s.pts));
      svgEl.append(under);
    }
    for (const [i, s] of this.strokes.entries()) {
      const path = this.strokePath(s.pts, s.pen);
      path.dataset.i = String(i);
      svgEl.append(path);
    }
    const box = boundsOf(picked.map((i) => this.strokes[i].pts));
    if (box) {
      const pad = Math.max(...picked.map((i) => (this.strokes[i].pen.width * PT_PX) / 2)) + 6;
      const rect = document.createElementNS(NS, "rect");
      rect.setAttribute("class", "ink-sel-box");
      rect.setAttribute("x", String(box.left - pad));
      rect.setAttribute("y", String(box.top - pad));
      rect.setAttribute("width", String(box.right - box.left + pad * 2));
      rect.setAttribute("height", String(box.bottom - box.top + pad * 2));
      svgEl.append(rect);
    }
    if (this.live) svgEl.append(this.live.path);
    if (this.loop) svgEl.append(this.loop.path);
  }

  /** Remember the strokes as they are now, for Undo stroke. */
  private snapshot() {
    this.history.push(this.strokes.map((s) => ({ pts: s.pts, pen: s.pen })));
  }

  /** Where the held strokes are: the box round them, with a little room. */
  private pickedBox() {
    const picked = Array.from(this.picked).filter((i) => this.strokes[i]);
    const box = boundsOf(picked.map((i) => this.strokes[i].pts));
    if (!box) return null;
    const pad = Math.max(...picked.map((i) => (this.strokes[i].pen.width * PT_PX) / 2)) + 6;
    return { left: box.left - pad, top: box.top - pad, right: box.right + pad, bottom: box.bottom + pad };
  }

  private pick(list: number[]) {
    this.picked = new Set(list);
    this.redraw();
    this.refresh?.();
  }

  // -- the pointer -----------------------------------------------------------------------------------------

  private radius(): number {
    return session.eraserSize * PT_PX;
  }

  private down(e: PointerEvent) {
    if (e.button !== 0 || !this.svg || this.live || this.loop || this.erasing || this.move) return;
    e.preventDefault();
    this.layer!.setPointerCapture(e.pointerId);
    const p = this.point(e);
    if (session.tool === "erase") {
      this.erasing = { changed: false };
      this.snapshot();
      this.erase(p);
      return;
    }
    if (session.tool === "lasso") {
      const box = this.pickedBox();
      if (box && p.x >= box.left && p.x <= box.right && p.y >= box.top && p.y <= box.bottom) {
        // Inside the box round the held strokes: a drag moves them.
        this.move = { from: p, dx: 0, dy: 0 };
        return;
      }
      this.picked.clear();
      const path = document.createElementNS(NS, "path");
      path.setAttribute("class", "ink-loop");
      path.setAttribute("d", this.path([p]));
      this.loop = { pts: [p], path };
      this.redraw();
      this.refresh?.();
      return;
    }
    const pen = this.pen;
    const path = this.strokePath([p], pen);
    this.live = { pts: [p], path };
    this.svg.append(path);
  }

  private moved(e: PointerEvent) {
    const events = typeof e.getCoalescedEvents === "function" ? e.getCoalescedEvents() : [];
    const list = events.length ? events : [e];
    if (session.tool === "erase") {
      const p = this.point(e);
      this.showCursor(p);
      if (this.erasing) for (const ev of list) this.erase(this.point(ev));
      return;
    }
    if (this.move) {
      const p = this.point(e);
      this.move.dx = p.x - this.move.from.x;
      this.move.dy = p.y - this.move.from.y;
      const shift = `translate(${this.move.dx} ${this.move.dy})`;
      for (const el of Array.from(this.svg!.querySelectorAll<SVGElement>(".ink-sel, .ink-sel-box"))) el.setAttribute("transform", shift);
      for (const i of this.picked) this.svg!.querySelector(`path.ink-stroke[data-i="${i}"]`)?.setAttribute("transform", shift);
      return;
    }
    if (this.loop) {
      for (const ev of list) this.loop.pts.push(this.point(ev));
      this.loop.path.setAttribute("d", this.path(this.loop.pts));
      return;
    }
    if (!this.live) return;
    for (const ev of list) {
      const p = this.point(ev);
      const last = this.live.pts[this.live.pts.length - 1];
      // Points closer than a pixel add weight, not shape.
      if (Math.hypot(p.x - last.x, p.y - last.y) >= 1) this.live.pts.push(p);
    }
    this.live.path.setAttribute("d", this.path(this.live.pts));
  }

  private up(e: PointerEvent) {
    this.layer?.releasePointerCapture?.(e.pointerId);
    if (this.erasing) {
      // Pen up: whatever the eraser took is one step; if it took nothing, there is nothing to undo.
      if (!this.erasing.changed) this.history.pop();
      this.erasing = null;
      this.syncStrip();
      return;
    }
    if (this.move) {
      const { dx, dy } = this.move;
      this.move = null;
      if (Math.hypot(dx, dy) >= 0.5) {
        this.snapshot();
        for (const i of this.picked) this.strokes[i] = { ...this.strokes[i], pts: this.strokes[i].pts.map((p) => ({ x: p.x + dx, y: p.y + dy })) };
      }
      this.redraw();
      this.refresh?.();
      return;
    }
    if (this.loop) {
      const loop = this.loop.pts;
      this.loop = null;
      this.pick(loop.length >= 3 ? this.strokes.flatMap((s, i) => (caughtBy(loop, s.pts) ? [i] : [])) : []);
      return;
    }
    if (!this.live) return;
    this.snapshot();
    this.strokes.push({ pts: this.live.pts, pen: this.pen });
    this.live = null;
    this.redraw();
    this.refresh?.();
  }

  // -- erasing ------------------------------------------------------------------------------------------------

  private showCursor(p: Pt) {
    if (!this.layer) return;
    if (!this.cursor) {
      this.cursor = h("div", { class: "ink-eraser", "aria-hidden": "true" });
      this.layer.append(this.cursor);
    }
    const r = this.radius();
    this.cursor.style.cssText = `display:block;left:${p.x - r}px;top:${p.y - r}px;width:${r * 2}px;height:${r * 2}px`;
  }

  /** Take what is under the eraser at a point: a whole stroke, or the part inside the circle. */
  private erase(p: Pt) {
    this.showCursor(p);
    const r = this.radius();
    const next: Stroke[] = [];
    let changed = false;
    for (const s of this.strokes) {
      if (session.eraser === "stroke") {
        if (distToStroke(p, s.pts) <= r + (s.pen.width * PT_PX) / 2) changed = true;
        else next.push(s);
        continue;
      }
      // The pen's own width counts: the circle reaches the edge of the line.
      const cut = cutOut(s.pts, p, r + (s.pen.width * PT_PX) / 2);
      if (!cut) next.push(s);
      else {
        changed = true;
        for (const piece of cut) next.push({ pts: piece, pen: s.pen });
      }
    }
    if (!changed) return;
    this.strokes = next;
    if (this.erasing) this.erasing.changed = true;
    this.picked.clear();
    this.redraw();
    this.count!.textContent = tn(this.strokes.length, "{n} stroke", "{n} strokes");
  }

  // -- taking back and finishing ---------------------------------------------------------------------------

  private deletePicked() {
    if (!this.picked.size) return;
    this.snapshot();
    this.strokes = this.strokes.filter((_, i) => !this.picked.has(i));
    this.picked.clear();
    this.redraw();
    this.refresh?.();
  }

  private undoStroke() {
    if (this.live || this.loop || this.erasing || this.move) return;
    const before = this.history.pop();
    if (!before) return;
    this.strokes = before;
    this.picked.clear();
    this.redraw();
    this.refresh?.();
  }

  /** The paragraph a drawing started at a stage point belongs to: the one under it, else the nearest in the text. */
  private paragraphAt(stage: Pt): HTMLElement | null {
    const r = this.ed.stageElement.getBoundingClientRect();
    const z = this.ed.zoom || 1;
    const x = r.left + stage.x * z;
    const y = r.top + stage.y * z;
    const layer = this.layer;
    if (layer) layer.style.pointerEvents = "none";
    const under = document.elementsFromPoint(x, y).map((el) => el.closest<HTMLElement>("[data-p]")).find((p) => p && this.ed.flow.contains(p) && !p.closest(".atom"));
    if (layer) layer.style.pointerEvents = "";
    if (under) return under;
    let best: { el: HTMLElement; d: number } | null = null;
    for (const el of Array.from(this.ed.flow.querySelectorAll<HTMLElement>("[data-p]"))) {
      if (el.closest(".atom")) continue;
      const b = el.getBoundingClientRect();
      const d = y < b.top ? b.top - y : y > b.bottom ? y - b.bottom : 0;
      if (!best || d < best.d) best = { el, d };
    }
    return best?.el ?? null;
  }

  /** Put the strokes in the text: one drawing for each pen, all in one undo step. */
  finish() {
    if (!this.layer || this.live || this.loop || this.erasing || this.move) return;
    const strokes = this.strokes;
    const editing = this.editing;
    // A drawing taken back in goes out again in its own place, in the same undo step (the old one is replaced).
    const gone = editing ? ({ op: "delete", from: editing.at, to: { p: editing.at.p, o: editing.at.o + 1 } } as Op) : null;
    if (!strokes.length) {
      if (!gone) return this.cancel();
      this.close();
      this.ed.exec(gone, { select: { p: editing!.at.p, o: editing!.at.o } });
      this.ed.focus();
      return;
    }
    const para = editing ? this.ed.paraElement(editing.at.p) : this.paragraphAt(strokes[0].pts[0]);
    this.close();
    if (!para) return;
    // Points are counted from the column and the paragraph, like the offsets of any shape that floats in the text.
    const frame = frameOf(this.ed.floatHost, para, undefined, undefined);
    const rel = (p: Pt): [number, number] => [Math.round(((p.x - frame.h[0]) / PT_PX) * 10) / 10, Math.round(((p.y - frame.v[0]) / PT_PX) * 10) / 10];
    const at = editing ? { p: editing.at.p, o: editing.at.o } : { p: Number(para.dataset.p), o: 0 };
    // Strokes with the same pen go in one drawing; the pens keep the order they were first used in, so what was drawn last stays on top.
    const groups = new Map<string, Stroke[]>();
    for (const s of strokes) {
      const k = `${s.pen.color}|${s.pen.width}|${s.pen.alpha}|${s.pen.cap}`;
      groups.set(k, [...(groups.get(k) ?? []), s]);
    }
    const ops: Op[] = [...groups.values()].map((list) => {
      const pen = list[0].pen;
      const op: Record<string, unknown> = { op: "insert_ink", at, strokes: list.map((s) => s.pts.map(rel)), color: pen.color, width_pt: pen.width };
      if (pen.alpha < 1) op.alpha = pen.alpha;
      if (pen.cap !== "round") op.cap = pen.cap;
      return op as Op;
    });
    if (gone) ops.unshift(gone);
    if (this.ed.exec(ops, { select: { anchor: at, focus: { p: at.p, o: at.o + 1 } } })) this.shapes.selectAt(at.p, at.o);
  }

  cancel() {
    this.close();
    this.ed.focus();
  }

  private close() {
    if (this.editing?.atom.isConnected) this.editing.atom.style.visibility = "";
    this.editing = null;
    this.off?.();
    this.off = null;
    this.layer?.remove();
    this.strip?.remove();
    this.layer = this.svg = this.strip = this.count = this.cursor = this.refresh = null;
    this.strokes = [];
    this.history = [];
    this.picked.clear();
    this.live = this.loop = this.move = this.erasing = null;
  }
}
