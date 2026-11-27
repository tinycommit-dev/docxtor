// The folding marks: a small arrow in the left margin of each heading that has text under it, to fold or unfold
// it (it shows while the pointer is over the heading, and always while the heading is folded). In the Outline
// view every block gets a mark: the arrow for a heading with text under it, a dash for a heading without, and a
// dot for body text. Like the bookmark ribbons (bookmarks.ts) they sit on a layer of their own over the stage, outside the editable
// flow, so they take no room, never reach the selection and printing leaves them out.
import { t } from "@docxtor/extensions";
import { isFar } from "./far";
import type { Folds } from "./folds";

export interface FoldMarksHost {
  stage: HTMLElement;
  flow: HTMLElement;
  folds: Folds;
  /** The body text is on show (not a header, footer or note being edited). */
  active(): boolean;
  /** An arrow was used on a heading. */
  toggle(id: number): void;
}

const SIZE = 16;

const CHEVRON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.25" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m8 10 4 4 4-4"/></svg>';

export class FoldMarks {
  readonly el: HTMLElement;
  private frame = 0;
  private range = document.createRange();
  private hot = 0;

  constructor(private host: FoldMarksHost) {
    this.el = document.createElement("div");
    this.el.className = "fm-layer";
    host.stage.append(this.el);
    host.stage.addEventListener("pointermove", (e) => this.hover(e));
    host.stage.addEventListener("pointerleave", () => this.setHot(0));
  }

  invalidate() {
    if (this.frame) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      this.redraw();
    });
  }

  /** The arrow of the heading under the pointer shows. */
  private hover(e: PointerEvent) {
    const target = e.target as HTMLElement | null;
    const el = target?.closest?.<HTMLElement>(".flow > [data-ol]");
    this.setHot(el ? Number(el.dataset.id) : target?.closest?.(".fm-btn") ? this.hot : 0);
  }

  private setHot(id: number) {
    if (id === this.hot) return;
    this.hot = id;
    for (const b of Array.from(this.el.querySelectorAll<HTMLElement>(".fm-btn"))) b.classList.toggle("hot", Number(b.dataset.id) === id);
  }

  redraw() {
    window.cancelAnimationFrame(this.frame);
    this.frame = 0;
    const { flow, stage, folds } = this.host;
    if (!this.host.active()) {
      this.el.replaceChildren();
      return;
    }
    const s = stage.getBoundingClientRect();
    const sc = s.width / (stage.offsetWidth || 1) || 1;
    const outline = folds.outline;
    const frag = document.createDocumentFragment();
    let drawn = 0;
    // Without the Outline view only headings get a mark: the browser finds them faster than a walk over every block of a long text.
    const blocks = outline ? (Array.from(flow.children) as HTMLElement[]) : Array.from(flow.querySelectorAll<HTMLElement>(":scope > [data-ol]"));
    for (const el of blocks) {
      if (el.dataset.fold || el.classList.contains("cols") || isFar(el)) continue;
      const level = folds.levelOf(el);
      if (!level && !outline) continue;
      const content = level > 0 && folds.hasContent(el);
      if (!outline && !content) continue;
      if (++drawn > 1500) break;
      const id = Number(el.dataset.id);
      const pr = el.getBoundingClientRect();
      if (!pr.width && !pr.height) continue;
      let top = pr.top;
      let height = Math.min(pr.height, 20);
      const text = this.firstText(el);
      if (text) {
        this.range.setStart(text, 0);
        this.range.setEnd(text, Math.min(1, text.length));
        const r = this.range.getClientRects()[0];
        if (r && r.height > 0) {
          top = r.top;
          height = r.height;
        }
      }
      const x = Math.max(2, (pr.left - s.left) / sc - SIZE - 8);
      const y = (top - s.top) / sc + (height / sc - SIZE) / 2;
      const at = `left:${x.toFixed(1)}px;top:${y.toFixed(1)}px`;
      if (content) {
        const folded = folds.isFolded(id);
        const b = document.createElement("button");
        b.type = "button";
        b.className = `fm-btn${folded ? " folded on" : ""}${id === this.hot ? " hot" : ""}`;
        b.dataset.id = String(id);
        b.tabIndex = -1;
        const label = folded ? t("Expand heading") : t("Collapse heading");
        b.title = label;
        b.setAttribute("aria-label", label);
        b.setAttribute("aria-expanded", String(!folded));
        b.innerHTML = CHEVRON;
        b.style.cssText = at;
        // The click must not move the caret or the selection out of the text.
        b.addEventListener("mousedown", (e) => e.preventDefault());
        b.addEventListener("click", () => this.host.toggle(id));
        frag.append(b);
      } else if (outline) {
        const m = document.createElement("span");
        m.className = `fm-dot ${level ? "dash" : "dot"}`;
        m.setAttribute("aria-hidden", "true");
        m.style.cssText = at;
        frag.append(m);
      }
    }
    this.el.replaceChildren(frag);
  }

  /** The first text node of a block (where its first line is). */
  private firstText(el: HTMLElement): Text | null {
    const w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, { acceptNode: (n) => (n.textContent && !(n.parentElement?.closest(".lbl, .pgsp")) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT) });
    return w.nextNode() as Text | null;
  }
}
