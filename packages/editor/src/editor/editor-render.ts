// Drawing the document: applying an engine result to the pages, tabs, stories and layout passes.
import type { Editor } from "./editor";
import { knowPara, paraLength, writeSelection } from "./selection";
import type { MPos, PageSetup, RBlock, RenderOut } from "@docxtor/core";
import { lineBreakFor } from "./east-asian";
import { boxKey, faceOf, renderBlock } from "./render";
import { FAR } from "./far";
import { layoutTabs } from "./tabs";
import { sideMargins } from "./layout";
import { setPageMetrics } from "./place";
import { placeFloats } from "./float-place";
import { placeFrames } from "./frames";

/** The font faces the page has loaded now. */
/** Whether a face has finished loading that is not in `had` (the faces of the last whole layout). */
function newFaceSince(had: Set<FontFace>): boolean {
  if (typeof document === "undefined" || !document.fonts) return false;
  for (const f of document.fonts) if (f.status === "loaded" && !had.has(f)) return true;
  return false;
}

function loadedFaces(): Set<FontFace> {
  const out = new Set<FontFace>();
  if (typeof document === "undefined" || !document.fonts) return out;
  document.fonts.forEach((f) => {
    if (f.status === "loaded") out.add(f);
  });
  return out;
}

export const renderMethods = {
  /** Show a tab in this pane. Not an edit: it works in a read-only document and takes no undo step. False when there is no such tab. */
  openTab(this: Editor, id: string): boolean {
    if (!this.tabs.tabs.some((x) => x.id === id)) return false;
    if (this.openTabId === id) return true;
    // A header, footer or note being edited belongs to a page of the tab that goes: back to the text first, so the caret below is the text's.
    if (this.story) this.exitStory(false);
    this.openTabId = id;
    this.markTabs(false);
    // The caret was in the tab that is now hidden: it goes to the first paragraph of the tab shown.
    if (this.tabOfCaret(this.tabs.tabs)) {
      const tab = this.tabs.tabs.find((x) => x.id === id);
      const el = tab?.ids.length ? this.root.querySelector<HTMLElement>(`[data-id="${tab.ids[0]}"]`) : null;
      const p = el?.dataset.p ?? el?.querySelector<HTMLElement>("[data-p]")?.dataset.p;
      this.sel = p ? { anchor: { p: Number(p), o: 0 }, focus: { p: Number(p), o: 0 } } : null;
    }
    this.layoutNow();
    if (this.sel && this.hasFocus()) writeSelection(this.root, this.sel);
    this.hooks.onChange?.();
    return true;
  },
  /**
   * Draw what the engine says changed. A keystroke sends the blocks it touched and not the order (`orderKept`), and
   * then nothing here walks the document: the blocks that changed and their neighbours are looked at, by the index
   * the order was last given with. `typing` is a keystroke's own render.
   */
  apply(this: Editor, out: RenderOut, typing = false): void {
    this.applied++;
    // The document's East Asian line-breaking rules: the strict level is a style of the flow, the custom lists are drawn by the paragraphs.
    this.env.asian = this.engine.asianSettings();
    const strict = lineBreakFor(this.env.asian);
    if (strict) this.flow.style.setProperty("--ea-line-break", strict);
    else this.flow.style.removeProperty("--ea-line-break");
    if (out.full) {
      this.paginator?.farForget();
      this.flow.innerHTML = "";
      this.blocks.clear();
      this.json.clear();
      this.views.clear();
      this.drawn = null;
    }
    // The order moved, or this is the first the editor hears of it.
    const kept = !!out.orderKept && !out.full;
    const order = kept ? this.order : (out.order ?? []);
    const orderIdx = kept ? this.orderIdx : new Map(order.map((id, i) => [id, i]));
    const fresh = new Map<number, RBlock>();
    for (const b of out.blocks) fresh.set(b.id, b);
    // A block is drawn from the one before it too (space after, contextual spacing, shared borders: see `renderBlock`), so a changed
    // block's successor is drawn again, but only when what it reads changed. Typing changes the text alone: drawing the next
    // paragraph again for every key took its spelling marks off and put them back, and laid the page out twice.
    const rerender = new Set(fresh.keys());
    for (const id of fresh.keys()) {
      const next = order[(orderIdx.get(id) ?? -2) + 1];
      if (next !== undefined && !fresh.has(next) && this.json.has(next) && faceOf(this.json.get(id)) !== faceOf(fresh.get(id))) rerender.add(next);
    }
    // Section breaks live on paragraphs: one added, changed or removed means the sections changed.
    const breakOf = (b?: RBlock) => (b?.t === "p" ? b.sectBreak : undefined);
    for (const [id, b] of fresh) if (breakOf(this.json.get(id)) !== breakOf(b)) this.sectionsDirty = true;
    if (!kept) for (const [id, b] of this.json) if (!orderIdx.has(id) && breakOf(b)) this.sectionsDirty = true;
    for (const [id, b] of fresh) {
      const old = this.json.get(id);
      if (old) this.indexViews(old, false);
      this.json.set(id, b);
      this.indexViews(b, true);
    }
    // Which places of the order to look at: all of them, or (the order did not move) the blocks that changed and their neighbours.
    let visit: number[];
    if (kept) {
      const at = new Set<number>();
      for (const id of rerender) {
        const i = orderIdx.get(id);
        if (i !== undefined) for (const k of [i - 1, i, i + 1]) if (k >= 0 && k < order.length) at.add(k);
      }
      visit = [...at].sort((x, y) => x - y);
    } else visit = order.map((_, i) => i);
    // A block drawn that had no element before has to be put in its place: the slow way, even when the order did not move.
    let placed = true;
    const rendered: number[] = [];
    for (const i of visit) {
      const id = order[i];
      const b = this.json.get(id);
      const prev = this.json.get(order[i - 1]);
      const next = this.json.get(order[i + 1]);
      // Paragraphs with identical borders form one box, so a bordered paragraph also draws again when
      // a neighbour joined or left its box (the neighbour changed, or a paragraph between them went).
      const reboxed = !!b && b.t === "p" && !!b.fmt.borders && (this.blocks.get(id)?.dataset.bj ?? "") !== boxKey(prev, b, next);
      if (b && (rerender.has(id) || reboxed)) {
        const el = renderBlock(b, this.env, prev, false, next);
        this.decorate(el);
        const old = this.blocks.get(id);
        if (old) {
          // A block that is left out of the layout comes back before another takes its place (its stretch is kept around it).
          if (old.hasAttribute(FAR)) this.paginator?.showBlock(old);
          old.replaceWith(el);
          this.paginator.carrySides(el, old);
          this.paginator.carryFar(el, old);
          // Typing in a line with tabs: put them where they belong now, not after the layout pause.
          if (el.querySelector(".ftab, .fptab")) layoutTabs(el, this.setup?.defaultTab || 720);
        } else placed = false;
        this.blocks.set(id, el);
        rendered.push(id);
        this.paginator?.drew(el);
        if (el.dataset.p) knowPara(this.flow, id, el);
        if (this.drawn && this.drawn.push(el) > 2000) this.drawn = null;
      }
    }
    if (kept && placed) return this.afterApply(out, typing, true, rendered);
    const wanted = new Set(order);
    for (const [id, el] of this.blocks) {
      if (!wanted.has(id)) {
        this.paginator?.removing(el);
        el.remove();
        this.blocks.delete(id);
        const gone = this.json.get(id);
        if (gone) this.indexViews(gone, false);
        this.json.delete(id);
      }
    }
    // Reconcile order. Blocks of a section with columns sit inside its wrapper: a new block joins its neighbour's
    // parent, and the paginator moves it where it belongs when the sections say otherwise.
    const cur = this.paginator.blocks();
    const moved = new Set<HTMLElement>();
    const drawnNow = new Set(rendered);
    let k = 0;
    let last: HTMLElement | null = null;
    for (const id of order) {
      const el = this.blocks.get(id);
      if (!el) continue;
      while (k < cur.length && moved.has(cur[k])) k++;
      if (cur[k] === el) {
        k++;
      } else {
        // A block that was already there and moved: the page map no longer describes it (one just drawn is listed already).
        if (!drawnNow.has(id)) this.paginator.invalidate();
        if (k < cur.length) cur[k].before(el);
        else if (last) last.after(el);
        else this.flow.append(el);
        this.paginator.placed(el);
        moved.add(el);
      }
      last = el;
    }
    this.order = order;
    this.orderIdx = orderIdx;
    this.afterApply(out, typing, false, rendered);
  },
  /** What follows drawing blocks: the tabs. A keystroke in a document without tabs has nothing to do (no tab can appear without a block coming). */
  afterApply(this: Editor, out: RenderOut, typing: boolean, kept: boolean, rendered: number[]): void {
    if (typing && kept && !out.full && !this.tabs.tabs.length) return;
    // The order did not move: if the tabs did not either, only the blocks drawn just now need their tab's mark.
    this.markTabs(true, kept && !out.full ? rendered : undefined);
  },
  /**
   * A document with tabs shows one at a time: the top-level blocks of the other tabs are hidden and the pages leave them
   * out. The caret in a hidden tab (after an undo there, or a jump) shows that tab.
   */
  markTabs(this: Editor, followCaret = true, only?: number[]): void {
    const info = this.engine?.tabsInfo() ?? { tabs: [] };
    const { tabs } = info;
    const ids = tabs.map((x) => x.id);
    let open = this.openTabId;
    if (open && !ids.includes(open)) {
      // The tab went (deleted here, or in the other pane): the one before it, as the tab list does.
      const was = this.knownTabs.indexOf(open);
      open = was >= 0 && ids.length ? ids[Math.min(Math.max(was - 1, 0), ids.length - 1)] : null;
    }
    if (!open || !ids.includes(open)) open = info.active && ids.includes(info.active) ? info.active : (ids[0] ?? null);
    for (let pass = 0; pass < 2; pass++) {
      this.paintTabs(tabs, open, only);
      const owner = !followCaret || this.allTabs || tabs.length < 2 ? null : this.tabOfCaret(tabs);
      if (!owner || owner === open) break;
      open = owner;
    }
    this.openTabId = open;
    this.knownTabs = ids;
    this.tabs = open ? { tabs, active: open } : { tabs };
    // The file keeps the tab of the first pane, written when it is saved (not an edit).
    if (this.savesOpenTab && open && info.active !== open) {
      try {
        this.engine.setOpenTab(open);
      } catch {
        /* a document without tabs has nothing to remember */
      }
    }
  },
  /**
   * Hide the blocks of the tabs that are not `open`. With `only` (the order did not move) and the same tabs as last time, just those
   * blocks are looked at: the others still have the mark they were given, and walking every block costs the length of the document.
   */
  paintTabs(this: Editor, tabs: import("@docxtor/core").TabInfo[], open: string | null, only?: number[]): void {
    let keep: Set<number> | null = null;
    if (tabs.length > 1 && !this.allTabs) {
      const tab = tabs.find((x) => x.id === open) ?? tabs[0];
      keep = new Set(tab.ids);
    }
    const sig = `${open}|${keep ? 1 : 0}|${tabs.map((x) => `${x.id}:${x.ids.length}:${x.ids[0]}:${x.ids[x.ids.length - 1]}`).join(",")}`;
    const ids = only && sig === this.paintedTabs ? only : this.order;
    this.paintedTabs = sig;
    for (const id of ids) {
      const el = this.blocks.get(id);
      if (!el) continue;
      if (!keep || keep.has(id)) delete el.dataset.tabHidden;
      else el.dataset.tabHidden = "";
    }
  },
  /** The tab the caret is in when that tab is hidden, else null. */
  tabOfCaret(this: Editor, tabs: import("@docxtor/core").TabInfo[]): string | null {
    const p = this.sel?.focus.p;
    return p === undefined ? null : this.hiddenTabOf(p, tabs);
  },
  /** The tab that holds paragraph (or block) `p` when that tab is hidden, else null. */
  hiddenTabOf(this: Editor, p: number, tabs: import("@docxtor/core").TabInfo[] = this.tabs.tabs): string | null {
    const el = this.flow.querySelector(`[data-p="${p}"]`) ?? this.flow.querySelector(`[data-id="${p}"]`);
    const hidden = el?.closest("[data-tab-hidden]");
    if (!hidden) return null;
    const id = Number((hidden as HTMLElement).dataset.id ?? (hidden as HTMLElement).dataset.p);
    return tabs.find((x) => x.ids.includes(id))?.id ?? null;
  },
  decorate(this: Editor, el: HTMLElement): void {
    if (!this.aiMarks.size) return;
    const mark = (p: HTMLElement) => {
      if (this.aiMarks.has(Number(p.dataset.p))) p.classList.add("ai-changed");
    };
    if (el.dataset.p) mark(el);
    el.querySelectorAll<HTMLElement>("[data-p]").forEach(mark);
  },
  /** Re-render whatever the engine reports as changed and re-paginate. */
  refresh(this: Editor, opts: { keepScroll?: boolean; quiet?: boolean } = {}): void {
    // Undo, redo and edits made outside the editor move text the other ranges point at.
    this.extra.clear();
    const scroll = this.canvas.scrollTop;
    this.applyOut(this.engine.renderChanged());
    this.layoutNow({ incremental: true });
    if (this.sel && !this.root.querySelector(`[data-p="${this.sel.focus.p}"]`)) this.sel = null;
    if (opts.keepScroll) this.canvas.scrollTop = scroll;
    // An update as the file opens isn't the person's change.
    if (!opts.quiet) this.hooks.onChange?.();
    // Undo, redo and edits made outside the editor (the assistant, extensions) all end here.
    this.hooks.onFontsMayChange?.();
  },
  /** Render everything again (the fonts a document can use changed), keeping the caret and scroll. */
  rerender(this: Editor): void {
    if (!this.engine) return;
    const scroll = this.canvas.scrollTop;
    const all = this.engine.renderAll();
    this.apply(all);
    for (const p of this.peers) p.mirror(all);
    this.reloadStories();
    this.layoutNow();
    this.canvas.scrollTop = scroll;
    if (this.sel && this.hasFocus()) writeSelection(this.root, this.sel);
  },
  reloadPageSetup(this: Editor): void {
    this.reloadStories();
    this.layoutNow();
  },
  /** Apply a render result; headers/footers/notes come with the page setup. */
  applyOut(this: Editor, out: RenderOut, typing = false): void {
    this.apply(out, typing);
    for (const p of this.peers) p.mirror(out, typing);
    if (out.stories || this.sectionsDirty) this.reloadStories();
  },
  /** Another pane of a split window changed the document: draw what changed there, and keep this pane's caret inside the text. */
  mirror(this: Editor, out: RenderOut, typing = false): void {
    this.extra.clear();
    this.apply(out, typing);
    this.pending = null;
    if (this.sel) {
      if (!this.root.querySelector(`[data-p="${this.sel.focus.p}"]`) || !this.root.querySelector(`[data-p="${this.sel.anchor.p}"]`)) this.sel = null;
      else {
        const clamp = (m: MPos) => ({ p: m.p, o: Math.min(m.o, paraLength(this.root, m.p)) });
        this.sel = { anchor: clamp(this.sel.anchor), focus: clamp(this.sel.focus) };
      }
    }
    this.scheduleLayout(true);
  },
  /** Where floats measure from: the text's left edge and width, which the gutter narrows. */
  floatMetrics(this: Editor, page: PageSetup): void {
    // The gutter is a property of the section (the last one is what the page setup describes).
    const info = page.sections?.[page.sections.length - 1] ?? { margin: page.margin };
    const side = sideMargins(info, { gutterAtTop: !!page.gutterAtTop });
    setPageMetrics((page.width - side.left - side.right) / 20, side.left / 20);
  },
  /** With `onlyIfChanged`, nothing is done when the engine's description of the page setup is what it was (an undo of typing leaves it, and the pages are not laid out from the top for it). */
  reloadStories(this: Editor, fromPeer = false, onlyIfChanged = false): void {
    const page = this.engine.pageSetup();
    const key = JSON.stringify(page);
    if (onlyIfChanged && key === this.setupKey && !this.sectionsDirty && !this.story) {
      // The peers read their own.
      if (!fromPeer) for (const p of this.peers) p.reloadStories(true, true);
      return;
    }
    this.setupKey = key;
    if (!fromPeer) for (const p of this.peers) p.reloadStories(true);
    this.setup = page;
    this.floatMetrics(page);
    this.sectionsDirty = false;
    this.paginator.configure(page);
    this.showRulerSection();
    // Sections can change the widest page.
    this.applyAutoFit();
    if (this.story) this.renderStory();
    if (fromPeer) this.scheduleLayout();
  },
  /**
   * Lay the pages out now rather than after the short delay typing waits for. With `incremental` (the blocks drawn since the
   * last layout are all that changed) the paginator starts at the first of them; anything else asks for the whole text.
   */
  layoutNow(this: Editor, opts: { incremental?: boolean } = {}): void {
    window.clearTimeout(this.layoutTimer);
    this.folds.apply();
    // Document tabs, folds and the outline, and a header or footer being edited change what the pages are made of without a block being drawn.
    // A font that arrived since the last whole layout has already changed the text it is used in, before its `loadingdone` event
    // asks for a layout: a layout that starts part-way down would measure what is below against records of the text above as it was.
    const fontCame = !!opts.incremental && newFaceSince(this.facesAtFull);
    const incremental = !!opts.incremental && !this.pendingFull && !fontCame && !this.story && !this.allTabs && !this.tabs.tabs.length && !this.folds.active;
    if (opts.incremental && !incremental) {
      const why = fontCame && !this.pendingFull ? "a font arrived" : "something else changed";
      this.paginator.stats.fallbacks[why] = (this.paginator.stats.fallbacks[why] ?? 0) + 1;
    }
    this.pendingFull = false;
    const fullBefore = this.paginator.stats.fullPasses;
    let pages = this.paginator.layout({ incremental });
    // The faces a whole layout measured the text with (no font can arrive while it runs: it is one task).
    if (this.paginator.stats.fullPasses > fullBefore) this.facesAtFull = loadedFaces();
    // Page numbers and the like in the text, and field codes, change what the text takes up: lay out again once.
    const filled = this.paginator.fillBodyFields();
    if (this.fieldCodes.apply(this.flow) || filled) {
      this.paginator.invalidate();
      pages = this.paginator.layout();
      this.paginator.fillBodyFields();
      this.fieldCodes.apply(this.flow);
    }
    // Objects placed from the page (behind or in front of the text) go where their position counts from.
    placeFloats(this.floatHost, this.stage);
    placeFrames({
      flow: this.flow,
      pageAt: (y) => this.paginator.pageAt(y),
      page: (k) => this.paginator.page(k),
      marginTop: (k) => this.paginator.sectionMargins(k).top,
      bottomMargin: (k) => this.paginator.sectionMargins(k).bottom,
      sheets: () => Array.from(this.sheets.children) as HTMLElement[],
    });
    if (this.sel && this.hasFocus()) writeSelection(this.root, this.sel);
    this.updateVRuler();
    this.marks.invalidate();
    this.bookmarkMarks.invalidate();
    this.foldMarks.invalidate();
    this.guides.invalidate();
    this.spell.relaid(this.paginator.stats.fullPasses > fullBefore);
    this.extra.invalidate();
    this.grid.refresh();
    this.updateFar();
    this.sizeZoom();
    this.hooks.onLayout?.(pages);
  },
  /** Lay the pages out after the short pause. `incremental`: only blocks drawn changed, as `layoutNow` says. */
  scheduleLayout(this: Editor, incremental = false): void {
    window.clearTimeout(this.layoutTimer);
    if (!incremental) this.pendingFull = true;
    this.layoutTimer = window.setTimeout(() => {
      if (this.composing) return this.scheduleLayout(true);
      this.layoutNow({ incremental: true });
    }, 120);
  },
};

declare module "./editor" {
  interface Editor {
    openTab(id: string): boolean;
    apply(out: RenderOut, typing?: boolean): void;
    afterApply(out: RenderOut, typing: boolean, kept: boolean, rendered: number[]): void;
    markTabs(followCaret?: boolean, only?: number[]): void;
    paintTabs(tabs: import("@docxtor/core").TabInfo[], open: string | null, only?: number[]): void;
    tabOfCaret(tabs: import("@docxtor/core").TabInfo[]): string | null;
    hiddenTabOf(p: number, tabs?: import("@docxtor/core").TabInfo[]): string | null;
    decorate(el: HTMLElement): void;
    refresh(opts?: { keepScroll?: boolean; quiet?: boolean; }): void;
    rerender(): void;
    reloadPageSetup(): void;
    applyOut(out: RenderOut, typing?: boolean): void;
    mirror(out: RenderOut, typing?: boolean): void;
    floatMetrics(page: PageSetup): void;
    reloadStories(fromPeer?: boolean, onlyIfChanged?: boolean): void;
    layoutNow(opts?: { incremental?: boolean; }): void;
    scheduleLayout(incremental?: boolean): void;
  }
}
