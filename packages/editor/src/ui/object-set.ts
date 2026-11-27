// The objects that are selected together, whatever they are. Shift-click selects several shapes and text boxes (the
// shape tool owns that); a picture or a chart is selected alone by the picture tool. Align, distribute and the
// Selection pane need a set that can hold any mix, so this keeps the others beside the one that is really
// selected and draws an outline round each. The set ends as soon as the selection goes somewhere else.
import type { MPos } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { overlayLayer } from "./dom";
import type { ImageTool } from "./image-tool";
import { ObjectFrame } from "./object-frame";
import type { ShapeTool } from "./shape-tool";

const same = (a: MPos, b: MPos) => a.p === b.p && a.o === b.o;

export class ObjectSet {
  /** Objects selected besides the one the shape or picture tool has selected. */
  private extra: MPos[] = [];
  /** The tool's own selection when the set was made: a different one ends the set. */
  private base: MPos | null = null;
  private outlines: ObjectFrame[] = [];

  constructor(
    private ed: Editor,
    private shapes: ShapeTool,
    private images: ImageTool,
  ) {
    ed.canvas.addEventListener("scroll", () => this.place(), { passive: true });
    ed.life.on(window, "resize", () => this.place());
  }

  /** The object a tool has selected (the first of its shapes), or null. */
  private native(): MPos | null {
    return this.shapes.positions[0] ?? this.images.targetPos;
  }

  /** Every selected object: the tool's own first, then the ones the set holds. */
  all(): MPos[] {
    const own = this.shapes.positions.length ? this.shapes.positions : this.images.targetPos ? [this.images.targetPos] : [];
    return [...own, ...this.extra.filter((e) => !own.some((o) => same(o, e)))];
  }

  private isShape(p: MPos): boolean {
    return !!this.ed.paraElement(p.p)?.querySelector(`:scope > .atom.shape[data-o="${p.o}"]`);
  }

  private exists(p: MPos): boolean {
    return !!this.ed.paraElement(p.p)?.querySelector(`:scope > .atom[data-o="${p.o}"]`);
  }

  /** Select these objects together (the first is the one the frame is on). */
  select(list: MPos[]) {
    const found = list.filter((p) => this.exists(p));
    this.drop();
    if (!found.length) return;
    const shapes = found.filter((p) => this.isShape(p));
    const others = found.filter((p) => !this.isShape(p));
    if (shapes.length && found[0] === shapes[0]) {
      this.shapes.selectShapes(shapes);
      this.extra = others;
    } else if (!shapes.length) {
      this.shapes.hide();
      this.images.selectAt(others[0].p, others[0].o);
      this.extra = others.slice(1);
    } else {
      // The first is a picture: the shapes join it as outlines.
      this.shapes.hide();
      this.images.selectAt(found[0].p, found[0].o);
      this.extra = found.slice(1);
    }
    this.base = this.native();
    this.place();
  }

  /** Forget the others (the tools keep their own selection). */
  clear() {
    this.drop();
  }

  private drop() {
    this.extra = [];
    this.base = null;
    for (const f of this.outlines) f.hide();
  }

  /** Re-check after the document or the selection changed: the set ends when the tool's selection moved elsewhere. */
  refresh() {
    if (!this.extra.length) return;
    const now = this.native();
    if (!now || !this.base || !same(now, this.base)) return this.drop();
    this.extra = this.extra.filter((p) => this.exists(p));
    this.place();
  }

  private place() {
    const c = this.ed.canvas.getBoundingClientRect();
    while (this.outlines.length < this.extra.length) {
      const f = new ObjectFrame(() => {}, {});
      f.setHandles(false);
      overlayLayer().append(this.ed.life.own(f.el));
      this.outlines.push(f);
    }
    this.outlines.forEach((f, i) => {
      const p = this.extra[i];
      const para = p ? this.ed.paraElement(p.p) : null;
      // Measured where it is: the block is drawn first.
      if (para) this.ed.ensureDrawn(para);
      const atom = p ? para?.querySelector<HTMLElement>(`:scope > .atom[data-o="${p.o}"]`) : null;
      if (!atom) return f.hide();
      const media = atom.querySelector<HTMLElement>("img, svg") ?? atom;
      const r = (atom.classList.contains("img") ? media : atom).getBoundingClientRect();
      f.place({ left: r.left, top: r.top, width: r.width, height: r.height }, r.bottom > c.top && r.top < c.bottom);
    });
  }
}

const sets = new WeakMap<Editor, ObjectSet>();

/** The one object set of an editor. */
export function objectSetOf(ed: Editor, shapes: ShapeTool, images: ImageTool): ObjectSet {
  let s = sets.get(ed);
  if (!s) sets.set(ed, (s = new ObjectSet(ed, shapes, images)));
  return s;
}
