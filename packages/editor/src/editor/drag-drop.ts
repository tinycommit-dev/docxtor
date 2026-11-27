// Dragging text: drag the selection to move it, hold Alt (Option) to copy it. A caret shows where it will land.
// The selection travels in the clipboard formats (Docxtor's own as well as HTML and text), so a drag into
// another Docxtor window keeps its formatting, and what other programs drop on the page goes through the
// same converters as a paste. A move or a copy within the document is one undo step.
import type { Fragment, MPos, Op } from "@docxtor/core";
import { h, overlayLayer } from "../ui/dom";
import type { Editor } from "./editor";
import { clipFromData, FRAGMENT_MIME } from "./paste-convert";
import { textToMarkdown } from "./paste";
import { caretRangeAt, domToModel } from "./selection";

interface Source {
  range: [MPos, MPos];
  fragment: Fragment | null;
  text: string;
}

export class DragDrop {
  private src: Source | null = null;
  /** The drop was handled by this editor (so the source must not delete the text again). */
  private done = false;
  private caret: HTMLElement | null = null;

  constructor(private ed: Editor) {}

  /** Whether a drag is carrying text this editor can take. */
  private accepts(e: DragEvent): boolean {
    const types = e.dataTransfer?.types;
    if (!types || this.ed.readOnly) return false;
    return !!this.src || types.includes(FRAGMENT_MIME) || types.includes("text/html") || types.includes("text/plain") || types.includes("text/rtf");
  }

  start(e: DragEvent) {
    const ed = this.ed;
    const r = ed.range();
    const dt = e.dataTransfer;
    const el = e.target instanceof Element ? e.target : (e.target as Node).parentElement;
    // Only the selected text is dragged; pictures, shapes and tables are moved with their own tools.
    if (!r || !dt || ed.collapsed() || ed.multi || el?.closest("img, .obj-frame, .shape, .tbl-handle")) {
      e.preventDefault();
      return;
    }
    const p = ed.copyPayload(r);
    dt.setData("text/plain", p.text);
    if (p.html) dt.setData("text/html", p.html);
    if (p.fragment) dt.setData(FRAGMENT_MIME, p.fragment);
    dt.effectAllowed = ed.readOnly ? "copy" : "copyMove";
    this.src = { range: r, fragment: p.fragment ? (JSON.parse(p.fragment) as Fragment) : null, text: p.text };
    this.done = false;
  }

  over(e: DragEvent) {
    if (!this.accepts(e) || !e.dataTransfer) return;
    e.preventDefault();
    const copy = e.altKey || !this.src;
    try {
      e.dataTransfer.dropEffect = copy ? "copy" : "move";
    } catch {
      /* the source allowed only one effect */
    }
    const pos = this.posAt(e);
    if (pos && !(this.src && this.inside(pos))) this.showCaret(e);
    else this.hideCaret();
  }

  leave(e: DragEvent) {
    // Only when the pointer leaves the editing area, not when it crosses between paragraphs.
    const to = e.relatedTarget as Node | null;
    if (!to || !this.ed.canvas.contains(to)) this.hideCaret();
  }

  drop(e: DragEvent) {
    const ed = this.ed;
    if (!this.accepts(e) || !e.dataTransfer) return;
    e.preventDefault();
    e.stopPropagation();
    this.hideCaret();
    const pos = this.posAt(e);
    if (!pos) return;
    const src = this.src;
    if (src) {
      this.done = true;
      if (this.inside(pos)) return;
      this.relocate(src, pos, e.altKey);
      return;
    }
    // Something from another window or program: the paste path, at the drop point.
    this.done = true;
    ed.select(pos);
    const clip = clipFromData(e.dataTransfer);
    const file = clip.files.find((f) => f.type.startsWith("image/"));
    if (file) void ed.insertImage(file);
    else void ed.pasteFlow.paste(clip);
  }

  end(e: DragEvent) {
    const src = this.src;
    this.src = null;
    this.hideCaret();
    // Moved into another window: that window inserted the text, so it goes from here.
    if (src && !this.done && e.dataTransfer?.dropEffect === "move" && !this.ed.readOnly) {
      this.ed.exec({ op: "delete", from: src.range[0], to: src.range[1] }, { select: src.range[0] });
    }
    this.done = false;
  }

  /** Move or copy text within this document as one step. */
  private relocate(src: Source, pos: MPos, copy: boolean) {
    const ed = this.ed;
    const [a, b] = src.range;
    const ops: Op[] = [];
    let at = pos;
    if (!copy) {
      ops.push({ op: "delete", from: a, to: b });
      at = this.afterDelete(pos, a, b);
    }
    ops.push(src.fragment ? { op: "insert_fragment", at, fragment: src.fragment, mode: "keep" } : { op: "paste_markdown", at, markdown: textToMarkdown(src.text.replace(/\r\n?/g, "\n")) });
    const res = ed.exec(ops, { select: at });
    const caret = res ? [...res].reverse().find((x) => x.caret)?.caret : null;
    if (caret) ed.select({ anchor: at, focus: caret });
    ed.focus();
    ed.hooks.onFontsMayChange?.();
  }

  /** Where a position falls once the dragged text is gone. */
  private afterDelete(pos: MPos, a: MPos, b: MPos): MPos {
    if (pos.p !== b.p || pos.o < b.o) return pos;
    return a.p === b.p ? { p: a.p, o: pos.o - (b.o - a.o) } : { p: a.p, o: a.o + (pos.o - b.o) };
  }

  private inside(pos: MPos): boolean {
    const src = this.src;
    if (!src) return false;
    const order = this.ed.root.querySelectorAll<HTMLElement>("[data-p]");
    const index = new Map(Array.from(order).map((el, i) => [Number(el.dataset.p), i]));
    const key = (m: MPos): [number, number] | null => (index.has(m.p) ? [index.get(m.p)!, m.o] : null);
    const [a, b, p] = [key(src.range[0]), key(src.range[1]), key(pos)];
    if (!a || !b || !p) return false;
    const cmp = (x: [number, number], y: [number, number]) => x[0] - y[0] || x[1] - y[1];
    return cmp(a, p) <= 0 && cmp(p, b) <= 0;
  }

  private posAt(e: DragEvent): MPos | null {
    const r = caretRangeAt(e.clientX, e.clientY);
    if (!r || !this.ed.root.contains(r.startContainer)) return null;
    return domToModel(this.ed.root, r.startContainer, r.startOffset);
  }

  private showCaret(e: DragEvent) {
    const r = caretRangeAt(e.clientX, e.clientY);
    if (!r) return;
    let box = r.getClientRects()[0] ?? r.getBoundingClientRect();
    if (!box || (!box.height && !box.width)) {
      const para = (r.startContainer instanceof Element ? r.startContainer : r.startContainer.parentElement)?.closest<HTMLElement>("[data-p]");
      if (para) {
        const pr = para.getBoundingClientRect();
        box = new DOMRect(pr.left, pr.top, 0, Math.min(pr.height, 20));
      }
    }
    if (!box) return;
    if (!this.caret) {
      this.caret = h("div", { class: "drop-caret", "aria-hidden": "true" });
      overlayLayer().append(this.caret);
    }
    this.caret.style.left = `${box.left - 1}px`;
    this.caret.style.top = `${box.top}px`;
    this.caret.style.height = `${box.height || 18}px`;
  }

  private hideCaret() {
    this.caret?.remove();
    this.caret = null;
  }
}
