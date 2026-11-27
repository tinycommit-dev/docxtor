// The view: zoom and fit, continuous layout, folds, drawn pages, far pages, and the view switches.
import type { ContinuousSpec, Editor } from "./editor";
import type { PageSetup, RBlock, RPara, SectionInfo } from "@docxtor/core";
import type { ContinuousLayout, PageBox } from "./layout";
import { clampZoom } from "./zoom";
import { FAR, firstBlock, isFar, isTopBlock, nextBlock, prevBlock, topBlockOf } from "./far";
import { sideMargins } from "./layout";
import { tw } from "./render";
import { columnWidth } from "@docxtor/extensions";
import { paraIn, readSelection } from "./selection";

export const viewMethods = {
  /** The view of a paragraph as the engine last rendered it (tables and content controls included), or undefined. */
  paragraphView(this: Editor, id: number): RPara | undefined {
    const v = this.views.get(id);
    return v?.t === "p" ? v : undefined;
  },
  /** A block as the engine last rendered it, found at any depth (inside content controls and table cells too). */
  blockView(this: Editor, id: number): RBlock | undefined {
    return this.views.get(id);
  },
  /**
   * The block elements drawn since the last call (a keystroke draws one), so a tool that decorates what is drawn looks at
   * those and not at the whole page. Null the first time, after a full draw, and when more than 2,000 came: look at everything.
   * One tool asks (the content controls); a second would have to share what it gets.
   */
  takeDrawn(this: Editor): HTMLElement[] | null {
    const d = this.drawn;
    this.drawn = [];
    return d;
  },
  /** Put a rendered block, and the blocks inside it, in `views` or take them out. */
  indexViews(this: Editor, b: RBlock, add: boolean): void {
    const walk = (x: RBlock) => {
      if (add) this.views.set(x.id, x);
      else if (this.views.get(x.id) === x) this.views.delete(x.id);
      if (x.t === "c") x.blocks.forEach(walk);
      else if (x.t === "tbl") for (const r of x.rows) for (const c of r.cells) c.blocks.forEach(walk);
    };
    walk(b);
  },
  /** Where page k (from 0) sits in the stage and how big it is, as laid out now. */
  pageBox(this: Editor, k: number): Readonly<PageBox> {
    return this.paginator.page(k);
  },
  /** Set the zoom factor (1 is 100%). A person's choice (`manual`, the default) makes it a fixed factor; the zoomer itself passes false. */
  setZoom(this: Editor, z: number, manual = true): void {
    if (manual) return this.zoomer.set(z);
    this.zoom = clampZoom(z);
    // The pages are always laid out at 100% and only drawn larger or smaller (a transform; CSS `zoom` would lay the text out again at the new
    // scale, and Chrome rounds its lines differently there: a novel had 1,164 pages at 100% and 1,160 to 1,162 at others, where Word's pages do not
    // depend on the zoom). So a zoom needs no layout: the page map is the 100% one. Only what depends on the view is looked at again.
    this.viewEl.style.transform = this.zoom === 1 ? "" : `scale(${this.zoom})`;
    this.zoomSet = true;
    this.sizeZoom();
    this.sizeRuler();
    this.farSoon();
    this.marks.invalidate();
    this.spell.invalidate();
    this.extra.invalidate();
    this.foldMarks.invalidate();
    this.hooks.onLayout?.(this.pages);
  },
  /** Give the picture's box the size the scaled pages take (the scroll extent and the centring follow it). */
  sizeZoom(this: Editor): void {
    const z = this.zoom;
    const w = this.viewEl.offsetWidth * z;
    const h = this.viewEl.offsetHeight * z;
    if (this.fitEl.style.width !== `${w}px`) this.fitEl.style.width = `${w}px`;
    if (this.fitEl.style.height !== `${h}px`) this.fitEl.style.height = `${h}px`;
  },
  /** The ruler's box takes the ruler's size at the zoom (the ruler itself is scaled as the pages are). */
  sizeRuler(this: Editor): void {
    const z = this.zoom;
    const r = this.ruler.el;
    r.style.transform = z === 1 ? "" : `scale(${z})`;
    this.rulerHost.style.width = `${r.offsetWidth * z}px`;
    this.rulerHost.style.height = `${r.offsetHeight * z}px`;
    this.rulerHost.style.marginBottom = `${12 * z}px`;
  },
  /** Return to automatic fit-to-width zoom. */
  fitWidth(this: Editor): void {
    this.zoomer.setMode("auto");
  },
  /** Work the zoom out again for its mode: after a resize, a new document or a change of the widest page. */
  applyAutoFit(this: Editor): void {
    this.zoomer.apply();
  },
  /** The first paragraph that starts on page k (0-based), or null. */
  firstParagraphOnPage(this: Editor, k: number): number | null {
    const top = this.paginator.page(k).top;
    // The page and the next are drawn while they are looked at (the first paragraph on a page can only be on it, or start the one after).
    return this.withDrawn(
      () => {
        const stage = this.stage.getBoundingClientRect().top;
        // The page map says where to look: from the block before the first one that starts on this page or a later one (it may run onto the page).
        const from = topBlockOf(this.flow, this.paginator.firstBlockFrom(k) ?? null);
        let child: HTMLElement | null = from ? (prevBlock(from) ?? from) : firstBlock(this.flow);
        for (; child; child = nextBlock(child)) {
          // A block left out of the layout is not on the pages drawn here (its rectangles are zeros).
          if (isFar(child)) continue;
          const own = (child as HTMLElement).dataset.p !== undefined ? [child as HTMLElement] : [];
          for (const el of [...own, ...Array.from(child.querySelectorAll<HTMLElement>("[data-p]"))]) {
            if (el.closest("[data-fold]")) continue;
            if ((el.getBoundingClientRect().top - stage) / this.zoom >= top - 1) return Number(el.dataset.p);
          }
        }
        return null;
      },
      { from: k, to: k + 1 },
    );
  },
  /**
   * Lay the text out as one unbroken sheet (Pageless, Reading, Outline), or back on pages with null. `width: "window"`
   * fits the column to the window and follows it as it changes.
   */
  setContinuous(this: Editor, spec: ContinuousSpec | null): void {
    this.contSpec = spec;
    // The header or footer being edited is not shown in a continuous layout: back to the text, where the caret was.
    if (spec && this.story && this.story.zone !== "notes" && this.story.zone !== "endnotes") this.exitStory();
    this.canvas.classList.toggle("continuous", !!spec);
    this.stage.classList.toggle("cont", !!spec);
    if (!this.engine) return;
    if (this.grid.on) this.zoomer.setMode("width");
    this.paginator.setContinuous(spec ? this.continuousLayout(spec, this.setup ?? this.engine.pageSetup()) : null);
    this.reloadPageSetup();
  },
  /** The sizes of a continuous layout for the window as it is now. */
  continuousLayout(this: Editor, spec: ContinuousSpec, page: PageSetup): ContinuousLayout {
    const first = page.sections?.[0] ?? page;
    const side = sideMargins(first as SectionInfo, { gutterAtTop: !!page.gutterAtTop });
    const ml = spec.marginLeft ?? spec.margin ?? tw(side.left);
    const mr = spec.margin ?? tw(side.right);
    const out: ContinuousLayout = { ml, mr, mt: spec.marginTop, mb: spec.marginTop };
    if (spec.width === "window") {
      const cs = getComputedStyle(this.canvas);
      const avail = (this.canvas.clientWidth - parseFloat(cs.paddingLeft || "0") - parseFloat(cs.paddingRight || "0")) / (this.zoom || 1);
      const text = columnWidth(spec, tw(first.width) - ml - mr, avail - ml - mr);
      out.width = text + ml + mr;
    }
    return out;
  },
  /** The window changed size: a continuous layout that fits the window sets its column again. */
  reflowContinuous(this: Editor): boolean {
    const spec = this.contSpec;
    if (!spec || spec.width !== "window" || !this.engine || !this.paginator) return false;
    const next = this.continuousLayout(spec, this.setup ?? this.engine.pageSetup());
    if (Math.abs((next.width ?? 0) - this.paginator.stageWidth) < 1) return false;
    this.paginator.setContinuous(next);
    this.reloadPageSetup();
    return true;
  },
  /** Fold or unfold a heading (the arrow beside it). */
  toggleFold(this: Editor, id: number): void {
    this.folds.toggle(id);
    this.foldsChanged();
  },
  /** Something folded or unfolded: lay out again and keep the caret out of what is folded away. */
  foldsChanged(this: Editor): void {
    this.layoutNow();
    const sel = this.sel;
    const at = sel && this.flow.querySelector<HTMLElement>(`[data-p="${sel.focus.p}"]`);
    let block = at?.parentElement ? topBlockOf(this.flow, at) : null;
    if (block?.dataset.fold) {
      while (block && block.dataset.fold) block = prevBlock(block);
      if (block?.dataset.p) this.select({ p: Number(block.dataset.p), o: 0 });
    }
    this.hooks.onLayout?.(this.pages);
  },
  /** Scroll a block (by id) into view without moving the caret. */
  revealBlock(this: Editor, id: number): void {
    // A block the view itself moves to (the Outline view): not opened out of a fold or a tab, only drawn.
    const el = this.ensureDrawn(id, { open: false });
    el?.scrollIntoView({ block: "nearest" });
  },
  /**
   * The one way in for everything that jumps to a block or measures one on demand (Find, Go to, bookmarks, the Navigation pane, links,
   * the accessibility checker, AI changes, selecting an object, the spelling dialog, the caret after an edit far from the view): it makes the
   * block measurable before anyone measures it, and returns its element (null when it is not in the text). Call it, then measure.
   *
   * What it does: a place in a tab not shown opens that tab; headings folded over it are opened (`open: false` leaves both be); and the page
   * it is on is drawn, which in C1 is nothing (every page is) and from C2 on is the one place that puts a far page back in the layout.
   * `target` is a paragraph or block id, or an element.
   */
  ensureDrawn(this: Editor, target: number | Element, opts: { open?: boolean } = {}): HTMLElement | null {
    let el: HTMLElement | null = null;
    if (typeof target === "number") {
      if (opts.open !== false) this.revealPara(target);
      // Found by id from the map the editor keeps (a search of the page grows with the document).
      el = paraIn(this.flow, target) ?? this.flow.querySelector<HTMLElement>(`[data-id="${target}"]`);
    } else el = target as HTMLElement;
    if (el) this.drawBlock(el);
    return el;
  },
  /** Scroll the paragraph (by id; in the text or the story being edited) into view without moving the caret: drawn first, then scrolled to. */
  scrollParaIntoView(this: Editor, p: number): void {
    const el = this.paraElement(p);
    if (!el) return;
    this.ensureDrawn(el);
    el.scrollIntoView({ block: "nearest" });
  },
  /**
   * Make sure the block holding `el` is in the layout: if it was left out (`data-far`) it comes back, and stays until the view has settled
   * (the next frame, when `updateFar` takes the selection and the view into account again). The page map says where it goes.
   */
  drawBlock(this: Editor, el: Element): void {
    const pg = this.paginator;
    if (!pg?.farBlocks) return;
    const block = pg.blockOf(el);
    if (!block?.hasAttribute(FAR)) return;
    this.farPins.add(block);
    pg.showBlock(block);
    this.farStamp++;
    this.pinFrame ||= requestAnimationFrame(() => {
      this.pinFrame = 0;
      this.farPins.clear();
      this.updateFar();
    });
  },
  /**
   * Run something that reads the pages of the whole document, or of pages `from` to `to` (0-based), as drawn: printing, the PDF, a page's
   * picture, the page grid, the page numbers a table of contents takes. It is the one way to lay everything out for output, as `showAllTabs`
   * and `asPages` do for tabs and views. With far pages it brings every page (or those) into the layout, and puts the far ones back when
   * `fn` is done (when it returns a promise, once that settles).
   */
  withDrawn<T>(this: Editor, fn: () => T, pages?: { from: number; to: number }): T {
    const pg = this.paginator;
    if (!pg?.farBlocks) return fn();
    this.farHold++;
    if (pages) pg.showPages(pages.from, pages.to);
    else pg.farShowAll();
    this.farStamp++;
    let release = () => {
      if (--this.farHold === 0) this.updateFar();
    };
    let result: T;
    try {
      result = fn();
    } catch (e) {
      release();
      throw e;
    }
    if (result && typeof (result as { then?: unknown }).then === "function") {
      (result as unknown as Promise<unknown>).then(release, release);
      release = () => {};
    } else release();
    return result;
  },
  /** The top-level blocks that may reach into a band of the stage (stage px at 100%), from the page map; undefined when it is not up to date. */
  blocksBetween(this: Editor, y0: number, y1: number): HTMLElement[] | undefined {
    return this.paginator.blocksBetween(y0, y1);
  },
  /**
   * The overlays that draw for the blocks that are drawn only (bookmark ribbons, fold arrows, extra selection boxes, marks, underlines) draw
   * again when the drawn stretch moved: scrolling can bring pages in and take others out, and they redrew only on a layout before. Nothing
   * changes the stretch in C1, so this does nothing yet.
   */
  followDrawn(this: Editor): void {
    if (this.farSeen === this.farStamp) return;
    this.farSeen = this.farStamp;
    this.marks.invalidate();
    this.spell.invalidate();
    this.bookmarkMarks.invalidate();
    this.foldMarks.invalidate();
    this.extra.invalidate();
  },
  /** Work out the drawn stretch in the next frame (a scroll, a selection that moved). */
  farSoon(this: Editor): void {
    if (this.farFrame || !this.paginator || this.farKeep === null) return;
    this.farFrame = requestAnimationFrame(() => {
      this.farFrame = 0;
      this.updateFar();
    });
  },
  /** The pages in view (stage px at 100%): the first one whose bottom is below the top of the view, and the last whose top is above its bottom. */
  viewPages(this: Editor): [number, number] {
    const c = this.canvas.getBoundingClientRect();
    const s = this.stage.getBoundingClientRect();
    const sc = s.width / (this.stage.offsetWidth || 1) || 1;
    const top = (c.top - s.top) / sc;
    const bottom = (c.bottom - s.top) / sc;
    return [this.paginator.firstPageBelow(top), this.paginator.pageAt(bottom)];
  },
  /**
   * Leave out of the layout the blocks on pages far from the view: the pages in view and `farKeep`
   * either side, the pages of both ends of the selection (and a page either side of where the caret is), of every other selected range and of
   * the text being composed, and what `ensureDrawn` brought back. Everything else is `display: none`, its height kept. Called after each
   * layout, on scroll and when the selection moves. Does nothing while `withDrawn` runs, or before a layout has described the pages.
   */
  updateFar(this: Editor): void {
    const pg = this.paginator;
    if (!pg || this.farHold) return;
    const keep = this.farKeep;
    if (keep === null) {
      if (pg.farBlocks && pg.setFar(null, new Set())) {
        this.farStamp++;
        this.followDrawn();
      }
      return;
    }
    const [v0, v1] = this.viewPages();
    const last = Math.max(0, this.pages - 1);
    const ranges: [number, number][] = [[Math.max(0, v0 - keep), Math.min(last, v1 + keep)]];
    const pins = new Set<HTMLElement>(this.farPins);
    const mark = (p: number, wide: boolean) => {
      const el = paraIn(this.flow, p);
      const block = el && pg.blockOf(el);
      if (!block) return;
      pins.add(block);
      const at = pg.pagesOf(block);
      if (at) ranges.push([Math.max(0, at[0] - (wide ? 1 : 0)), Math.min(last, at[1] + (wide ? 1 : 0))]);
    };
    const live = this.composing ?? (this.hasFocus() ? readSelection(this.root) : null) ?? this.sel;
    if (live) {
      mark(live.focus.p, true);
      mark(live.anchor.p, false);
    }
    for (const r of this.extra.ranges) {
      mark(r.from.p, false);
      mark(r.to.p, false);
    }
    // The same stretch as the last time, and the layout only went over blocks that stayed in it (a key): nothing to do.
    const sig = `${ranges.map((r) => r.join("-")).join(",")}|${[...pins].map((b) => b.dataset.id ?? "").join(",")}|${this.pages}`;
    if (sig === this.farSig && pg.farKept(ranges, pins)) return;
    this.farSig = sig;
    const changed = pg.setFar(ranges, pins);
    if (!changed) return;
    this.farStamp++;
    // The check compares the stretch with everything drawn, which only agree while the page map describes the layout: it waits for a whole
    // layout that is still to run.
    // @ts-ignore: set by the bundler; false in the production build, which drops the check with it
    if (import.meta.env.DEV && pg.verify && !this.pendingFull) void import("./layout-check").then((m) => m.checkFar(pg, () => pg.setFar(ranges, pins)));
    this.followDrawn();
  },
  /** The paragraphs of the text in order, with how many lines each shows, from the page map (Go to line); paragraphs folded away or in a tab not shown show none. */
  paragraphLines(this: Editor): { p: number; n: number }[] {
    return this.withDrawn(() => {
      const out: { p: number; n: number }[] = [];
      for (const el of this.paginator.pageBlockList()) {
        const l = this.paginator.blockLines(el);
        l.ids.forEach((p, i) => {
          if (p >= 0 && l.counts[i] > 0) out.push({ p, n: l.counts[i] });
        });
      }
      return out;
    });
  },
  /** The lines of the text of the document and of the boxes in it, from the page map (Word count). */
  lineTotals(this: Editor): { main: number; boxes: number } {
    return this.withDrawn(() => this.paginator.lineTotals());
  },
  /** Open the headings that are folded over a paragraph, so a match, a link or a click from the Navigation pane lands where it can be seen. */
  revealPara(this: Editor, p: number): void {
    // A place in a tab not shown opens that tab first: Find, the navigation pane, links, bookmarks and Go to all come here.
    const tab = this.allTabs ? null : this.hiddenTabOf(p);
    if (tab) this.openTab(tab);
    if (!this.folds.folded.size) return;
    const el = this.flow.querySelector<HTMLElement>(`[data-p="${p}"]`);
    const block = topBlockOf(this.flow, el);
    if (!block?.dataset.fold) return;
    let opened = false;
    for (let h = prevBlock(block); h; h = prevBlock(h)) {
      const id = Number(h.dataset.id);
      if (this.folds.isFolded(id) && this.folds.under(h).includes(block)) {
        this.folds.set(id, false);
        opened = true;
      }
    }
    if (opened) this.foldsChanged();
  },
  /** The block (a paragraph, a table, a wrapper) of the main text that holds paragraph `p`, as its id. */
  topBlockId(this: Editor, p: number): number | null {
    const el = this.flow.querySelector<HTMLElement>(`[data-p="${p}"]`);
    if (!el) return null;
    for (let n: HTMLElement | null = el; n && n !== this.flow; n = n.parentElement) {
      if (isTopBlock(this.flow, n)) return Number(n.dataset.id) || null;
    }
    return null;
  },
  showMarks(this: Editor, on: boolean): void {
    this.marks.show(on);
    this.syncHidden();
  },
  showGridlines(this: Editor, on: boolean): void {
    this.guides.setGridlines(on);
  },
  showBoundaries(this: Editor, on: boolean): void {
    this.guides.setBoundaries(on);
  },
  /** Draw the labelled rules for page, column and section breaks without the other marks (the Draft view). */
  showBreaks(this: Editor, on: boolean): void {
    this.marks.showBreaks(on);
  },
  /** Where the view is, as a page and how far down it: panes with different documents keep in step by this (View, Synchronous scrolling). */
  scrollPosition(this: Editor): { page: number; fraction: number } {
    const y = (this.canvas.getBoundingClientRect().top - this.stage.getBoundingClientRect().top) / (this.zoom || 1);
    const page = this.paginator.firstPageBelow(y);
    const b = this.paginator.page(page);
    return { page, fraction: b.h ? (y - b.top) / b.h : 0 };
  },
  /** Scroll to a page and a fraction of it (see `scrollPosition`). A page past the end goes to the last one. */
  scrollToPosition(this: Editor, pos: { page: number; fraction: number }): void {
    const b = this.paginator.page(Math.max(0, Math.min(this.pages - 1, pos.page)));
    const y = b.top + pos.fraction * b.h;
    const delta = this.stage.getBoundingClientRect().top + y * (this.zoom || 1) - this.canvas.getBoundingClientRect().top;
    this.canvas.scrollTop += delta;
  },
  /** Call `fn` whenever the view scrolls. Returns a function that stops it. */
  onScrolled(this: Editor, fn: () => void): () => void {
    this.canvas.addEventListener("scroll", fn, { passive: true });
    return () => this.canvas.removeEventListener("scroll", fn);
  },
  showHiddenText(this: Editor, on: boolean): void {
    this.hiddenOn = on;
    this.syncHidden();
  },
  syncHidden(this: Editor): void {
    const on = this.hiddenShown;
    if (this.stage.classList.contains("show-hidden") === on) return;
    this.stage.classList.toggle("show-hidden", on);
    // Hidden text takes room once it shows.
    if (this.engine) this.layoutNow();
  },
  showRuler(this: Editor, on: boolean): void {
    this.ruler.el.hidden = !on;
    this.rulerHost.hidden = !on;
    this.vruler.show(on);
    if (on) this.updateVRuler();
  },
  scrollCaretIntoView(this: Editor): void {
    const s = window.getSelection();
    if (!s || !s.rangeCount) return;
    // The caret can have been moved far by an edit, an undo or a paste: its block is drawn before it is measured.
    const at = s.anchorNode;
    const block = at ? (at.nodeType === Node.ELEMENT_NODE ? (at as Element) : at.parentElement) : null;
    if (block && this.flow.contains(block)) this.drawBlock(block);
    const r = s.getRangeAt(0).getBoundingClientRect();
    const c = this.canvas.getBoundingClientRect();
    if (r.height === 0 && r.top === 0) return;
    if (r.bottom > c.bottom - 40) this.canvas.scrollTop += r.bottom - c.bottom + 80;
    else if (r.top < c.top + 40) this.canvas.scrollTop -= c.top - r.top + 80;
  },
};

declare module "./editor" {
  interface Editor {
    paragraphView(id: number): RPara | undefined;
    blockView(id: number): RBlock | undefined;
    takeDrawn(): HTMLElement[] | null;
    indexViews(b: RBlock, add: boolean): void;
    pageBox(k: number): Readonly<PageBox>;
    setZoom(z: number, manual?: boolean): void;
    sizeZoom(): void;
    sizeRuler(): void;
    fitWidth(): void;
    applyAutoFit(): void;
    firstParagraphOnPage(k: number): number | null;
    setContinuous(spec: ContinuousSpec | null): void;
    continuousLayout(spec: ContinuousSpec, page: PageSetup): ContinuousLayout;
    reflowContinuous(): boolean;
    toggleFold(id: number): void;
    foldsChanged(): void;
    revealBlock(id: number): void;
    ensureDrawn(target: number | Element, opts?: { open?: boolean; }): HTMLElement | null;
    scrollParaIntoView(p: number): void;
    drawBlock(el: Element): void;
    withDrawn<T>(fn: () => T, pages?: { from: number; to: number; }): T;
    blocksBetween(y0: number, y1: number): HTMLElement[] | undefined;
    followDrawn(): void;
    farSoon(): void;
    viewPages(): [number, number];
    updateFar(): void;
    paragraphLines(): { p: number; n: number; }[];
    lineTotals(): { main: number; boxes: number; };
    revealPara(p: number): void;
    topBlockId(p: number): number | null;
    showMarks(on: boolean): void;
    showGridlines(on: boolean): void;
    showBoundaries(on: boolean): void;
    showBreaks(on: boolean): void;
    scrollPosition(): { page: number; fraction: number; };
    scrollToPosition(pos: { page: number; fraction: number; }): void;
    onScrolled(fn: () => void): () => void;
    showHiddenText(on: boolean): void;
    syncHidden(): void;
    showRuler(on: boolean): void;
    scrollCaretIntoView(): void;
  }
}
