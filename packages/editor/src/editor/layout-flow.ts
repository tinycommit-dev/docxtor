// Running a layout: the blocks in order, starting part-way down, the body of a pass and the margins and indents it mirrors.
import type { Paginator } from "./layout";
import { isFarBox, isTopBlock, topBlocks } from "./far";
import { ROOM_EPS, StartOver, chasedOnly, moveOnly, raise, sameRoom, snap } from "./layout-units";
import { checkAgainstFullPass } from "./layout-check";

/** What a layout made, as lines of text: pages, boxes, notes, every margin a pass set and every spacer it put in, and where it put them. */
function addBlocks(el: HTMLElement): HTMLElement[] {
  const out: HTMLElement[] = [];
  addBlock(el, out);
  return out;
}

function addBlock(el: HTMLElement, out: HTMLElement[]) {
  if (!el.classList.contains("fc")) {
    out.push(el);
    return;
  }
  for (const c of Array.from(el.children) as HTMLElement[]) if (!c.classList.contains("fc-label")) addBlock(c, out);
}

export const flowMethods = {
  clear(this: Paginator) {
    for (const sp of Array.from(this.flow.querySelectorAll(".pgsp, .hdr-repeat"))) {
      const parent = sp.parentNode as HTMLElement | null;
      sp.remove();
      parent?.normalize();
    }
    for (const t of Array.from(this.flow.querySelectorAll<HTMLElement>("table.ft[data-cut]"))) {
      t.style.removeProperty("mask-image");
      delete t.dataset.cut;
    }
    for (const el of this.pageBlocks()) if (!isTopBlock(this.flow, el)) el.style.marginTop = el.dataset.base ?? "0px";
    for (const el of topBlocks(this.flow)) {
      el.style.marginTop = el.dataset.base ?? "0px";
      if (el.classList.contains("cols")) {
        if (el.dataset.un) this.clearUnequal(el);
        if (el.dataset.vt) this.clearVertical(el);
        // Back to one column, as a wrapper the pass has not seen yet is: what follows is measured the same whatever the last pass did.
        for (const prop of ["column-count", "column-gap", "column-fill", "column-height", "column-wrap", "row-gap"]) el.style.removeProperty(prop);
        el.style.marginBottom = "";
        el.removeAttribute("data-pull");
        for (const b of Array.from(el.children)) b.removeAttribute("data-cs");
      }
    }
    this.rules = [];
  },
  /** The top-level blocks in order, those inside a section's wrapper included (the editor's blocks, as the model has them). */
  blocks(this: Paginator): HTMLElement[] {
    const out: HTMLElement[] = [];
    for (const k of topBlocks(this.flow)) {
      if (k.classList.contains("cols")) {
        for (const b of Array.from(k.children) as HTMLElement[]) if (!b.classList.contains("colsp")) out.push(b);
      } else out.push(k);
    }
    return out;
  },
  /** As `blocks`, with the blocks inside a block-level content control in its place: what sections and pages see. */
  flatBlocks(this: Paginator): HTMLElement[] {
    const out: HTMLElement[] = [];
    for (const b of this.blocks()) addBlock(b, out);
    return out;
  },
  /**
   * The blocks the pages are made of: the flow's children, with the blocks inside a block-level content control in its
   * place, so a cover page, or a table of contents in a control, breaks across pages like any other text.
   */
  pageBlocks(this: Paginator): HTMLElement[] {
    // Walk the siblings: a copy of the flow's live children (9,000 blocks in a novel) costs more than the rest of the loop.
    const out: HTMLElement[] = [];
    for (let k = this.flow.firstElementChild as HTMLElement | null; k; k = k.nextElementSibling as HTMLElement | null) {
      if (k.classList.contains("fc")) addBlock(k, out);
      else if (isFarBox(k)) for (let b = k.firstElementChild as HTMLElement | null; b; b = b.nextElementSibling as HTMLElement | null) out.push(b);
      else out.push(k);
    }
    return out;
  },
  /**
   * Where a block's top is in the flow, in whole layout units. Exact, not the browser's `offsetTop`: that rounds to a pixel by
   * where the block sits, so what the pass makes of a block would depend on how far the text above it happens to be shifted,
   * and a pass that starts part-way down could not give what a pass from the top gives (see `layout`).
   */
  topOf(this: Paginator, el: HTMLElement, fr?: DOMRect, sc?: number): number {
    return snap((el.getBoundingClientRect().top - (fr ?? this.flow.getBoundingClientRect()).top) / (sc ?? this.scale()));
  },
  /** A block's height in whole layout units (exact, as `topOf`). */
  hOf(this: Paginator, el: HTMLElement, sc?: number): number {
    return snap(el.getBoundingClientRect().height / (sc ?? this.scale()));
  },
  /**
   * Keep the flow's shape in step with the sections: every section with columns has one wrapper holding all of its
   * blocks, and every other block is a direct child of the flow. A wrapper is kept (with the blocks inside it) while
   * its section stays, so editing a block never rebuilds it.
   */
  arrange(this: Paginator) {
    const flow = this.flow;
    const wrappers = Array.from(flow.children).filter((k): k is HTMLElement => k.classList.contains("cols"));
    if (!wrappers.length && !this.secs.some((_, i) => this.multi(i))) return;
    const flat: HTMLElement[] = [];
    for (const k of topBlocks(flow)) {
      if (k.classList.contains("cols")) flat.push(...(Array.from(k.children) as HTMLElement[]));
      else flat.push(k);
    }
    const secOf = this.sectionsOf(flat);
    const claimed = new Set<HTMLElement>();
    const plan: { si: number; run: HTMLElement[]; wrap?: HTMLElement }[] = [];
    for (let a = 0, b = 0; a < flat.length; a = b) {
      while (b < flat.length && secOf[b] === secOf[a]) b++;
      const run = flat.slice(a, b);
      let wrap: HTMLElement | undefined;
      if (this.multi(secOf[a])) {
        wrap = run.map((e) => e.parentElement).find((p): p is HTMLElement => !!p && p.classList.contains("cols") && p.parentElement === flow && !claimed.has(p));
        if (!wrap) {
          wrap = document.createElement("div");
          wrap.className = "cols";
        }
        claimed.add(wrap);
      }
      plan.push({ si: secOf[a], run, wrap });
    }
    // A wrapper whose section no longer has columns gives its blocks back to the flow where it stood.
    for (const w of wrappers) {
      if (claimed.has(w)) continue;
      while (w.firstChild) flow.insertBefore(w.firstChild, w);
      w.remove();
    }
    const want: HTMLElement[] = [];
    for (const { si, run, wrap } of plan) {
      if (!wrap) {
        want.push(...run);
        continue;
      }
      wrap.dataset.sec = String(si);
      for (const e of run) if (e.parentElement !== wrap) this.enter(e);
      this.reconcile(wrap, run);
      want.push(wrap);
    }
    this.reconcile(flow, want);
  },
  /** Put `list` in order as the children of `parent`, moving only what is out of place. */
  reconcile(this: Paginator, parent: HTMLElement, list: HTMLElement[]) {
    let cursor: ChildNode | null = parent.firstChild;
    for (const el of list) {
      if (el === cursor) cursor = cursor.nextSibling;
      else parent.insertBefore(el, cursor);
    }
  },
  /** A block going into a wrapper: the wrapper places it, so it carries no push or side offsets of its own. */
  enter(this: Paginator, el: HTMLElement) {
    el.style.marginTop = el.dataset.base ?? "0px";
    this.setSides(el, 0, 0, false);
  },
  /**
   * With several page widths or side margins each top-level block sits within its own section's margins on its
   * (centred) sheet: the flow has no side padding and the block's side margins carry the offsets. A block's own
   * side margins (a bordered paragraph reaches outside its text) are kept as the base.
   */
  applySides(this: Paginator, kids: HTMLElement[]) {
    if (this.uniform) {
      if (this.sidesOn) {
        for (const el of kids) if (el.dataset.sk !== undefined) this.setSides(el, 0, 0, false);
        this.sidesOn = false;
      }
      return;
    }
    this.sidesOn = true;
    for (let i = 0; i < kids.length; i++) {
      const s = this.secs[this.ix ? this.ix.sec[i] : 0];
      const gutter = (this.stageW - s.w) / 2;
      this.setSides(kids[i], gutter + s.ml, gutter + s.mr, true);
    }
  },
  setSides(this: Paginator, el: HTMLElement, left: number, right: number, on: boolean) {
    const key = on ? `${left}|${right}` : undefined;
    if (el.dataset.sk === key) return;
    if (key === undefined) delete el.dataset.sk;
    else el.dataset.sk = key;
    const bl = parseFloat(el.dataset.bl ?? "0") || 0;
    const br = parseFloat(el.dataset.br ?? "0") || 0;
    el.style.marginLeft = on || bl ? `${bl + left}px` : "";
    el.style.marginRight = on || br ? `${br + right}px` : "";
  },
  /** A block drawn again in place of another takes over its side offsets (until the next layout sets them). */
  carrySides(this: Paginator, el: HTMLElement, old: HTMLElement) {
    const key = old.dataset.sk;
    if (key === undefined) return;
    const [l, r] = key.split("|").map(Number);
    this.setSides(el, l, r, true);
  },
  /**
   * Lay out pages (twice more when footnotes need room). Returns the page count.
   *
   * With `incremental` (the editor drew some blocks since the last layout and nothing else changed), the pass starts at
   * the block before the first one drawn instead of at the top, and stops at the first block that is reached in the same
   * state as the last pass reached it (same page, same place on it, same footnote room): everything after that is as it
   * was. The result is the one a full pass gives (`verify` checks it). What the incremental pass cannot do it hands to the
   * full one, and counts why (`stats.fallbacks`).
   */
  layout(this: Paginator, opts: { incremental?: boolean } = {}): number {
    let inc: { at: number; maxDirty: number } | null = null;
    if (opts.incremental) {
      const why = this.cannotStartAhead();
      if (why) this.count(why);
      else {
        const plan = this.startOfPass();
        if (typeof plan === "string") this.count(plan);
        else inc = plan;
      }
    }
    // An incremental layout goes on from the footnote room of the last one, so that a document with notes does not lay out two or three
    // times for every edit; a full one gives the pages their notes' room as its first pass fills them (`roomInPass`), or, with that off,
    // goes on from the last room too. Either is a first guess: the rounds in `layoutBody` still correct it.
    // (A document without notes has none: what is left over from the last one would still shape the rows.)
    const start = this.setup?.notes.length ? this.reserve.slice() : null;
    const pages = this.layoutBody(inc, start);
    // @ts-ignore: set by the bundler; false in the production build, which drops the check with it
    if (import.meta.env.DEV && this.verify && (inc || start?.some((v) => v) || this.usedRoomInPass)) checkAgainstFullPass(this, pages, inc ? start : null);
    return pages;
  },
  count(this: Paginator, why: string) {
    this.stats.fallbacks[why] = (this.stats.fallbacks[why] ?? 0) + 1;
  },
  /** Why a pass cannot start part-way down at all, or null. */
  cannotStartAhead(this: Paginator): string | null {
    if (!this.mapValid || this.allDirty) return "everything changed";
    return this.cannotResume();
  },
  /** What stops a pass from starting part-way down whatever changed (the page map, if there is one, is not asked about here). */
  cannotResume(this: Paginator): string | null {
    if (this.cont) return "continuous view";
    if (this.secs.length !== 1) return "several sections";
    const s = this.secs[0];
    if (this.multi(0)) return "columns";
    if (s.info.vAlign) return "vertical alignment";
    if (s.info.docGrid || this.gridOn) return "document grid";
    if (s.info.footnotePr?.pos === "beneathText") return "notes under the text";
    if (s.info.endnotePr?.pos === "sectEnd") return "endnotes at section ends";
    if (this.flow.querySelector(":scope > .cols")) return "columns";
    return null;
  },
  /** Where an incremental pass starts: the block it starts at, and the last block drawn (the pass cannot stop before it). Or why it cannot. */
  startOfPass(this: Paginator): { at: number; maxDirty: number } | string {
    const kids = this.pageBlocks();
    const index = new Map<HTMLElement, number>();
    kids.forEach((k, i) => index.set(k, i));
    let first = Infinity;
    let last = -1;
    for (const el of this.dirtyKids) {
      const at = index.get(el);
      if (at !== undefined) {
        first = Math.min(first, at);
        last = Math.max(last, at);
      } else if (el.isConnected && el.classList.contains("fc")) {
        // A block-level content control: the blocks inside it are the pages' blocks.
        for (const k of addBlocks(el)) {
          const j = index.get(k);
          if (j === undefined) return "a block drawn is not in the flow";
          first = Math.min(first, j);
          last = Math.max(last, j);
        }
      } else if (el.isConnected) return "a block drawn is not in the flow";
      // A block drawn and gone again (removed by a later edit) leaves nothing to start from: its neighbours were marked when it went.
    }
    if (first === Infinity) {
      // Nothing is left of what was drawn: the blocks that were removed are marked through their neighbours, so this is a layout with no change at all.
      return "no block to start from";
    }
    // The block before the first one drawn is laid out again too (its page may take more or less of the next block now).
    const at = this.backUp(kids, first - 1);
    if (at > 0 && !this.recs.get(kids[at])) return "a block has no place in the page map";
    // A pass that starts part-way down measures a block for about three times what a pass over the whole text does (it works in small batches):
    // when the blocks drawn are spread over most of the text (an undo of typing in two places), the pass from the top is cheaper. With far pages left
    // out the incremental pass is the cheaper one whatever the spread: it draws only the far blocks it reaches, where a full pass draws them all.
    if (!this.farN && 3 * (last - at + 1) > kids.length && kids.length > 200) return "a change spread over most of the text";
    return { at, maxDirty: last };
  },
  /** Where a pass that has to lay block `at` out again starts: also at any block before it that depends on the one after it (keep with next, a floating table that moves the block after it). */
  backUp(this: Paginator, kids: HTMLElement[], at: number): number {
    at = Math.max(0, Math.min(kids.length - 1, at));
    while (at > 0 && (kids[at - 1].dataset.kn || kids[at - 1].classList.contains("tfloat") || kids[at - 1].classList.contains("pfloat"))) at--;
    return at;
  },
  /** A pass; one that started part-way down and met something it cannot finish that way is made again from the top. */
  runPass(this: Paginator, from: { at: number; maxDirty: number } | null): boolean {
    try {
      this.pass(from);
      return true;
    } catch (e) {
      if (!(e instanceof StartOver)) throw e;
      this.count(e.why);
      this.pass(null);
      return false;
    }
  },
  /** The layout proper: pass, footnote room, vertical alignment, endnotes, fields, sheets. `inc` starts the first pass part-way down. */
  layoutBody(this: Paginator, inc: { at: number; maxDirty: number } | null, from: number[] | null, retryFromTop = false): number {
    const measured0 = this.stats.measured;
    this.docGrid();
    this.reserve = from ? from.slice() : [];
    if (!inc) this.notesByPage = [];
    this.vshift = new Map();
    this.lastPlan = "";
    this.findSectionEndnotes();
    this.refParas = null;
    this.reset = [];
    this.touched = null;
    this.slack = [];
    this.roomInPass = this.notesInPass && !inc && !retryFromTop && !!this.setup?.notes.length;
    this.usedRoomInPass = this.roomInPass;
    try {
      this.runPass(inc);
    } finally {
      this.roomInPass = false;
    }
    if (this.setup?.notes.length) {
      // The first pass of a full layout has given every block a place in the page map, so the passes that follow it can start where the
      // room changed, as those of an incremental layout do (unless something makes that impossible: `cannotResume`). `retryFromTop`
      // is the check's way of asking for the layout from scratch.
      let resume = !retryFromTop && (!!inc || !this.cannotResume());
      // The room a page was given and the room it needs can chase each other: room on page N pushes a note's reference to page N+1,
      // which leaves N needing none, and with none the reference comes back. Word keeps the line with the reference and its note
      // together, so a line that room for its own note pushes on goes on, with the note, and the page it left stays short.
      // Here: when the rooms come round to one they already had (the layout would go on for ever, and end on whichever of them the
      // number of rounds gives), the larger room of each page in that round is kept as a floor, and the rounds go on from it.
      // The floor keeps the page short, the reference and its note stay together on the next one, and the result is the same
      // whichever room the layout started from, and again when it starts from the result.
      // A page that is chased keeps no more than it takes to move the line with the reference (`chased`: the least room that moves
      // it, so every line above the reference stays, as in Word), not the whole room of the note, which would leave it lines short. That room is read in the layouts of the cycle, where the line is on the page: they are the same whichever
      // room the rounds started from. A page after the first chased page of the cycle that is not chased in a layout gives the floor
      // nothing from it: its room only changed because a chase before it moved the text, and it takes what it needs in the round it is
      // in (a floor of the larger room of every page left such pages holding room for notes they no longer had). Pages before the
      // first chase give what they need, as before. Each cycle settles the chases it shows and the next one appears once the text has
      // moved, so chases that meet take a cycle each: the rounds stop after twelve in which no page got a floor it did not have
      // (the floor only grows, so they end), and at sixty. The floor of a page comes from the cycle's own layouts, which are the
      // same from any start that reaches the cycle, so the answer does not depend on the room the rounds began with.
      let floor: number[] = [];
      let seen: number[][] = [this.reserve.slice()];
      // chases[i]: `chased` of the layout that gave seen[i].
      let chases: number[][] = [[]];
      // What the last look at the notes found, while nothing has been laid out since: the look after the rounds is not made again.
      let settled: number[] | null = null;
      // The rounds stop after twelve without a page getting a floor it did not have (the floor only grows, so each new one is progress
      // and chases that meet, which take a cycle each, can all be settled), and at sixty whatever happens.
      let idle = 0;
      // The pages whose layout the first pass made before the room they ended with was known, where that room could change it (see `looked`).
      const verify: number[] = [];
      if (this.usedRoomInPass) this.slack.forEach((v, k) => v < 1 && verify.push(k));
      for (let i = 0; idle < 12 && i < 60; i++, idle++) {
        settled = null;
        let need = this.noteNeeds();
        const chase = this.chased;
        if (floor.length) need = raise(moveOnly(need, chase), floor);
        let verifyNow = false;
        if (sameRoom(need, this.reserve)) {
          if (!verify.length) {
            settled = need;
            break;
          }
          verifyNow = true;
        }
        const again0 = verifyNow ? -1 : seen.findIndex((v) => sameRoom(v, need));
        if (again0 >= 0) {
          // The rooms of the rounds since it was last like this: the layouts of the cycle are the ones that gave them (the last one gave
          // `need`), and a page that has a line to move takes the room that moves it in place of the room it needs there.
          const gave = [...seen.slice(again0 + 1), need];
          const moved = [...chases.slice(again0 + 1), chase];
          const had = floor;
          const first = Math.min(...moved.map((c) => (c.findIndex((x) => x > 0) + 1 || Infinity) - 1));
          gave.forEach((v, j) => (floor = raise(floor, chasedOnly(v, moved[j], first))));
          if (floor.some((v, k) => v - (had[k] ?? 0) >= 1)) idle = 0;
          need = raise(moveOnly(need, chase), floor);
          seen = [];
          chases = [];
          if (sameRoom(need, this.reserve)) {
            settled = need;
            break;
          }
        }
        seen.push(need);
        chases.push(chase);
        // The blocks the pass lays out again start at the block before the first one that starts on a page whose room changed
        // (that block may run on into it).
        const was = this.reserve;
        let changed = 0;
        while (changed < Math.max(need.length, this.reserve.length) && Math.abs((need[changed] ?? 0) - (this.reserve[changed] ?? 0)) < ROOM_EPS) changed++;
        this.reserve = need;
        let again: { at: number; maxDirty: number } | null = null;
        if (resume) {
          const kids = this.pageBlocks();
          const kid = this.firstOnPage[changed];
          const at = this.backUp(kids, kid ? kids.indexOf(kid) - 1 : kids.length - 1);
          if (at >= 0 && (at === 0 || this.recs.get(kids[at]))) again = { at, maxDirty: -1 };
          else if (inc) this.count("footnote room");
          // A pass that starts part-way down measures a block for about three times what a pass over the whole text does (it works in
          // small batches), and cannot stop before the last page whose room changed: in a full layout, where nothing asks for the
          // short way, the pass from the top is cheaper when the change reaches far down. (An incremental layout has no such choice.)
          if (again && !inc) {
            let last = -1;
            for (let k = 0; k < Math.max(need.length, was.length); k++) if (Math.abs((need[k] ?? 0) - (was[k] ?? 0)) >= ROOM_EPS) last = k;
            const end = this.firstOnPage[last + 1] ? kids.indexOf(this.firstOnPage[last + 1]) : kids.length;
            if (3 * (end - again.at) + 1500 > kids.length) again = null;
          }
        }
        if (verifyNow) {
          // The first pass settled on rooms its pages' notes are happy with, but it laid out some pages before it knew their room, and
          // what it looked at there could have gone the other way with it (a heading kept with the next block that the room of the next
          // block's note pushes on). The layout is the one the rounds give only if the pass made with these rooms from the start of those
          // pages is the same: it starts at the first of them, and goes on at least to the last (`reserveDiff`).
          again = null;
          const kids = this.pageBlocks();
          if (resume) {
            const kid = this.firstOnPage[verify[0]];
            const at = this.backUp(kids, kid ? kids.indexOf(kid) - 1 : kids.length - 1);
            const end = this.firstOnPage[verify[verify.length - 1] + 1] ? kids.indexOf(this.firstOnPage[verify[verify.length - 1] + 1]) : kids.length;
            if (at >= 0 && (at === 0 || this.recs.get(kids[at])) && 3 * (end - at) + 1500 <= kids.length) {
              again = { at, maxDirty: -1 };
              this.passReserve = this.reserve.slice();
              this.passReserve[verify[verify.length - 1]] = (this.passReserve[verify[verify.length - 1]] ?? 0) + 1000;
            }
          }
          verify.length = 0;
        }
        if (!this.runPass(again) && again) resume = false;
      }
      const last = settled ?? (floor.length ? raise(moveOnly(this.noteNeeds(), this.chased), floor) : this.noteNeeds());
      this.notesSettled = sameRoom(last, this.reserve);
    } else this.notesSettled = true;
    // Pages whose section centres, spreads or bottom-aligns its text: lay out again with the extra space in place.
    if (!this.cont && this.secs.some((s) => s.info.vAlign)) {
      this.vshift = this.verticalShifts();
      if (this.vshift.size) this.pass();
    }
    this.placeEndnotes();
    if (this.cont) this.fitSheet();
    this.findChapters();
    this.renderSheets(!!inc);
    this.mirrorMargins();
    this.mirrorIndents();
    this.drawLineNumbers();
    this.recordLines();
    this.mapValid = true;
    this.allDirty = false;
    // A full layout is one pass for what the stats say about "the last pass" even when the notes asked for more.
    if (!inc) {
      this.stats.lastMeasured = this.stats.measured - measured0;
      this.stats.lastIncremental = false;
    }
    this.dirtyKids.clear();
    return this.pages;
  },
  /**
   * With mirrored margins the text sits further right on odd pages and further left on even ones (the flow's own side
   * margins are the odd pages'). Each block on an even page moves by the difference; the width stays the same.
   */
  mirrorMargins(this: Paginator) {
    if (!this.secs.some((s) => s.mirror)) {
      for (const el of Array.from(this.flow.querySelectorAll<HTMLElement>("[data-mm]"))) {
        el.style.translate = "";
        delete el.dataset.mm;
      }
      return;
    }
    // The blocks inside a content control count one by one, as in `pass`: a cover's section ends inside its control.
    const kids = this.pageBlocks();
    const ix = this.sectionIndex(kids);
    kids.forEach((el, i) => {
      if (el.classList.contains("cols")) return;
      const s = this.secs[ix ? ix.sec[i] : 0];
      const even = s.mirror && this.box(this.recs.get(el)?.page ?? this.pageFor(this.topOf(el))).num % 2 === 0;
      const dx = even ? s.mlE - s.ml : 0;
      el.style.translate = dx ? `${dx}px` : "";
      if (dx) el.dataset.mm = "1";
      else delete el.dataset.mm;
    });
  },
  /**
   * Put the layout's value in the page-dependent fields of the text (page numbers, STYLEREF, the date elements), after
   * the pages are laid out. The text itself keeps the result the file has: a field whose value differs shows the new one
   * through `data-val` (see editor.css), so positions in the paragraph stay what the model says. Returns whether
   * anything changed that could move the text.
   */
  fillBodyFields(this: Paginator): boolean {
    const top = this.stage.getBoundingClientRect().top;
    const scale = this.scale();
    return this.fillGroups(this.flow, true, (el) => this.markPage(el, top, scale));
  },
  /** Paragraphs with mirrored indents swap their left and right indents on even pages (inside and outside). */
  mirrorIndents(this: Paginator) {
    const list = this.flow.querySelectorAll<HTMLElement>(".fp[data-mi]");
    if (!list.length) return;
    const fr = this.flow.getBoundingClientRect();
    const sc = this.scale();
    for (const el of Array.from(list)) {
      // The paddings the file gives, which are the ones for a right-hand (odd) page.
      el.dataset.mil ??= String(parseFloat(el.style.paddingLeft) || 0);
      el.dataset.mir ??= String(parseFloat(el.style.paddingRight) || 0);
      // A paragraph that is a block of the text is on the page the map has; one in a table cell is measured.
      const at = this.recs.get(el)?.page ?? this.pageFor((el.getBoundingClientRect().top - fr.top) / sc);
      const even = this.box(at).num % 2 === 0;
      el.style.paddingLeft = `${even ? el.dataset.mir : el.dataset.mil}px`;
      el.style.paddingRight = `${even ? el.dataset.mil : el.dataset.mir}px`;
    }
  },
};

declare module "./layout" {
  interface Paginator {
    clear(): void;
    blocks(): HTMLElement[];
    flatBlocks(): HTMLElement[];
    pageBlocks(): HTMLElement[];
    topOf(el: HTMLElement, fr?: DOMRect, sc?: number): number;
    hOf(el: HTMLElement, sc?: number): number;
    arrange(): void;
    reconcile(parent: HTMLElement, list: HTMLElement[]): void;
    enter(el: HTMLElement): void;
    applySides(kids: HTMLElement[]): void;
    setSides(el: HTMLElement, left: number, right: number, on: boolean): void;
    carrySides(el: HTMLElement, old: HTMLElement): void;
    layout(opts?: { incremental?: boolean; }): number;
    count(why: string): void;
    cannotStartAhead(): string | null;
    cannotResume(): string | null;
    startOfPass(): { at: number; maxDirty: number; } | string;
    backUp(kids: HTMLElement[], at: number): number;
    runPass(from: { at: number; maxDirty: number; } | null): boolean;
    layoutBody(inc: { at: number; maxDirty: number; } | null, from: number[] | null, retryFromTop?: boolean): number;
    mirrorMargins(): void;
    fillBodyFields(): boolean;
    mirrorIndents(): void;
  }
}
