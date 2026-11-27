// Bookmark markers: a small ribbon in the left margin of the line where each visible bookmark starts.
//
// Like the non-printing characters (marks.ts), the markers sit on a layer of their own over the stage,
// outside the editable flow, so they take no room in the text, never reach the selection or the model,
// and printing leaves the layer out. Clicking one opens the bookmark's popover. They are redrawn in the
// next frame after a layout, an edit, a zoom or when pages are drawn or left out (`Editor.followDrawn`). Only blocks that are drawn get one.
import type { BookmarkInfo } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { isFar } from "./far";
import { modelToDom } from "./selection";

export interface BookmarkMarksHost {
  stage: HTMLElement;
  flow: HTMLElement;
  /** The pages, whose header, footer and note areas hold the bookmarks that are not in the text. */
  sheets: HTMLElement;
  /** The visible bookmarks (empty before a document is open). */
  bookmarks(): BookmarkInfo[];
  /** The body text is on show (not a header, footer or footnote being edited). */
  active(): boolean;
  /** A marker was clicked. */
  open(name: string, marker: HTMLElement): void;
}

const SVG = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 4h10a1 1 0 0 1 1 1v15l-6-4.5L6 20V5a1 1 0 0 1 1-1z"/></svg>';
const SIZE = 14;

/** Whether the paragraph marks a bookmark people can see (hidden names start with an underscore). */
const visible = (el: Element) => (el.getAttribute("data-bm") ?? "").split(" ").some((n) => n && !n.startsWith("_"));

export class BookmarkMarks {
  readonly el: HTMLElement;
  private frame = 0;
  private range = document.createRange();

  constructor(private host: BookmarkMarksHost) {
    this.el = document.createElement("div");
    this.el.className = "bm-layer";
    host.stage.append(this.el);
  }

  /** Draw again in the next frame. */
  invalidate() {
    if (this.frame) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      this.redraw();
    });
  }

  /** The marker of a bookmark, once drawn. */
  marker(name: string): HTMLElement | null {
    return Array.from(this.el.querySelectorAll<HTMLElement>(".bm-mark")).find((m) => m.dataset.name === name) ?? null;
  }

  redraw() {
    window.cancelAnimationFrame(this.frame);
    this.frame = 0;
    const { flow, stage, sheets } = this.host;
    // Most documents have no bookmarks: the paragraphs say so without asking the engine.
    const on = this.host.active();
    const holders = on ? Array.from(flow.querySelectorAll<HTMLElement>("[data-bm]")).filter((el) => visible(el) && !el.closest("[data-fold]") && !isFar(el)) : [];
    // The header, footer and note areas of every page (a header shows on each page, so its ribbon does too).
    const inAreas = on ? Array.from(sheets.querySelectorAll<HTMLElement>(".hf [data-bm], .notes [data-bm]")).filter(visible) : [];
    if (!holders.length && !inAreas.length) {
      this.el.replaceChildren();
      return;
    }
    let list: BookmarkInfo[] = [];
    try {
      list = this.host.bookmarks();
    } catch {
      list = [];
    }
    const s = stage.getBoundingClientRect();
    const sc = s.width / (stage.offsetWidth || 1) || 1;
    const byPara = new Map<string, HTMLElement>();
    for (const el of holders) byPara.set(el.dataset.p ?? "", el);
    const areaParas = new Map<string, HTMLElement[]>();
    // Paragraphs drawn read-only on a page carry their id as data-id.
    for (const el of inAreas) areaParas.set(el.dataset.id ?? "", [...(areaParas.get(el.dataset.id ?? "") ?? []), el]);
    const frag = document.createDocumentFragment();
    // Bookmarks that start on the same line stand side by side, nearest the text first.
    const used = new Map<number, number>();
    const places: { b: BookmarkInfo; para: HTMLElement; root: HTMLElement; attr: string }[] = [];
    for (const b of list.slice(0, 500)) {
      if (b.story) {
        for (const para of areaParas.get(String(b.p)) ?? []) {
          const root = para.closest<HTMLElement>(".hf, .notes");
          if (root) places.push({ b, para, root, attr: "data-id" });
        }
        continue;
      }
      const para = byPara.get(String(b.p)) ?? flow.querySelector<HTMLElement>(`[data-p="${b.p}"]`);
      // A block left out of the layout has no line to put a ribbon beside.
      if (para && !isFar(para)) places.push({ b, para, root: flow, attr: "data-p" });
    }
    for (const { b, para, root, attr } of places.slice(0, 800)) {
      const at = modelToDom(root, { p: b.p, o: b.o }, attr);
      let line: { top: number; height: number } | null = null;
      if (at) {
        this.range.setStart(at.node, at.offset);
        this.range.collapse(true);
        const r = this.range.getClientRects()[0] ?? this.range.getBoundingClientRect();
        if (r.height > 0) line = { top: r.top, height: r.height };
      }
      const pr = para.getBoundingClientRect();
      line ??= { top: pr.top, height: Math.min(pr.height, 18) };
      const y = Math.round(line.top);
      // The margin beside the paragraph; inside a table cell or a column, no further out than the cell or column.
      const slot = used.get(y) ?? 0;
      used.set(y, slot + 1);
      const container = para.closest("td, th");
      let x = (pr.left - s.left) / sc - SIZE - 3 - slot * (SIZE + 2);
      if (container) x = Math.max(x, (container.getBoundingClientRect().left - s.left) / sc + 1);
      const mark = document.createElement("button");
      mark.type = "button";
      mark.className = "bm-mark";
      mark.dataset.name = b.name;
      mark.tabIndex = -1;
      mark.title = t("Bookmark {name}", { name: b.name });
      mark.setAttribute("aria-label", t("Bookmark {name}", { name: b.name }));
      mark.innerHTML = SVG;
      mark.style.cssText = `left:${Math.max(0, x).toFixed(1)}px;top:${((line.top - s.top) / sc + (line.height / sc - SIZE) / 2).toFixed(1)}px`;
      // The click must not move the caret or the selection out of the text.
      mark.addEventListener("mousedown", (e) => e.preventDefault());
      mark.addEventListener("click", () => this.host.open(b.name, mark));
      frag.append(mark);
    }
    this.el.replaceChildren(frag);
  }
}
