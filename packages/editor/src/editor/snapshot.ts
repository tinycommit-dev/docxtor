// A picture of one page, made from the laid-out document: the sheet (with its header, footer and notes) and the
// blocks that fall on it, copied to their places and clipped to the sheet. It is what page thumbnails (the
// Navigation pane) and the grid of several pages show. The copy is inert: it carries no ids, can't be edited and
// is never part of the document.
import type { Editor } from "./editor";
import { stripFar } from "./far";

/** What a page's picture is made from. */
export interface SnapshotSource {
  /** The flow whose children are the blocks. */
  flow: HTMLElement;
  /** The sheet elements, one per page. */
  sheets: HTMLElement;
  /** Where page k sits, and how big it is. */
  pageBox(k: number): { top: number; left: number; w: number; h: number };
  /** The blocks that may reach into a band of the stage, from the page map (all of the flow's when it is not given). */
  blocksBetween?(y0: number, y1: number): HTMLElement[] | undefined;
  /** Run something that reads these pages as drawn (see `Editor.withDrawn`). */
  withDrawn?<T>(fn: () => T, pages: { from: number; to: number }): T;
}

/** Make an element inert: no ids, no editing, no focus. */
function inert(root: HTMLElement) {
  for (const el of [root, ...Array.from(root.querySelectorAll<HTMLElement>("*"))]) {
    el.removeAttribute("data-p");
    el.removeAttribute("id");
    el.removeAttribute("contenteditable");
    el.removeAttribute("tabindex");
    el.removeAttribute("draggable");
  }
}

/** A copy of page `k`, `w` by `h` pixels at 100%, to be scaled by the caller. */
export function snapshotPage(src: SnapshotSource, k: number): HTMLElement {
  const make = () => pageSnapshot(src, k);
  return src.withDrawn ? src.withDrawn(make, { from: k, to: k }) : make();
}

function pageSnapshot(src: SnapshotSource, k: number): HTMLElement {
  const box = src.pageBox(k);
  const snap = document.createElement("div");
  snap.className = "pg-snap";
  snap.setAttribute("aria-hidden", "true");
  snap.style.width = `${box.w}px`;
  snap.style.height = `${box.h}px`;
  const sheet = src.sheets.children[k] as HTMLElement | undefined;
  if (sheet) {
    const copy = sheet.cloneNode(true) as HTMLElement;
    copy.style.top = "0px";
    copy.style.left = "0px";
    inert(copy);
    snap.append(copy);
  }
  const flow = document.createElement("div");
  flow.className = "flow pg-flow";
  // The page map says which blocks are on this page; without it every block is looked at.
  const near = src.blocksBetween?.(box.top, box.top + box.h) ?? (Array.from(src.flow.children) as HTMLElement[]);
  for (const kid of near) {
    if (kid.dataset.fold) continue;
    const top = kid.offsetTop;
    const h = kid.offsetHeight;
    if (top + h <= box.top || top >= box.top + box.h) continue;
    const copy = kid.cloneNode(true) as HTMLElement;
    copy.style.margin = "0";
    copy.style.boxSizing = "border-box";
    copy.style.position = "absolute";
    copy.style.left = `${kid.offsetLeft - box.left}px`;
    copy.style.top = `${top - box.top}px`;
    copy.style.width = `${kid.offsetWidth}px`;
    stripFar(copy);
    inert(copy);
    flow.append(copy);
  }
  snap.append(flow);
  return snap;
}

/** The editor as a source of page pictures. */
export const editorSnapshots = (ed: Editor): SnapshotSource => ({
  flow: ed.flow,
  sheets: ed.sheetsElement,
  pageBox: (k) => ed.pageBox(k),
  blocksBetween: (y0, y1) => ed.blocksBetween(y0, y1),
  withDrawn: (fn, pages) => ed.withDrawn(fn, pages),
});
