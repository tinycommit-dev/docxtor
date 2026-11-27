// The paginator's records: runs of far blocks, endnote areas, page boxes, sections, notes and per-block state.
import type { RNote, SectionInfo } from "@docxtor/core";

/** A run of blocks left out of the layout: the first and last of them, and the block after them that carries their height (see `Paginator.farApply`). */
export interface Stretch {
  first: HTMLElement;
  last: HTMLElement;
  carrier: HTMLElement;
}
/** The endnotes shown on one page: where the area starts (stage y) and how tall it is. */
export interface EndArea {
  top: number;
  h: number;
  notes: RNote[];
}

export type StoryZone = "header" | "footer" | "notes" | "endnotes";

/** An editable header/footer/notes element hosted on a sheet while it is being edited. */
export interface StoryHost {
  zone: StoryZone;
  page: number;
  el: HTMLElement;
}

/** One sheet of paper as laid out: where it sits in the stage and the geometry of the section that owns it. */
export interface PageBox {
  /** Top edge of the sheet in the stage (CSS px). Pages stack with `GAP` between them. */
  top: number;
  /** Left edge of the sheet in the stage: sheets are centred in a stage as wide as the widest one. */
  left: number;
  w: number;
  h: number;
  /** Top and bottom margins that apply, header and footer growth included. */
  mT: number;
  mB: number;
  /** Left and right margins of the owning section. */
  ml: number;
  mr: number;
  /** Index into `PageSetup.sections` of the section that owns the page (the one of its first block). */
  section: number;
  /** The first page the section owns. */
  first: boolean;
  /** The page's number as printed: sections can restart the numbering (`pageStart`). */
  num: number;
}

/**
 * One unbroken sheet instead of pages (Pageless, Reading and Outline views): no page breaks, headers, footers or page
 * borders, sections run on without starting a new page, and every section has the same width and margins. What is left
 * out here, the layout takes from the first section.
 */
export interface ContinuousLayout {
  /** The sheet's width in px (the first section's paper width when left out). */
  width?: number;
  /** Margins in px (the first section's when left out). */
  ml?: number;
  mr?: number;
  mt?: number;
  mb?: number;
}

/** A section's numbers in CSS px, with the part of the document that decides how it starts. */
export interface Sec {
  info: SectionInfo;
  start: string;
  w: number;
  h: number;
  mt: number;
  mb: number;
  ml: number;
  mr: number;
  /** The side margins on the even (left-hand) pages when the margins are mirrored; the same as `ml` and `mr` otherwise. */
  mlE: number;
  mrE: number;
  mirror: boolean;
  hdr: number;
  ftr: number;
  end?: number;
  /** Number of columns (1 where the browser can't lay columns out) and the gap between them. */
  cols: number;
  gap: number;
  /** Columns of unequal width: each one's width and its distance from the text's left edge (px). The paginator lays these out itself, since CSS columns are always equal. */
  colW?: number[];
  colX?: number[];
  /** Text that runs down the page (`tbRl`: lines from the right) or up it (`btLr`): the section is laid out as one vertical page. */
  vert?: "tbRl" | "btLr";
}

/**
 * What the last pass knew of one top-level block: the page map, per block. A pass that starts part-way down the document
 * (see `Paginator.layout`) takes its starting state from here, and stops where the state on arriving at a block is the
 * one this record has.
 */
/** A footnote reference in the text, its note, and whether the pass has given the note its room (`roomOnPage`). */
export interface NoteRef {
  ref: HTMLElement;
  note: RNote;
  done: boolean;
  /** The page that took this note's room back when its line went on (`takeBack`): the note gets no room there. */
  leftOn?: number;
}
/** A line of a paragraph where the pass has it (`shift`: the paragraph's own queued spacers above it), with the references on it. */
export interface NoteLine {
  top: number;
  bottom: number;
  shift: number;
  n?: NoteRef[];
}
/** Room the pass gave a page for a note: how much, and for which reference on which line. */
export interface Given {
  k: number;
  add: number;
  r: NoteRef;
  line: NoteLine;
}

export interface BlockRec {
  /** Where the pass stood on reaching the block: the page it was on, everything moved so far (pending pushes and spacers, px), and whether a page break was due. */
  entryPage: number;
  acc: number;
  force: boolean;
  /** Where the block sits with nothing pushed or cut anywhere (flow px, in whole layout units), and that relative to the text area of the entry page. */
  cx: number;
  rel: number;
  /** The page the block starts on. */
  page: number;
  /** The page it ends on (the same unless a page cuts it). */
  last: number;
  /** The block sits whole on that page (it ends on it, inside its text area): everything in it is on that page too, with nothing to measure. */
  whole: boolean;
  /** The lines of the block's paragraphs, recorded when the block was measured (see `Paginator.blockLines`); undefined until something asks for them. */
  lines?: BlockLines;
  /** For a paragraph of a section with line numbers: the lines it numbers on each page from the one it starts on, as the last layout drew them. */
  numbered?: number[];
}

/** How many lines the paragraphs of one top-level block show, recorded with the page map so that nothing has to count them on the page. */
export interface BlockLines {
  /** The paragraphs of the block in document order (-1 for one with no id, drawn read-only), and the lines each shows. */
  ids: number[];
  counts: number[];
  /** The lines of the paragraphs of the text, and of those in text boxes inside it. */
  main: number;
  boxes: number;
}

/** What the paginator did, by count: read by the performance harness and the tests (`Paginator.stats`). */
export interface PaginatorStats {
  /** Passes over the blocks: full ones, incremental ones, and the blocks measured in all of them. */
  fullPasses: number;
  incPasses: number;
  measured: number;
  /** Incremental passes that stopped at a block, leaving the rest as it was (the others ran to the end of the text). */
  synced: number;
  /** Blocks measured by the last pass, and whether it was incremental. */
  lastMeasured: number;
  lastIncremental: boolean;
  /** Why layouts that were asked to be incremental were full ones instead, by reason. */
  fallbacks: Record<string, number>;
}

