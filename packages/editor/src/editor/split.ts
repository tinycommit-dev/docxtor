// Split window (View, Split): two panes of the same document, one above the other with a divider to drag. The first
// pane is the editor the page was built with; the second is another `Editor` bound to the same engine document. Every
// change either pane makes is a change to the engine, and the pane that made it hands the changed blocks to the other
// (`Editor.peers`), so both always show the same text and undo is one history for both. What stays apart is what a
// person sees: each pane has its own caret and selection, scroll position, zoom and view (print layout in one,
// Outline or Draft in the other).
//
// The second pane leaves headers, footers and notes to the first (they are edited on the pages of the first pane), and
// has no spelling underlines of its own. Taking the split away destroys the second editor, which releases its
// listeners, observers and elements.
import type { Engine } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { Editor } from "./editor";

/** What the split asks of the editor component that owns it. */
export interface SplitHost {
  /** The page area the panes go in. */
  surface: HTMLElement;
  primary: Editor;
  engine(): Engine | null;
  /** The panes are about to move (or have moved): the view of the first pane leaves its element and comes back to it. */
  leaveView(e: Editor): void;
  enterView(e: Editor, id: string): void;
  viewOf(e: Editor): string;
  /** Give the second pane its hooks (what the editor component wants to hear from it). */
  hooksFor(second: Editor): Editor["hooks"];
  /** The pane with the keyboard changed. */
  activate(e: Editor): void;
  /** The second pane is going away: run what its view undid. */
  closing(second: Editor): void;
  /** The split came or went. */
  changed(on: boolean): void;
}

const MIN_PANE = 120;

export class SplitWindow {
  private root: HTMLElement | null = null;
  private firstPane: HTMLElement | null = null;
  private secondPane: HTMLElement | null = null;
  private divider: HTMLElement | null = null;
  private editor: Editor | null = null;
  private ac: AbortController | null = null;
  /** The first pane's share of the height, from 0 to 1. */
  private ratio = 0.5;

  constructor(private host: SplitHost) {}

  get on(): boolean {
    return !!this.editor;
  }

  /** The second pane's editor, or null when the window is not split. */
  get second(): Editor | null {
    return this.editor;
  }

  /** Every pane, the first one first. */
  panes(): Editor[] {
    return this.editor ? [this.host.primary, this.editor] : [this.host.primary];
  }

  /** The element that holds a pane's editor and carries its view's classes. */
  hostOf(e: Editor): HTMLElement {
    if (e === this.editor && this.secondPane) return this.secondPane;
    return this.firstPane ?? this.host.surface;
  }

  /** The first pane's share of the height. */
  get share(): number {
    return this.ratio;
  }

  /** Split the window in two at `ratio` of the height (half by default). Returns false when there is no document. */
  open(ratio = 0.5): boolean {
    const engine = this.host.engine();
    const primary = this.host.primary;
    if (this.editor || !engine) return false;
    const view = this.host.viewOf(primary);
    this.host.leaveView(primary);
    const had = primary.hasFocus();
    const scroll = primary.canvas.scrollTop;
    const position = primary.scrollPosition();

    const root = document.createElement("div");
    root.className = "split-root";
    const first = document.createElement("div");
    first.className = "split-pane";
    const divider = document.createElement("div");
    divider.className = "split-divider";
    divider.setAttribute("role", "separator");
    divider.setAttribute("aria-orientation", "horizontal");
    divider.setAttribute("aria-label", t("Resize the panes"));
    divider.tabIndex = 0;
    const secondPane = document.createElement("div");
    secondPane.className = "split-pane";
    root.append(first, divider, secondPane);
    this.host.surface.insertBefore(root, primary.canvas);
    first.append(primary.canvas);
    this.root = root;
    this.firstPane = first;
    this.secondPane = secondPane;
    this.divider = divider;
    this.setRatio(ratio);

    const second = new Editor(secondPane);
    second.noStories = true;
    // The first pane owns the file, so it is the one that tells the engine which tab to save.
    second.savesOpenTab = false;
    second.load(engine);
    primary.copySettingsTo(second);
    second.hooks = this.host.hooksFor(second);
    primary.peers.add(second);
    second.peers.add(primary);
    this.editor = second;

    const ac = new AbortController();
    this.ac = ac;
    const { signal } = ac;
    first.addEventListener("focusin", () => this.host.activate(primary), { signal });
    secondPane.addEventListener("focusin", () => this.host.activate(second), { signal });
    first.addEventListener("pointerdown", () => this.host.activate(primary), { signal, capture: true });
    secondPane.addEventListener("pointerdown", () => this.host.activate(second), { signal, capture: true });
    divider.addEventListener("pointerdown", (e) => this.drag(e), { signal });
    divider.addEventListener("keydown", (e) => this.key(e), { signal });

    primary.canvas.scrollTop = scroll;
    second.scrollToPosition(position);
    this.host.enterView(primary, view);
    if (had) primary.focus();
    this.host.changed(true);
    return true;
  }

  /** Take the split away: the second pane is destroyed and the first one has the window again. */
  close() {
    const second = this.editor;
    if (!second || !this.root || !this.firstPane) return;
    const primary = this.host.primary;
    this.host.closing(second);
    const view = this.host.viewOf(primary);
    this.host.leaveView(primary);
    const scroll = primary.canvas.scrollTop;
    const had = primary.hasFocus() || second.hasFocus();
    this.host.activate(primary);
    this.ac?.abort();
    this.ac = null;
    primary.peers.delete(second);
    second.destroy();
    this.host.surface.insertBefore(primary.canvas, this.root);
    this.root.remove();
    this.root = this.firstPane = this.secondPane = this.divider = null;
    this.editor = null;
    primary.canvas.scrollTop = scroll;
    this.host.enterView(primary, view);
    if (had) primary.focus();
    this.host.changed(false);
  }

  private setRatio(r: number) {
    const total = this.root?.getBoundingClientRect().height || 0;
    const min = total ? Math.min(0.45, MIN_PANE / total) : 0.1;
    this.ratio = Math.max(min, Math.min(1 - min, r));
    if (!this.firstPane || !this.secondPane) return;
    this.firstPane.style.flex = `${this.ratio} 1 0`;
    this.secondPane.style.flex = `${1 - this.ratio} 1 0`;
    this.divider?.setAttribute("aria-valuenow", String(Math.round(this.ratio * 100)));
    this.divider?.setAttribute("aria-valuemin", "10");
    this.divider?.setAttribute("aria-valuemax", "90");
  }

  private drag(e: PointerEvent) {
    const root = this.root;
    const divider = this.divider;
    if (!root || !divider || e.button !== 0) return;
    e.preventDefault();
    divider.setPointerCapture(e.pointerId);
    divider.classList.add("dragging");
    const move = (m: PointerEvent) => {
      const box = root.getBoundingClientRect();
      const bar = divider.offsetHeight;
      this.setRatio((m.clientY - box.top - bar / 2) / Math.max(1, box.height - bar));
    };
    const stop = () => {
      divider.classList.remove("dragging");
      divider.removeEventListener("pointermove", move);
      divider.removeEventListener("pointerup", stop);
      divider.removeEventListener("pointercancel", stop);
    };
    divider.addEventListener("pointermove", move);
    divider.addEventListener("pointerup", stop);
    divider.addEventListener("pointercancel", stop);
  }

  private key(e: KeyboardEvent) {
    const step = e.shiftKey ? 0.1 : 0.03;
    if (e.key === "ArrowUp") this.setRatio(this.ratio - step);
    else if (e.key === "ArrowDown") this.setRatio(this.ratio + step);
    else if (e.key === "Home") this.setRatio(0);
    else if (e.key === "End") this.setRatio(1);
    else return;
    e.preventDefault();
  }
}
