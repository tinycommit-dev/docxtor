// Far pages, continued: stretches of blocks, drawing and hiding them, and choosing which pages are shown.
import type { Paginator } from "./layout";
import type { BlockRec, Stretch } from "./layout-types";
import { FAR, firstBlock, isFarBox, isTopBlock, lastBlock, nextBlock, prevBlock, topBlockOf } from "./far";
import { layoutUnit, layoutUnits } from "./layout-units";

export const farMethods = {
  /** Where a block's margin box starts, from the page map (undefined when it has no record). */
  edgeOf(this: Paginator, el: HTMLElement): number | undefined {
    const r = this.recs.get(el);
    return r ? this.marginEdge(el, r) : undefined;
  },
  /** The stretch a block left out of the layout is in: the one whose first block does not start after it and whose last block does not end before it. Undefined when the records cannot say. */
  stretchOf(this: Paginator, el: HTMLElement): Stretch | undefined {
    const e = this.edgeOf(el);
    if (e === undefined) return undefined;
    const list = this.stretches;
    let lo = 0;
    let hi = list.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const f = this.edgeOf(list[mid].first);
      if (f === undefined) return undefined;
      if (f <= e + 1 / 256) lo = mid + 1;
      else hi = mid;
    }
    // Blocks that stand at one place (no height) can tie: the nearest few before the place are looked at.
    for (let i = lo - 1; i >= 0 && i >= lo - 3; i--) {
      const s = list[i];
      if (s.first === el || s.last === el) return s;
      const l = this.edgeOf(s.last);
      if (l !== undefined && e <= l + 1 / 256) return s;
    }
    return undefined;
  },
  /** The place of a stretch in the list. */
  stretchAt(this: Paginator, s: Stretch): number {
    const e = this.edgeOf(s.first);
    if (e !== undefined) {
      const list = this.stretches;
      let lo = 0;
      let hi = list.length;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        const f = this.edgeOf(list[mid].first);
        if (f === undefined) break;
        if (f < e - 1 / 256) lo = mid + 1;
        else hi = mid;
      }
      for (let i = lo; i < list.length && i < lo + 3; i++) if (list[i] === s) return i;
    }
    return this.stretches.indexOf(s);
  },
  stretchAdd(this: Paginator, s: Stretch) {
    const e = this.edgeOf(s.first) ?? 0;
    const list = this.stretches;
    let lo = 0;
    let hi = list.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if ((this.edgeOf(list[mid].first) ?? 0) <= e) lo = mid + 1;
      else hi = mid;
    }
    list.splice(lo, 0, s);
    this.byFirst.set(s.first, s);
    this.byCarrier.set(s.carrier, s);
  },
  stretchDrop(this: Paginator, s: Stretch) {
    const at = this.stretchAt(s);
    if (at >= 0) this.stretches.splice(at, 1);
    if (this.byFirst.get(s.first) === s) this.byFirst.delete(s.first);
    if (this.byCarrier.get(s.carrier) === s) this.byCarrier.delete(s.carrier);
  },
  /**
   * The nearest block of the run of `el` in a direction, going over the blocks that take no room (folded away, in a tab not shown): the next block
   * left out, or null when the first block that takes room comes first (and `farBail` says whether something the records cannot follow came instead).
   */
  farSide(this: Paginator, el: HTMLElement, dir: 1 | -1): HTMLElement | null {
    let n = dir > 0 ? nextBlock(el) : prevBlock(el);
    while (n) {
      this.farVisits++;
      if (n.hasAttribute(FAR)) return n;
      if (n.classList.contains("fc") || this.takesRoom(n)) return null;
      n = dir > 0 ? nextBlock(n) : prevBlock(n);
    }
    return null;
  },
  /** The first block after `el` that takes room in the layout and is drawn (null when there is none, or a block-level content control comes first). */
  farCarrierAfter(this: Paginator, el: HTMLElement): HTMLElement | null {
    let n = nextBlock(el);
    while (n) {
      this.farVisits++;
      if (n.hasAttribute(FAR) || n.classList.contains("fc")) return null;
      if (this.takesRoom(n)) return n;
      n = nextBlock(n);
    }
    return null;
  },
  /** Can this block (a child of the flow) leave the layout? As `canLeave`, for one block, without the list of blocks. */
  canLeaveEl(this: Paginator, el: HTMLElement): boolean {
    this.farVisits++;
    if (!isTopBlock(this.flow, el) || el === firstBlock(this.flow) || el === lastBlock(this.flow) || !this.recs.get(el)) return false;
    if (el.dataset.fold || this.inHiddenTab(el)) return false;
    let can = this.leavable.get(el);
    if (can === undefined) {
      can = !(el.classList.contains("cols") || el.classList.contains("pfloat") || el.classList.contains("tfloat") || el.dataset.frame || el.querySelector(".atom[data-abs], .abs-frame"));
      this.leavable.set(el, can);
    }
    return can;
  },
  /**
   * Draw one block that is left out. Its stretch is cut in two around it (or loses an end, or goes): only the block, the stretch's ends and the blocks
   * that carry heights change. False when the records cannot say where the block is (the caller does it the long way).
   */
  farDraw(this: Paginator, el: HTMLElement): boolean {
    this.farVisits++;
    const s = this.farIdx ? this.stretchOf(el) : undefined;
    if (!s) return false;
    let prev: HTMLElement | null = null;
    let next: HTMLElement | null = null;
    if (s.first !== el) {
      prev = this.farSide(el, -1);
      if (!prev) return this.farFail();
    }
    if (s.last !== el) {
      next = this.farSide(el, 1);
      if (!next) return this.farFail();
    }
    el.removeAttribute(FAR);
    this.boxOut(el);
    this.farN--;
    this.farMoved = true;
    const c = s.carrier;
    this.farTouched.add(c);
    this.farTouched.add(el);
    if (prev && next) {
      const at = this.stretchAt(s);
      const left: Stretch = { first: s.first, last: prev, carrier: el };
      const right: Stretch = { first: next, last: s.last, carrier: c };
      this.stretches.splice(at, 1, left, right);
      this.byFirst.set(s.first, left);
      this.byFirst.set(next, right);
      this.byCarrier.set(el, left);
      this.byCarrier.set(c, right);
    } else if (prev) {
      this.byCarrier.delete(c);
      s.last = prev;
      s.carrier = el;
      this.byCarrier.set(el, s);
    } else if (next) {
      this.byFirst.delete(el);
      s.first = next;
      this.byFirst.set(next, s);
    } else this.stretchDrop(s);
    return true;
  },
  farFail(this: Paginator): false {
    this.farLost();
    return false;
  },
  /**
   * Leave out one block that is drawn: it joins the stretch before it and the one after it (the records of both are joined). False when the block
   * stays (it cannot leave, or the ends of its stretch are not known to the page map).
   */
  farHide(this: Paginator, el: HTMLElement): boolean {
    if (!this.canLeaveEl(el)) return false;
    const left = this.farSide(el, -1);
    const right = this.farSide(el, 1);
    const sl = left ? this.byCarrier.get(el) : undefined;
    const sr = right ? this.byFirst.get(right) : undefined;
    if ((left && (!sl || sl.last !== left)) || (right && (!sr || sr.first !== right))) return this.farFail();
    const carrier = sr ? sr.carrier : this.farCarrierAfter(el);
    const first = sl ? sl.first : el;
    if (!carrier || !this.recs.get(first) || !this.recs.get(carrier)) return false;
    el.setAttribute(FAR, "");
    this.boxIn(el);
    this.farN++;
    this.farMoved = true;
    this.farTouched.add(el);
    this.farTouched.add(carrier);
    if (sl && sr) {
      this.byCarrier.delete(el);
      this.byFirst.delete(sr.first);
      this.byCarrier.delete(sr.carrier);
      const at = this.stretchAt(sr);
      if (at >= 0) this.stretches.splice(at, 1);
      sl.last = sr.last;
      sl.carrier = sr.carrier;
      this.byCarrier.set(sl.carrier, sl);
    } else if (sl) {
      this.byCarrier.delete(el);
      this.byCarrier.delete(sl.carrier);
      sl.last = el;
      sl.carrier = carrier;
      this.byCarrier.set(carrier, sl);
    } else if (sr) {
      this.byFirst.delete(sr.first);
      sr.first = el;
      this.byFirst.set(el, sr);
    } else this.stretchAdd({ first: el, last: el, carrier });
    return true;
  },
  /** Set the gaps of the blocks whose stretch changed (a stretch the page map cannot measure is drawn). */
  farFlush(this: Paginator, check = true) {
    if (!this.farTouched.size) {
      if (check && this.verify) this.farCheckRecords();
      return;
    }
    const touched = [...this.farTouched];
    this.farTouched.clear();
    for (const el of touched) {
      if (!el.isConnected) continue;
      const s = this.byCarrier.get(el);
      let want = 0;
      if (s) {
        const a = this.recs.get(s.first);
        const b = this.recs.get(el);
        const h = a && b ? layoutUnits(this.marginEdge(el, b) - this.marginEdge(s.first, a)) : -1;
        // A carrier the layout has not reached yet (a block drawn again) is looked at again after it.
        if (!b && !this.mapReady) {
          this.farTouched.add(el);
          continue;
        }
        if (h < 0) {
          this.farDissolve(s);
          continue;
        }
        want = h;
      }
      this.setGap(el, want);
    }
    if (check && this.verify) this.farCheckRecords();
  },
  /** A stretch whose height cannot be told is not left out: every block of it is drawn. */
  farDissolve(this: Paginator, s: Stretch) {
    this.stretchDrop(s);
    for (let el: HTMLElement | null = s.first; el; ) {
      this.farVisits++;
      const next: HTMLElement | null = el === s.last ? null : nextBlock(el);
      if (el.hasAttribute(FAR)) {
        el.removeAttribute(FAR);
        this.boxOut(el);
        this.farN--;
      }
      el = next;
    }
    this.farMoved = true;
    this.setGap(s.carrier, 0);
  },
  /**
   * Tests and development: the records are what the blocks say (the stretches, the gaps, the count). Throws when they are not.
   */
  farCheckRecords(this: Paginator) {
    if (!this.farIdx) return;
    const kids = this.pageBlocks();
    const want: Stretch[] = [];
    const n = kids.length;
    let count = 0;
    for (let i = 0; i < n; ) {
      if (!kids[i].hasAttribute(FAR)) {
        i++;
        continue;
      }
      let last = i;
      let j = i + 1;
      while (j < n && (kids[j].hasAttribute(FAR) || !this.takesRoom(kids[j]))) {
        if (kids[j].hasAttribute(FAR)) last = j;
        j++;
      }
      if (j >= n) throw new Error("Docxtor: far pages: a stretch has no block after it");
      want.push({ first: kids[i], last: kids[last], carrier: kids[j] });
      i = j;
    }
    for (const k of kids) if (k.hasAttribute(FAR)) count++;
    const same = want.length === this.stretches.length && want.every((w, i) => w.first === this.stretches[i].first && w.last === this.stretches[i].last && w.carrier === this.stretches[i].carrier);
    const gaps = new Set(want.map((w) => w.carrier));
    let gapsOk = true;
    // The heights are exact when the page map describes the text (while a layout runs, the blocks of a stretch that lost a block still hold its height).
    if (this.mapReady) {
      for (const k of kids) {
        const g = this.gapOf(k);
        if (!gaps.has(k) ? g !== 0 : Math.abs(g - this.stretchHeight(want.find((w) => w.carrier === k)!)) > layoutUnit()) gapsOk = false;
      }
    }
    // The boxes: every far block is in one, a box holds only far blocks, and none is empty.
    for (const k of kids) if (k.hasAttribute(FAR) !== isFarBox(k.parentElement)) throw new Error(`Docxtor: far pages: a block ${k.hasAttribute(FAR) ? "left out is not in a box" : "drawn is in a box"}`);
    for (let k = this.flow.firstElementChild; k; k = k.nextElementSibling) if (isFarBox(k) && !k.firstElementChild) throw new Error("Docxtor: far pages: an empty box");
    if (!same || count !== this.farN || !gapsOk || this.byFirst.size !== want.length || this.byCarrier.size !== want.length) {
      throw new Error(`Docxtor: far pages: the records of the stretches differ from the blocks (${this.stretches.length} stretches against ${want.length}, ${this.farN} blocks against ${count}, gaps ${gapsOk ? "right" : "wrong"})`);
    }
  },
  stretchHeight(this: Paginator, s: Stretch): number {
    const a = this.recs.get(s.first);
    const b = this.recs.get(s.carrier);
    return a && b ? layoutUnits(this.marginEdge(s.carrier, b) - this.marginEdge(s.first, a)) : 0;
  },
  /** Draw these blocks (those of them that are left out). */
  farDrawAll(this: Paginator, list: Iterable<HTMLElement>): void {
    const todo = [...list].filter((e) => e.hasAttribute(FAR));
    if (!todo.length) return;
    let at = 0;
    while (at < todo.length && this.farDraw(todo[at])) at++;
    if (at < todo.length) {
      const rest = new Set(todo.slice(at));
      this.farRestructure(this.pageBlocks(), (_, e) => e.hasAttribute(FAR) && !rest.has(e));
    }
    this.farFlush();
  },
  /** Draw the top-level block holding `el` if it is left out of the layout (the rest of its stretch stays out). */
  showBlock(this: Paginator, el: Element) {
    if (!this.farN) return;
    const block = this.blockOf(el);
    if (!block?.hasAttribute(FAR)) return;
    this.farDrawAll([block]);
  },
  /** Draw the blocks that start or end on pages `from` to `to`, and those holding any of `blocks`. */
  showPages(this: Paginator, from: number, to: number) {
    if (!this.farN || !this.mapValid) return;
    if (this.mapReady && this.farIdx) {
      const want: HTMLElement[] = [];
      for (const e of this.blocksOnPages(from, to)) {
        this.farVisits++;
        const r = this.recs.get(e);
        if (e.hasAttribute(FAR) && r && r.last >= from && r.page <= to) want.push(e);
      }
      this.farDrawAll(want);
      return;
    }
    this.farRestructure(this.pageBlocks(), (_, e) => {
      if (!e.hasAttribute(FAR)) return false;
      const r = this.recs.get(e);
      return !r || r.last < from || r.page > to;
    });
  },
  /**
   * Leave out every block that is not on one of `shown` (page ranges, inclusive) and is not one of `keep`; draw the rest. Does nothing
   * until a pass has described the flow and nothing has been drawn since. Returns whether anything changed.
   */
  setFar(this: Paginator, shown: ReadonlyArray<readonly [number, number]> | null, keep: ReadonlySet<HTMLElement>): boolean {
    if (!this.mapValid || this.allDirty || this.dirtyKids.size) return false;
    if (!shown) return this.farShowAll();
    const on = (r: BlockRec) => shown.some(([lo, hi]) => r.last >= lo && r.page <= hi);
    // Nothing is left out yet, or the records are not to be trusted: every block changes, or is looked at: the long way.
    if (!this.farN || !this.farIdx) return this.farSetLong(shown, keep);
    this.farMoved = false;
    let changed = false;
    // Blocks to draw: those of the pages shown, and those kept, that are left out now.
    const draw = new Set<HTMLElement>();
    for (const el of keep) if (el.hasAttribute(FAR)) draw.add(el);
    for (const [lo, hi] of shown) {
      for (const el of this.blocksOnPages(lo, hi)) {
        this.farVisits++;
        const r = this.recs.get(el);
        if (el.hasAttribute(FAR) && r && r.last >= lo && r.page <= hi) draw.add(el);
      }
    }
    if (draw.size) {
      const before = this.farN;
      this.farDrawAll(draw);
      if (this.farN !== before) changed = true;
    }
    if (!this.farIdx) return this.farSetLong(shown, keep) || changed;
    // Blocks to leave out: the drawn ones (the stretches are jumped over) that are on no page shown and not held.
    const hide: HTMLElement[] = [];
    for (let el = firstBlock(this.flow); el; ) {
      this.farVisits++;
      if (el.hasAttribute(FAR)) {
        const s = this.byFirst.get(el);
        if (!s) return this.farSetLong(shown, keep) || changed;
        const c = topBlockOf(this.flow, s.carrier);
        if (!c) return this.farSetLong(shown, keep) || changed;
        el = c;
        continue;
      }
      const r = this.recs.get(el);
      if (r && !keep.has(el) && !on(r)) hide.push(el);
      el = nextBlock(el);
    }
    for (const el of hide) {
      if (this.farHide(el)) changed = true;
      else if (!this.farIdx) return this.farSetLong(shown, keep) || changed;
    }
    this.farFlush();
    this.farMoved = false;
    return changed;
  },
  /** `setFar` the long way: every block is looked at, and the records are built again. */
  farSetLong(this: Paginator, shown: ReadonlyArray<readonly [number, number]>, keep: ReadonlySet<HTMLElement>): boolean {
    const kids = this.pageBlocks();
    const changed = this.farApply(
      kids,
      kids.map((el, i) => {
        if (!this.canLeave(kids, i) || keep.has(el)) return false;
        const r = this.recs.get(el)!;
        return !shown.some(([lo, hi]) => r.last >= lo && r.page <= hi);
      }),
    );
    this.farMoved = false;
    return changed;
  },
  /**
   * Is what a layout left of the drawn stretch as `setFar` would make it, for the blocks its passes went over? A layout that went over only
   * blocks that are still where they were in the drawn stretch changes nothing, and the rest of the text is as it was.
   */
  farKept(this: Paginator, shown: ReadonlyArray<readonly [number, number]>, keep: ReadonlySet<HTMLElement>): boolean {
    if (!this.touched || !this.mapValid || this.allDirty || this.dirtyKids.size) return false;
    const kids = this.passKids;
    const [a, b] = this.touched;
    if (this.farMoved || this.flow.childElementCount !== this.passCount) return false;
    for (let i = Math.max(0, a - 1); i <= Math.min(kids.length - 1, b + 1); i++) {
      this.farVisits++;
      const el = kids[i];
      const r = this.recs.get(el);
      const hide = this.canLeave(kids, i) && !keep.has(el) && !!r && !shown.some(([lo, hi]) => r.last >= lo && r.page <= hi);
      if (hide !== el.hasAttribute(FAR)) return false;
    }
    return true;
  },
  /** The pages the top-level block holding `el` covers, from the page map (undefined when it does not know). */
  pagesOf(this: Paginator, el: Element): [number, number] | undefined {
    if (!this.mapValid || this.allDirty) return undefined;
    const r = this.recOf(el);
    return r ? [r.page, r.last] : undefined;
  },
  /** The top-level block that holds `el` (null when `el` is not in the text). */
  blockOf(this: Paginator, el: Element): HTMLElement | null {
    return topBlockOf(this.flow, el);
  },
};

declare module "./layout" {
  interface Paginator {
    edgeOf(el: HTMLElement): number | undefined;
    stretchOf(el: HTMLElement): Stretch | undefined;
    stretchAt(s: Stretch): number;
    stretchAdd(s: Stretch): void;
    stretchDrop(s: Stretch): void;
    farSide(el: HTMLElement, dir: 1 | -1): HTMLElement | null;
    farCarrierAfter(el: HTMLElement): HTMLElement | null;
    canLeaveEl(el: HTMLElement): boolean;
    farDraw(el: HTMLElement): boolean;
    farFail(): false;
    farHide(el: HTMLElement): boolean;
    farFlush(check?: boolean): void;
    farDissolve(s: Stretch): void;
    farCheckRecords(): void;
    stretchHeight(s: Stretch): number;
    farDrawAll(list: Iterable<HTMLElement>): void;
    showBlock(el: Element): void;
    showPages(from: number, to: number): void;
    setFar(shown: ReadonlyArray<readonly [number, number]> | null, keep: ReadonlySet<HTMLElement>): boolean;
    farSetLong(shown: ReadonlyArray<readonly [number, number]>, keep: ReadonlySet<HTMLElement>): boolean;
    farKept(shown: ReadonlyArray<readonly [number, number]>, keep: ReadonlySet<HTMLElement>): boolean;
    pagesOf(el: Element): [number, number] | undefined;
    blockOf(el: Element): HTMLElement | null;
  }
}
