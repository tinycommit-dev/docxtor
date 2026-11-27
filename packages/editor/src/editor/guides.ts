// Page guides: gridlines (View, Gridlines) and text boundaries (View, Text boundaries). Both are drawn on one layer that
// sits between the sheets and the text, in the stage's own coordinates, so they follow zoom and scroll for free. They
// are only for the screen: the layer is left out of printing, and the PDF is written from the layout, not from this.
//
// Gridlines follow a section's document grid (`w:docGrid`: a line every `linePitch`, and with a character grid a
// column every character) and, where a section has none, Word's drawing grid. Text boundaries are dotted lines around
// the text area of each page and around each of its columns. The boundaries of table cells and text boxes are plain
// CSS on the canvas (`show-boundaries`, see view-window.css), because the cells are already there.
import type { PageSetup, SectionInfo } from "@docxtor/core";
import type { PageBox } from "./layout";
import { charPitch } from "./grid";

/** Word's default spacing for the drawing grid, in inches. */
const DEFAULT_GRID_IN = 0.13;
const PX_IN = 96;
const TWIP_PX = 1 / 15;

export interface GuidesHost {
  canvas: HTMLElement;
  stage: HTMLElement;
  /** The element the layer goes in front of (the flow: the text is drawn over the guides). */
  before: HTMLElement;
  pages(): number;
  box(k: number): Readonly<PageBox>;
  setup(): PageSetup | null;
}

export class PageGuides {
  readonly el: HTMLElement;
  private grid = false;
  private bounds = false;
  private frame = 0;
  private built = "";

  constructor(private host: GuidesHost) {
    this.el = document.createElement("div");
    this.el.className = "pg-guides";
    this.el.setAttribute("aria-hidden", "true");
    this.el.hidden = true;
    host.stage.insertBefore(this.el, host.before);
  }

  get gridlines(): boolean {
    return this.grid;
  }

  get boundaries(): boolean {
    return this.bounds;
  }

  setGridlines(on: boolean) {
    if (on === this.grid) return;
    this.grid = on;
    this.changed();
  }

  setBoundaries(on: boolean) {
    if (on === this.bounds) return;
    this.bounds = on;
    this.host.canvas.classList.toggle("show-boundaries", on);
    this.changed();
  }

  private changed() {
    this.built = "";
    this.el.hidden = !this.grid && !this.bounds;
    this.invalidate();
  }

  /** The pages moved or changed: draw again in the next frame. */
  invalidate() {
    if (!this.grid && !this.bounds) {
      if (this.el.firstChild) this.el.replaceChildren();
      this.built = "";
      return;
    }
    if (this.frame) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      this.draw();
    });
  }

  /** Release the pending frame (the editor is going away). */
  destroy() {
    window.cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.el.remove();
    this.host.canvas.classList.remove("show-boundaries");
  }

  private draw() {
    if (!this.grid && !this.bounds) return;
    const setup = this.host.setup();
    const pages = this.host.pages();
    const parts: HTMLElement[] = [];
    let signature = `${this.grid}${this.bounds}`;
    for (let k = 0; k < pages; k++) {
      const b = this.host.box(k);
      const info = setup?.sections?.[b.section];
      signature += `|${b.top},${b.left},${b.w},${b.h},${b.mT},${b.mB},${b.ml},${b.mr},${b.section}`;
      if (this.grid) parts.push(this.gridOf(b, info));
      if (this.bounds) parts.push(...this.boundariesOf(b, info));
    }
    if (signature === this.built) return;
    this.built = signature;
    this.el.replaceChildren(...parts);
  }

  private box(cls: string, left: number, top: number, w: number, h: number): HTMLElement {
    const el = document.createElement("div");
    el.className = cls;
    el.style.cssText = `left:${left.toFixed(2)}px;top:${top.toFixed(2)}px;width:${Math.max(0, w).toFixed(2)}px;height:${Math.max(0, h).toFixed(2)}px`;
    return el;
  }

  /** The grid of a page: a section's document grid inside its text area, or Word's drawing grid over the whole sheet. */
  private gridOf(b: Readonly<PageBox>, info: SectionInfo | undefined): HTMLElement {
    const grid = info?.docGrid;
    const pitchY = grid?.linePitch ? grid.linePitch * TWIP_PX : 0;
    if (grid && grid.type !== "default" && pitchY > 0) {
      const el = this.box("pg-grid", b.left + b.ml, b.top + b.mT, b.w - b.ml - b.mr, b.h - b.mT - b.mB);
      const pitchX = charPitch(grid, info?.gridBase);
      el.style.setProperty("--grid-y", `${pitchY.toFixed(3)}px`);
      if (pitchX) {
        el.style.setProperty("--grid-x", `${pitchX.toFixed(3)}px`);
        el.classList.add("both");
      } else el.classList.add("rows");
      return el;
    }
    const step = DEFAULT_GRID_IN * PX_IN;
    const el = this.box("pg-grid both", b.left, b.top, b.w, b.h);
    el.style.setProperty("--grid-x", `${step.toFixed(3)}px`);
    el.style.setProperty("--grid-y", `${step.toFixed(3)}px`);
    return el;
  }

  /** The dotted lines of a page: its text area and, inside it, a box for each column. */
  private boundariesOf(b: Readonly<PageBox>, info: SectionInfo | undefined): HTMLElement[] {
    const left = b.left + b.ml;
    const top = b.top + b.mT;
    const w = b.w - b.ml - b.mr;
    const h = b.h - b.mT - b.mB;
    const out = [this.box("pg-text", left, top, w, h)];
    const cols = info?.cols;
    if (cols && cols.num > 1 && cols.num <= 6) {
      const widths = cols.widths.length === cols.num ? cols.widths.map((x) => x * TWIP_PX) : null;
      const gap = (cols.space ?? 0) * TWIP_PX;
      const each = widths ? 0 : (w - gap * (cols.num - 1)) / cols.num;
      let x = 0;
      for (let i = 0; i < cols.num; i++) {
        const cw = widths ? widths[i] : each;
        out.push(this.box("pg-col", left + x, top, cw, h));
        x += cw + (widths ? (cols.spaces?.[i] ?? cols.space ?? 0) * TWIP_PX : gap);
      }
    }
    return out;
  }
}
