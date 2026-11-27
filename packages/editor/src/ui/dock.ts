// A side panel over the right edge of the pages, for tools that stay open while you work (Format shape, Selection pane).
// It is not a dialog: the page stays live behind it. One panel per id; opening it again brings the same one back.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, overlayLayer } from "./dom";

export interface Dock {
  /** The panel's element (in the overlay layer). */
  readonly el: HTMLElement;
  /** Where the content goes. */
  readonly body: HTMLElement;
  setTitle(title: string): void;
  close(): void;
}

const open = new Map<string, Dock>();

/** The open panel with this id, if any. */
export function dockOf(id: string): Dock | undefined {
  return open.get(id);
}

/**
 * Open the panel `id` over the right edge of the editor's canvas and return it. `onClose` runs once when it closes
 * (the close button, Escape inside it, or the editor going away).
 */
export function openDock(ed: Editor, id: string, title: string, onClose?: () => void): Dock {
  const have = open.get(id);
  if (have) return have;
  const titleEl = h("h2", { class: "dock-title" }, title);
  const closeBtn = h("button", { type: "button", class: "tb dock-close", title: t("Close"), "aria-label": t("Close") }, "×");
  const body = h("div", { class: "dock-body" });
  const el = h("section", { class: "dock", role: "region", "aria-label": title, "data-dock": id }, h("header", { class: "dock-head" }, titleEl, closeBtn), body);
  const place = () => {
    const c = ed.canvas.getBoundingClientRect();
    el.style.top = `${Math.max(8, c.top + 8)}px`;
    el.style.maxHeight = `${Math.max(160, Math.min(window.innerHeight, c.bottom) - Math.max(8, c.top + 8) - 8)}px`;
    el.style.right = `${Math.max(8, window.innerWidth - c.right + 8)}px`;
  };
  let closed = false;
  let forget = () => {};
  const dock: Dock = {
    el,
    body,
    setTitle(s) {
      titleEl.textContent = s;
      el.setAttribute("aria-label", s);
    },
    close() {
      if (closed) return;
      closed = true;
      open.delete(id);
      forget();
      window.removeEventListener("resize", place);
      el.remove();
      onClose?.();
    },
  };
  closeBtn.addEventListener("click", () => dock.close());
  el.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      dock.close();
      ed.focus();
    }
  });
  overlayLayer().append(el);
  // The editor going away closes the panel; the panel closing first lets that go, so opening and closing it again and again adds nothing.
  forget = ed.life.defer(() => dock.close());
  window.addEventListener("resize", place);
  place();
  open.set(id, dock);
  return dock;
}
