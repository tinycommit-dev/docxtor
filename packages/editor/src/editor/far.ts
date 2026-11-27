// Pages far from the view. Blocks on them are taken out of Chrome's layout (`display: none`, marked
// `data-far`), so a rectangle, an `offsetTop` or `scrollIntoView` on one of them tells nothing: everything that measures or jumps to a block
// goes through `Editor.ensureDrawn`, everything that lays the whole text out for output through `Editor.withDrawn`.
//
// A run of far blocks is kept in one hidden element, a box: Chrome tells the input method where the caret is on every key by walking
// the children of the editable flow, hidden or not, so 9,000 hidden siblings cost about 1 ms a key and one box costs nothing. A box is a child
// of the flow that is not a block: every walk over the top-level blocks goes through the functions here, which step through boxes.
//
// What is here is the one place the rest asks:
//  - `farKeepFor(pages)`: whether far pages are left out in a document of this length, and how many pages either side of the view stay drawn;
//  - `isFar(el)`: whether a block is left out of the layout now;
//  - `stripFar(root)`: a copy of the document carries no mark and no box;
//  - `FAR_GAP`: the margin a block carries for the far blocks before it;
//  - `isFarBox`, `isTopBlock`, `topBlockOf`, `nextBlock`, `prevBlock`, `firstBlock`, `lastBlock`, `topBlocks`: the top-level blocks in order,
//    boxes stepped through;
//  - `paintRange`: a range for a CSS highlight that keeps its text when a block moves into or out of a box.

/** The attribute that marks a top-level block as left out of the layout. */
export const FAR = "data-far";
/**
 * The block after a stretch of far blocks keeps their height as part of its top margin: this attribute holds that part (px), and the
 * block's `margin-top` is its own (base and push, as the paginator sets it) plus this. Never read by anything but the paginator.
 */
export const FAR_GAP = "data-fg";
/** A block's exact own top margin, kept beside the style text it was set with (a style's text holds six digits: too few for a gap of 30,000 px). Only the paginator reads it. */
export const FAR_OWN = "data-fm";

/** Far pages are left out in documents above this many pages, keeping `FAR_KEEP` pages either side of the view drawn. */
export const FAR_PAGES_ABOVE = 100;
export const FAR_KEEP = 2;

let switched: number | null | undefined;

/**
 * The test switch: `globalThis.__DOCXTOR_FAR_PAGES__ = n` before the app starts keeps `n` pages either side of the view drawn in any document,
 * however short (so the whole browser suite can run with a one-page stretch); a negative `n` draws every page in every document (the
 * performance harness's runs without far pages). Null when the switch is not set: the rule above applies.
 */
export function farPages(): number | null {
  if (switched === undefined) {
    const v = (globalThis as { __DOCXTOR_FAR_PAGES__?: unknown }).__DOCXTOR_FAR_PAGES__;
    switched = typeof v === "number" && Number.isFinite(v) ? (v < 0 ? -1 : Math.floor(v)) : null;
  }
  return switched;
}

/** How many pages either side of the view stay drawn in a document of `pages` pages, or null when every page is drawn. */
export function farKeepFor(pages: number): number | null {
  const sw = farPages();
  if (sw !== null) return sw < 0 ? null : sw;
  return pages > FAR_PAGES_ABOVE ? FAR_KEEP : null;
}

/** The class of the hidden element a run of far blocks is kept in. It carries `data-far` too, so `isFar` and the CSS that hides far blocks cover it. */
export const FAR_BOX = "far-box";

/** Is this element a box of far blocks? */
export function isFarBox(el: Element | null | undefined): boolean {
  return !!el && el.tagName === "DIV" && el.classList.contains(FAR_BOX);
}

/** Is `el` a top-level block of `flow`: a child of the flow that is not a box, or a child of a box? */
export function isTopBlock(flow: Element, el: Element): boolean {
  const p = el.parentElement;
  if (p === flow) return !isFarBox(el);
  return !!p && p.parentElement === flow && isFarBox(p);
}

/** The top-level block of `flow` that holds `el` (`el` itself when it is one; null when it is not in the flow, or is a box). */
export function topBlockOf(flow: Element, el: Element | null): HTMLElement | null {
  for (let n: Element | null = el; n; ) {
    const p: Element | null = n.parentElement;
    if (p === flow) return isFarBox(n) ? null : (n as HTMLElement);
    if (p && p.parentElement === flow && isFarBox(p)) return n as HTMLElement;
    n = p;
  }
  return null;
}

/** The first block in a box, or after it (boxes are not left empty; an empty one is stepped over). */
function enter(n: Element | null, dir: 1 | -1): HTMLElement | null {
  while (n && isFarBox(n)) n = (dir > 0 ? n.firstElementChild : n.lastElementChild) ?? (dir > 0 ? n.nextElementSibling : n.previousElementSibling);
  return n as HTMLElement | null;
}

/** The top-level block after `el` (a top-level block), boxes stepped through. */
export function nextBlock(el: Element): HTMLElement | null {
  const p = el.parentElement;
  return enter(el.nextElementSibling ?? (isFarBox(p) ? p!.nextElementSibling : null), 1);
}

/** The top-level block before `el` (a top-level block), boxes stepped through. */
export function prevBlock(el: Element): HTMLElement | null {
  const p = el.parentElement;
  return enter(el.previousElementSibling ?? (isFarBox(p) ? p!.previousElementSibling : null), -1);
}

/** The first top-level block of the flow. */
export function firstBlock(flow: Element): HTMLElement | null {
  return enter(flow.firstElementChild, 1);
}

/** The last top-level block of the flow. */
export function lastBlock(flow: Element): HTMLElement | null {
  return enter(flow.lastElementChild, -1);
}

/** The top-level blocks of the flow in order: its children, with the blocks of each box in its place. */
export function topBlocks(flow: Element): HTMLElement[] {
  const out: HTMLElement[] = [];
  for (let k = flow.firstElementChild; k; k = k.nextElementSibling) {
    if (isFarBox(k)) for (let b = k.firstElementChild; b; b = b.nextElementSibling) out.push(b as HTMLElement);
    else out.push(k as HTMLElement);
  }
  return out;
}

/**
 * A range to paint with a CSS highlight: fixed to its nodes (a `StaticRange`), so it keeps painting its text when the block moves into or out of
 * a box. A live `Range` there collapses to where the block stood the moment it moves (measure with a live one, after `Editor.ensureDrawn`).
 */
export function paintRange(r: Range): AbstractRange {
  const Fixed = (globalThis as { StaticRange?: typeof StaticRange }).StaticRange;
  return Fixed ? new Fixed({ startContainer: r.startContainer, startOffset: r.startOffset, endContainer: r.endContainer, endOffset: r.endOffset }) : r;
}

/** Is this block (or the block holding this element) left out of the layout now? Its rectangles are zeros then. */
export function isFar(el: Element | null): boolean {
  return !!el?.closest(`[${FAR}]`);
}

/** Take the far mark off a copy of the document (a clipboard payload, a printed page, a PDF's pages): the copy shows everything, its blocks out of their boxes. */
export function stripFar(root: Element | DocumentFragment) {
  for (const box of Array.from(root.querySelectorAll(`.${FAR_BOX}`))) box.replaceWith(...Array.from(box.childNodes));
  const own = root instanceof Element && root.hasAttribute(FAR) ? [root] : [];
  for (const el of [...own, ...Array.from(root.querySelectorAll(`[${FAR}]`))]) el.removeAttribute(FAR);
  // The gap a block carries for the far blocks before it belongs to the copy's layout as little as the mark does, and the exact margin the paginator keeps
  // beside the style text (any block it pushed) is nobody else's.
  const kept = root instanceof HTMLElement && (root.hasAttribute(FAR_GAP) || root.hasAttribute(FAR_OWN)) ? [root] : [];
  for (const el of [...kept, ...Array.from(root.querySelectorAll<HTMLElement>(`[${FAR_GAP}], [${FAR_OWN}]`))]) {
    if (el.hasAttribute(FAR_GAP)) el.style.marginTop = `${parseFloat(el.getAttribute(FAR_OWN) ?? "0") || 0}px`;
    el.removeAttribute(FAR_GAP);
    el.removeAttribute(FAR_OWN);
  }
}
