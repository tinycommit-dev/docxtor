// Far pages, the blocks left out of the layout: removing and drawing them, their boxes and their carried heights.
import type { Paginator } from "./layout";
import { MAX_DIRTY, cut, layoutUnits } from "./layout-units";
import { FAR, FAR_BOX, FAR_GAP, FAR_OWN, isFarBox, isTopBlock, nextBlock, prevBlock, topBlockOf } from "./far";
import type { BlockRec, Stretch } from "./layout-types";

export const farBoxMethods = {
  /** The editor drew a block: the next pass may start there. */
  drew(this: Paginator, el: HTMLElement) {
    if (this.allDirty) return;
    this.dirtyKids.add(el);
    if (this.dirtyKids.size > MAX_DIRTY) this.invalidate();
  },
  /** A block is about to leave the page: its neighbours are the ones whose place changes (the one after it, or the one before when it was the last). */
  removing(this: Paginator, el: HTMLElement) {
    if (this.farN) this.farRemoving(el);
    // The last block of a box takes the box with it.
    const box = el.parentElement;
    if (isFarBox(box) && box!.childElementCount === 1) box!.replaceWith(el);
    if (this.allDirty) return;
    const near = nextBlock(el) ?? prevBlock(el);
    if (near) this.drew(near);
    else this.invalidate();
  },
  /**
   * A block drawn again takes the place of another: the old one is no longer listed as drawn (a burst of typing in one paragraph draws it again
   * for every key, and the old elements would count towards the limit that makes the next layout a full one), and the height the old one carried
   * for far blocks before it, with its place in the records, goes to the new one.
   */
  carryFar(this: Paginator, el: HTMLElement, old: HTMLElement) {
    this.dirtyKids.delete(old);
    const s = this.byCarrier.get(old);
    if (s) {
      // The new block carries the stretch from now on; it has no record yet, so its gap is set when the layout has given it one.
      this.byCarrier.delete(old);
      s.carrier = el;
      this.byCarrier.set(el, s);
      this.farTouched.add(el);
    } else if (old.hasAttribute(FAR_GAP)) this.farLost();
  },
  /** A block was put into the flow (a new one, or one that moved): next to a stretch, or a stretch's own block, the records cannot follow it. */
  placed(this: Paginator, el: HTMLElement) {
    if (!this.farN) return;
    // Put next to a far block, it may have landed in that block's box: it is drawn, so it comes out (in its place in the order).
    if (!el.hasAttribute(FAR)) this.boxOut(el);
    if (el.hasAttribute(FAR) || this.byCarrier.has(el) || prevBlock(el)?.hasAttribute(FAR)) this.farLost();
  },
  /**
   * A block is about to leave the text. A far block leaves its stretch (the stretch's height stays as it was: the pass that follows starts at the
   * block before the gap, draws what it reaches of the stretch from the front and sets the gaps from what is left), and a block that carries a
   * stretch's height hands the stretch to the next block.
   */
  farRemoving(this: Paginator, el: HTMLElement) {
    if (!this.farIdx) return;
    this.farVisits++;
    if (el.hasAttribute(FAR)) {
      const s = this.stretchOf(el);
      if (!s) return this.farLost();
      if (s.first === el && s.last === el) {
        this.stretchDrop(s);
        this.farTouched.add(s.carrier);
      } else if (s.first === el) {
        const n = this.farSide(el, 1);
        if (!n) return this.farLost();
        this.byFirst.delete(el);
        s.first = n;
        this.byFirst.set(n, s);
        this.farTouched.add(s.carrier);
      } else if (s.last === el) {
        const p = this.farSide(el, -1);
        if (!p) return this.farLost();
        s.last = p;
      }
      this.farN--;
      this.farMoved = true;
    } else {
      const s = this.byCarrier.get(el);
      if (!s) return;
      const far = this.farSide(el, 1);
      if (far) {
        // Two stretches with one block between them: they are one now.
        const s2 = this.byFirst.get(far);
        if (!s2) return this.farLost();
        this.stretchDrop(s2);
        this.byCarrier.delete(el);
        s.last = s2.last;
        s.carrier = s2.carrier;
        this.byCarrier.set(s.carrier, s);
        this.farTouched.add(s.carrier);
      } else {
        const c = this.farCarrierAfter(el);
        if (!c) return this.farLost();
        this.byCarrier.delete(el);
        s.carrier = c;
        this.byCarrier.set(c, s);
        this.farTouched.add(c);
      }
    }
    this.farFlush(false);
  },
  /** Something other than drawn blocks changed: the next pass is a full one. */
  invalidate(this: Paginator) {
    this.allDirty = true;
    this.dirtyKids.clear();
  },
  /** What a block carries in its top margin for the blocks left out before it (px). */
  gapOf(this: Paginator, el: HTMLElement): number {
    const g = el.getAttribute(FAR_GAP);
    return g ? parseFloat(g) || 0 : 0;
  },
  /**
   * A block's own top margin (base and push, as the pass set it), without the gap it carries. A style's text keeps six digits, which is too few
   * for a gap of 30,000 px and for a push a thousandth short of a layout unit, so the exact value is kept beside the text it was set with
   * (`data-fm`, as `own|text`) and used while the style still says that text.
   */
  ownOf(this: Paginator, el: HTMLElement): number {
    const kept = el.getAttribute(FAR_OWN);
    if (kept) {
      const cut = kept.indexOf("|");
      if (kept.slice(cut + 1) === el.style.marginTop) return Number(kept.slice(0, cut));
    }
    return parseFloat(el.style.marginTop) || 0;
  },
  /** Set a block's own top margin: what it carries for blocks left out comes on top. */
  setOwn(this: Paginator, el: HTMLElement, own: number) {
    const gap = this.gapOf(el);
    el.style.marginTop = `${own + gap}px`;
    if (gap || Math.abs(own - (parseFloat(el.dataset.base || "0") || 0)) > 1 / 4096) el.setAttribute(FAR_OWN, `${own}|${el.style.marginTop}`);
    else el.removeAttribute(FAR_OWN);
  },
  /** What the pass pushed a block by: its own margin less its base. */
  pushOf(this: Paginator, el: HTMLElement): number {
    return this.ownOf(el) - (parseFloat(el.dataset.base || "0") || 0);
  },
  /** Where the block's margin box starts (flow px): its natural top less its base margin, plus what the text above pushed it by. */
  marginEdge(this: Paginator, el: HTMLElement, rec: BlockRec): number {
    // The base margin as the browser takes it: whole layout units, cut toward zero.
    return rec.cx - cut(parseFloat(el.dataset.base || "0") || 0) + rec.acc;
  },
  /** Can this block leave the layout? Not: the first and last, a wrapper of columns, a floating table, frame or object (placed from its anchor's rectangle), a block inside a content control. */
  canLeave(this: Paginator, kids: HTMLElement[], i: number): boolean {
    const el = kids[i];
    if (i === 0 || i === kids.length - 1 || !isTopBlock(this.flow, el) || !this.recs.get(el)) return false;
    if (el.dataset.fold || this.inHiddenTab(el)) return false;
    // What a block is made of does not change while it is the same element (an edit draws a new one).
    let can = this.leavable.get(el);
    if (can === undefined) {
      can = !(el.classList.contains("cols") || el.classList.contains("pfloat") || el.classList.contains("tfloat") || el.dataset.frame || el.querySelector(".atom[data-abs], .abs-frame"));
      this.leavable.set(el, can);
    }
    return can;
  },
  /** Does a block take room in the layout apart from being left out (not folded away, not in a tab that is not shown)? */
  takesRoom(this: Paginator, el: HTMLElement): boolean {
    return !el.dataset.fold && !this.inHiddenTab(el);
  },
  /**
   * Is the block in a tab that is not shown? The mark is on a block of the flow, so a block that is a child of the flow answers from its own
   * attribute (a search up the tree for each of thousands of blocks is most of the time a layout of the far pages takes); only a block inside
   * a content control looks above it.
   */
  inHiddenTab(this: Paginator, el: Element): boolean {
    return isTopBlock(this.flow, el) ? el.hasAttribute("data-tab-hidden") : !!el.closest("[data-tab-hidden]");
  },
  /**
   * Leave out exactly the blocks `hide` says (by position among the page's blocks), keeping the stretches' heights in the blocks after them.
   * A stretch whose ends the page map does not know is not left out. Returns whether anything changed. This is the long way: it goes over every
   * block and builds the records of the stretches again; what changes a few blocks goes through `farDraw` and `farHide` instead.
   */
  farApply(this: Paginator, kids: HTMLElement[], hide: boolean[]): boolean {
    const n = kids.length;
    this.farVisits += n;
    this.farLong++;
    const gaps = new Map<number, number>();
    const runs: Stretch[] = [];
    for (let i = 0; i < n; ) {
      if (!hide[i]) {
        i++;
        continue;
      }
      let last = i;
      let j = i + 1;
      while (j < n && (hide[j] || !this.takesRoom(kids[j]))) {
        if (hide[j]) last = j;
        j++;
      }
      const first = this.recs.get(kids[i]);
      const carrier = j < n ? this.recs.get(kids[j]) : undefined;
      const height = first && carrier ? layoutUnits(this.marginEdge(kids[j], carrier) - this.marginEdge(kids[i], first)) : -1;
      if (height < 0) for (let k = i; k <= last; k++) hide[k] = false;
      else {
        gaps.set(j, height);
        runs.push({ first: kids[i], last: kids[last], carrier: kids[j] });
      }
      i = j;
    }
    let changed = false;
    let count = 0;
    for (let i = 0; i < n; i++) {
      const el = kids[i];
      if (hide[i]) count++;
      if (hide[i] !== el.hasAttribute(FAR)) {
        changed = true;
        if (hide[i]) el.setAttribute(FAR, "");
        else el.removeAttribute(FAR);
      }
      if (this.setGap(el, gaps.get(i) ?? 0)) changed = true;
    }
    this.farN = count;
    this.boxSync(kids);
    this.stretches = runs;
    this.byFirst.clear();
    this.byCarrier.clear();
    for (const r of runs) {
      this.byFirst.set(r.first, r);
      this.byCarrier.set(r.carrier, r);
    }
    this.farTouched.clear();
    this.farIdx = true;
    if (changed) this.farMoved = true;
    return changed;
  },
  /** Make a block carry `want` (px) for the far blocks before it (nothing when 0). Returns whether it changed. */
  setGap(this: Paginator, el: HTMLElement, want: number): boolean {
    const cur = this.gapOf(el);
    if (Math.abs(want - cur) > 1 / 128 || (!want && el.hasAttribute(FAR_GAP))) {
      const own = this.ownOf(el);
      if (want) el.setAttribute(FAR_GAP, String(want));
      else el.removeAttribute(FAR_GAP);
      this.setOwn(el, own);
      return true;
    }
    return false;
  },
  /** As `farApply`, with `want` saying for each block whether it is left out. */
  farRestructure(this: Paginator, kids: HTMLElement[], want: (i: number, el: HTMLElement) => boolean): boolean {
    const hide: boolean[] = new Array(kids.length);
    for (let i = 0; i < kids.length; i++) hide[i] = want(i, kids[i]) && this.canLeave(kids, i);
    return this.farApply(kids, hide);
  },
  /** The records say nothing any more (or nothing that can be followed): what is left out is found from the blocks, the long way, next time. */
  farLost(this: Paginator) {
    this.farIdx = false;
  },
  /**
   * Move blocks with `fn`, keeping the browser's selection: a node that moves is taken out and put back, and a selection inside it would fall to
   * the place it left (the caret on a block just drawn out of its box would end up on the flow).
   */
  keepingSelection(this: Paginator, scope: Element, fn: () => void) {
    const s = scope.ownerDocument.getSelection();
    const a = s?.anchorNode;
    const f = s?.focusNode;
    // Only a selection in a block is kept: one on the flow itself (between blocks, after the block it was in went) is the editor's to put right.
    const inBlock = (n: Node) => n !== this.flow && scope.contains(n);
    const kept = s && a && f && (inBlock(a) || inBlock(f)) ? ([a, s.anchorOffset, f, s.focusOffset] as const) : null;
    fn();
    // Put back only when the move lost it (setting it costs a walk of the flow for the input method).
    if (kept && (s!.anchorNode !== kept[0] || s!.anchorOffset !== kept[1] || s!.focusNode !== kept[2] || s!.focusOffset !== kept[3])) {
      try {
        s!.setBaseAndExtent(...kept);
      } catch {
        /* a node went */
      }
    }
  },
  newBox(this: Paginator): HTMLElement {
    const b = document.createElement("div");
    b.className = FAR_BOX;
    b.setAttribute(FAR, "");
    return b;
  },
  /** A block of the flow that is now left out goes into the box next to it, or into a new one in its place. */
  boxIn(this: Paginator, el: HTMLElement) {
    this.keepingSelection(el, () => this.boxInNow(el));
  },
  boxInNow(this: Paginator, el: HTMLElement) {
    const p = el.previousElementSibling;
    const n = el.nextElementSibling;
    if (isFarBox(p)) p!.append(el);
    else if (isFarBox(n)) n!.prepend(el);
    else {
      const b = this.newBox();
      el.before(b);
      b.append(el);
    }
  },
  /**
   * A block in a box comes out, keeping its place in the order: before the box when it is the first, after it when the last, and from the middle
   * the smaller side of the box goes to a new box (work in proportion to that side, which the bookkeeping counts as blocks looked at).
   */
  boxOut(this: Paginator, el: HTMLElement) {
    const b = el.parentElement;
    if (!b || !isFarBox(b)) return;
    this.keepingSelection(b, () => this.boxOutNow(el, b));
  },
  boxOutNow(this: Paginator, el: HTMLElement, b: HTMLElement) {
    if (!el.previousElementSibling) b.before(el);
    else if (!el.nextElementSibling) b.after(el);
    else {
      let l: Element | null = el.previousElementSibling;
      let r: Element | null = el.nextElementSibling;
      while (l && r) {
        l = l.previousElementSibling;
        r = r.nextElementSibling;
      }
      const nb = this.newBox();
      if (!l) {
        b.before(nb);
        while (b.firstElementChild !== el) {
          this.farVisits++;
          nb.append(b.firstElementChild!);
        }
        b.before(el);
      } else {
        b.after(nb);
        while (b.lastElementChild !== el) {
          this.farVisits++;
          nb.prepend(b.lastElementChild!);
        }
        b.after(el);
      }
    }
    if (!b.firstElementChild) b.remove();
  },
  /**
   * Put the boxes right for the blocks as they are marked (with `farApply`, the long way): each run of far blocks of the flow in one box, every
   * other block out of the boxes, in the order of `kids` (the page's blocks), and nothing moved that is already where it belongs.
   */
  boxSync(this: Paginator, kids: HTMLElement[]) {
    this.keepingSelection(this.flow, () => this.boxSyncNow(kids));
  },
  boxSyncNow(this: Paginator, kids: HTMLElement[]) {
    const flow = this.flow;
    // The two ways a whole pass changes everything, a run in one call: nothing left out (every box gives its blocks back), and the first stretch
    // after everything was drawn (no box yet: each run of far blocks goes into a new one).
    if (!this.farN) {
      for (let k = flow.firstElementChild; k; ) {
        const n = k.nextElementSibling;
        if (isFarBox(k)) k.replaceWith(...Array.from(k.childNodes));
        k = n;
      }
      return;
    }
    if (!flow.querySelector(`:scope > .${FAR_BOX}`)) {
      let run: HTMLElement[] = [];
      const close = () => {
        if (!run.length) return;
        const b = this.newBox();
        run[0].before(b);
        b.append(...run);
        run = [];
      };
      for (const el of kids) {
        if (el.parentElement === flow && el.hasAttribute(FAR)) run.push(el);
        else close();
      }
      close();
      return;
    }
    let at: Element | null = null;
    let box: HTMLElement | null = null;
    let inBox: Element | null = null;
    const used = new Set<Element>();
    const place = (unit: Element) => {
      if ((at ? at.nextElementSibling : flow.firstElementChild) !== unit) {
        if (at) at.after(unit);
        else flow.prepend(unit);
      }
      at = unit;
    };
    for (const el of kids) {
      const p = el.parentElement;
      const top = p === flow || isFarBox(p);
      if (top && el.hasAttribute(FAR)) {
        if (!box) {
          box = isFarBox(p) && !used.has(p!) ? p! : this.newBox();
          used.add(box);
          place(box);
          inBox = null;
        }
        if ((inBox ? inBox.nextElementSibling : box.firstElementChild) !== el) {
          if (inBox) inBox.after(el);
          else box.prepend(el);
        }
        inBox = el;
        continue;
      }
      box = null;
      // A block inside a content control: its wrapper is the flow's child, put in place once.
      const unit = top ? el : topBlockOf(flow, el);
      if (!unit || unit === at) continue;
      place(unit);
    }
    // What is left of the boxes is empty: their blocks went to other boxes or out.
    for (let k = flow.firstElementChild; k; ) {
      const n = k.nextElementSibling;
      if (isFarBox(k) && !k.firstElementChild) k.remove();
      k = n;
    }
  },
  /** Every block is drawn again, exactly as the last pass laid it out. Returns whether any was left out. */
  farShowAll(this: Paginator): boolean {
    if (!this.farN) {
      this.stretches = [];
      this.byFirst.clear();
      this.byCarrier.clear();
      this.farTouched.clear();
      this.farIdx = true;
      return false;
    }
    const kids = this.pageBlocks();
    this.farApply(kids, new Array(kids.length).fill(false));
    return true;
  },
  /** The text was thrown away (a new document is drawn): nothing is left out, and nothing is recorded. */
  farForget(this: Paginator) {
    this.farN = 0;
    this.stretches = [];
    this.byFirst.clear();
    this.byCarrier.clear();
    this.farTouched.clear();
    this.farIdx = true;
  },
};

declare module "./layout" {
  interface Paginator {
    drew(el: HTMLElement): void;
    removing(el: HTMLElement): void;
    carryFar(el: HTMLElement, old: HTMLElement): void;
    placed(el: HTMLElement): void;
    farRemoving(el: HTMLElement): void;
    invalidate(): void;
    gapOf(el: HTMLElement): number;
    ownOf(el: HTMLElement): number;
    setOwn(el: HTMLElement, own: number): void;
    pushOf(el: HTMLElement): number;
    marginEdge(el: HTMLElement, rec: BlockRec): number;
    canLeave(kids: HTMLElement[], i: number): boolean;
    takesRoom(el: HTMLElement): boolean;
    inHiddenTab(el: Element): boolean;
    farApply(kids: HTMLElement[], hide: boolean[]): boolean;
    setGap(el: HTMLElement, want: number): boolean;
    farRestructure(kids: HTMLElement[], want: (i: number, el: HTMLElement) => boolean): boolean;
    farLost(): void;
    keepingSelection(scope: Element, fn: () => void): void;
    newBox(): HTMLElement;
    boxIn(el: HTMLElement): void;
    boxInNow(el: HTMLElement): void;
    boxOut(el: HTMLElement): void;
    boxOutNow(el: HTMLElement, b: HTMLElement): void;
    boxSync(kids: HTMLElement[]): void;
    boxSyncNow(kids: HTMLElement[]): void;
    farShowAll(): boolean;
    farForget(): void;
  }
}
