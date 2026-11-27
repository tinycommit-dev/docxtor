// Curve, Freeform and Scribble: Insert, Drawing, Shape, Lines. A layer over the pages takes the pointer while a strip says
// what to do. The result is one `insert_freeform` op: a shape with its own outline, written as Word writes a freeform.
//
// Curve:    click to place points, the line is a smooth curve through them. Double-click, Enter or the first point finishes.
// Freeform: click for straight segments, press and drag for freehand ones. Double-click or Enter finishes; the first point
//           closes the shape.
// Scribble: press and drag, one stroke, smoothed.
import { t } from "@docxtor/extensions";
import type { Op, PathCmd } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { cmdsToD, smoothOpen, simplify, type Cmd, type Pt } from "../editor/path-edit";
import { h, overlayLayer } from "./dom";
import { offsetsIn, paragraphAt } from "./geo-common";
import type { GeometryTool } from "./geometry-icons";
import type { ShapeTool } from "./shape-tool";

const NS = "http://www.w3.org/2000/svg";
/** A click this close to the first point closes the shape; one this close to the last point finishes (a double-click). */
const CLOSE_PX = 8;
const SAME_PX = 4;
const DOUBLE_MS = 500;
const DEFAULT_FILL = "4472C4";
const DEFAULT_LINE = "2F528F";

/** The tool that is drawing now; starting another one lets go of it. */
let current: { cancel(): void } | null = null;
export function claimGeometry(tool: { cancel(): void }) {
  if (current && current !== tool) current.cancel();
  current = tool;
}
export function releaseGeometry(tool: { cancel(): void }) {
  if (current === tool) current = null;
}

/** One piece of a freeform: a corner the line runs straight to, or a freehand run. */
type Piece = { kind: "corner"; at: Pt } | { kind: "run"; pts: Pt[] };

export class FreeformTool {
  private layer: HTMLElement | null = null;
  private path: SVGPathElement | null = null;
  private marks: SVGGElement | null = null;
  private strip: HTMLElement | null = null;
  private done: HTMLButtonElement | null = null;
  private mode: Exclude<GeometryTool, "straight" | "elbow" | "curved"> = "curve";
  /** Where the line starts, and what came after it (stage pixels before zoom). */
  private first: Pt | null = null;
  private pieces: Piece[] = [];
  /** The pointer, when it is over the layer. */
  private hover: Pt | null = null;
  /** The press that is under way: where it started and the points a freehand run has so far. */
  private press: { at: Pt; moved: boolean; run: Pt[] } | null = null;
  private lastClick = { at: [0, 0] as Pt, time: 0 };
  private off: (() => void) | null = null;

  constructor(
    private ed: Editor,
    private shapes: ShapeTool,
  ) {}

  /** A drawing is under way. */
  get active(): boolean {
    return !!this.layer;
  }

  /** What is being drawn (null when nothing is). */
  get tool(): string | null {
    return this.layer ? this.mode : null;
  }

  /** The corners and runs placed so far. */
  get length(): number {
    return (this.first ? 1 : 0) + this.pieces.length;
  }

  /** The point under a pointer event, in stage pixels before zoom. */
  private point(e: { clientX: number; clientY: number }): Pt {
    const r = this.ed.stageElement.getBoundingClientRect();
    const z = this.ed.zoom || 1;
    return [(e.clientX - r.left) / z, (e.clientY - r.top) / z];
  }

  /** A stage point as a point on screen. */
  private client(p: Pt): Pt {
    const r = this.ed.stageElement.getBoundingClientRect();
    const z = this.ed.zoom || 1;
    return [r.left + p[0] * z, r.top + p[1] * z];
  }

  start(mode: "curve" | "freeform" | "scribble") {
    if (this.layer || this.ed.readOnly || this.ed.story) return;
    claimGeometry(this);
    // A shape that was selected is let go, as the pen does.
    const r = this.ed.range();
    this.shapes.hide();
    if (r) this.ed.select(r[0]);
    this.mode = mode;
    this.first = null;
    this.pieces = [];
    this.press = null;
    this.hover = null;
    const layer = h("div", { class: "ink-layer geo-layer", "aria-label": mode === "curve" ? t("Curve") : mode === "freeform" ? t("Freeform") : t("Scribble"), role: "application", "data-geo": mode });
    const svg = document.createElementNS(NS, "svg");
    svg.setAttribute("aria-hidden", "true");
    const path = document.createElementNS(NS, "path");
    path.setAttribute("class", "geo-path");
    const marks = document.createElementNS(NS, "g");
    svg.append(path, marks);
    layer.append(svg);
    this.ed.stageElement.append(layer);
    this.layer = layer;

    this.path = path;
    this.marks = marks;
    this.buildStrip();
    layer.addEventListener("pointerdown", (e) => this.down(e));
    layer.addEventListener("pointermove", (e) => this.move(e));
    layer.addEventListener("pointerup", (e) => this.up(e));
    layer.addEventListener("pointercancel", () => this.cancelPress());
    layer.addEventListener("pointerleave", () => {
      this.hover = null;
      this.redraw();
    });
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        this.cancel();
      } else if (e.key === "Enter" && !(e.target as HTMLElement).closest("select,button")) {
        e.preventDefault();
        e.stopPropagation();
        this.finish(false);
      } else if (e.key === "Backspace" && this.mode !== "scribble" && !this.press) {
        e.preventDefault();
        e.stopPropagation();
        this.undoPiece();
      }
    };
    document.addEventListener("keydown", key, true);
    this.off = () => document.removeEventListener("keydown", key, true);
    layer.tabIndex = -1;
    layer.focus({ preventScroll: true });
    this.redraw();
  }

  private buildStrip() {
    const hint = this.mode === "curve" ? t("Click to place points. Double-click to finish.") : this.mode === "freeform" ? t("Click for corners, drag to draw freehand. Double-click to finish.") : t("Press and drag to scribble.");
    const cancel = h("button", { type: "button", class: "tb", title: t("Cancel") }, t("Cancel"));
    cancel.addEventListener("click", () => this.cancel());
    const parts: HTMLElement[] = [h("span", { class: "readout obj-size", "aria-live": "polite" }, hint)];
    if (this.mode !== "scribble") {
      const done = h("button", { type: "button", class: "btn primary", title: t("Put the drawing in the document"), disabled: true }, t("Done"));
      done.addEventListener("click", () => this.finish(false));
      this.done = done;
      parts.push(cancel, done);
    } else parts.push(cancel);
    this.strip = h("div", { class: "obj-strip geo-strip", role: "toolbar", "aria-label": t("Drawing") }, ...parts);
    overlayLayer().append(this.strip);
    this.placeStrip();
  }

  private placeStrip() {
    if (!this.strip) return;
    const c = this.ed.canvas.getBoundingClientRect();
    const w = this.strip.offsetWidth || 420;
    this.strip.style.left = `${Math.max(8, c.left + c.width / 2 - w / 2)}px`;
    this.strip.style.top = `${Math.max(8, c.top + 12)}px`;
  }

  // -- the pointer -------------------------------------------------------------------------------

  private down(e: PointerEvent) {
    if (e.button !== 0 || !this.layer || this.press) return;
    e.preventDefault();
    this.layer.setPointerCapture(e.pointerId);
    const at = this.point(e);
    this.press = { at, moved: false, run: [at] };
    if (this.mode === "scribble") this.first = at;
  }

  private move(e: PointerEvent) {
    const p = this.point(e);
    this.hover = p;
    const press = this.press;
    if (press) {
      const dist = Math.hypot(p[0] - press.at[0], p[1] - press.at[1]) * (this.ed.zoom || 1);
      // A press that travels is a freehand run (a freeform's), or the scribble itself.
      if (!press.moved && dist >= 3 && this.mode !== "curve") press.moved = true;
      if (press.moved) {
        const events = typeof e.getCoalescedEvents === "function" ? e.getCoalescedEvents() : [];
        for (const ev of events.length ? events : [e]) {
          const q = this.point(ev);
          const last = press.run[press.run.length - 1];
          if (Math.hypot(q[0] - last[0], q[1] - last[1]) >= 1) press.run.push(q);
        }
      }
    }
    this.redraw();
  }

  private up(e: PointerEvent) {
    const press = this.press;
    if (!press || !this.layer) return;
    this.layer.releasePointerCapture?.(e.pointerId);
    this.press = null;
    const p = this.point(e);
    if (this.mode === "scribble") {
      if (press.run.length >= 2) this.commit(press.run, false);
      else this.redraw();
      return;
    }
    if (press.moved && press.run.length >= 2) {
      // A freehand run: the line is straight to where the press began, then follows the pointer.
      if (!this.first) this.first = press.run[0];
      else this.pieces.push({ kind: "corner", at: press.run[0] });
      this.pieces.push({ kind: "run", pts: press.run.slice(1) });
      this.lastClick = { at: p, time: 0 };
      return this.redraw();
    }
    this.click(p);
  }

  private cancelPress() {
    this.press = null;
    this.redraw();
  }

  /** A click: a new corner, the first point (closing the shape) or the last one again (finishing). */
  private click(p: Pt) {
    const z = this.ed.zoom || 1;
    const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]) * z;
    const now = performance.now();
    if (!this.first) {
      this.first = p;
      this.lastClick = { at: p, time: now };
      return this.redraw();
    }
    const end = this.end();
    if (this.pieces.length >= 2 && dist(p, this.first) <= CLOSE_PX) return this.finish(true);
    if (dist(p, end) <= SAME_PX && now - this.lastClick.time < DOUBLE_MS) return this.finish(false);
    this.pieces.push({ kind: "corner", at: p });
    this.lastClick = { at: p, time: now };
    this.redraw();
  }

  /** The point the line has reached. */
  private end(): Pt {
    const last = this.pieces[this.pieces.length - 1];
    if (!last) return this.first!;
    return last.kind === "corner" ? last.at : last.pts[last.pts.length - 1];
  }

  private undoPiece() {
    if (this.pieces.length) this.pieces.pop();
    else this.first = null;
    this.redraw();
  }

  // -- drawing the preview -----------------------------------------------------------------------

  /** The commands of what is drawn so far, with the pointer as the next point. */
  private preview(): { cmds: Cmd[]; vertices: Pt[] } {
    if (!this.first) {
      const run = this.press?.moved ? this.press.run : null;
      return { cmds: run ? [{ t: "M", pts: [run[0]] }, ...smoothOpen(run)] : [], vertices: [] };
    }
    const vertices: Pt[] = [this.first];
    const hover = !this.press && this.hover ? this.hover : null;
    if (this.mode === "scribble") {
      const run = this.press?.run ?? [this.first];
      return { cmds: [{ t: "M", pts: [run[0]] }, ...smoothOpen(run)], vertices: [] };
    }
    if (this.mode === "curve") {
      const pts: Pt[] = [this.first, ...this.pieces.flatMap((p) => (p.kind === "corner" ? [p.at] : p.pts))];
      vertices.push(...pts.slice(1));
      if (hover) pts.push(hover);
      return { cmds: [{ t: "M", pts: [pts[0]] }, ...smoothOpen(pts)], vertices };
    }
    const cmds: Cmd[] = [{ t: "M", pts: [this.first] }];
    let cur = this.first;
    for (const p of this.pieces) {
      if (p.kind === "corner") {
        cmds.push({ t: "L", pts: [p.at] });
        cur = p.at;
        vertices.push(p.at);
      } else {
        cmds.push(...smoothOpen([cur, ...p.pts]));
        cur = p.pts[p.pts.length - 1];
        vertices.push(cur);
      }
    }
    if (this.press?.moved) {
      cmds.push({ t: "L", pts: [this.press.run[0]] }, ...smoothOpen(this.press.run));
    } else if (hover) cmds.push({ t: "L", pts: [hover] });
    return { cmds, vertices };
  }

  private redraw() {
    if (!this.path || !this.marks) return;
    const { cmds, vertices } = this.preview();
    this.path.setAttribute("d", cmdsToD(cmds));
    this.marks.replaceChildren(
      ...vertices.map((v, i) => {
        const c = document.createElementNS(NS, "rect");
        const s = i === 0 ? 8 : 6;
        c.setAttribute("class", i === 0 ? "geo-mark first" : "geo-mark");
        c.setAttribute("x", String(v[0] - s / 2));
        c.setAttribute("y", String(v[1] - s / 2));
        c.setAttribute("width", String(s));
        c.setAttribute("height", String(s));
        return c;
      }),
    );
    if (this.done) this.done.disabled = this.length < 2;
  }

  // -- finishing ---------------------------------------------------------------------------------

  /** The commands for the engine: points relative to the first, which sits at the origin. */
  private commands(closed: boolean): PathCmd[] {
    const first = this.first!;
    const rel = (p: Pt): Pt => [Math.round((p[0] - first[0]) * 100) / 100, Math.round((p[1] - first[1]) * 100) / 100];
    const cmds: PathCmd[] = [{ t: "M", pts: [[0, 0]] }];
    for (const p of this.pieces) {
      if (p.kind === "corner") cmds.push({ t: "L", pts: [rel(p.at)] });
      else cmds.push({ t: "S", pts: simplify(p.pts, 0.35).map(rel) });
    }
    if (closed) cmds.push({ t: "Z", pts: [] });
    return cmds;
  }

  /** Put the drawing in the text. `closed` joins the end to the start. */
  finish(closed: boolean) {
    if (!this.layer || this.press || !this.first) return;
    if (this.pieces.length < 1 || (closed && this.pieces.length < 2)) return this.cancel();
    this.commit(null, closed);
  }

  private commit(scribble: Pt[] | null, closed: boolean) {
    const first = scribble ? scribble[0] : this.first!;
    const mode = this.mode;
    const cmds = scribble ? null : this.commands(closed);
    const points = scribble ? scribble.map((p) => [p[0] - first[0], p[1] - first[1]] as Pt) : null;
    const pieces = this.pieces.slice();
    this.close();
    const [cx, cy] = this.client(first);
    const para = paragraphAt(this.ed, cx, cy);
    if (!para) return;
    const [x, y] = offsetsIn(this.ed, para, cx, cy);
    const at = { p: Number(para.dataset.p), o: 0 };
    const fillOp = closed ? { fill: DEFAULT_FILL, line: { color: DEFAULT_LINE, width_pt: 1 } } : { line: { color: DEFAULT_FILL, width_pt: 1.5 } };
    const op: Record<string, unknown> = { op: "insert_freeform", at, x, y, ...fillOp, label: mode === "scribble" ? "Freeform: Scribble" : "Freeform: Shape" };
    if (mode === "curve") {
      // A curve is its points, smoothed by the engine; a closed one wraps round.
      const pts: Pt[] = [[0, 0], ...pieces.flatMap((p) => (p.kind === "corner" ? [[Math.round((p.at[0] - first[0]) * 100) / 100, Math.round((p.at[1] - first[1]) * 100) / 100] as Pt] : []))];
      op.points = pts;
      op.smooth = true;
      op.closed = closed;
    } else if (mode === "scribble") {
      op.points = points;
      op.smooth = true;
    } else op.cmds = cmds;
    const res = this.ed.exec(op as Op, { select: { anchor: at, focus: { p: at.p, o: 1 } } });
    const caret = res?.[0]?.caret;
    if (caret) this.shapes.selectAt(at.p, caret.o - 1);
  }

  /** Let go without drawing anything. */
  cancel() {
    const was = this.layer;
    this.close();
    if (was) this.ed.focus();
  }

  private close() {
    this.off?.();
    this.off = null;
    this.layer?.remove();
    this.strip?.remove();
    this.layer = this.path = this.marks = this.strip = this.done = null;
    this.first = null;
    this.pieces = [];
    this.press = null;
    this.hover = null;
    releaseGeometry(this);
  }
}

