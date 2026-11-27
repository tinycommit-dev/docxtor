// Zoom: the numbers (pure, tested in Node) and the controller that applies them to the editor. A zoom is a
// factor (1 is 100%) held by the editor; this file decides which factor, from a mode:
//
//   auto    follows the window width, never above 100% (the default, as it always was)
//   width   page width: the widest page fills the window, at any factor
//   page    whole page: the page in view fits the window, width and height
//   pages   several pages side by side: a read-only grid of whole pages (see page-grid.ts), `across` to a row
//   custom  a factor somebody chose (the slider, a preset, the zoom keys, Ctrl and the wheel, a pinch)
//
// Only the factor changes the layout's size; the pages are laid out at 100%.
import type { Editor } from "./editor";
import { clampZoom, fitPageZoom, fitWidthZoom, pagesZoom, pinchZoom, wheelZoom, type ZoomMode } from "./zoom-math";

export * from "./zoom-math";


export class Zoomer {
  mode: ZoomMode = "auto";
  /** Pages per row in the "pages" mode. */
  across = 2;
  private pinch: { z: number; d: number } | null = null;
  private touches = new Map<number, { x: number; y: number }>();
  private onChange = new Set<() => void>();

  constructor(private ed: Editor) {}

  /** Runs after the factor or the mode changed. */
  onDidChange(fn: () => void): () => void {
    this.onChange.add(fn);
    return () => this.onChange.delete(fn);
  }

  private changed() {
    for (const fn of this.onChange) fn();
  }

  /** A factor somebody chose. `anchor` is a point on screen to keep still (the pointer), or "centre" for the middle of the window; without one the view isn't scrolled. */
  set(z: number, anchor?: { x: number; y: number } | "centre") {
    const was = this.mode;
    this.mode = "custom";
    if (was === "pages") this.ed.grid.hide();
    this.setKeeping(clampZoom(z), anchor);
    this.changed();
  }

  /** Use a mode: it picks the factor now and keeps it as the window or the pages change. */
  setMode(mode: ZoomMode, across = this.across) {
    this.across = across;
    const was = this.mode;
    this.mode = mode;
    if (mode === "custom") return;
    if (was === "pages" && mode !== "pages") this.ed.grid.hide();
    this.apply();
  }

  /** Work out the factor for the mode (after a resize, a layout, a new document or a change of mode). */
  apply() {
    const ed = this.ed;
    if (!ed.engine || !ed.canvasReady) return;
    const cs = getComputedStyle(ed.canvas);
    const availW = ed.canvas.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    const availH = ed.canvas.clientHeight;
    const pageW = ed.stageWidth;
    const page = ed.pageBox(Math.max(0, ed.currentPage() - 1));
    let z = ed.zoom;
    switch (this.mode) {
      case "auto":
        z = fitWidthZoom(availW, pageW, 1);
        break;
      case "width":
        z = fitWidthZoom(availW, pageW);
        break;
      case "page":
        z = ed.continuous ? fitWidthZoom(availW, pageW) : fitPageZoom(availW, availH, pageW, page.h, ed.rulerHeight);
        break;
      case "pages":
        if (ed.continuous) {
          // One long sheet has no pages to put side by side.
          this.mode = "width";
          z = fitWidthZoom(availW, pageW);
          break;
        }
        z = pagesZoom(availW, ed.widestPage, this.across, 16);
        ed.grid.show(this.across, z);
        break;
      default:
        return;
    }
    if (z !== ed.zoom || !ed.zoomApplied) {
      ed.setZoom(z, false);
      this.changed();
    } else if (this.mode === "pages") this.changed();
  }

  /** Change the factor, keeping the point under `anchor` where it is. */
  private setKeeping(z: number, anchor?: { x: number; y: number } | "centre") {
    const ed = this.ed;
    if (!anchor) return ed.setZoom(z, false);
    const c = ed.canvas.getBoundingClientRect();
    const ax = anchor === "centre" ? c.left + c.width / 2 : anchor.x;
    const ay = anchor === "centre" ? c.top + c.height / 2 : anchor.y;
    const stage = ed.stageElement.getBoundingClientRect();
    const px = (ax - stage.left) / ed.zoom;
    const py = (ay - stage.top) / ed.zoom;
    // The pages are the same at every zoom (laid out at 100% and drawn scaled), so the point of the pages under the pointer stays under it.
    ed.setZoom(z, false);
    const after = ed.stageElement.getBoundingClientRect();
    ed.canvas.scrollLeft += after.left + px * ed.zoom - ax;
    ed.canvas.scrollTop += after.top + py * ed.zoom - ay;
  }

  /** Ctrl (or ⌘) with the wheel, and a trackpad pinch (which Chrome sends the same way). Returns true when it took the event. */
  wheel(e: WheelEvent): boolean {
    if (!(e.ctrlKey || e.metaKey) || !this.ed.engine) return false;
    e.preventDefault();
    const z = wheelZoom(this.ed.zoom, e.deltaY, e.deltaMode);
    if (z !== this.ed.zoom) this.set(z, { x: e.clientX, y: e.clientY });
    return true;
  }

  /** Two fingers on a touch screen. */
  pointer(e: PointerEvent) {
    if (e.pointerType !== "touch" || !this.ed.engine) return;
    if (e.type === "pointerdown") {
      this.touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (this.touches.size === 2) this.pinch = { z: this.ed.zoom, d: this.distance() };
    } else if (e.type === "pointermove") {
      if (!this.touches.has(e.pointerId)) return;
      this.touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (this.pinch && this.touches.size === 2) {
        const [a, b] = [...this.touches.values()];
        this.set(pinchZoom(this.pinch.z, this.pinch.d, this.distance()), { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
      }
    } else {
      this.touches.delete(e.pointerId);
      if (this.touches.size < 2) this.pinch = null;
    }
  }

  private distance(): number {
    const [a, b] = [...this.touches.values()];
    return a && b ? Math.hypot(a.x - b.x, a.y - b.y) : 0;
  }

  /** Listen for the wheel and for pinches on the editing area; the result undoes it. */
  bind(el: HTMLElement): () => void {
    const wheel = (e: WheelEvent) => void this.wheel(e);
    const pointer = (e: PointerEvent) => this.pointer(e);
    el.addEventListener("wheel", wheel, { passive: false });
    for (const type of ["pointerdown", "pointermove", "pointerup", "pointercancel"]) el.addEventListener(type, pointer as EventListener);
    return () => {
      el.removeEventListener("wheel", wheel);
      for (const type of ["pointerdown", "pointermove", "pointerup", "pointercancel"]) el.removeEventListener(type, pointer as EventListener);
    };
  }
}
