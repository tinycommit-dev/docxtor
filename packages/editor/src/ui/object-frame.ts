// The selection frame of an object on the page (a shape, and anything else that is drawn as a box): eight
// resize handles, an optional rotation handle, and the pointer tracking for dragging them. A tool owns the
// meaning of the gestures (what to change, which op to apply); the frame only draws and reports.
import { h } from "./dom";

export const HANDLES = ["nw", "n", "ne", "e", "se", "s", "sw", "w"] as const;
export type Handle = (typeof HANDLES)[number];

export interface FrameBox {
  /** The unrotated box in viewport pixels. */
  left: number;
  top: number;
  width: number;
  height: number;
  /** Degrees clockwise. */
  rot?: number;
}

export class ObjectFrame {
  readonly el: HTMLElement;
  private rotHandle: HTMLElement | null = null;

  /**
   * @param onHandle called on pointer-down on a resize handle, or `"rot"` on the rotation handle
   * @param opts `rotate`: show the rotation handle; `onMove`: the frame's edges drag the object
   */
  constructor(onHandle: (e: PointerEvent, handle: Handle | "rot") => void, opts: { rotate?: boolean; onMove?: (e: PointerEvent) => void } = {}) {
    this.el = h("div", { class: "obj-frame", style: "display:none" });
    if (opts.onMove) {
      for (const side of ["n", "e", "s", "w"]) {
        const edge = h("span", { class: `obj-edge ${side}` });
        edge.addEventListener("pointerdown", (e) => opts.onMove!(e));
        this.el.append(edge);
      }
    }
    for (const k of HANDLES) {
      const hd = h("span", { class: `obj-handle ${k}`, "data-h": k });
      hd.addEventListener("pointerdown", (e) => onHandle(e, k));
      this.el.append(hd);
    }
    if (opts.rotate) {
      this.rotHandle = h("span", { class: "obj-handle rot", "data-h": "rot" });
      this.rotHandle.addEventListener("pointerdown", (e) => onHandle(e, "rot"));
      this.el.append(h("span", { class: "rot-stem" }), this.rotHandle);
    }
  }

  /** Show the frame around a box (or hide it when it is off screen). */
  place(box: FrameBox, visible = true) {
    this.el.style.cssText = `left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;display:${visible ? "" : "none"}${box.rot ? `;transform:rotate(${box.rot}deg)` : ""}`;
  }

  hide() {
    this.el.style.display = "none";
  }

  /** Hide the resize and rotation handles (the box stays outlined). */
  setHandles(on: boolean) {
    this.el.classList.toggle("no-handles", !on);
  }
}

/**
 * Follow the pointer from a pointer-down until it is released. `move` gets the distance from the start in
 * client pixels; `end` gets whether the pointer travelled further than a click's wobble.
 */
export function track(e: MouseEvent | PointerEvent, move: (dx: number, dy: number, ev: PointerEvent) => void, end: (moved: boolean, ev: PointerEvent) => void): void {
  const target = e.currentTarget as HTMLElement | null;
  const sx = e.clientX;
  const sy = e.clientY;
  let moved = false;
  const id = "pointerId" in e ? e.pointerId : null;
  if (id !== null) target?.setPointerCapture?.(id);
  const onMove = (ev: PointerEvent) => {
    const dx = ev.clientX - sx;
    const dy = ev.clientY - sy;
    if (!moved && Math.hypot(dx, dy) < 3) return;
    moved = true;
    move(dx, dy, ev);
  };
  const onUp = (ev: PointerEvent) => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onUp);
    if (id !== null) target?.releasePointerCapture?.(id);
    end(moved, ev);
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onUp);
}
