// Pagination over a single contenteditable flow.
//
// The flow is one continuous editable column laid over absolutely positioned
// "paper" sheets. After every change we measure blocks once, then push blocks that
// cross a page boundary down to the next page (via margin), splitting long
// paragraphs at line boundaries and long tables at row boundaries with
// non-editable spacer elements. Spacers are zero-length for selection mapping.
//
// Every page has a box (`PageBox`): the paper size, margins, header and footer of the section that owns
// it (the one of the first block placed on it), stacked with a gap and centred in a stage as wide as the
// widest page. A section starts where its break says: a new page, the same page (continuous, unless the
// paper changes), or the next even or odd page number with a blank page in between. Blocks sit within
// their own section's side margins: the flow's padding when every section has the same width and side
// margins (the common case), otherwise per-block margins (`applySides`).
//
// A section with columns is one wrapper (`div.cols`) in the flow, holding that section's blocks. The browser
// lays the columns out (CSS multi-column with `column-wrap`): each row of columns is exactly one page's text
// area, so the pages a section uses are the rows of its wrapper. See `placeColumns`.
import type { PageSetup, RNote } from "@docxtor/core";
import { type RefPara } from "./field-text";
import { type RenderEnv } from "./render";
import type { BlockRec, ContinuousLayout, EndArea, PageBox, PaginatorStats, Sec, StoryHost, Stretch } from "./layout-types";
import { measureLayoutUnit } from "./layout-units";
import { farBoxMethods } from "./layout-far-boxes";
import { farMethods } from "./layout-far";
import { measureMethods } from "./layout-measure";
import { pageMethods } from "./layout-pages";
import { noteMethods } from "./layout-notes";
import { passMethods } from "./layout-pass";
import { flowMethods } from "./layout-flow";
import { splitMethods } from "./layout-split";
import { columnMethods } from "./layout-columns";
import { sheetMethods } from "./layout-sheets";

export const GAP = 28;
export class Paginator {
  pages = 1;
  boxes: PageBox[] = [];
  secs: Sec[] = [];
  setup: PageSetup | null = null;
  /** The widest page: the stage and the flow are this wide, narrower sheets are centred in it. */
  stageW = 816;
  /** Every section has the same page width and side margins: the flow's own padding is the margin. */
  uniform = true;
  /** Blocks carry their own side offsets (a section with other width or side margins exists). */
  sidesOn = false;
  /** Which section each top-level block belongs to, and which blocks open one (null with a single section). */
  ix: { sec: number[]; start: boolean[] } | null = null;
  /** The section the block being placed belongs to: pages created on the way belong to it. */
  cur = 0;
  /** While a pass gives pages their notes' room as it fills them: per page, how much a decision that looked at the page's text bottom (`looked`) could lose to room given after it. */
  slack: number[] = [];
  seen: boolean[] = [];
  sectionPages: number[] = [];
  /** Lines drawn between columns, in flow coordinates (see `placeColumns`). */
  rules: { page: number; x: number; y1: number; y2: number }[] = [];
  story: StoryHost | null = null;
  /** The layout is one unbroken sheet (see `ContinuousLayout`), or null for pages. */
  cont: ContinuousLayout | null = null;
  /** Called with the root of fields that were just filled (a header or footer, the text), for field codes. */
  fieldHook: ((root: HTMLElement) => void) | null = null;
  /** Footnote space reserved at the bottom of each page. */
  reserve: number[] = [];
  /**
   * The first pass of a full layout gives each page the room of its notes as it fills the page (`roomOnPage`), instead of laying the
   * whole text out with the room of the last layout (or none) and then again with what that found: a book with notes on many pages
   * took three whole passes to open and to zoom. It keeps no floors: a reference that the room for its own note pushes on is left to
   * the rounds after the pass, which still compare what the pages need with what they were given. The pass lays a page out before it knows
   * all the room the page ends with, so what it looked at on the way could have gone the other way with that room (`looked`, `slack`): the
   * rounds make one more pass over the pages where it could (see `verify` in `layoutBody`) before they take its answer.
   */
  roomInPass = false;
  /** Whether a full layout gives pages their notes' room as it fills them (`roomInPass`). Off in tests that look at the rounds after it. */
  notesInPass = true;
  /** Whether the last layout's first pass did (the check then compares it with the rounds' answer). */
  usedRoomInPass = false;
  /** Footnotes shown on each page (by where their reference falls, and notes that carried on from the page before). */
  notesByPage: RNote[][] = [];
  /** Endnotes that go at the end of their section (`sectEnd`): per section, the notes and the room they take below its last block. */
  endBySec = new Map<number, { notes: RNote[]; h: number }>();
  /** The room (px) each section's last block keeps below it for those endnotes, by position in the flow. */
  endPad = new Map<number, number>();
  /** Per page, for sections that put the chapter number before the page number: the chapter's number and the page's number in it. */
  chapterAt: ({ label: string; n: number } | undefined)[] = [];
  /** Per page: do the notes start with ones that carried on from the page before, and do some go on to the next page. */
  noteRuns: { from: boolean; to: boolean }[] = [];
  /**
   * Per page, from the last `noteNeeds`: the room that moves only the line holding the first reference its room does not leave the line for
   * (the page's text bottom less that line's top), or 0 where every line with a reference fits above its notes' room.
   */
  chased: number[] = [];
  noteH = new Map<string, number>();
  /** Endnotes shown on each page, after the last block of the text (they flow onto further pages). */
  endByPage: (EndArea | undefined)[] = [];
  /** Where the last block of the text ends (flow y), as of the last pass. */
  bodyEnd = 0;
  /** Measured heights of header/footer parts at a width, and of notes (cleared when the page setup is read again). */
  partH = new Map<string, number>();
  /** What the last pass saw on each page, for vertical alignment: the blocks placed whole, where the last one ends (flow y), and pages with a block cut by the page. */
  onPage: number[][] = [];
  pageEnd: number[] = [];
  dirty = new Set<number>();
  /** Extra space above blocks (px, by position in the flow) that vertical alignment asks for; the next pass applies it. */
  vshift = new Map<number, number>();
  /** The page map (see `BlockRec`). A block drawn again is a new element, so it has no record until a pass reaches it. */
  recs = new WeakMap<HTMLElement, BlockRec>();
  /** Per page: the first block that starts on that page or a later one (what a block's own page is looked up in, in order). */
  firstOnPage: HTMLElement[] = [];
  /** The pages the text takes, as the last pass counted them (endnotes can add pages after it). */
  bodyPages = 1;
  /** The footnote reserves the last pass was made with. */
  passReserve: number[] = [];
  /** The page map describes the flow as the last pass left it, apart from the blocks listed in `dirtyKids`. */
  mapValid = false;
  /** Blocks the editor drew since the last pass; with `allDirty` (something else changed, or too many blocks) the next pass is a full one. */
  dirtyKids = new Set<HTMLElement>();
  allDirty = true;
  /**
   * Asked before the paginator measures something in a block the page map cannot answer for (a mark in a paragraph cut by a page, in a table
   * cut by a page, a wrapper of columns): the block is drawn (taken back into the layout) if it was left out. The editor puts the far mark
   * back after the layout (`Editor.updateFar`).
   */
  drawFar: ((el: HTMLElement) => void) | null = (el) => this.showBlock(el);
  /** Counts of what the paginator did (see `PaginatorStats`). */
  readonly stats: PaginatorStats = { fullPasses: 0, incPasses: 0, synced: 0, measured: 0, lastMeasured: 0, lastIncremental: false, fallbacks: {} };
  /**
   * Checks for tests and development: after every incremental layout, a full one is made in place and the two are compared
   * (page by page, spacer by spacer, margin by margin); a difference throws. Set by the host (never on in production).
   */
  verify = (globalThis as { __DOCXTOR_PAGINATION_CHECKS__?: unknown }).__DOCXTOR_PAGINATION_CHECKS__ === true;

  constructor(
    public flow: HTMLElement,
    public sheets: HTMLElement,
    public stage: HTMLElement,
    public env: RenderEnv,
  ) {
    measureLayoutUnit();
  }

  /** The page map can be read: a pass has described the flow, and nothing has been drawn since. */
  get mapReady(): boolean {
    return this.mapValid && !this.allDirty && this.dirtyKids.size === 0;
  }

  get continuous(): boolean {
    return !!this.cont;
  }

  // ---------------------------------------------------------------------------
  // Pages far from the view
  // ---------------------------------------------------------------------------
  //
  // A far top-level block is `display: none` (`data-far`), so it takes no part in Chrome's layout. The stretch of them keeps its height as
  // a part of the top margin of the first block drawn after it (`data-fg`, the "gap"): the height of the stretch is the distance between
  // the margin edges of the first block left out and of that block, which the page map has (`me`: where the block's margin box starts,
  // from the record's natural top, its base margin and everything pushed above it). Hiding and drawing blocks only moves marks and gaps;
  // each block keeps its own margin (base and push) and spacers, so drawing everything again gives exactly the layout the pass made.

  /** How many blocks are left out of the layout now. */
  farN = 0;
  /** The blocks the passes of the last layout laid out again (their places among the page's blocks, first to last), and the blocks they went over. */
  touched: [number, number] | null = null;
  passKids: HTMLElement[] = [];
  passCount = 0;
  /** Blocks were drawn or left out since `setFar` last set the stretch. */
  farMoved = false;

  // The stretches, kept as records (first block, last block, the block that carries the gap), in the order of the text. Drawing or leaving out a block
  // finds its stretch among them (by where the page map puts the block), changes the records of that stretch and of its neighbours, and sets the
  // gaps of the blocks whose stretch changed: work in proportion to the blocks that change, never to the length of the text. What changes many blocks
  // at once (a full pass, `farShowAll`, the first stretch after opening) is `farApply`, which goes over every block and builds the records again.
  stretches: Stretch[] = [];
  byFirst = new Map<HTMLElement, Stretch>();
  byCarrier = new Map<HTMLElement, Stretch>();
  /** The records describe the blocks. A change they cannot follow clears this, and the next change is made the long way (which builds them again). */
  farIdx = true;
  /** The blocks whose gap has to be worked out again (their stretch changed), before the next layout reads anything. */
  farTouched = new Set<HTMLElement>();
  /** Blocks looked at by the bookkeeping of far pages since the start (exact; the performance harness reads it per key and per undo), and times it went over every block. */
  farVisits = 0;
  farLong = 0;

  leavable = new WeakMap<HTMLElement, boolean>();

  // ---- The boxes (a run of far blocks in one hidden element, see far.ts) ----

  // ---- The records (the short way) ----

  /** How many blocks are left out of the layout now (exact, for the tests and the performance harness). */
  get farBlocks(): number {
    return this.farN;
  }

  /** Something asked for line counts: from now on every pass records them with the page map (see `blockLines`). */
  wantLines = false;

  /** The blocks the passes of this layout measured (their records were reset): the ones whose lines are recorded after it. */
  reset: HTMLElement[] = [];

  /** The document's default tab stop width (twips). */
  get defaultTab(): number {
    return this.setup?.defaultTab || 720;
  }

  /** Width of the stage: the widest page. */
  get stageWidth(): number {
    return this.stageW;
  }

  /** Whether any paragraph has a document grid put on it (so it must be taken off when the grid goes). */
  gridOn = false;

  /** Whether the footnote room the last layout ended with is what its notes need (more on a page that room for its own note pushed a reference off: see `layoutBody`). */
  notesSettled = true;
  /** What the last pass did, for the check's message. */
  lastPlan = "";
  /** The last page whose footnote room differs from what the last pass had (-1: none). */
  reserveDiff = -1;

  /**
   * The editor's zoom, which the host sets (`Editor`): the flow is laid out at 100% and drawn scaled by it (a transform on an
   * ancestor), so every rectangle read from the page is the zoom times the layout's.
   */
  zoomOf: (() => number) | null = null;

  /**
   * Spacers `splitParagraph` has chosen but not yet put in the page, and their total height. Inserting a spacer
   * changes the layout, and the next measurement would make the browser lay the whole document out again: once per page
   * boundary, which made a 1,400-page novel take 20 seconds. So `pass` measures everything first and inserts the spacers
   * together (`flushSpacers`), before anything reads the page again.
   */
  queued: { node: Node; offset: number; h: number }[] = [];
  queuedH = 0;

  // ---------------------------------------------------------------------------
  // Columns
  // ---------------------------------------------------------------------------

  /**
   * What each sheet was last made from (see `sheetKey`). An incremental layout, the pause after typing, makes again only the sheets whose
   * ingredients differ: making all of them is most of what a pause costs in a long text (every page of the novel took its header, notes and
   * page number off and put them on again, 1,164 sheets, which Chrome then styled and laid out again: about 25 ms of 55).
   */
  sheetState: { sig: string; refs: unknown[] }[] = [];
  /** A header or footer asked for the text of the body (STYLEREF): the sheets then depend on what is typed, and are made again every time. */
  sheetsUseText = false;
  sheetFill: string | undefined = undefined;

  /** The paragraphs of the text with their pages, styles and numbers, for STYLEREF (made when first asked for in a layout). */
  refParas: RefPara[] | null = null;

}

// The methods that live in the modules beside this file (see the imports) are the class's own: installed on its prototype, as class methods are
// (not enumerable).
for (const methods of [farBoxMethods, farMethods, measureMethods, pageMethods, noteMethods, passMethods, flowMethods, splitMethods, columnMethods, sheetMethods]) {
  for (const [name, value] of Object.entries(methods)) Object.defineProperty(Paginator.prototype, name, { value, writable: true, configurable: true });
}

// Each module's methods must match what its `declare module` block says the class has, name by name and signature by signature:
// a method missing from the block, or one whose parameters or result differ from it, fails to compile here.
type Declared<T> = Pick<Paginator, keyof T & keyof Paginator> & Record<Exclude<keyof T, keyof Paginator>, never>;
void ([farBoxMethods, farMethods, measureMethods, pageMethods, noteMethods, passMethods, flowMethods, splitMethods, columnMethods, sheetMethods] satisfies [Declared<typeof farBoxMethods>, Declared<typeof farMethods>, Declared<typeof measureMethods>, Declared<typeof pageMethods>, Declared<typeof noteMethods>, Declared<typeof passMethods>, Declared<typeof flowMethods>, Declared<typeof splitMethods>, Declared<typeof columnMethods>, Declared<typeof sheetMethods>]);

/** Add a block to a list, or the blocks inside it when it is a block-level content control. */
/**
 * Cut a text node at `offset` and return the node holding the text from there on (the node itself at 0), as
 * `splitText` does. Not `splitText` itself: while the garbage of a large redraw waits to be collected (after Undo
 * replaced every paragraph of a novel), Chrome takes about 0.3 s for each `splitText`, minutes for a novel's page ends,
 * while the other ways of changing text stay under a millisecond (measured; one collection makes it fast again).
 */
export function cutText(t: Text, offset: number): Text {
  if (offset <= 0) return t;
  const rest = document.createTextNode(t.data.slice(offset));
  t.after(rest);
  t.deleteData(offset, t.length - offset);
  return rest;
}

export * from "./layout-types";
export * from "./layout-units";
