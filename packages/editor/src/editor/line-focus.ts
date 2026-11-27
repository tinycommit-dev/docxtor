// Line focus (View, Immersive, Line focus): the page is dimmed except for one, three or five lines, which a person can move
// down and up with the arrow keys, the scroll wheel, a click on a line or the bar. It helps people who lose their place
// in a line of text (dyslexia, attention difficulties) and anyone reading a long page.
//
// The dimming is two veils laid over the page area, outside the text, so nothing in the document changes and the text
// stays selectable and copyable (it can't be edited while the mode is on). The lines are found from where the browser put
// the text (client rects of each paragraph's contents), so they follow the font, zoom and window. For a screen reader the
// lines in view are written into a polite live region each time they change; the veils and the bar's icons are hidden
// from it. The veils move without animation when the person prefers reduced motion.
import { t } from "@docxtor/extensions";
import type { Editor } from "./editor";
import { caretRangeAt } from "./selection";

export const LINE_COUNTS = [1, 3, 5] as const;
export type LineCount = (typeof LINE_COUNTS)[number];

interface Line {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

/** Where the focus is: a paragraph and one of its lines. */
interface Spot {
  id: number;
  line: number;
}

/** Merge the client rects of a paragraph's contents into its lines, top to bottom. */
export function linesOf(fp: HTMLElement): Line[] {
  const range = document.createRange();
  range.selectNodeContents(fp);
  const rects = Array.from(range.getClientRects()).filter((r) => r.height > 1 && r.width >= 0);
  rects.sort((a, b) => a.top - b.top || a.left - b.left);
  const lines: Line[] = [];
  for (const r of rects) {
    const last = lines[lines.length - 1];
    // A rect belongs to the line above when it sits on the same band (most of its height overlaps).
    if (last && r.top < last.bottom - Math.min(r.height, last.bottom - last.top) * 0.5) {
      last.top = Math.min(last.top, r.top);
      last.bottom = Math.max(last.bottom, r.bottom);
      last.left = Math.min(last.left, r.left);
      last.right = Math.max(last.right, r.right);
    } else lines.push({ top: r.top, bottom: r.bottom, left: r.left, right: r.right });
  }
  if (!lines.length) {
    const r = fp.getBoundingClientRect();
    if (r.height > 0) lines.push({ top: r.top, bottom: r.bottom, left: r.left, right: r.right });
  }
  return lines;
}

export class LineFocus {
  readonly el: HTMLElement;
  private above: HTMLElement;
  private below: HTMLElement;
  private bar: HTMLElement;
  private live: HTMLElement;
  private counts = new Map<number, HTMLButtonElement>();
  private ac = new AbortController();
  private frame = 0;
  private wheel = 0;
  private spot: Spot | null = null;
  /** The text that was last put in the live region. */
  private said = "";
  private count: LineCount;

  constructor(
    private ed: Editor,
    private host: HTMLElement,
    private opts: { count?: LineCount; onExit(): void; onCount?(n: LineCount): void; label?(command: string): string },
  ) {
    this.count = opts.count ?? 3;
    this.above = this.veil("lf-veil lf-above");
    this.below = this.veil("lf-veil lf-below");
    this.live = document.createElement("div");
    this.live.className = "lf-live";
    this.live.setAttribute("role", "status");
    this.live.setAttribute("aria-live", "polite");
    this.live.setAttribute("aria-atomic", "true");
    this.bar = this.makeBar();
    this.el = document.createElement("div");
    this.el.className = "lf-layer";
    this.el.append(this.above, this.below, this.live, this.bar);
    host.append(this.el);
    host.classList.add("line-focus");

    const { signal } = this.ac;
    // Keys go to the mode wherever the keyboard is on the page, except in a field or a dialog.
    document.addEventListener("keydown", (e) => this.key(e), { signal, capture: true });
    ed.canvas.addEventListener("wheel", (e) => this.onWheel(e), { signal, passive: false });
    ed.canvas.addEventListener("scroll", () => this.schedule(), { signal, passive: true });
    ed.canvas.addEventListener("click", (e) => this.onClick(e), { signal });
    window.addEventListener("resize", () => this.schedule(), { signal });
    this.reset();
    this.schedule();
  }

  get lines(): LineCount {
    return this.count;
  }

  setLines(n: LineCount) {
    this.count = n;
    this.opts.onCount?.(n);
    this.sync();
    this.schedule();
  }

  /** Put the focus on the first line in view. */
  private reset() {
    const view = this.ed.canvas.getBoundingClientRect();
    const all = this.paragraphs();
    for (const fp of all) {
      const lines = linesOf(fp);
      const i = lines.findIndex((l) => l.bottom > view.top + 8);
      if (i >= 0 && lines[i].top < view.bottom) {
        this.spot = { id: Number(fp.dataset.p), line: i };
        return;
      }
    }
    const first = all[0];
    this.spot = first ? { id: Number(first.dataset.p), line: 0 } : null;
  }

  /** Every paragraph that takes room, in reading order. */
  private paragraphs(): HTMLElement[] {
    return Array.from(this.ed.flow.querySelectorAll<HTMLElement>(".fp[data-p]")).filter((fp) => fp.offsetHeight > 0);
  }

  /** The focus moved by `n` lines (up when negative), over paragraph boundaries. */
  move(n: number) {
    if (!this.spot) return;
    const all = this.paragraphs();
    let i = all.findIndex((fp) => Number(fp.dataset.p) === this.spot!.id);
    if (i < 0) return this.reset();
    let line = this.spot.line;
    let lines = linesOf(all[i]);
    line = Math.min(line, Math.max(0, lines.length - 1));
    let left = Math.abs(n);
    const step = n < 0 ? -1 : 1;
    while (left > 0) {
      const next = line + step;
      if (next >= 0 && next < lines.length) {
        line = next;
        left--;
        continue;
      }
      const j = i + step;
      if (j < 0 || j >= all.length) break;
      i = j;
      lines = linesOf(all[i]);
      line = step > 0 ? 0 : Math.max(0, lines.length - 1);
      left--;
    }
    this.spot = { id: Number(all[i].dataset.p), line };
    this.reveal();
    this.schedule();
  }

  /** Go to the first or the last line of the document. */
  edge(last: boolean) {
    const all = this.paragraphs();
    const fp = last ? all[all.length - 1] : all[0];
    if (!fp) return;
    this.spot = { id: Number(fp.dataset.p), line: last ? Math.max(0, linesOf(fp).length - 1) : 0 };
    this.reveal();
    this.schedule();
  }

  /** Scroll so the window of lines is well inside the view. */
  private reveal() {
    const w = this.window();
    if (!w) return;
    const view = this.ed.canvas.getBoundingClientRect();
    const room = Math.min(80, view.height / 5);
    if (w.top < view.top + room || w.bottom > view.bottom - room) {
      const mid = (w.top + w.bottom) / 2;
      this.ed.canvas.scrollTop += mid - (view.top + view.height * 0.4);
    }
  }

  /** The lines in the window: the one with the focus and as many before and after it as the window has room for. */
  private visible(): Line[] {
    if (!this.spot) return [];
    const all = this.paragraphs();
    const at = all.findIndex((fp) => Number(fp.dataset.p) === this.spot!.id);
    if (at < 0) return [];
    const side = (this.count - 1) / 2;
    const here = linesOf(all[at]);
    if (!here.length) return [];
    const line = Math.min(this.spot.line, here.length - 1);
    const before = here.slice(0, line);
    for (let i = at - 1; i >= 0 && before.length < side; i--) before.unshift(...linesOf(all[i]));
    const after = here.slice(line + 1);
    for (let i = at + 1; i < all.length && after.length < side; i++) after.push(...linesOf(all[i]));
    return [...(side ? before.slice(-side) : []), here[line], ...after.slice(0, side)];
  }

  private window(): { top: number; bottom: number } | null {
    const lines = this.visible();
    if (!lines.length) return null;
    return { top: Math.min(...lines.map((l) => l.top)), bottom: Math.max(...lines.map((l) => l.bottom)) };
  }

  private schedule() {
    if (this.frame) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      this.place();
    });
  }

  private place() {
    const w = this.window();
    const box = this.host.getBoundingClientRect();
    const view = this.ed.canvas.getBoundingClientRect();
    const top = view.top - box.top;
    const height = view.height;
    if (!w) {
      this.above.style.cssText = `top:${top}px;height:${height}px`;
      this.below.style.cssText = "display:none";
      return;
    }
    const y0 = Math.max(0, Math.min(height, w.top - view.top));
    const y1 = Math.max(0, Math.min(height, w.bottom - view.top));
    this.above.style.cssText = `top:${top}px;height:${y0}px`;
    this.below.style.cssText = `top:${top + y1}px;height:${Math.max(0, height - y1)}px`;
    this.announce();
  }

  /** Write the words of the lines in view into the live region. */
  private announce() {
    const text = this.visible()
      .map((l) => this.textOf(l))
      .filter(Boolean)
      .join(" ");
    if (!text || text === this.said) return;
    this.said = text;
    this.live.textContent = text;
  }

  /** The words on a line, read back from where the browser put them. */
  private textOf(line: Line): string {
    const y = (line.top + line.bottom) / 2;
    const a = caretRangeAt(line.left + 1, y);
    const b = caretRangeAt(line.right - 1, y);
    if (a && b && this.ed.flow.contains(a.startContainer) && this.ed.flow.contains(b.startContainer)) {
      const r = document.createRange();
      try {
        r.setStart(a.startContainer, a.startOffset);
        r.setEnd(b.startContainer, b.startOffset);
        const s = r.toString().replace(/\s+/g, " ").trim();
        if (s) return s;
      } catch {
        /* fall through to the paragraph */
      }
    }
    const el = document.elementFromPoint(line.left + 1, y)?.closest<HTMLElement>(".fp");
    return el?.textContent?.replace(/\s+/g, " ").trim() ?? "";
  }

  // -- input ---------------------------------------------------------------------

  private key(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    // Fields, dialogs and menus keep their keys; so does the bar's own buttons for Enter and Space.
    if (target?.closest("input, textarea, select, [role=dialog], [role=menu], .pop, .scrim, .qp")) return;
    if (target?.isContentEditable && !this.ed.flow.contains(target)) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    switch (e.key) {
      case "ArrowDown":
        this.move(1);
        break;
      case "ArrowUp":
        this.move(-1);
        break;
      case "PageDown":
        this.move(this.count);
        break;
      case "PageUp":
        this.move(-this.count);
        break;
      case "Home":
        this.edge(false);
        break;
      case "End":
        this.edge(true);
        break;
      case "Escape":
        this.opts.onExit();
        break;
      default:
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  }

  private onWheel(e: WheelEvent) {
    if (e.ctrlKey) return;
    e.preventDefault();
    const unit = e.deltaMode === 1 ? 1 : e.deltaMode === 2 ? 10 : 40;
    this.wheel += e.deltaY / unit;
    const n = Math.trunc(this.wheel);
    if (!n) return;
    this.wheel -= n;
    this.move(n);
  }

  private onClick(e: MouseEvent) {
    const fp = (e.target as HTMLElement).closest<HTMLElement>(".fp[data-p]");
    if (!fp) return;
    const lines = linesOf(fp);
    let line = lines.findIndex((l) => e.clientY >= l.top && e.clientY <= l.bottom);
    if (line < 0) line = lines.length ? (e.clientY < lines[0].top ? 0 : lines.length - 1) : 0;
    this.spot = { id: Number(fp.dataset.p), line };
    this.schedule();
  }

  // -- the bar -------------------------------------------------------------------

  private veil(cls: string): HTMLElement {
    const v = document.createElement("div");
    v.className = cls;
    v.setAttribute("aria-hidden", "true");
    return v;
  }

  private makeBar(): HTMLElement {
    const bar = document.createElement("div");
    bar.className = "lf-bar";
    bar.setAttribute("role", "toolbar");
    bar.setAttribute("aria-label", t("Line focus"));
    const button = (label: string, text: string, run: () => void, extra = "") => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = `tb lf-btn ${extra}`.trim();
      b.title = label;
      b.setAttribute("aria-label", label);
      b.textContent = text;
      b.addEventListener("click", run);
      return b;
    };
    const up = button(t("Move up"), "", () => this.move(-1), "lf-up");
    const down = button(t("Move down"), "", () => this.move(1), "lf-down");
    up.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 19V5M6 11l6-6 6 6"/></svg>';
    down.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 5v14M6 13l6 6 6-6"/></svg>';
    const group = document.createElement("div");
    group.className = "lf-counts";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", t("Lines in view"));
    for (const n of LINE_COUNTS) {
      const b = button(n === 1 ? t("Show one line") : n === 3 ? t("Show three lines") : t("Show five lines"), String(n), () => this.setLines(n), "lf-count");
      b.dataset.lines = String(n);
      this.counts.set(n, b);
      group.append(b);
    }
    const close = button(t("Close line focus"), t("Close"), () => this.opts.onExit(), "lf-close");
    bar.append(up, down, group, close);
    // A click on the bar must not take the selection out of the text.
    bar.addEventListener("mousedown", (e) => e.preventDefault());
    this.sync();
    return bar;
  }

  private sync() {
    for (const [n, b] of this.counts) {
      b.classList.toggle("on", n === this.count);
      b.setAttribute("aria-pressed", String(n === this.count));
    }
  }

  destroy() {
    this.ac.abort();
    window.cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.host.classList.remove("line-focus");
    this.el.remove();
  }
}
