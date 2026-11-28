// How many lines of text a paragraph shows on the page: the rows its text and objects fall on. One count for Word count, Go to line and the
// line numbers of far pages; the paginator records it per block with the page map (`Paginator.blockLines`), so nothing has to count the
// lines of a block that is not drawn.

// Made when first needed: importing the module must not touch the DOM (a server imports the package).
let range: Range | undefined;
const scratch = () => (range ??= document.createRange());

/** One row of a paragraph: where it is on the screen (viewport px) and its left edge. */
export interface LineRow {
  top: number;
  bottom: number;
  left: number;
}

/** The rows a paragraph's text and objects fall on, top to bottom (a paragraph with nothing drawn but a box is one row; one with no box, none). Measured where the paragraph is: it must be drawn. */
export function lineRows(el: HTMLElement): LineRow[] {
  const rects: DOMRect[] = [];
  const ownText = (n: Node) => n.parentElement?.closest(".fp") === el && !n.parentElement.closest(".shape-text, .lbl, .ghost, .pgsp, [hidden]");
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (!(n as Text).data.trim() || !ownText(n)) continue;
    const rg = scratch();
    rg.selectNodeContents(n);
    for (const r of Array.from(rg.getClientRects())) if (r.height > 1) rects.push(r);
  }
  // Pictures and other objects sit on a line too.
  for (const o of Array.from(el.querySelectorAll<HTMLElement>("img, .atom"))) {
    if (o.closest(".fp") !== el || o.closest(".shape-text, .lbl, .ghost, .pgsp")) continue;
    const r = o.getBoundingClientRect();
    if (r.height > 1 && r.width > 0) rects.push(r);
  }
  if (!rects.length) {
    const b = el.getBoundingClientRect();
    return b.height > 1 ? [{ top: b.top, bottom: b.bottom, left: b.left }] : [];
  }
  rects.sort((a, b) => a.top - b.top);
  const rows: LineRow[] = [];
  for (const r of rects) {
    const last = rows[rows.length - 1];
    if (!last || (r.top + r.bottom) / 2 > last.bottom) rows.push({ top: r.top, bottom: r.bottom, left: r.left });
    else {
      last.bottom = Math.max(last.bottom, r.bottom);
      last.left = Math.min(last.left, r.left);
    }
  }
  return rows;
}

/** How many lines of text a paragraph shows on the page: the rows its text and objects fall on. */
export function linesIn(el: HTMLElement): number {
  return lineRows(el).length;
}

/** The paragraphs of a block, in document order: the block itself when it is one, and those inside it (a table's, a text box's). A repeated table header is the same paragraph again, not counted. */
export function paragraphsOf(block: HTMLElement): HTMLElement[] {
  const out: HTMLElement[] = [];
  if (block.classList.contains("fp")) out.push(block);
  for (const el of Array.from(block.querySelectorAll<HTMLElement>(".fp"))) if (!el.closest(".hdr-repeat")) out.push(el);
  return out;
}
