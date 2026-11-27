// Editing on a tablet. The text is the browser's own editable text, so tapping, the selection handles and the on-screen
// keyboard work as they do anywhere; this adds what a page of a document needs on top: the caret kept above the
// keyboard when it opens, and a long press that opens Docxtor's menu where the browser sends no contextmenu event of its
// own (Safari on an iPad). Two-finger pinch zooms the pages (zoom.ts).
import type { Editor } from "./editor";

/** How long a finger stays still before it is a long press. */
const LONG_PRESS = 550;
/** How far a finger may wander and still be pressing. */
const SLOP = 10;

export function touchEditing(ed: Editor, signal: AbortSignal) {
  // The on-screen keyboard takes the bottom of the window: the caret goes back into view above it. Only then: after a
  // touch, with a caret (not a selection), when the visible part got shorter at the same width. Resizing the window or
  // zooming the browser with a mouse and keyboard leaves the view where the person scrolled it.
  const vv = window.visualViewport;
  let touched = false;
  let width = vv?.width ?? 0;
  let height = vv?.height ?? 0;
  window.addEventListener("pointerdown", (e) => (touched ||= e.pointerType === "touch"), { signal, capture: true, passive: true });
  vv?.addEventListener(
    "resize",
    () => {
      const keyboard = touched && Math.abs(vv.width - width) < 1 && vv.height < height && vv.height < window.innerHeight;
      width = vv.width;
      height = vv.height;
      if (!keyboard || !ed.hasFocus()) return;
      const s = window.getSelection();
      if (!s?.rangeCount || !s.isCollapsed) return;
      const r = s.getRangeAt(0).getBoundingClientRect();
      const bottom = vv.offsetTop + vv.height - 24;
      if (r.height && r.bottom > bottom) ed.canvas.scrollTop += r.bottom - bottom;
    },
    { signal },
  );

  let timer = 0;
  let start: { x: number; y: number; id: number } | null = null;
  let menuCame = false;
  const cancel = () => {
    window.clearTimeout(timer);
    start = null;
  };
  ed.canvas.addEventListener("contextmenu", () => (menuCame = true), { signal, capture: true });
  ed.canvas.addEventListener(
    "pointerdown",
    (e) => {
      if (e.pointerType !== "touch") return;
      cancel();
      menuCame = false;
      start = { x: e.clientX, y: e.clientY, id: e.pointerId };
      timer = window.setTimeout(() => {
        const at = start;
        start = null;
        // The browser opened its own menu (Chrome on Android): nothing to do.
        window.setTimeout(() => {
          if (menuCame || !at) return;
          const target = document.elementFromPoint(at.x, at.y);
          target?.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: at.x, clientY: at.y, button: 2 }));
        }, 150);
      }, LONG_PRESS);
    },
    { signal },
  );
  ed.canvas.addEventListener(
    "pointermove",
    (e) => {
      if (start && e.pointerId === start.id && Math.hypot(e.clientX - start.x, e.clientY - start.y) > SLOP) cancel();
    },
    { signal },
  );
  for (const type of ["pointerup", "pointercancel"] as const) ed.canvas.addEventListener(type, cancel, { signal });
}
