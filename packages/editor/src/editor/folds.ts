// Folding: collapsible headings in the text and the Outline view's levels. A heading can be folded: the blocks
// after it, down to the next heading of its level or higher, take no room (`data-fold` on the block; the paginator
// skips them). The Outline view adds a level limit (show headings down to level n and no body text), and marks
// each block with the level it is shown at. Folding is a way of looking at the document, not part of it: it isn't
// saved, isn't undone, and ends when another document opens.
import { outlineLevel } from "./outline-level";
import { nextBlock, topBlocks } from "./far";

export { outlineLevel };

export interface FoldsHost {
  /** The flow whose children are the document's blocks. */
  flow: HTMLElement;
}

export class Folds {
  /** Headings (block ids) that are folded. */
  readonly folded = new Set<number>();
  /** The Outline view is on: blocks are marked with the level they show at, and `levels` and `firstLine` count. */
  outline = false;
  /** Outline view: show headings down to this level and hide body text; 0 shows everything. */
  levels = 0;
  /** Outline view: body text shows its first line only. */
  firstLine = false;
  /** Nothing was folded or marked at the last `apply`, so there is nothing to clear. */
  private clean = true;

  constructor(private host: FoldsHost) {}

  /** Anything to apply (a fold, a level limit, the outline's marks) or to clear from the last time. */
  get active(): boolean {
    return this.folded.size > 0 || this.outline || !this.clean;
  }

  /** The level a flow block shows at: its heading level, or 0. */
  levelOf(el: HTMLElement): number {
    return Number(el.dataset.ol) || 0;
  }

  private blocks(): HTMLElement[] {
    return topBlocks(this.host.flow);
  }

  private idOf(el: HTMLElement): number {
    return Number(el.dataset.id);
  }

  /** The blocks under a heading (down to the next heading of its level or higher). */
  under(heading: HTMLElement): HTMLElement[] {
    const level = this.levelOf(heading);
    const out: HTMLElement[] = [];
    if (!level) return out;
    for (let el = nextBlock(heading); el; el = nextBlock(el)) {
      const l = this.levelOf(el);
      if (l && l <= level) break;
      out.push(el);
    }
    return out;
  }

  /** Does a heading have anything under it to fold? */
  hasContent(heading: HTMLElement): boolean {
    const next = nextBlock(heading);
    if (!next) return false;
    const l = this.levelOf(next);
    return !(l && l <= this.levelOf(heading));
  }

  isFolded(id: number): boolean {
    return this.folded.has(id);
  }

  toggle(id: number): boolean {
    if (this.folded.has(id)) this.folded.delete(id);
    else this.folded.add(id);
    return this.folded.has(id);
  }

  set(id: number, folded: boolean) {
    if (folded) this.folded.add(id);
    else this.folded.delete(id);
  }

  /** Fold every heading that has something under it, or only those of `level` and below (deeper ones). */
  foldAll(fromLevel = 1) {
    for (const el of this.blocks()) {
      const l = this.levelOf(el);
      if (l >= fromLevel && this.hasContent(el)) this.folded.add(this.idOf(el));
    }
  }

  unfoldAll() {
    this.folded.clear();
  }

  /** Forget folds for blocks that are gone (an edit removed them). */
  prune() {
    if (!this.folded.size) return;
    const ids = new Set(this.blocks().map((el) => this.idOf(el)));
    for (const id of this.folded) if (!ids.has(id)) this.folded.delete(id);
  }

  /** Mark the blocks for the paginator and the outline's styling. Returns true when any block is folded away. */
  apply(): boolean {
    if (!this.active) return false;
    this.prune();
    let foldLevel = 0;
    let owner = 0;
    let any = false;
    const kids = this.blocks();
    kids.forEach((el, i) => {
      const lvl = this.levelOf(el);
      if (lvl && foldLevel && lvl <= foldLevel) foldLevel = 0;
      let hide = foldLevel > 0;
      if (!hide && this.outline && this.levels > 0 && (lvl === 0 || lvl > this.levels)) hide = true;
      this.mark(el, "fold", hide ? "1" : undefined);
      any ||= hide;
      if (!hide && lvl && this.folded.has(this.idOf(el))) foldLevel = lvl;
      if (this.outline) {
        // Body text sits one level in from the heading above it; a heading's mark says whether it has anything under it.
        this.mark(el, "ob", lvl ? undefined : String(owner));
        const next = kids[i + 1];
        const nl = next ? this.levelOf(next) : 0;
        const leaf = !next || (nl > 0 && nl <= lvl);
        this.mark(el, "oc", lvl ? (leaf ? "leaf" : this.folded.has(this.idOf(el)) ? "folded" : "open") : undefined);
        this.mark(el, "ofl", !lvl && this.firstLine ? "1" : undefined);
      } else {
        this.mark(el, "ob", undefined);
        this.mark(el, "oc", undefined);
        this.mark(el, "ofl", undefined);
      }
      if (lvl) owner = lvl;
    });
    this.clean = !this.folded.size && !this.outline && !any;
    return any;
  }

  private mark(el: HTMLElement, key: string, value: string | undefined) {
    if (value === undefined) {
      if (key in el.dataset) delete el.dataset[key];
    } else if (el.dataset[key] !== value) el.dataset[key] = value;
  }

  /** Forget everything (a new document). */
  reset() {
    this.folded.clear();
    this.levels = 0;
    this.firstLine = false;
  }
}
