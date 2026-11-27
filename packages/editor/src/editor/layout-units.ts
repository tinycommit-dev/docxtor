// The browser's layout unit and the lengths and room helpers built on it, the section helpers, and the paginator's constants.
import type { SectionInfo } from "@docxtor/core";
import { tw } from "./render";

/**
 * The browser's layout unit: Chrome and Safari keep lengths in 1/64 px and cut a length they are given toward zero; Firefox keeps
 * them in 1/60 px and rounds. Every length the paginator works out goes through `layoutUnits`, `snap` and `cut`, so it must use the
 * browser's own: with 1/64 in Firefox each block was off by up to a 120th of a pixel, which far pages add up (a third of a pixel after
 * a few hundred blocks, the far-pages check failing). Measured once, by `measureLayoutUnit`, before the first layout.
 */
let PER = 64;
let TRUNC = true;
let measuredUnit = false;

/** Find the browser's layout unit with a probe 1.6/60 px high: it comes out 1/64 cut (Chrome, Safari), 2/64, 2/60 rounded (Firefox) or 1/60. */
export function measureLayoutUnit(): { per: number; trunc: boolean } {
  if (measuredUnit || typeof document === "undefined" || !document.body) return { per: PER, trunc: TRUNC };
  const probe = document.createElement("div");
  probe.style.cssText = "position:absolute;left:0;top:0;width:1px;height:0.0266667px;margin:0;padding:0;border:0;visibility:hidden;contain:strict";
  document.body.append(probe);
  const h = probe.getBoundingClientRect().height;
  probe.remove();
  const known: [number, number, boolean][] = [
    [1 / 64, 64, true],
    [2 / 64, 64, false],
    [2 / 60, 60, false],
    [1 / 60, 60, true],
  ];
  const best = known.reduce((a, b) => (Math.abs(b[0] - h) < Math.abs(a[0] - h) ? b : a));
  // Nothing near one of the four (a zoomed page, a browser that keeps floats): keep Chrome's, as before.
  if (Math.abs(best[0] - h) < 1 / 512) [, PER, TRUNC] = best;
  measuredUnit = true;
  return { per: PER, trunc: TRUNC };
}

/**
 * A length in whole layout units (see `PER`). The paginator adds up the margins and spacers it inserts instead of measuring again
 * after each one; given whole units they move the text by exactly what was added, so the sum stays true over thousands of pages.
 */
export const layoutUnits = (px: number) => Math.round(px * PER) / PER;
/** The browser's layout unit in px (1/64 in Chrome and Safari, 1/60 in Firefox). */
export const layoutUnit = () => 1 / PER;
/** Chrome can wrap rows of columns (`column-wrap`); where it can't, a section with columns shows as one column. */
export const COLUMNS = typeof CSS !== "undefined" && typeof CSS.supports === "function" && CSS.supports("column-wrap", "wrap") && CSS.supports("column-height", "100px");
/** A section with columns that starts with less than this much room on the page starts on the next one. */
export const COLS_ROOM = 48;
/** A table row that may split starts on a page only when at least this much of it (px) fits there. */
export const SPLIT_ROOM = 36;
/** Columns end level (before a continuous section break) only in sections of this many pages or fewer: the browser's balancing costs more the more rows there are. */
export const BALANCE_ROWS = 8;

/**
 * The side margins a section's text really has, in twips: the gutter (extra room for binding) is added to the left
 * margin, or to the right one when the section says so (`rtlGutter`, for books bound on the right).
 */
export function sideMargins(info: { margin: number[]; gutter?: number; rtlGutter?: boolean }, o: { gutterAtTop?: boolean; mirrored?: boolean; even?: boolean } = {}): { left: number; right: number } {
  // A gutter at the top of the page is added to the top margin instead (see `topMargin`).
  const gutter = o.gutterAtTop ? 0 : info.gutter ?? 0;
  const left = info.margin[3] + (info.rtlGutter ? 0 : gutter);
  const right = info.margin[1] + (info.rtlGutter ? gutter : 0);
  // With mirrored margins the inside and outside swap on the left-hand (even) pages.
  return o.mirrored && o.even ? { left: right, right: left } : { left, right };
}

/** The top margin of a section in twips: the gutter goes here when the document puts it at the top. */
export function topMargin(info: { margin: number[]; gutter?: number }, o: { gutterAtTop?: boolean } = {}): number {
  return info.margin[0] + (o.gutterAtTop ? info.gutter ?? 0 : 0);
}

/** A page number as a section prints it: 1, i, I, a or A. */
export function pageLabel(n: number, format?: string): string {
  if (format === "lowerRoman" || format === "upperRoman") {
    let out = "";
    let v = Math.max(1, Math.floor(n));
    for (const [d, r] of [[1000, "m"], [900, "cm"], [500, "d"], [400, "cd"], [100, "c"], [90, "xc"], [50, "l"], [40, "xl"], [10, "x"], [9, "ix"], [5, "v"], [4, "iv"], [1, "i"]] as const)
      while (v >= d) {
        out += r;
        v -= d;
      }
    return format === "upperRoman" ? out.toUpperCase() : out;
  }
  if (format === "lowerLetter" || format === "upperLetter") {
    // Word's letters repeat after z: aa, bb, …
    const v = Math.max(1, Math.floor(n));
    const out = String.fromCharCode(97 + ((v - 1) % 26)).repeat(Math.floor((v - 1) / 26) + 1);
    return format === "upperLetter" ? out.toUpperCase() : out;
  }
  return String(n);
}
/** Space between body text and the footnote area (separator rule included). */
export const NOTE_GAP = 14;
/** Notes never take the room of the last lines of text: a page keeps at least this much (px) for body text, and notes that don't fit go on to the next page. */
export const MIN_BODY = 72;
/** Space between the end of the text and the separator of the endnotes, and the room the separator takes above the first note. */
export const END_GAP = 18;
export const END_PAD = 8;

/** The height a continuous sheet is laid out in: more than any document needs. The sheet is cut to the text afterwards. */
export const CONTINUOUS_HEIGHT = 1e7;

/** Word's vertical text directions as the two this paginator draws. */
export function verticalDirection(info: SectionInfo): "tbRl" | "btLr" | undefined {
  const d = info.textDirection;
  if (d === "tbRl" || d === "tbRlV") return "tbRl";
  if (d === "btLr") return "btLr";
  return undefined;
}

/** The widths and positions of a section's columns when they differ (null for equal columns). */
export function unequalColumns(info: SectionInfo): { w: number[]; x: number[] } | null {
  const c = info.cols;
  if (!c || c.num < 2 || c.num > 6 || c.widths.length !== c.num) return null;
  const w = c.widths.map(tw);
  const x: number[] = [];
  let at = 0;
  w.forEach((width, i) => {
    x.push(at);
    at += width + tw(c.spaces?.[i] ?? c.space);
  });
  return { w, x };
}

export const ZONES = ["header", "footer"] as const;

/** The fields (and date elements) whose value the layout works out. */
export const DYNAMIC = new Set(["PAGE", "NUMPAGES", "SECTIONPAGES", "SECTION", "STYLEREF", "pgNum", "dayShort", "dayLong", "monthShort", "monthLong", "yearShort", "yearLong"]);

/**
 * One scratch Range for each place that measures text, kept and used again. A Range made for every paragraph (the pagination measures
 * thousands of them in a pass) stays registered with the document until the garbage collector runs, and the browser walks every one
 * of them each time a node is removed from the page: with thousands about, taking a paragraph's underline boxes off cost 0.2 ms each.
 */
export const scratch = new Map<string, Range>();
export function scratchRange(name: string): Range {
  let r = scratch.get(name);
  if (!r) scratch.set(name, (r = document.createRange()));
  return r;
}

export const snap = (v: number) => Math.round(v * PER) / PER;

/** A length as the browser lays it out: whole layout units (the pages are laid out at 100%), cut toward zero in Chrome and Safari, rounded in Firefox. */
export const cut = (v: number) => (TRUNC ? Math.trunc(v * PER) : Math.round(v * PER)) / PER;

/** Do two lists of footnote room (one value per page) give every page the same room, to the pixel? */
/**
 * Two rooms are the same page to the rounds when they differ by less than a pixel (`sameRoom`), but a pass reuses what lies after the last
 * page whose room changed only when none of them changed at all (to a hair): a floor 0.8 px over what the notes need can be what keeps a
 * line on the next page, and the page that holds it is laid out again with the room it is given, whatever the rounds called the same.
 */
export const ROOM_EPS = 1 / 128;
export const sameRoom = (a: number[], b: number[]) => {
  for (let k = 0; k < Math.max(a.length, b.length); k++) if (Math.abs((a[k] ?? 0) - (b[k] ?? 0)) >= 1) return false;
  return true;
};
/** The room each page needs, where a page with a line to move (`chased`, from the same layout) takes the room that moves it if that is less. */
export const moveOnly = (need: number[], chased: number[]) => need.map((v, k) => (chased[k] > 0 ? Math.min(v, chased[k]) : v));
/**
 * The room a layout of a cycle gives to the floor. `first` is the first page that has a line to move (`chased`) in any layout of the
 * cycle. A page before it gives what it needs (no chase before it moves its text), a page with a line to move gives the room that
 * moves it, and a page after `first` that has none gives none: its room only changed because a chase before it moved the text, and it
 * takes what it needs in the round it is in.
 */
export const chasedOnly = (need: number[], chased: number[], first: number) =>
  need.map((v, k) => (k < first ? v : chased[k] > 0 ? Math.min(v, chased[k]) : 0));
/** The larger room of each page. */
export const raise = (a: number[], b: number[]) => Array.from({ length: Math.max(a.length, b.length) }, (_, k) => Math.max(a[k] ?? 0, b[k] ?? 0));

/** Thrown by a pass that started part-way down when it meets something it cannot finish that way: the layout is made again from the top. */
export class StartOver extends Error {
  constructor(readonly why: string) {
    super(why);
  }
}
/** More blocks drawn than this since the last pass: not worth starting from the first of them. */
export const MAX_DIRTY = 64;

