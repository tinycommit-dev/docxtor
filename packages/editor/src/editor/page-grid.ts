// Several pages side by side (Zoom, Several pages): a read-only grid of whole pages drawn from the laid-out document
// (see snapshot.ts). The pages are made as they scroll into view, so a long document costs only what is on screen. A
// click on a page goes back to the text, at that page. While the grid is on the live pages stay laid out but hidden,
// so the layout and the page count carry on as usual.
import { t } from "@docxtor/extensions";
import type { Editor } from "./editor";
import { editorSnapshots, snapshotPage } from "./snapshot";

const GAP = 16;

export class PageGrid {
  readonly el: HTMLElement;
  private observer: IntersectionObserver | null = null;
  private across = 2;
  private scale = 1;
  private shown = false;
  private release: (() => void) | null = null;
  private timer = 0;

  constructor(private ed: Editor) {
    this.el = document.createElement("div");
    this.el.className = "page-grid";
    this.el.hidden = true;
    this.el.setAttribute("role", "list");
    this.el.setAttribute("aria-label", t("Pages side by side"));
    this.el.addEventListener("click", (e) => {
      const cell = (e.target as HTMLElement).closest<HTMLElement>(".pg-cell");
      if (cell) this.open(Number(cell.dataset.page) - 1);
    });
    this.el.addEventListener("keydown", (e) => {
      const cell = (e.target as HTMLElement).closest<HTMLElement>(".pg-cell");
      if (!cell || e.altKey || e.ctrlKey || e.metaKey) return;
      const k = Number(cell.dataset.page) - 1;
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        this.open(k);
        return;
      }
      // The arrows go from page to page as the grid shows them, Home and End to the first and last.
      const last = this.ed.pages - 1;
      const to = { ArrowLeft: k - 1, ArrowRight: k + 1, ArrowUp: k - this.across, ArrowDown: k + this.across, Home: 0, End: last }[e.key];
      if (to === undefined) return;
      e.preventDefault();
      this.focus(Math.max(0, Math.min(last, to)));
    });
  }

  get on(): boolean {
    return this.shown;
  }

  /** The page the keyboard was last on in the grid (0-based), or, when the grid opened, the caret's page (the text had the keyboard) or the page at the top of the view. */
  private page = 0;

  /**
   * The keyboard on a page of the grid (`k`, 0-based). The text is hidden while the grid shows, so the keyboard would otherwise be on the
   * page body, where no key does anything.
   */
  focus(k = this.page) {
    const cell = this.el.querySelector<HTMLElement>(`.pg-cell[data-page="${k + 1}"]`) ?? this.el.querySelector<HTMLElement>(".pg-cell");
    if (!cell) return;
    this.page = Number(cell.dataset.page) - 1;
    cell.focus({ preventScroll: true });
    cell.scrollIntoView({ block: "nearest" });
  }

  /** Show the grid: `across` pages to a row, drawn at `scale`. */
  show(across: number, scale: number) {
    const a = document.activeElement;
    // The keyboard in the text (or nowhere, or on a page of the grid that is drawn again) goes to a page of the grid.
    const take = !a || a === document.body || this.ed.canvas.contains(a);
    if (!this.shown) {
      // The caret's page when the text has the keyboard (where the person is working, and known from the page map); otherwise the page at
      // the top of the view (which depends on where the scroll has settled).
      const caret = this.ed.hasFocus() ? this.ed.caretPage() : undefined;
      this.page = caret ?? Math.max(0, this.ed.currentPage() - 1);
      this.shown = true;
      this.release = this.ed.lock("page grid");
      this.ed.canvas.classList.add("grid-on");
      this.el.hidden = false;
    }
    this.across = across;
    this.scale = scale;
    this.build();
    if (take) this.focus();
  }

  hide() {
    if (!this.shown) return;
    // The keyboard on a page of the grid goes back to the text when the grid goes (another view, a zoom), not to the page body.
    const had = this.el.contains(document.activeElement);
    this.shown = false;
    this.observer?.disconnect();
    this.observer = null;
    window.clearTimeout(this.timer);
    this.el.hidden = true;
    this.el.replaceChildren();
    this.ed.canvas.classList.remove("grid-on");
    this.release?.();
    this.release = null;
    if (had) this.ed.focus();
  }

  /** The layout changed: draw the pages again (soon, so a burst of changes costs one rebuild). */
  refresh() {
    if (!this.shown) return;
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.shown && this.build(), 150);
  }

  private build() {
    const ed = this.ed;
    this.observer?.disconnect();
    const pages = ed.pages;
    const cells: HTMLElement[] = [];
    this.el.style.gap = `${GAP}px`;
    this.el.style.setProperty("--across", String(this.across));
    const had = this.el.contains(document.activeElement);
    for (let k = 0; k < pages; k++) {
      const b = ed.pageBox(k);
      const cell = document.createElement("div");
      cell.className = "pg-cell";
      cell.dataset.page = String(k + 1);
      cell.setAttribute("role", "listitem");
      cell.tabIndex = 0;
      cell.setAttribute("aria-label", t("Page {page}", { page: k + 1 }));
      cell.style.width = `${Math.round(b.w * this.scale)}px`;
      cell.style.height = `${Math.round(b.h * this.scale)}px`;
      cells.push(cell);
    }
    this.el.replaceChildren(...cells);
    const src = editorSnapshots(ed);
    this.observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const cell = entry.target as HTMLElement;
          if (entry.isIntersecting && !cell.firstChild) {
            const k = Number(cell.dataset.page) - 1;
            const snap = snapshotPage(src, k);
            snap.style.transform = `scale(${this.scale})`;
            snap.style.transformOrigin = "0 0";
            cell.append(snap);
          } else if (!entry.isIntersecting && cell.firstChild) cell.replaceChildren();
        }
      },
      { root: ed.canvas, rootMargin: "600px 0px" },
    );
    for (const c of cells) this.observer.observe(c);
    // Drawn again (the layout changed): the keyboard stays on its page.
    if (had) this.focus();
  }

  /** Back to the text, at a page. */
  private open(k: number) {
    const ed = this.ed;
    ed.zoomer.setMode("width");
    const first = ed.firstParagraphOnPage(k);
    if (first !== null) {
      ed.scrollToParagraph(first);
      // The keyboard goes with it, at the start of the page (the page it was on is gone).
      ed.select({ p: first, o: 0 });
    }
    ed.focus();
  }
}
