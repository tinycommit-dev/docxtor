// Footnotes and endnotes: the room they take on each page, chasing lines to the next page, and the endnote area.
import type { Paginator } from "./layout";
import type { EndArea, Given, NoteLine, NoteRef } from "./layout-types";
import { topBlockOf } from "./far";
import { END_GAP, END_PAD, MIN_BODY, layoutUnits } from "./layout-units";
import type { RNote } from "@docxtor/core";
import { noteLabel, withLabel } from "./note-layout";
import { renderBlocks } from "./render";

export const noteMethods = {
  /**
   * For a pass that gives pages the room of their notes as it fills them: each block's footnote references, with the notes' heights
   * measured now at the text's width (once: `noteH` keeps them). Null when there are none.
   */
  notesForPass(this: Paginator, kids: HTMLElement[]): Map<HTMLElement, NoteRef[]> | null {
    const notes = this.setup?.notes ?? [];
    if (!notes.length) return null;
    const byId = new Map(notes.map((n) => [n.id, n]));
    const out = new Map<HTMLElement, NoteRef[]>();
    const tops = new Set(kids);
    for (const ref of Array.from(this.flow.querySelectorAll<HTMLElement>(".atom.note[data-note]:not(.endnote)"))) {
      const note = byId.get(ref.dataset.note!);
      if (!note || this.inHiddenTab(ref)) continue;
      const block = topBlockOf(this.flow, ref);
      if (!block || !tops.has(block)) continue;
      let list = out.get(block);
      if (!list) out.set(block, (list = []));
      list.push({ ref, note, done: false });
    }
    if (!out.size) return null;
    const b = this.box(0);
    const w = b.w - b.ml - b.mr;
    this.sepHeight("separator", w);
    for (const list of out.values()) {
      for (const { note } of list) {
        const key = `${note.id}@${w}`;
        if (!this.noteH.has(key)) this.noteH.set(key, this.measure(note.blocks, "notes-measure", w));
      }
    }
    return out;
  },
  /**
   * The lines of a paragraph where the pass has them (`off`: what moves the page as it is to the pass: pushes and queued spacers),
   * each with the references on it whose notes have no room yet.
   */
  noteLines(this: Paginator, el: HTMLElement, refs: NoteRef[], off: number): NoteLine[] {
    const fr = this.flow.getBoundingClientRect();
    const sc = this.scale();
    const lines: NoteLine[] = this.lines(el).map((l) => ({ top: l.top + off, bottom: l.bottom + off, shift: 0 }));
    for (const r of refs) {
      if (r.done) continue;
      const rr = r.ref.getBoundingClientRect();
      const mid = ((rr.top + rr.bottom) / 2 - fr.top) / sc + off;
      const line = lines.find((l) => mid >= l.top - 1 && mid < l.bottom + 1);
      if (line) (line.n ??= []).push(r);
    }
    return lines;
  },
  /** A decision of the pass that looked at where `page`'s text ends, with `s` px to spare: room given to the page after it takes from that. */
  looked(this: Paginator, page: number, s: number) {
    this.slack[page] = Math.min(this.slack[page] ?? Infinity, s);
  },
  /**
   * Give `page` the room of the notes whose references are on its lines, line by line, as `noteNeeds` counts it (a separator above the
   * first note, the notes under it), while the line with the reference still fits above the room with its note. A line that does not
   * stops it: the page gives no more room, the line falls where it falls, and the rounds after the pass settle it (a reference that the
   * room for its own note would push on chases it: see `layoutBody`). A note that would take the page past what notes may take is left to
   * them too. `given` records the room given.
   */
  roomOnPage(this: Paginator, lines: NoteLine[], page: number, room: number[], given: Given[]) {
    const b = this.box(page);
    const w = b.w - b.ml - b.mr;
    const cap = Math.max(MIN_BODY, b.h - b.mT - b.mB - MIN_BODY);
    const top = this.contentTop(page);
    const raw = b.top + b.h - b.mB;
    for (const l of lines) {
      if (l.top >= this.contentBottom(page) - 0.5) return;
      if (!l.n || l.top < top - 1) continue;
      for (const r of l.n) {
        if (r.done) continue;
        if (r.leftOn === page) return;
        const h = this.noteH.get(`${r.note.id}@${w}`);
        if (h === undefined) continue;
        const has = room[page] ?? 0;
        const add = h + (has ? 0 : this.sepHeight("separator", w));
        if (has && has + add > cap) continue;
        if (l.bottom > raw - (has + add) + 0.5) return;
        room[page] = has + add;
        this.reserve[page] = room[page];
        if (this.slack[page] !== undefined) this.slack[page] -= add;
        r.done = true;
        given.push({ k: page, add, r, line: l });
      }
    }
  },
  /**
   * The lines of a paragraph that hold references given room on `page` (since `from` in `given`) go on to the next page (`goes`): the
   * room goes with them (they get none on this page, `leftOn`). Returns whether any did.
   */
  takeBack(this: Paginator, page: number, room: number[], given: Given[], from: number, goes: (l: NoteLine) => boolean): boolean {
    let k0 = given.length;
    while (k0 > from && given[k0 - 1].k === page && goes(given[k0 - 1].line)) k0--;
    if (k0 === given.length) return false;
    while (given.length > k0) {
      const g = given.pop()!;
      g.r.done = false;
      g.r.leftOn = page;
      room[g.k] -= g.add;
      this.reserve[g.k] = room[g.k];
    }
    return true;
  },
  /**
   * For page `k`, whose notes `upTo` lists with the room up to and with each (a separator, the notes before it, the note itself): the room
   * that moves only the first line that holds a reference whose room does not leave the line (the page's text bottom, less the bottom of
   * that line, and a pixel: the least room that moves it), or 0 when there is none. That line, and what follows it, go on to the next page; every line above it stays. A single-line
   * paragraph that sits whole on the page is the block (the pass moves it whole, by its box); of a longer one, the line.
   * The answer is a function of the layout (where the lines stand and how much room the page has) and of nothing before it.
   */
  chasedRoom(this: Paginator, k: number, upTo: { n: RNote; room: number }[], marks: Map<string, HTMLElement>, flowTop: number, sc: number): number {
    const b = this.page(k);
    const raw = b.top + b.h - b.mB;
    for (const { n, room } of upTo) {
      const ref = marks.get(n.id);
      if (!ref || ref.closest("td, th")) continue;
      let holder: HTMLElement | null = null;
      for (let e: HTMLElement | null = ref; e && e !== this.flow; e = e.parentElement) {
        if (this.recs.has(e)) {
          holder = e;
          break;
        }
      }
      // Tables, wrappers of columns and the like are left to the larger room of the rounds.
      if (!holder || !holder.classList.contains("fp")) continue;
      const lines = this.lines(holder);
      const rr = ref.getBoundingClientRect();
      const mid = ((rr.top + rr.bottom) / 2 - flowTop) / sc;
      const at = lines.findIndex((l) => mid >= l.top - 1 && mid < l.bottom + 1);
      if (at < 0) continue;
      let top = lines[at].top;
      let bottom = lines[at].bottom;
      if (lines.length === 1 && this.recs.get(holder)?.whole) {
        const hr = holder.getBoundingClientRect();
        top = (hr.top - flowTop) / sc;
        bottom = (hr.bottom - flowTop) / sc;
      } else if (at > 0) top = Math.max(top, lines[at - 1].bottom);
      if (bottom <= raw - room + 0.5) continue;
      // The least room that moves the line, not the most (the page's text bottom less the line's top): any room between them moves the same
      // line, and the answer must not depend on which of them the rounds happened to reach. A page that needs more than that anyway (the
      // notes of the lines above it) keeps what it needs, and one that finds the line chased in only one of two layouts holds it with the least.
      return Math.max(0, layoutUnits(raw - bottom) + 1);
    }
    return 0;
  },
  /**
   * The footnote space each page needs, given the current layout, and which notes each page shows. A note shows on the page
   * of its reference; those that don't fit under the text carry on to the next page, after a continuation separator. A
   * section that starts the numbering again on each page gets its marks renumbered here.
   */
  noteNeeds(this: Paginator): number[] {
    const notes = this.setup?.notes ?? [];
    const byId = new Map(notes.map((n) => [n.id, n]));
    const fr = this.flow.getBoundingClientRect();
    const sc = this.scale();
    const refs: RNote[][] = [];
    const marks = new Map<string, HTMLElement>();
    const all = Array.from(this.flow.querySelectorAll<HTMLElement>(".atom.note[data-note]:not(.endnote)"));
    // The blocks that hold a reference the page map cannot place are drawn together, in one go: each one drawn alone is a pass over every block of the text.
    this.drawFarFor(all.filter((ref) => byId.has(ref.dataset.note!) && !this.inHiddenTab(ref)));
    for (const ref of all) {
      const n = byId.get(ref.dataset.note!);
      // A mark in a tab that is not shown has no place on these pages.
      if (!n || this.inHiddenTab(ref)) continue;
      const k = this.markPage(ref, fr.top, sc);
      const list = (refs[k] ??= []);
      if (!list.includes(n)) list.push(n);
      marks.set(n.id, ref);
    }
    // Numbering that starts again on each page: the marks in the text and the notes themselves take the new numbers.
    refs.forEach((list, k) => {
      const pr = this.secs[this.page(k).section]?.info.footnotePr;
      list.forEach((n, i) => {
        // Notes with a mark of their own (an empty label) keep it.
        const label = pr?.restart === "eachPage" && n.label !== "" ? noteLabel(pr.start + list.filter((m, j) => j < i && m.label !== "").length, pr.fmt) : n.label;
        if (label === n.label) return;
        const next = withLabel(n, label);
        list[i] = next;
        const el = marks.get(n.id);
        if (el && el.textContent !== label) el.textContent = label;
      });
    });
    const pages: RNote[][] = [];
    const need: number[] = [];
    this.noteRuns = [];
    this.chased = [];
    const last = this.pages - 1;
    let carry: RNote[] = [];
    for (let k = 0; k <= last; k++) {
      const queue = [...carry, ...(refs[k] ?? [])];
      if (!queue.length) {
        carry = [];
        continue;
      }
      const b = this.page(k);
      const w = b.w - b.ml - b.mr;
      const cap = Math.max(MIN_BODY, b.h - b.mT - b.mB - MIN_BODY);
      const from = carry.length > 0;
      let acc = this.sepHeight(from ? "continuation" : "separator", w);
      const take: RNote[] = [];
      // The room up to and with each note whose reference is on this page (a note carried from the page before has its reference there).
      const own = new Set(refs[k] ?? []);
      const upTo: { n: RNote; room: number }[] = [];
      for (const n of queue) {
        const key = `${n.id}@${w}`;
        if (!this.noteH.has(key)) this.noteH.set(key, this.measure(n.blocks, "notes-measure", w));
        const h = this.noteH.get(key)!;
        // The first note of a page always stays; the last page keeps every note.
        if (take.length && acc + h > cap && k < last) break;
        take.push(n);
        acc += h;
        if (own.has(n)) upTo.push({ n, room: acc });
      }
      // A page that has less room than its notes need may hold a line whose reference the room does not leave the line for.
      if (acc - (this.reserve[k] ?? 0) >= 1 && upTo.length) {
        const c = this.chasedRoom(k, upTo, marks, fr.top, sc);
        if (c > 0) this.chased[k] = c;
      }
      carry = queue.slice(take.length);
      const to = carry.length > 0;
      if (to) acc += this.noticeHeight(w);
      pages[k] = take;
      need[k] = acc;
      this.noteRuns[k] = { from, to };
    }
    this.notesByPage = pages;
    return need;
  },
  /** The endnotes each section collects at its own end (the sections whose options say `sectEnd`), and how tall they are. */
  findSectionEndnotes(this: Paginator) {
    this.endBySec = new Map();
    const notes = this.setup?.endnotes ?? [];
    if (!notes.length || !this.secs.some((s) => s.info.endnotePr?.pos === "sectEnd")) return;
    const byId = new Map(notes.map((n) => [n.id, n]));
    const all = this.flatBlocks();
    const secOf = this.sectionsOf(all);
    all.forEach((el, i) => {
      const si = secOf[i] ?? 0;
      if (this.secs[si]?.info.endnotePr?.pos !== "sectEnd" || el.closest("[data-tab-hidden]")) return;
      for (const ref of Array.from(el.querySelectorAll<HTMLElement>(".atom.note.endnote[data-note]"))) {
        const n = byId.get(ref.dataset.note!);
        if (!n) continue;
        const e = this.endBySec.get(si) ?? { notes: [], h: 0 };
        if (!e.notes.includes(n)) e.notes.push(n);
        this.endBySec.set(si, e);
      }
    });
    for (const [si, e] of this.endBySec) {
      const sec = this.secs[si];
      const w = sec.w - sec.ml - sec.mr;
      let h = END_GAP + END_PAD;
      for (const n of e.notes) {
        const key = `end${n.id}@${w}`;
        if (!this.noteH.has(key)) this.noteH.set(key, this.measure(n.blocks, "notes-measure", w));
        h += this.noteH.get(key)!;
      }
      e.h = h;
    }
  },
  /** The endnotes whose mark is in a tab that is shown: a document with tabs lays out only the open tab's endnotes. */
  shownEndnotes(this: Paginator, all: RNote[]): RNote[] {
    if (!all.length || !this.flow.querySelector("[data-tab-hidden]")) return all;
    const shown = new Set<string>();
    for (const ref of Array.from(this.flow.querySelectorAll<HTMLElement>(".atom.note.endnote[data-note]"))) {
      if (!ref.closest("[data-tab-hidden]")) shown.add(ref.dataset.note!);
    }
    return all.filter((n) => shown.has(n.id));
  },
  /**
   * Lay out the endnotes after the last block of the text (Word's "end of document"): a separator, then each note,
   * on the last page and on further pages when they don't fit. A note is not split across pages.
   */
  placeEndnotes(this: Paginator) {
    this.endByPage = [];
    const all = this.shownEndnotes(this.setup?.endnotes ?? []);
    if (!all.length) return;
    const areas: EndArea[] = [];
    // Endnotes at the end of their section go right under its last block (the room was kept by `padSectionEnds`).
    const sectional = new Set<RNote>();
    if (this.endBySec.size) {
      const flat = this.flatBlocks();
      const secOf = this.sectionsOf(flat);
      for (const [si, e] of this.endBySec) {
        let li = -1;
        secOf.forEach((x, i) => {
          if (x === si) li = i;
        });
        const el = flat[li];
        if (!el || el.classList.contains("cols")) continue;
        // Where its last block ends: measured by the pass that placed it (section endnotes make every pass a full one), the block drawn first.
        this.drawFar?.(el);
        const bottom = this.topOf(el) + this.hOf(el);
        const k = this.pageAt(bottom - 1);
        const area = areas[k] ?? { top: bottom + END_GAP, h: 0, notes: [] };
        area.notes.push(...e.notes);
        area.h += e.h - END_GAP;
        areas[k] = area;
        for (const n of e.notes) sectional.add(n);
      }
    }
    const notes = all.filter((n) => !sectional.has(n));
    let k = this.pages - 1;
    if (notes.length) {
      // The rest at the end of the document: after the last block, on the last page and on further pages when they don't fit.
      const taken = areas[k];
      let area: EndArea = { top: Math.max(this.bodyEnd, this.contentTop(k), taken ? taken.top + taken.h : 0) + END_GAP, h: END_PAD, notes: [] };
      for (const n of notes) {
        const b = this.page(k);
        const w = b.w - b.ml - b.mr;
        const key = `end${n.id}@${w}`;
        if (!this.noteH.has(key)) this.noteH.set(key, this.measure(n.blocks, "notes-measure", w));
        const h = this.noteH.get(key)!;
        // Doesn't fit below what is already on the page: carry on at the top of the next one.
        if (area.top + area.h + h > this.contentBottom(k) + 0.5 && (area.notes.length || area.top > this.contentTop(k) + 1)) {
          if (area.notes.length) areas[k] = areas[k] ? { ...areas[k], notes: [...areas[k].notes, ...area.notes], h: areas[k].h + area.h } : area;
          k++;
          this.box(k);
          area = { top: this.contentTop(k), h: END_PAD, notes: [] };
        }
        area.notes.push(n);
        area.h += h;
      }
      areas[k] = areas[k] ? { ...areas[k], notes: [...areas[k].notes, ...area.notes], h: areas[k].h + area.h } : area;
    }
    this.endByPage = areas;
    this.pages = Math.max(this.pages, k + 1);
  },
  /** Give a page's notes the separator above them and the notice below that the notes part has of its own. */
  decorateNotes(this: Paginator, el: HTMLElement, k: number) {
    const run = this.noteRuns[k];
    const seps = this.setup?.noteSeps?.footnote;
    if (run?.from) el.dataset.cont = "1";
    const sep = run?.from ? seps?.continuation : seps?.separator;
    if (sep) {
      el.dataset.sep = "custom";
      const holder = document.createElement("div");
      holder.className = "notes-sep";
      if (sep.line) {
        const line = document.createElement("div");
        line.className = "notes-line";
        holder.append(line);
      }
      for (const b of renderBlocks(sep.blocks ?? [], { ...this.env, readonly: true })) holder.append(b);
      el.prepend(holder);
    }
    if (run?.to && seps?.notice?.blocks?.length) {
      const notice = document.createElement("div");
      notice.className = "notes-notice";
      for (const b of renderBlocks(seps.notice.blocks, { ...this.env, readonly: true })) notice.append(b);
      el.append(notice);
    }
  },
};

declare module "./layout" {
  interface Paginator {
    notesForPass(kids: HTMLElement[]): Map<HTMLElement, NoteRef[]> | null;
    noteLines(el: HTMLElement, refs: NoteRef[], off: number): NoteLine[];
    looked(page: number, s: number): void;
    roomOnPage(lines: NoteLine[], page: number, room: number[], given: Given[]): void;
    takeBack(page: number, room: number[], given: Given[], from: number, goes: (l: NoteLine) => boolean): boolean;
    chasedRoom(k: number, upTo: { n: RNote; room: number; }[], marks: Map<string, HTMLElement>, flowTop: number, sc: number): number;
    noteNeeds(): number[];
    findSectionEndnotes(): void;
    shownEndnotes(all: RNote[]): RNote[];
    placeEndnotes(): void;
    decorateNotes(el: HTMLElement, k: number): void;
  }
}
