// The layout pass: placing every block on its page.
import type { Paginator } from "./layout";
import { layoutTabs } from "./tabs";
import { FAR, isFarBox } from "./far";
import { COLS_ROOM, ROOM_EPS, StartOver, cut, layoutUnits, snap } from "./layout-units";
import type { BlockRec, Given } from "./layout-types";

export const passMethods = {
  /**
   * One pass over the blocks: where each goes on the pages (a push as a top margin, a cut as a spacer). With `from` null it is
   * the whole text: everything is cleared and measured, as it always was. With `from` the blocks before `at` stay as they are;
   * from there on, blocks are cleared and measured a few at a time as the pass reaches them, and the pass ends at the first
   * block it reaches in the state the last pass had (see `layout`), leaving everything after it untouched.
   *
   * The blocks before `at` carry the pushes and spacers of the last pass, which move the blocks after them. The block at `at`
   * is given a negative top margin that takes exactly that away while the pass measures, so the measurements are those of a
   * cleared text, and the accounting (`push`) starts from what the last pass had moved by then. At the end the margin is
   * the one the pass asked for. A pass that has stopped at a block leaves the same margins and spacers a full pass would.
   */
  pass(this: Paginator, from: { at: number; maxDirty: number } | null = null) {
    const inc = !!from;
    if (!inc) {
      // A pass from the top lays everything out: what was left out of the layout is drawn first, and the editor leaves it out again after.
      this.farShowAll();
      this.clear();
      this.arrange();
    }
    const kids = this.pageBlocks();
    this.ix = this.sectionIndex(kids);
    const ix = this.ix;
    this.applySides(kids);
    this.padSectionEnds(kids, ix);
    // Tabs are as wide as their stops ask, which can change how lines break: do it before anything is measured.
    if (!inc) layoutTabs(this.flow, this.defaultTab);
    const n = kids.length;
    const start = from?.at ?? 0;
    const maxDirty = from?.maxDirty ?? n;
    const sc = this.scale();
    const oldBoxes = this.boxes;
    const oldFirst = this.firstOnPage;
    const oldReserve = this.passReserve;
    if (!inc) {
      this.boxes = [];
      this.seen = [];
      this.cur = 0;
    }
    this.onPage = [];
    this.pageEnd = [];
    this.dirty = new Set();
    this.queued = [];
    this.queuedH = 0;
    const stats = this.stats;
    if (inc) stats.incPasses++;
    else stats.fullPasses++;
    stats.lastIncremental = inc;
    stats.lastMeasured = 0;
    const tops: number[] = new Array(n);
    const hs: number[] = new Array(n);
    const bases: number[] = new Array(n);
    const pushes = new Array(n).fill(0);
    // The pending pushes and spacers (`push`, `live`), the page, and whether a page break is due: where the pass stands.
    let push = 0; // pending margin pushes (applied at the end)
    let live = 0; // spacer heights already inserted into the DOM
    let page = 0;
    let forceNext = false;
    // The first block that starts on each page or a later one.
    const first: HTMLElement[] = [];
    let filled = -1;
    // How far what is above the block the pass starts at moves it (nothing, from the top of the text).
    let away = 0;
    let startAt = 0;
    if (inc && start > 0) {
      const rec = this.recs.get(kids[start])!;
      const s = kids[start];
      // The block the pass starts at is measured where it is: it is drawn first.
      if (this.farN && kids[start].hasAttribute(FAR)) this.farDrawAll([kids[start]]);
      const here = this.topOf(s, undefined, sc) - this.pushOf(s);
      away = snap(here - rec.cx);
      startAt = rec.cx;
      this.lastPlan += `{start block ${start}: old cx ${rec.cx}, now ${here}, away ${away}, margin ${s.style.marginTop} base ${s.dataset.base}} `;
      page = rec.entryPage;
      push = rec.acc;
      forceNext = rec.force;
      const before = this.recs.get(kids[start - 1]);
      filled = before ? before.page : -1;
      for (let k = 0; k <= filled; k++) first[k] = oldFirst[k];
    }
    this.reserveDiff = inc ? this.lastDifference(oldReserve) : -1;
    // Blocks are cleared and measured from `measured` on, in batches that double (a full pass does the whole text in one).
    let measured = inc ? start : 0;
    let batch = 4;
    const reach = (upto: number) => {
      while (measured <= upto && measured < n) {
        const a = measured;
        const b = inc ? Math.min(n, Math.max(upto + 1, a + batch)) : n;
        batch = Math.min(256, batch * 2);
        if (inc) {
          // Blocks left out of the layout are drawn before they are measured (what is left of a stretch of them keeps its height).
          if (this.farN) {
            const far: HTMLElement[] = [];
            for (let j = a; j < b; j++) if (kids[j].hasAttribute(FAR)) far.push(kids[j]);
            if (far.length) this.farDrawAll(far);
          }
          this.clearKids(kids, a, b);
          // The block's own margin is what the browser takes it to be (whole layout units, cut toward zero) less what the text above moves it by.
          if (a === start && away) this.setOwn(kids[a], cut(parseFloat(kids[a].dataset.base || "0") || 0) - away);
        }
        const measure = () => {
          // The flow can move in the window when the text gets shorter (the scroll is cut back): read where it is with the blocks.
          const ft = this.flow.getBoundingClientRect().top;
          for (let j = a; j < b; j++) {
            const r = kids[j].getBoundingClientRect();
            bases[j] = parseFloat(kids[j].dataset.base || "0") || 0;
            // A block that is not drawn (folded away, in a tab not shown) has no place: the browser reports the window's top, so its
            // record would follow the scroll. It stands where the block before it ends (where it was, for the first block of a pass
            // that starts part-way down), the same in every pass.
            if (!r.height && !r.width && !kids[j].getClientRects().length) {
              hs[j] = 0;
              // The block before, when this pass has measured it (an earlier batch of the same pass too), as a pass from the top does.
              if (j > 0 && tops[j - 1] !== undefined) tops[j] = tops[j - 1] + hs[j - 1];
              else if (inc && j > 0) tops[j] = this.recs.get(kids[j])?.cx ?? 0;
              else tops[j] = 0;
              continue;
            }
            tops[j] = snap((r.top - ft) / sc) - (live - this.queuedH);
            hs[j] = snap(r.height / sc);
          }
        };
        measure();
        if (inc && a === start && start > 0) {
          // The first block must be where it was. A margin above it (the block before it keeps one below) can take the negative margin
          // for less than it says: put it right by what is left over. Far down a long text the browser reports a position to a float's
          // precision (a thirty-second of a pixel from 262,144 px down, a sixteenth from 524,288 px), coarser than the layout unit it
          // is made of, so one correction by the difference measured can land a step to the other side: when it does, the margin is
          // searched between the two sides, a layout unit apart at the end.
          let off = tops[a] - startAt;
          let lo: number | null = null;
          let hi: number | null = null;
          for (let tries = 0; Math.abs(off) > 1 / 128; tries++) {
            const own = this.ownOf(kids[a]);
            if (off < 0) lo = own;
            else hi = own;
            const next = lo !== null && hi !== null ? (lo + hi) / 2 : own - off;
            if (tries >= 8 || next === own || (lo !== null && hi !== null && hi - lo < 1 / (64 * sc) / 4)) throw new StartOver("the block to start at could not be put back where it was");
            this.setOwn(kids[a], next);
            measure();
            off = tops[a] - startAt;
          }
        }
        stats.measured += b - a;
        stats.lastMeasured += b - a;
        measured = b;
      }
    };
    // Room for notes as the pages fill: the notes' heights are measured first (measuring one is a change to the page, so not among the
    // blocks' measures), and the references of each block are found once.
    const notesIn = this.roomInPass && !inc ? this.notesForPass(kids) : null;
    if (notesIn) {
      this.reserve = [];
      this.slack = [];
    }
    const room: number[] = [];
    const given: Given[] = [];
    if (!inc) reach(n);
    let sync = -1;
    let prevEl: HTMLElement | undefined;
    let prevRec: BlockRec | undefined;
    const finish = (el: HTMLElement | undefined, rec: BlockRec | undefined) => {
      if (!el || !rec) return;
      rec.last = Math.max(rec.page, page);
      while (filled < rec.page) first[++filled] = el;
    };

    for (let i = start; i < n; i++) {
      const el = kids[i];
      finish(prevEl, prevRec);
      let rec = this.recs.get(el);
      if (i >= measured) {
        // A block not cleared yet (nothing is cleared that the pass may not need): where is it, if it is where the last pass had it we are done.
        if (i > maxDirty && rec) {
          // A block left out of the layout is drawn for a moment to see where it stands (its top does not depend on what is after it).
          // In a box, the box lets its blocks into the layout for that moment (`display: contents`); the others keep their mark and stay out.
          const left = el.hasAttribute(FAR);
          const box = left && isFarBox(el.parentElement) ? el.parentElement! : null;
          if (left) el.removeAttribute(FAR);
          if (box) box.style.display = "contents";
          const raw = this.topOf(el, undefined, sc) - this.pushOf(el);
          if (box) box.style.removeProperty("display");
          if (left) el.setAttribute(FAR, "");
          if (this.sameState(rec, page, forceNext, raw + this.queuedH + push - this.contentTop(page))) {
            sync = i;
            tops[i] = raw - (live - this.queuedH);
            break;
          }
        }
        reach(i);
      }
      const h = hs[i];
      // Where the pass stands on reaching the block.
      const rel = tops[i] + live + push - this.contentTop(page);
      if (!rec) this.recs.set(el, (rec = { entryPage: 0, acc: 0, force: false, cx: 0, rel: 0, page: 0, last: 0, whole: false }));
      rec.entryPage = page;
      rec.acc = live + push;
      rec.force = forceNext;
      rec.cx = tops[i];
      rec.rel = rel;
      rec.page = page;
      rec.last = page;
      rec.whole = false;
      rec.lines = undefined;
      rec.numbered = undefined;
      if (this.wantLines) this.reset.push(el);
      prevEl = el;
      prevRec = rec;
      // A block folded away (a collapsed heading's text, a level the outline hides) takes no room.
      if (el.dataset.fold || this.inHiddenTab(el)) continue;
      if (ix) this.cur = ix.sec[i];
      // The first block of a section says how the section starts, and that decides where it goes.
      let parity = 0; // 1: needs an odd page number, 2: an even one
      if (ix?.start[i]) {
        const s = this.secs[this.cur];
        const here = this.boxes[page];
        let kind = this.cont ? "continuous" : s.start;
        if (kind === "nextColumn") kind = "nextPage";
        // Word starts a continuous section on a new page when the paper changes.
        if (kind === "continuous" && here && (Math.abs(here.w - s.w) > 0.5 || Math.abs(here.h - s.h) > 0.5)) kind = "nextPage";
        if (kind !== "continuous") {
          forceNext = true;
          parity = kind === "evenPage" ? 2 : kind === "oddPage" ? 1 : 0;
        }
      }
      let top = tops[i] + live + push;
      while (top >= this.contentBottom(page) - 0.5) page++;
      const pushTo = (k: number) => {
        const delta = layoutUnits(this.contentTop(k) - top);
        if (delta > 0) {
          pushes[i] += delta;
          push += delta;
          top += delta;
        }
        page = k;
        rec!.page = k;
      };
      rec.page = page;
      if (top < this.contentTop(page) - 0.5 && page > 0) pushTo(page);
      const atTop = top <= this.contentTop(page) + 1;
      const cols = el.classList.contains("cols");
      // A wrapper's own page break is its first block's.
      const pbb = cols ? (el.firstElementChild as HTMLElement | null)?.dataset.pbb : el.dataset.pbb;
      if (!this.cont && (forceNext || pbb) && !atTop) pushTo(page + 1);
      // An even-page or odd-page section starts on a page of that number, leaving a blank page when it has to.
      if (parity && (this.box(page).num % 2 === 0) !== (parity === 2)) pushTo(page + 1);
      forceNext = !this.cont && (!!el.dataset.pbr || (!ix && !!el.dataset.sect));
      // Vertical alignment (centred, bottom, spread out): the extra space the last pass found this page can spare.
      const spare = layoutUnits(this.vshift.get(i) ?? 0);
      if (spare && !cols) {
        pushes[i] += spare;
        push += spare;
        top += spare;
      }

      if (cols) {
        // Columns, a break inside a paragraph and tables measure the page as it is: the queued spacers go in first.
        this.flushSpacers();
        if (notesIn) this.looked(page, 0);
        // Columns starting almost at the bottom of a page start on the next one; vertical text always starts a page of its own.
        if (top > this.contentTop(page) + 1 && (this.contentBottom(page) - top < COLS_ROOM || this.secs[this.cur].vert)) pushTo(page + 1);
        const nextStart = ix && i + 1 < kids.length && ix.start[i + 1] ? this.startKind(ix.sec[i + 1]) : "";
        let r: { pull: number; live: number; page: number };
        try {
          r = this.secs[this.cur].vert
            ? this.placeVertical(el, this.cur, page, h)
            : this.secs[this.cur].colW
              ? this.placeUnequal(el, this.cur, top, page, h)
              : this.placeColumns(el, this.cur, top, page, h, nextStart === "continuous");
        } catch (e) {
          // Whatever went wrong, the section shows in one column rather than breaking the page.
          console.error("Docxtor: columns could not be laid out", e);
          this.resetCols(el);
          r = { pull: 0, live: el.offsetHeight - h, page };
        }
        if (r.pull) {
          pushes[i] += r.pull;
          push += r.pull;
        }
        live += r.live;
        if (notesIn) for (let k = page; k <= r.page; k++) this.looked(k, 0);
        for (let k = page; k <= r.page; k++) this.dirty.add(k);
        page = r.page;
        continue;
      }

      if (el.dataset.pbr && !this.cont) {
        this.flushSpacers();
        if (notesIn) this.looked(page, 0);
        // The text before the break is laid out like any paragraph: when its last line does not end above the page's text bottom the paragraph
        // goes to the next page (it used to run on past it, into the room of the notes and the margin, and nothing a page was given moved it).
        if (!atTop) {
          const atom = el.querySelector<HTMLElement>(".atom.pbr, .atom.cbr");
          if (atom) {
            const ar = atom.getBoundingClientRect();
            const mid = ((ar.top + ar.bottom) / 2 - this.flow.getBoundingClientRect().top) / sc;
            const own = this.lines(el).find((l) => mid >= l.top - 1 && mid < l.bottom + 1);
            if (own && own.bottom + push > this.contentBottom(page) + 0.5) pushTo(page + 1);
          }
        }
        const extra = this.breakInside(el, push, page);
        if (extra !== null && inc) throw new StartOver("a page break inside a paragraph");
        if (extra === null) {
          // Like Word, a paragraph that ends with its page break stays where the break occurs.
          this.place(page, i, top + h);
          continue;
        }
        // Text after an in-paragraph break continues on the next page.
        live += extra;
        this.dirty.add(page);
        this.dirty.add(page + 1);
        page += 1;
        forceNext = false;
        continue;
      }
      if (el.classList.contains("tfloat") || el.classList.contains("pfloat")) {
        // A floating table is out of the flow. One that runs past the page goes to the next page with the paragraph it
        // is anchored to (the block after it), which is pushed by the same amount.
        if (top + h > this.contentBottom(page) + 0.5 && top > this.contentTop(page) + 1 && i + 1 < kids.length) {
          const delta = layoutUnits(this.contentTop(page + 1) - top);
          pushes[i] += delta;
          pushes[i + 1] += delta;
          push += delta;
          page += 1;
        } else if (notesIn) this.looked(page, this.contentBottom(page) + 0.5 - (top + h));
        continue;
      }
      const refs = el.classList.contains("fp") ? notesIn?.get(el) : undefined;
      let bottom = top + h;
      const keepNext = el.dataset.kn && i + 1 < kids.length && !el.classList.contains("ftw");
      if (keepNext && !(top <= this.contentTop(page) + 1)) {
        reach(i + 1);
        const nextFirst = Math.min(hs[i + 1], 28);
        if (bottom + nextFirst > this.contentBottom(page)) {
          pushTo(page + 1);
          bottom = top + h;
        } else if (notesIn) this.looked(page, this.contentBottom(page) - (bottom + nextFirst));
      }
      // The last block of a section with its endnotes under it: the notes have to fit on the page too.
      const pad = this.endPad.get(i) ?? 0;
      if (pad && bottom <= this.contentBottom(page) + 0.5 && bottom + pad > this.contentBottom(page) + 0.5 && !(top <= this.contentTop(page) + 1)) {
        pushTo(page + 1);
        bottom = top + h;
      } else if (pad && notesIn) this.looked(page, this.contentBottom(page) + 0.5 - (bottom + pad));
      // The notes of this paragraph's references take their room on the page as its lines fill it (`roomOnPage`).
      const given0 = given.length;
      const lines = refs ? this.noteLines(el, refs, push + this.queuedH) : null;
      if (lines) this.roomOnPage(lines, page, room, given);
      if (bottom <= this.contentBottom(page) + 0.5) {
        rec.whole = true;
        this.place(page, i, bottom);
        continue;
      }

      // Overflows the page: split if possible, otherwise move it to the next page.
      const isTop = top <= this.contentTop(page) + 1;
      let split: { added: number; page: number } | null = null;
      if (el.classList.contains("fp") && !el.dataset.kl) split = this.splitParagraph(el, push, page, isTop, true, lines ? { lines, room, given, from: given0 } : undefined);
      else if (el.classList.contains("ftw")) {
        this.flushSpacers();
        split = this.splitTable(el, push, page);
      }
      // The paragraph stays whole and goes on to the next page: its notes' room goes with it, and if it fits without that room, it stays.
      if (!split && lines && !isTop && this.takeBack(page, room, given, given0, () => true) && bottom <= this.contentBottom(page) + 0.5) {
        rec.whole = true;
        this.place(page, i, bottom);
        continue;
      }
      if (split) {
        live += split.added;
        for (let k = page; k <= split.page; k++) this.dirty.add(k);
        page = split.page;
      } else if (!isTop) {
        pushTo(page + 1);
        if (refs) this.roomOnPage(this.noteLines(el, refs, push + this.queuedH), page, room, given);
        if (top + h <= this.contentBottom(page) + 0.5) {
          rec.whole = true;
          this.place(page, i, top + h);
        } else this.dirty.add(page);
      } else {
        this.dirty.add(page);
      }
    }
    this.flushSpacers();
    const stop = sync >= 0 ? sync : n;
    this.passKids = kids;
    this.passCount = this.flow.childElementCount;
    this.touched = this.touched ? [Math.min(this.touched[0], inc ? start : 0), Math.max(this.touched[1], stop)] : [inc ? start : 0, stop];
    for (let i = start; i < stop; i++) {
      if (!pushes[i] && !(i === start && away)) continue;
      let m = bases[i] + pushes[i];
      // Two negative margins that meet do not add up (only the more negative counts): a wrapper pulled up
      // right after one whose bottom margin pulled the layout up asks for both.
      const prev = kids[i - 1];
      if (m < 0 && prev?.classList.contains("cols")) {
        const mb = parseFloat(prev.style.marginBottom) || 0;
        if (mb < 0) m += mb;
      }
      this.setOwn(kids[i], m);
    }
    this.lastPlan += `[${inc ? "from" : "full from"} ${start}, drawn up to ${maxDirty}, stopped at ${sync}, ${n} blocks, measured ${stats.lastMeasured}, room changed up to page ${this.reserveDiff}] `;
    if (sync >= 0) {
      stats.synced++;
      // Everything from here on is as the last pass left it, except that the blocks sit where the text above them put them now: their
      // records move by what the text above changed (`cx`, the place with nothing pushed) and what the pass has pushed (`acc`).
      const old = this.recs.get(kids[sync])!;
      const dc = tops[sync] - old.cx;
      const da = live + push - old.acc;
      if (dc || da) for (let j = sync; j < n; j++) {
        const r = this.recs.get(kids[j]);
        if (r) {
          r.cx += dc;
          r.acc += da;
        }
      }
      // The pages from this block on: the first block of the pages it opens, then the last pass's.
      while (filled < page) first[++filled] = kids[sync];
      for (let k = page + 1; k < oldFirst.length; k++) first[k] = oldFirst[k];
      this.firstOnPage = first;
      this.pages = this.bodyPages;
      this.boxes.length = Math.min(this.boxes.length, this.pages);
      this.passReserve = this.reserve.slice();
      return;
    }
    finish(prevEl, prevRec);
    this.firstOnPage = first;
    // The last page is the one holding the bottom of the last block.
    const last = [...kids].reverse().find((k) => !k.dataset.fold && !k.closest("[data-tab-hidden]"));
    // The pass measures the last block itself (its block is drawn first, whatever is left out) and the map keeps the result (`bodyEnd`, `bodyPages`):
    // an incremental pass that stops before the end leaves both as they are, since a block it stops at is where the last pass had it.
    // A wrapper's bottom margin pulls what follows up to where its columns really end.
    if (last) this.drawFar?.(last);
    const end = last ? this.topOf(last, undefined, sc) + this.hOf(last, sc) + (last.classList.contains("cols") ? parseFloat(last.style.marginBottom) || 0 : 0) : 0;
    this.pages = Math.max(1, page + 1, this.pageFor(Math.max(0, end - 1)) + 1);
    this.boxes.length = Math.min(this.boxes.length, this.pages);
    this.bodyPages = this.pages;
    this.bodyEnd = end;
    this.passReserve = this.reserve.slice();
    void oldBoxes;
  },
  lastDifference(this: Paginator, was: number[]): number {
    let last = -1;
    for (let k = 0; k < Math.max(was.length, this.reserve.length); k++) if (Math.abs((was[k] ?? 0) - (this.reserve[k] ?? 0)) >= ROOM_EPS) last = k;
    return last;
  },
  /** Is the pass standing where the last pass stood on reaching this block (same page, same place on it, same footnote room from there on)? */
  sameState(this: Paginator, rec: BlockRec, page: number, force: boolean, rel: number): boolean {
    return rec.entryPage === page && rec.force === force && Math.abs(rec.rel - rel) < 1 / 128 && page > this.reserveDiff;
  },
  /** Take the pushes and cuts of the last pass out of blocks `a` to `b` (exclusive), ready to be measured. */
  clearKids(this: Paginator, kids: HTMLElement[], a: number, b: number) {
    for (let j = a; j < b; j++) {
      const el = kids[j];
      for (const sp of Array.from(el.querySelectorAll(".pgsp, .hdr-repeat"))) {
        const parent = sp.parentNode as HTMLElement | null;
        sp.remove();
        parent?.normalize();
      }
      for (const t of Array.from(el.querySelectorAll<HTMLElement>("table.ft[data-cut]"))) {
        t.style.removeProperty("mask-image");
        delete t.dataset.cut;
      }
      if (this.gapOf(el)) this.setOwn(el, parseFloat(el.dataset.base ?? "0") || 0);
      else el.style.marginTop = el.dataset.base ?? "0px";
      for (let p = el.parentElement; p && p !== this.flow; p = p.parentElement) if (p.classList.contains("fc")) p.style.marginTop = p.dataset.base ?? "0px";
      layoutTabs(el, this.defaultTab);
    }
  },
  /** The continuous sheet ends where the text does: after the last block, the endnotes and the room the footnotes keep, and the bottom margin. */
  fitSheet(this: Paginator) {
    const b = this.boxes[0];
    if (!b) return;
    let end = this.bodyEnd;
    for (const a of this.endByPage) if (a) end = Math.max(end, a.top + a.h);
    b.h = Math.ceil(end + (this.reserve[0] ?? 0) + b.mB);
  },
  /**
   * Chapter page numbers (2-1, 2-2): each page takes the chapter of the first heading of the section's chapter level
   * that starts on it, else the chapter before; the number counts the pages since that heading's page.
   */
  findChapters(this: Paginator) {
    this.chapterAt = [];
    if (!this.secs.some((s) => s.info.chapStyle)) return;
    const all = this.flatBlocks();
    const secOf = this.sectionsOf(all);
    const counts = new Map<number, number>();
    const starts: { page: number; label: string }[] = [];
    for (const [i, el] of all.entries()) {
      if (!el.classList.contains("fp")) continue;
      const lv = this.secs[secOf[i] ?? 0]?.info.chapStyle;
      const m = /^heading ?(\d)$/i.exec(el.dataset.style ?? "");
      if (!lv || !m || Number(m[1]) !== lv) continue;
      // The number the heading shows (its list label); a heading without one is counted.
      const lbl = el.querySelector(".lbl")?.textContent ?? "";
      const token = /([0-9]+|[A-Za-z]+)\W*$/.exec(lbl.trim())?.[1];
      const n = (counts.get(lv) ?? 0) + 1;
      counts.set(lv, n);
      starts.push({ page: this.recs.get(el)?.page ?? this.pageAt(this.topOf(el)), label: token ?? String(n) });
    }
    let cur: { label: string; page: number } | undefined;
    for (let k = 0; k < this.pages; k++) {
      const here = starts.find((x) => x.page === k);
      if (here) cur = { label: here.label, page: k };
      if (cur) this.chapterAt[k] = { label: cur.label, n: k - cur.page + 1 };
    }
  },
  /** Leave room under the last block of each such section: its bottom margin grows by the height of the endnotes. */
  padSectionEnds(this: Paginator, kids: HTMLElement[], ix: { sec: number[]; start: boolean[] } | null) {
    for (const el of kids) {
      if (el.dataset.ep === undefined) continue;
      el.style.marginBottom = el.dataset.ep;
      delete el.dataset.ep;
    }
    this.endPad = new Map();
    if (!this.endBySec.size) return;
    const last = new Map<number, number>();
    kids.forEach((_, i) => last.set(ix ? ix.sec[i] : 0, i));
    for (const [si, e] of this.endBySec) {
      const i = last.get(si);
      const el = i === undefined ? undefined : kids[i];
      if (i === undefined || !el || el.classList.contains("cols")) continue;
      el.dataset.ep = el.style.marginBottom;
      el.style.marginBottom = `${(parseFloat(el.style.marginBottom) || 0) + e.h}px`;
      this.endPad.set(i, e.h);
    }
  },
  /** A block that sits whole on a page, ending at flow y `bottom`. */
  place(this: Paginator, page: number, i: number, bottom: number) {
    (this.onPage[page] ??= []).push(i);
    this.pageEnd[page] = Math.max(this.pageEnd[page] ?? 0, bottom);
  },
  /**
   * The extra space (px, above blocks by position in the flow) that centres, bottom-aligns or spreads out the text of
   * the pages of sections that ask for it. A page with a block cut by the page, a column wrapper or a break inside a
   * paragraph keeps its text at the top.
   */
  verticalShifts(this: Paginator): Map<number, number> {
    const out = new Map<number, number>();
    this.onPage.forEach((list, page) => {
      if (!list?.length || this.dirty.has(page)) return;
      const how = this.secs[this.page(page).section]?.info.vAlign;
      if (!how) return;
      const free = this.contentBottom(page) - (this.pageEnd[page] ?? 0);
      if (free < 2) return;
      if (how === "center") out.set(list[0], free / 2);
      else if (how === "bottom") out.set(list[0], free);
      else if (list.length > 1) for (let j = 1; j < list.length; j++) out.set(list[j], free / (list.length - 1));
    });
    return out;
  },
};

declare module "./layout" {
  interface Paginator {
    pass(from?: { at: number; maxDirty: number; } | null): void;
    lastDifference(was: number[]): number;
    sameState(rec: BlockRec, page: number, force: boolean, rel: number): boolean;
    clearKids(kids: HTMLElement[], a: number, b: number): void;
    fitSheet(): void;
    findChapters(): void;
    padSectionEnds(kids: HTMLElement[], ix: { sec: number[]; start: boolean[]; } | null): void;
    place(page: number, i: number, bottom: number): void;
    verticalShifts(): Map<number, number>;
  }
}
