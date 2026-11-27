// Pages: the setup of each sheet, its box, margins, header and footer, sections and where a point falls.
import type { Paginator } from "./layout";
import { GAP } from "./layout";
import type { ContinuousLayout, PageBox, StoryZone } from "./layout-types";
import type { PageSetup, RBlock, RStory, SectionInfo } from "@docxtor/core";
import { renderBlocks, tw } from "./render";
import { COLUMNS, CONTINUOUS_HEIGHT, NOTE_GAP, ZONES, sideMargins, topMargin, unequalColumns, verticalDirection } from "./layout-units";
import { halfPage } from "./imposition";
import { layoutTabs } from "./tabs";
import { topBlocks } from "./far";
import { applyDocGrid } from "./grid";

export const pageMethods = {
  /** Lay the text out as one unbroken sheet (or back on pages with null). Call `configure` with the page setup again, then `layout`. */
  setContinuous(this: Paginator, c: ContinuousLayout | null) {
    this.cont = c;
    this.boxes = [];
    this.seen = [];
    this.invalidate();
  },
  /** The page a top-level block starts on, from the page map. Undefined when `el` is not a top-level block or the map is not up to date (blocks were drawn since the last layout: ask the page). */
  pageOfBlock(this: Paginator, el: HTMLElement): number | undefined {
    return this.mapReady ? this.recs.get(el)?.page : undefined;
  },
  /** The page the top-level block holding `el` starts on (a paragraph in a table: the table's page). Undefined when the map does not know. */
  pageOf(this: Paginator, el: Element): number | undefined {
    if (!this.mapReady) return undefined;
    for (let n: Element | null = el; n && n !== this.flow; n = n.parentElement) {
      const r = this.recs.get(n as HTMLElement);
      if (r) return r.page;
    }
    return undefined;
  },
  /** The page everything inside the top-level block holding `el` is on, when the page map says the block sits whole on one page. Undefined otherwise (measure, with the block drawn). */
  wholePageOf(this: Paginator, el: Element): number | undefined {
    if (!this.mapReady) return undefined;
    const r = this.recOf(el);
    return r?.whole ? r.page : undefined;
  },
  /** The first top-level block that starts on page k or a later page (undefined when the map does not know). */
  firstBlockFrom(this: Paginator, k: number): HTMLElement | undefined {
    return this.mapReady ? this.firstOnPage[Math.max(0, k)] : undefined;
  },
  configure(this: Paginator, p: PageSetup) {
    this.invalidate();
    this.setup = p;
    this.noteH.clear();
    this.partH.clear();
    const infos: SectionInfo[] = p.sections?.length
      ? p.sections
      : [{ start: "nextPage", width: p.width, height: p.height, margin: p.margin, header: p.header, footer: p.footer, cols: { num: p.cols, space: 0, sep: false, widths: [] }, titlePage: p.titlePage, headers: {}, footers: {} }];
    // A book fold's margins are inside and outside, as mirrored margins are.
    const place = { gutterAtTop: !!p.gutterAtTop, mirrored: !!p.mirrorMargins || !!p.bookFold };
    this.secs = infos.map((info) => ({
      info,
      start: String(info.start),
      w: tw(info.width),
      h: tw(info.height),
      mt: tw(topMargin(info, place)),
      mb: tw(info.margin[2]),
      ml: tw(sideMargins(info, place).left),
      mr: tw(sideMargins(info, place).right),
      mlE: tw(sideMargins(info, { ...place, even: true }).left),
      mrE: tw(sideMargins(info, { ...place, even: true }).right),
      mirror: place.mirrored,
      hdr: tw(info.header),
      ftr: tw(info.footer),
      end: info.end,
      cols: COLUMNS || unequalColumns(info) ? Math.max(1, Math.min(6, Math.round(info.cols?.num ?? 1))) : 1,
      gap: tw(info.cols?.space ?? 0),
      colW: unequalColumns(info)?.w,
      colX: unequalColumns(info)?.x,
      vert: verticalDirection(info),
    }));
    // Two pages to a sheet (2 pages per sheet, Book fold): each page is half the paper, as Word lays it out.
    if (p.bookFold || p.printTwoOnOne) {
      for (const s of this.secs) Object.assign(s, halfPage(s.w, s.h));
    }
    if (this.cont) {
      const a = this.secs[0];
      const c = this.cont;
      const w = c.width ?? a.w;
      const ml = c.ml ?? a.ml;
      const mr = c.mr ?? a.mr;
      const mt = c.mt ?? a.mt;
      const mb = c.mb ?? a.mb;
      for (const s of this.secs) Object.assign(s, { start: "continuous", w, h: CONTINUOUS_HEIGHT, mt, mb, ml, mr, mlE: ml, mrE: mr, mirror: false, hdr: 0, ftr: 0, cols: 1, colW: undefined, colX: undefined, vert: undefined });
    }
    this.stageW = Math.max(...this.secs.map((s) => s.w));
    const a = this.secs[0];
    this.uniform = this.secs.every((s) => Math.abs(s.w - a.w) < 0.01 && Math.abs(s.ml - a.ml) < 0.01 && Math.abs(s.mr - a.mr) < 0.01);
    // Like Word, a header or footer taller than its margin pushes the body text inward.
    // Measure what the pages can show now, so the layout pass finds the heights ready.
    this.secs.forEach((_, si) => {
      for (const zone of ZONES) for (const v of ["default", "first", "even"]) if (this.partOf(si, zone, v)) this.height(si, zone, v);
    });
    // Until the next layout the pages stay as they were (the editor asks which section a page is in between
    // reading the page setup and laying out); a document not laid out yet gets its first page.
    if (!this.boxes.length) {
      this.cur = 0;
      this.seen = [];
      this.box(0);
    }
    const s = this.flow.style;
    s.width = `${this.stageW}px`;
    s.paddingTop = `${this.margins(0, this.secs[0].info.pageStart ?? 1, true).mT}px`;
    // With one page width and one pair of side margins the flow's padding is the margin (the common case).
    // Otherwise each block gets its own offsets (see `applySides`).
    s.paddingRight = this.uniform ? `${a.mr}px` : "0px";
    s.paddingLeft = this.uniform ? `${a.ml}px` : "0px";
    this.stage.style.width = `${this.stageW}px`;
    // Hyphenation covers the text, headers, footers and notes alike: they all sit in the stage.
    this.stage.style.hyphens = p.hyphenation ? "auto" : "";
    if (p.hyphenation) this.stage.lang = p.lang || "en";
    else this.stage.removeAttribute("lang");
  },
  measure(this: Paginator, blocks: RBlock[] | undefined, cls: string, width: number) {
    if (!blocks?.length) return 0;
    const box = document.createElement("div");
    box.className = cls;
    box.style.cssText = `position:absolute;visibility:hidden;left:0;top:0;width:${width}px`;
    for (const el of renderBlocks(blocks, { ...this.env, readonly: true })) box.append(el);
    this.stage.append(box);
    layoutTabs(box, this.defaultTab);
    const hgt = box.offsetHeight;
    box.remove();
    return hgt;
  },
  /** The story a section shows as its header or footer of this variant (`default`, `first`, `even`), if it has one. */
  partOf(this: Paginator, si: number, zone: "header" | "footer", variant: string): RStory | undefined {
    const p = this.setup;
    if (!p) return undefined;
    const info = this.secs[si]?.info;
    const name = info && (zone === "header" ? info.headers : info.footers)?.[variant];
    if (name) return p.parts?.[name];
    // A document that lists no sections: the page setup's own header and footer.
    return p.sections?.length ? undefined : (zone === "header" ? p.headers : p.footers)[variant];
  },
  /** Measured height of a header or footer part at the width of its section's text. */
  height(this: Paginator, si: number, zone: "header" | "footer", variant: string): number {
    const story = this.partOf(si, zone, variant);
    const s = this.secs[si];
    if (!story || !s) return 0;
    const w = s.w - s.ml - s.mr;
    const key = `${story.part}@${w}`;
    let h = this.partH.get(key);
    if (h === undefined) this.partH.set(key, (h = this.measure(story.blocks, "hf", w)));
    return h;
  },
  /** Top and bottom margins of page number `num` when section `si` owns it: the section's, or more if the header/footer needs the room. */
  margins(this: Paginator, si: number, num: number, first: boolean) {
    const s = this.secs[si];
    if (this.cont) return { mT: s.mt, mB: s.mb };
    const hv = this.variantOf(si, "header", num, first);
    const fv = this.variantOf(si, "footer", num, first);
    const hdr = hv ? this.height(si, "header", hv) : 0;
    const ftr = fv ? this.height(si, "footer", fv) : 0;
    return { mT: Math.max(s.mt, hdr ? s.hdr + hdr + 6 : 0), mB: Math.max(s.mb, ftr ? s.ftr + ftr + 6 : 0) };
  },
  /** Which variant of its header/footer page number `num` shows, given the section that owns it. */
  variantOf(this: Paginator, si: number, zone: "header" | "footer", num: number, first: boolean): string | undefined {
    const has = (v: string) => !!this.partOf(si, zone, v);
    if (this.secs[si]?.info.titlePage && first) return has("first") ? "first" : undefined;
    // Word shows even-page headers only when the document asks for them, on pages whose printed number is even. A document that
    // asks and has none for even pages shows nothing there, not the odd pages' header.
    if (this.setup?.evenAndOddHeaders && num % 2 === 0) return has("even") ? "even" : undefined;
    return has("default") ? "default" : undefined;
  },
  /** The box of page k, made (with the geometry of the section being placed) when the layout reaches it. */
  box(this: Paginator, k: number): PageBox {
    while (this.boxes.length <= k) {
      const n = this.boxes.length;
      const si = this.cur;
      const s = this.secs[si];
      const first = !this.seen[si];
      this.seen[si] = true;
      const prev = this.boxes[n - 1];
      // A section that restarts the numbering does it on its first page; other pages count on.
      const num = first && s.info.pageStart !== undefined ? s.info.pageStart : (prev?.num ?? 0) + 1;
      const m = this.margins(si, num, first);
      const even = s.mirror && num % 2 === 0;
      this.boxes.push({ top: prev ? prev.top + prev.h + GAP : 0, left: (this.stageW - s.w) / 2, w: s.w, h: s.h, mT: m.mT, mB: m.mB, ml: even ? s.mlE : s.ml, mr: even ? s.mrE : s.mr, section: si, first, num });
    }
    return this.boxes[k];
  },
  /** The box of page k (the last one for a page past the end). */
  page(this: Paginator, k: number): Readonly<PageBox> {
    return this.boxes[Math.max(0, Math.min(this.boxes.length - 1, k))] ?? this.box(0);
  },
  /** The top and bottom margins of the section that owns page k (px): where its text area starts and ends, header and footer growth left out. */
  sectionMargins(this: Paginator, k: number): { top: number; bottom: number } {
    const s = this.secs[this.page(k).section];
    return { top: s?.mt ?? 0, bottom: s?.mb ?? 0 };
  },
  /** Which header/footer variant shows on page k (`first`, `even`, `default`), if any. */
  variantAt(this: Paginator, k: number, zone: "header" | "footer"): string | undefined {
    if (!this.setup) return undefined;
    const b = this.page(k);
    return this.variantOf(b.section, zone, b.num, b.first);
  },
  /** The header or footer story page k shows. */
  storyAt(this: Paginator, k: number, zone: "header" | "footer"): RStory | undefined {
    const v = this.variantAt(k, zone);
    return v ? this.partOf(this.page(k).section, zone, v) : undefined;
  },
  contentTop(this: Paginator, k: number) {
    const b = this.box(k);
    return b.top + b.mT;
  },
  contentBottom(this: Paginator, k: number) {
    const b = this.box(k);
    return b.top + b.h - b.mB - (this.reserve[k] ?? 0);
  },
  /** The page and margin zone under a stage y coordinate. */
  zoneAt(this: Paginator, y: number): { page: number; zone: StoryZone | "body" } {
    const page = this.pageAt(y);
    const b = this.page(page);
    const yIn = y - b.top;
    if (yIn < b.mT) return { page, zone: "header" };
    const bottom = b.h - b.mB;
    if (yIn > bottom) return { page, zone: "footer" };
    if (this.notesByPage[page]?.length && yIn > bottom - (this.reserve[page] ?? 0) + NOTE_GAP / 2) return { page, zone: "notes" };
    const end = this.endByPage[page];
    if (end && y >= end.top && y <= end.top + end.h) return { page, zone: "endnotes" };
    return { page, zone: "body" };
  },
  /** The height of the separator above a page's notes (Word's line, or the notes part's own). */
  sepHeight(this: Paginator, which: "separator" | "continuation", w: number): number {
    const sep = this.setup?.noteSeps?.footnote?.[which];
    if (!sep) return NOTE_GAP;
    const key = `sep:${which}@${w}`;
    if (!this.noteH.has(key)) this.noteH.set(key, (sep.line ? 10 : 0) + this.measure(sep.blocks, "notes-measure", w) + 6);
    return this.noteH.get(key)!;
  },
  /** The height of the notice below notes that go on to the next page (0 where the notes part has none). */
  noticeHeight(this: Paginator, w: number): number {
    const sep = this.setup?.noteSeps?.footnote?.notice;
    if (!sep?.blocks?.length) return 0;
    const key = `notice@${w}`;
    if (!this.noteH.has(key)) this.noteH.set(key, this.measure(sep.blocks, "notes-measure", w));
    return this.noteH.get(key)!;
  },
  /** Page index containing a flow y coordinate (a page owns the gap below it). */
  pageAt(this: Paginator, y: number): number {
    const boxes = this.boxes;
    let lo = 0;
    let hi = Math.min(boxes.length, this.pages) - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (boxes[mid].top <= y) lo = mid;
      else hi = mid - 1;
    }
    return Math.max(0, lo);
  },
  /** The first page whose bottom edge is below a stage y (the first one in view when y is the top of the view). */
  firstPageBelow(this: Paginator, y: number): number {
    const boxes = this.boxes;
    let lo = 0;
    let hi = Math.min(boxes.length, this.pages) - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (boxes[mid].top + boxes[mid].h > y) hi = mid;
      else lo = mid + 1;
    }
    return Math.max(0, lo);
  },
  /** How a section really starts: a continuous start on other paper is a new page, and a new column is a new page here. */
  startKind(this: Paginator, si: number): string {
    const s = this.secs[si];
    const prev = this.secs[si - 1];
    let kind = s.start;
    if (kind === "nextColumn") kind = "nextPage";
    if (kind === "continuous" && prev && (Math.abs(prev.w - s.w) > 0.5 || Math.abs(prev.h - s.h) > 0.5)) kind = "nextPage";
    return kind;
  },
  /** The page holding a flow y, making pages (of the section being placed) as far as needed. */
  pageFor(this: Paginator, y: number): number {
    let k = 0;
    while (true) {
      const b = this.box(k);
      if (y < b.top + b.h + GAP) return k;
      k++;
    }
  },
  /** The section each of a list of top-level blocks is in (the paragraph carrying a section's properties is the last of its section). */
  sectionsOf(this: Paginator, blocks: HTMLElement[]): number[] {
    const out: number[] = new Array(blocks.length).fill(0);
    if (this.secs.length < 2) return out;
    let si = 0;
    // A block-level content control (a "Centered" cover) can hold the paragraph that ends a section: the section
    // changes after the whole control.
    const ends = (el: HTMLElement, end: number) => (el.dataset.p ? Number(el.dataset.p) === end : !!el.querySelector(`[data-p="${end}"]`));
    for (let i = 0; i < blocks.length; i++) {
      out[i] = si;
      while (si < this.secs.length - 1 && this.secs[si].end !== undefined && ends(blocks[i], this.secs[si].end!)) si++;
    }
    return out;
  },
  /** Is section si laid out in columns? */
  multi(this: Paginator, si: number) {
    return (this.secs[si]?.cols ?? 1) > 1 || !!this.secs[si]?.vert;
  },
  /** Which section each top-level item is in, and which items open one (null with a single section). A wrapper is a whole section. */
  sectionIndex(this: Paginator, kids: HTMLElement[]) {
    if (this.secs.length < 2) return null;
    const sec: number[] = new Array(kids.length);
    const start: boolean[] = new Array(kids.length);
    let si = 0;
    let opens = false;
    for (let i = 0; i < kids.length; i++) {
      sec[i] = si;
      start[i] = opens;
      opens = false;
      if (kids[i].classList.contains("cols")) {
        sec[i] = Number(kids[i].dataset.sec ?? si);
        if (si < this.secs.length - 1) {
          si++;
          opens = true;
        }
        continue;
      }
      const id = kids[i].dataset.p;
      if (id && si < this.secs.length - 1 && this.secs[si].end === Number(id)) {
        si++;
        opens = true;
      }
    }
    return { sec, start };
  },
  /** Sections with a document grid: lines snap to its pitch and East Asian characters take its width (see grid.ts). */
  docGrid(this: Paginator) {
    if (!this.gridOn && !this.secs.some((s) => s.info.docGrid)) return;
    const flat: HTMLElement[] = [];
    for (const k of topBlocks(this.flow)) {
      if (k.classList.contains("cols")) flat.push(...(Array.from(k.children) as HTMLElement[]));
      else flat.push(k);
    }
    const secOf = this.sectionsOf(flat);
    this.gridOn = applyDocGrid(flat, secOf.map((i) => this.secs[i].info));
  },
};

declare module "./layout" {
  interface Paginator {
    setContinuous(c: ContinuousLayout | null): void;
    pageOfBlock(el: HTMLElement): number | undefined;
    pageOf(el: Element): number | undefined;
    wholePageOf(el: Element): number | undefined;
    firstBlockFrom(k: number): HTMLElement | undefined;
    configure(p: PageSetup): void;
    measure(blocks: RBlock[] | undefined, cls: string, width: number): number;
    partOf(si: number, zone: "header" | "footer", variant: string): RStory | undefined;
    height(si: number, zone: "header" | "footer", variant: string): number;
    margins(si: number, num: number, first: boolean): { mT: number; mB: number; };
    variantOf(si: number, zone: "header" | "footer", num: number, first: boolean): string | undefined;
    box(k: number): PageBox;
    page(k: number): Readonly<PageBox>;
    sectionMargins(k: number): { top: number; bottom: number; };
    variantAt(k: number, zone: "header" | "footer"): string | undefined;
    storyAt(k: number, zone: "header" | "footer"): RStory | undefined;
    contentTop(k: number): number;
    contentBottom(k: number): number;
    zoneAt(y: number): { page: number; zone: StoryZone | "body"; };
    sepHeight(which: "separator" | "continuation", w: number): number;
    noticeHeight(w: number): number;
    pageAt(y: number): number;
    firstPageBelow(y: number): number;
    startKind(si: number): string;
    pageFor(y: number): number;
    sectionsOf(blocks: HTMLElement[]): number[];
    multi(si: number): boolean;
    sectionIndex(kids: HTMLElement[]): { sec: number[]; start: boolean[]; } | null;
    docGrid(): void;
  }
}
