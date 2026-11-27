// DOM ⇄ model position mapping. A model position is (paragraph id, UTF-16 offset).
// Inside a paragraph element: text nodes count their length, `.atom` elements count 1,
// list labels, page-split spacers and placeholder <br>s count 0.
import type { MPos } from "@docxtor/core";

export function isZeroWidth(el: Element): boolean {
  return el.classList.contains("lbl") || el.classList.contains("pgsp") || el.classList.contains("ghost") || (el.tagName === "BR" && (el as HTMLElement).dataset.ph === "1");
}

function nodeLen(n: Node): number {
  if (n.nodeType === Node.TEXT_NODE) return (n as Text).data.length;
  if (n.nodeType !== Node.ELEMENT_NODE) return 0;
  const el = n as Element;
  if (isZeroWidth(el)) return 0;
  if (el.classList.contains("atom")) return 1;
  let s = 0;
  for (const c of Array.from(el.childNodes)) s += nodeLen(c);
  return s;
}

/**
 * Paragraph elements found by id, per editing root. Looking one up with `querySelector` walks the document until it meets
 * it (a novel has 27,000 elements), and a key does it a dozen times; here an element once found is remembered, and
 * `knowPara` is told about the ones the editor draws, so the paragraph being typed in is found at once. An entry is used
 * only while it is still in the root, so a paragraph drawn again (a new element) is looked for the slow way once.
 */
const known = new WeakMap<HTMLElement, Map<number, HTMLElement>>();

/** The editor drew `el` as the paragraph `id`: later lookups find it without a search. */
export function knowPara(root: HTMLElement, id: number, el: HTMLElement) {
  let m = known.get(root);
  if (!m) known.set(root, (m = new Map()));
  m.set(id, el);
}

/** The element of paragraph `id` in `root`, or null. */
export function paraIn(root: HTMLElement, id: number): HTMLElement | null {
  const m = known.get(root);
  const hit = m?.get(id);
  if (hit && hit.isConnected && root.contains(hit)) return hit;
  const el = root.querySelector<HTMLElement>(`[data-p="${id}"]`);
  if (el) knowPara(root, id, el);
  else m?.delete(id);
  return el;
}

export function paraOf(node: Node | null): HTMLElement | null {
  const el = node && (node.nodeType === Node.ELEMENT_NODE ? (node as Element) : node.parentElement);
  return (el?.closest("[data-p]") as HTMLElement) ?? null;
}

/** Offset of (node, offset) within paragraph `p`. */
function offsetIn(p: HTMLElement, node: Node, offset: number): number {
  // Inside an atom or zero-width element: snap to before/after it.
  let n: Node | null = node;
  while (n && n !== p) {
    if (n.nodeType === Node.ELEMENT_NODE) {
      const el = n as Element;
      if (el.classList.contains("atom") || isZeroWidth(el)) {
        const before = countBefore(p, el);
        return el.classList.contains("atom") && node === el && offset > 0 ? before + 1 : before;
      }
    }
    n = n.parentNode;
  }
  if (node.nodeType === Node.TEXT_NODE) return countBefore(p, node) + offset;
  // Element container: position before child[offset].
  const kids = node.childNodes;
  let s = countBefore(p, node);
  for (let i = 0; i < offset && i < kids.length; i++) s += nodeLen(kids[i]);
  return s;
}

/** Content length in `p` before `target` (in document order). */
function countBefore(p: Node, target: Node): number {
  let total = 0;
  let found = false;
  const walk = (n: Node) => {
    for (const c of Array.from(n.childNodes)) {
      if (found) return;
      if (c === target) {
        found = true;
        return;
      }
      if (c.contains(target)) {
        if (c.nodeType === Node.ELEMENT_NODE && ((c as Element).classList.contains("atom") || isZeroWidth(c as Element))) {
          found = true;
          return;
        }
        walk(c);
        return;
      }
      total += nodeLen(c);
    }
  };
  if (p === target) return 0;
  walk(p);
  return total;
}

export function domToModel(root: HTMLElement, node: Node, offset: number): MPos | null {
  // Between the paragraphs of a shape's text (select all in a text box): the nearest paragraph in it, not the paragraph holding the shape.
  const between = node.nodeType === Node.ELEMENT_NODE && (node as Element).classList.contains("shape-text");
  let p = between ? null : paraOf(node);
  if (p && root.contains(p)) return { p: Number(p.dataset.p), o: offsetIn(p, node, offset) };
  // Selection sitting on a container element between blocks: snap to the nearest paragraph.
  if (node.nodeType === Node.ELEMENT_NODE) {
    const el = node as Element;
    const kids = el.childNodes;
    const after = kids[offset] as Element | undefined;
    const before = kids[offset - 1] as Element | undefined;
    // The blocks of the tabs not shown are still in the page: a selection that ends among them (select all, a drag
    // past the end) ends at the open tab's first or last paragraph instead, so an edit never reaches a tab out of sight.
    const hidden = (e?: Element | null) => !!e?.matches?.("[data-tab-hidden]");
    let a: Element | null | undefined = after;
    while (hidden(a)) a = a!.nextElementSibling;
    let b: Element | null | undefined = before;
    while (hidden(b)) b = b!.previousElementSibling;
    const SHOWN = "[data-p]:not([data-tab-hidden] [data-p])";
    const first = (e?: Element | null) => (e?.matches?.("[data-p]") ? e : e?.querySelector?.(SHOWN)) as HTMLElement | null;
    const last = (e?: Element | null) => {
      if (!e) return null;
      if (e.matches?.("[data-p]")) return e as HTMLElement;
      const all = e.querySelectorAll?.(SHOWN);
      return (all && all.length ? all[all.length - 1] : null) as HTMLElement | null;
    };
    p = first(a) ?? null;
    if (p) return { p: Number(p.dataset.p), o: 0 };
    p = last(b);
    if (p) return { p: Number(p.dataset.p), o: nodeLen(p) };
  }
  return null;
}

/**
 * DOM position for a model position (prefers positions inside text nodes). `attr` names the attribute that holds
 * the paragraph id: `data-p` in the text and in a story being edited, `data-id` in the read-only headers,
 * footers and notes drawn on the pages.
 */
export function modelToDom(root: HTMLElement, pos: MPos, attr = "data-p"): { node: Node; offset: number } | null {
  const p = attr === "data-p" ? paraIn(root, pos.p) : (root.querySelector(`[${attr}="${pos.p}"]`) as HTMLElement | null);
  return p ? paraPoint(p, pos.o) : null;
}

/** DOM position for an offset in a paragraph element that is already at hand (many lookups in one pass, as the find bar does). */
export function paraPoint(p: HTMLElement, o: number): { node: Node; offset: number } | null {
  let remaining = o;
  let lastText: Text | null = null;
  let result: { node: Node; offset: number } | null = null;
  const walk = (n: Node): boolean => {
    for (const c of Array.from(n.childNodes)) {
      if (c.nodeType === Node.TEXT_NODE) {
        const len = (c as Text).data.length;
        if (remaining <= len) {
          result = { node: c, offset: remaining };
          // At a boundary, prefer the end of this text over the start of the next node.
          return true;
        }
        remaining -= len;
        lastText = c as Text;
      } else if (c.nodeType === Node.ELEMENT_NODE) {
        const el = c as Element;
        if (isZeroWidth(el)) {
          if (el.tagName === "BR" && remaining === 0) {
            result = { node: n, offset: Array.prototype.indexOf.call(n.childNodes, c) };
            return true;
          }
          continue;
        }
        if (el.classList.contains("atom")) {
          if (remaining === 0) {
            result = lastText ? { node: lastText, offset: lastText.data.length } : { node: el.parentNode!, offset: Array.prototype.indexOf.call(el.parentNode!.childNodes, el) };
            return true;
          }
          remaining -= 1;
          if (remaining === 0) {
            const parent = el.parentNode!;
            result = { node: parent, offset: Array.prototype.indexOf.call(parent.childNodes, el) + 1 };
            // Don't return yet: if a text node follows immediately, prefer its start.
            const next = el.nextSibling;
            if (next && next.nodeType === Node.TEXT_NODE) result = { node: next, offset: 0 };
            else if (next instanceof HTMLElement && next.classList.contains("s") && next.firstChild?.nodeType === Node.TEXT_NODE) result = { node: next.firstChild, offset: 0 };
            return true;
          }
          lastText = null;
        } else if (walk(el)) return true;
      }
    }
    return false;
  };
  if (!walk(p)) {
    if (lastText) return { node: lastText, offset: (lastText as Text).data.length };
    // Empty paragraph (only label/placeholder): caret before the placeholder.
    const ph = p.querySelector("br[data-ph]");
    if (ph) return { node: ph.parentNode!, offset: Array.prototype.indexOf.call(ph.parentNode!.childNodes, ph) };
    return { node: p, offset: p.childNodes.length };
  }
  return result;
}

export interface ModelSel {
  anchor: MPos;
  focus: MPos;
}

/**
 * Firefox selects table cells as such: a drag, or Shift+click, from one cell into another makes one range per cell, each running
 * from one child of a table row to the next (`<tr>:0` to `<tr>:1`), and the selection's anchor and focus are table rows, not text.
 * Read as they stand, they put the caret at the start of a cell's neighbour. The selection is from the start of the first cell's
 * first paragraph to the end of the last cell's last one, which is what Chrome and Safari give for the same gesture.
 */
function cellRanges(root: HTMLElement, s: Selection): ModelSel | null {
  if (s.rangeCount < 2) return null;
  const first = s.getRangeAt(0);
  const last = s.getRangeAt(s.rangeCount - 1);
  for (const r of [first, last]) if (!(r.startContainer instanceof HTMLTableRowElement) || r.startContainer !== r.endContainer) return null;
  if (!root.contains(first.startContainer) || !root.contains(last.endContainer)) return null;
  const anchor = domToModel(root, first.startContainer, first.startOffset);
  // The end of the last range is the boundary after its cell: the cell before it is where the selection ends.
  const lastCell = last.endContainer.childNodes[last.endOffset - 1];
  const paras = lastCell instanceof Element ? lastCell.querySelectorAll<HTMLElement>("[data-p]") : null;
  const tail = paras && paras.length ? paras[paras.length - 1] : null;
  if (!anchor || !tail) return null;
  return { anchor, focus: { p: Number(tail.dataset.p), o: nodeLen(tail) } };
}

export function readSelection(root: HTMLElement): ModelSel | null {
  const s = window.getSelection();
  if (!s || s.rangeCount === 0 || !s.anchorNode || !s.focusNode) return null;
  if (!root.contains(s.anchorNode) || !root.contains(s.focusNode)) return null;
  const cells = cellRanges(root, s);
  if (cells) return cells;
  const anchor = domToModel(root, s.anchorNode, s.anchorOffset);
  const focus = domToModel(root, s.focusNode, s.focusOffset);
  if (!anchor || !focus) return null;
  return { anchor, focus };
}

export function writeSelection(root: HTMLElement, sel: ModelSel) {
  const a = modelToDom(root, sel.anchor);
  const f = modelToDom(root, sel.focus);
  if (!a || !f) return;
  const s = window.getSelection();
  if (!s) return;
  try {
    s.setBaseAndExtent(a.node, a.offset, f.node, f.offset);
  } catch {
    /* node vanished */
  }
}

/** Order two positions by document order using paragraph order from the DOM. */
export function orderPositions(root: HTMLElement, a: MPos, b: MPos): [MPos, MPos] {
  if (a.p === b.p) return a.o <= b.o ? [a, b] : [b, a];
  const pa = paraIn(root, a.p);
  const pb = paraIn(root, b.p);
  if (pa && pb && pa.compareDocumentPosition(pb) & Node.DOCUMENT_POSITION_PRECEDING) return [b, a];
  return [a, b];
}

export function paraLength(root: HTMLElement, id: number): number {
  const p = paraIn(root, id);
  return p ? nodeLen(p) : 0;
}

/** Paragraph ids in document order. */
export function paragraphIds(root: HTMLElement): number[] {
  return Array.from(root.querySelectorAll<HTMLElement>("[data-p]")).map((e) => Number(e.dataset.p));
}

export function textBefore(root: HTMLElement, pos: MPos, n: number): string {
  const p = paraIn(root, pos.p);
  if (!p) return "";
  let text = "";
  const walk = (node: Node) => {
    for (const c of Array.from(node.childNodes)) {
      if (c.nodeType === Node.TEXT_NODE) text += (c as Text).data;
      else if (c.nodeType === Node.ELEMENT_NODE) {
        const el = c as Element;
        if (isZeroWidth(el)) continue;
        if (el.classList.contains("atom")) text += "￼";
        else walk(el);
      }
    }
  };
  walk(p);
  return text.slice(Math.max(0, pos.o - n), pos.o);
}

/**
 * The collapsed range where the point (client coordinates) falls in the text, or null. Chrome and Safari call this
 * `caretRangeFromPoint`; Firefox has only the standard `caretPositionFromPoint`. Every caller that turns a mouse position
 * into a place in the text goes through here, so none of them quietly does nothing in Firefox.
 */
export function caretRangeAt(x: number, y: number): Range | null {
  const doc = document as Document & {
    caretRangeFromPoint?(x: number, y: number): Range | null;
    caretPositionFromPoint?(x: number, y: number): { offsetNode: Node; offset: number } | null;
  };
  if (doc.caretRangeFromPoint) return doc.caretRangeFromPoint(x, y);
  const at = doc.caretPositionFromPoint?.(x, y);
  if (!at) return null;
  const r = document.createRange();
  r.setStart(at.offsetNode, at.offset);
  r.collapse(true);
  return r;
}
