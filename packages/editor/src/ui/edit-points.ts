// Edit points: the vertices and curve handles of a shape's outline, drawn over the shape. Drag a vertex or a handle, click
// the outline to add a point, Delete takes the selected point out, and a small menu makes a point a corner, smooth or
// straight. A preset shape (a rectangle, an oval) becomes a custom outline with its first change, as in Word. Every change
// is one `set_shape_path` op, so it is one undo step, and the shape's box follows the outline.
import { t } from "@docxtor/extensions";
import type { MPos, Op, RShape } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import {
  anchors,
  deleteAnchor,
  handlesOf,
  movePoint,
  nearest,
  point,
  pointType,
  setPointType,
  splitAt,
  svgToCmds,
  type Cmd,
  type PointType,
  type Pt,
  type Ref,
} from "../editor/path-edit";
import { presetPaths } from "../editor/presets";
import { h, overlayLayer, popover, toast } from "./dom";
import { atomAt, boxOf, fromScreen, isConnector, toScreen, type Box } from "./geo-common";
import { claimGeometry, releaseGeometry } from "./freeform-tool";
import type { ShapeTool } from "./shape-tool";
import { track } from "./object-frame";

const NS = "http://www.w3.org/2000/svg";

/** A path of the outline being edited: its commands in its own `w` by `h` space. */
interface PathState {
  w: number;
  h: number;
  cmds: Cmd[];
  fill: boolean;
  stroke: boolean;
}

/** A point of the outline: which path, and which command and point in it. */
interface Sel extends Ref {
  path: number;
}

const same = (a: Sel | null, b: Sel | null) => !!a && !!b && a.path === b.path && a.cmd === b.cmd && a.pt === b.pt;

export class EditPoints {
  private svg: SVGSVGElement | null = null;
  private strip: HTMLElement | null = null;
  private pos: MPos | null = null;
  private shape: RShape | null = null;
  private paths: PathState[] = [];
  private sel: Sel | null = null;
  private busy = false;
  private offSelection: (() => void) | null = null;
  private offs: (() => void)[] = [];

  constructor(
    private ed: Editor,
    private shapes: ShapeTool,
  ) {}

  /** Points are being edited. */
  get active(): boolean {
    return !!this.svg;
  }

  /** The shape that is selected has an outline whose points can be edited. */
  get possible(): boolean {
    const s = this.shapes.current;
    return !!s && !!this.shapes.targetPos && !this.shapes.isMember && s.kind === "shape" && s.source === "drawingml" && !isConnector(s) && !this.ed.readOnly && !this.shapes.typing;
  }

  /** The points of the selected anchor: how many anchors the outline has (for tests and the status line). */
  get count(): number {
    return this.paths.reduce((n, p) => n + anchors(p.cmds).length, 0);
  }

  /** The point type of the selected anchor, if one is selected. */
  get selectedType(): PointType | null {
    const s = this.sel;
    return s ? pointType(this.paths[s.path].cmds, s) : null;
  }

  // -- starting and stopping ---------------------------------------------------------------------

  /** The outline of a shape as editable paths: its own when it is custom, else the preset drawn with the same formulas. */
  private pathsOf(s: RShape): PathState[] | null {
    if (s.geometry === "custom") {
      const paths = s.custom?.paths ?? [];
      if (!paths.length || paths.some((p) => !p.cmds?.length)) return null;
      return paths.map((p) => ({ w: p.w, h: p.h, cmds: (p.cmds ?? []).map((c) => ({ t: c.t, pts: c.pts.map((q) => [q[0], q[1]] as Pt) })), fill: p.fill, stroke: p.stroke }));
    }
    return presetPaths(s.geometry, s.w, s.h, s.adj).map((p) => ({ w: s.w || 1, h: s.h || 1, cmds: svgToCmds(p.d), fill: p.fill, stroke: p.stroke }));
  }

  start(): boolean {
    if (this.svg) return true;
    if (!this.possible) return false;
    const s = this.shapes.current!;
    const pos = this.shapes.targetPos!;
    const paths = this.pathsOf(s);
    if (!paths || paths.every((p) => anchors(p.cmds).length < 2)) {
      toast(t("This outline can't be edited point by point."));
      return false;
    }
    claimGeometry(this);
    this.pos = pos;
    this.shape = s;
    this.paths = paths;
    this.sel = null;
    this.shapes.suspendFrame(true, "points");
    const svg = document.createElementNS(NS, "svg");
    svg.setAttribute("class", "edit-points");
    svg.setAttribute("aria-label", t("Edit points"));
    overlayLayer().append(svg);
    this.svg = svg;
    this.buildStrip();
    this.offSelection = this.shapes.onSelection(() => this.sync());
    const outside = (e: MouseEvent) => {
      const el = e.target as HTMLElement;
      if (this.busy || el.closest(".edit-points, .ep-strip, .pop, .shape-strip")) return;
      // The shape itself stays: it can be moved or text typed in. Anywhere else leaves the mode.
      if (el.closest(".atom.shape") && this.pos && el.closest(".atom.shape") === atomAt(this.ed, this.pos)) return;
      this.exit();
    };
    const key = (e: KeyboardEvent) => this.onKey(e);
    const redraw = () => this.render();
    document.addEventListener("mousedown", outside, true);
    document.addEventListener("keydown", key, true);
    this.ed.canvas.addEventListener("scroll", redraw, { passive: true });
    window.addEventListener("resize", redraw);
    this.offs = [
      () => document.removeEventListener("mousedown", outside, true),
      () => document.removeEventListener("keydown", key, true),
      () => this.ed.canvas.removeEventListener("scroll", redraw),
      () => window.removeEventListener("resize", redraw),
    ];
    this.render();
    return true;
  }

  exit() {
    if (!this.svg) return;
    this.offSelection?.();
    this.offSelection = null;
    for (const f of this.offs) f();
    this.offs = [];
    this.svg.remove();
    this.strip?.remove();
    this.svg = this.strip = null;
    this.pos = this.shape = null;
    this.paths = [];
    this.sel = null;
    this.shapes.suspendFrame(false, "points");
    releaseGeometry(this);
  }

  /** Let go (for the claim other tools make on the pointer). */
  cancel() {
    this.exit();
  }

  /** The shape changed or another one was chosen: follow it, or leave. */
  private sync() {
    if (!this.svg || this.busy) return;
    const s = this.shapes.current;
    const pos = this.shapes.targetPos;
    if (!s || !pos || !this.pos || pos.p !== this.pos.p || pos.o !== this.pos.o || s.kind !== "shape") return this.exit();
    const paths = this.pathsOf(s);
    if (!paths) return this.exit();
    this.shape = s;
    this.paths = paths;
    if (this.pendingSel) {
      const a = anchors(this.paths[this.pendingSel.path]?.cmds ?? [])[this.pendingSel.ord];
      this.sel = a ? { path: this.pendingSel.path, ...a } : null;
      this.pendingSel = null;
    } else if (this.sel && !this.paths[this.sel.path]?.cmds[this.sel.cmd]?.pts[this.sel.pt]) this.sel = null;
    this.render();
  }

  // -- the keyboard ------------------------------------------------------------------------------

  private onKey(e: KeyboardEvent) {
    if (!this.svg || this.busy || (e.target as HTMLElement).closest("input,select,textarea")) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      this.exit();
    } else if ((e.key === "Delete" || e.key === "Backspace") && this.sel) {
      e.preventDefault();
      e.stopPropagation();
      this.deleteSelected();
    }
  }

  // -- where things are on screen ----------------------------------------------------------------

  private box(): Box | null {
    if (!this.pos || !this.shape) return null;
    const atom = atomAt(this.ed, this.pos);
    return atom ? boxOf(this.ed, atom, this.shape) : null;
  }

  /** A point of a path on screen. */
  private screen(b: Box, path: PathState, p: Pt): Pt {
    return toScreen(b, (p[0] / (path.w || 1)) * b.w, (p[1] / (path.h || 1)) * b.h);
  }

  /** A point on screen in a path's own space. */
  private inPath(b: Box, path: PathState, x: number, y: number): Pt {
    const [lx, ly] = fromScreen(b, x, y);
    return [(lx / (b.w || 1)) * path.w, (ly / (b.h || 1)) * path.h];
  }

  // -- drawing -----------------------------------------------------------------------------------

  private el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>): SVGElementTagNameMap[K] {
    const e = document.createElementNS(NS, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
    return e;
  }

  private render() {
    const svg = this.svg;
    const b = this.box();
    if (!svg) return;
    if (!b) {
      svg.replaceChildren();
      return;
    }
    const out: SVGElement[] = [];
    const d = (path: PathState) => {
      const s = (p: Pt) => this.screen(b, path, p).map((v) => Math.round(v * 100) / 100).join(",");
      return path.cmds.map((c) => (c.t === "Z" ? "Z" : `${c.t}${c.pts.map(s).join(" ")}`)).join(" ");
    };
    this.paths.forEach((path, i) => {
      const hit = this.el("path", { class: "ep-hit", d: d(path), "data-path": i });
      hit.addEventListener("pointerdown", (e) => this.downOutline(e, i));
      hit.addEventListener("contextmenu", (e) => this.menuOutline(e, i));
      out.push(this.el("path", { class: "ep-outline", d: d(path) }), hit);
    });
    // The handles of the selected point, and of the points next to it.
    if (this.sel) {
      const path = this.paths[this.sel.path];
      const hs = handlesOf(path.cmds, this.sel);
      for (const ref of [hs.in, hs.out]) {
        if (!ref) continue;
        const [a, c] = [this.screen(b, path, point(path.cmds, this.sel)), this.screen(b, path, point(path.cmds, ref))];
        out.push(this.el("line", { class: "ep-stem", x1: a[0], y1: a[1], x2: c[0], y2: c[1] }));
        const dot = this.el("circle", { class: "ep-handle", cx: c[0], cy: c[1], r: 4.5, "data-handle": `${ref.cmd}.${ref.pt}` });
        dot.addEventListener("pointerdown", (e) => this.downHandle(e, this.sel!.path, ref));
        out.push(dot);
      }
    }
    this.paths.forEach((path, pi) => {
      anchors(path.cmds).forEach((ref) => {
        const [x, y] = this.screen(b, path, point(path.cmds, ref));
        const sel = same(this.sel, { path: pi, ...ref });
        const r = this.el("rect", { class: sel ? "ep-anchor selected" : "ep-anchor", x: x - 4.5, y: y - 4.5, width: 9, height: 9, "data-anchor": `${pi}.${ref.cmd}.${ref.pt}` });
        r.addEventListener("pointerdown", (e) => this.downAnchor(e, pi, ref));
        r.addEventListener("contextmenu", (e) => this.menuAnchor(e, pi, ref));
        out.push(r);
      });
    });
    svg.replaceChildren(...out);
    this.placeStrip();
    this.updateStrip();
  }

  // -- the strip ---------------------------------------------------------------------------------

  private typeButtons: Partial<Record<PointType | "delete", HTMLButtonElement>> = {};

  private buildStrip() {
    const mk = (id: PointType | "delete", label: string, run: () => void) => {
      const b = h("button", { type: "button", class: "tb text-tb", title: label, "aria-label": label }, label);
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", run);
      this.typeButtons[id] = b;
      return b;
    };
    const done = h("button", { type: "button", class: "btn primary", title: t("Stop editing points") }, t("Done"));
    done.addEventListener("click", () => this.exit());
    this.strip = h(
      "div",
      { class: "obj-strip ep-strip", role: "toolbar", "aria-label": t("Edit points") },
      h("span", { class: "readout obj-size" }, t("Edit points")),
      mk("corner", t("Corner"), () => this.retype("corner")),
      mk("smooth", t("Smooth"), () => this.retype("smooth")),
      mk("straight", t("Straight"), () => this.retype("straight")),
      mk("delete", t("Delete point"), () => this.deleteSelected()),
      done,
    );
    overlayLayer().append(this.strip);
  }

  private updateStrip() {
    const type = this.selectedType;
    for (const id of ["corner", "smooth", "straight"] as const) {
      const b = this.typeButtons[id];
      if (!b) continue;
      b.disabled = !this.sel;
      b.classList.toggle("on", type === id);
      b.setAttribute("aria-pressed", String(type === id));
    }
    if (this.typeButtons.delete) this.typeButtons.delete.disabled = !this.sel;
  }

  /** The strip sits at the top of the pages, where the drawing tools keep theirs, clear of the strip that follows the shape. */
  private placeStrip() {
    if (!this.strip) return;
    const w = this.strip.offsetWidth || 360;
    const c = this.ed.canvas.getBoundingClientRect();
    this.strip.style.left = `${Math.max(8, c.left + c.width / 2 - w / 2)}px`;
    this.strip.style.top = `${Math.max(8, c.top + 12)}px`;
  }

  // -- changing ----------------------------------------------------------------------------------

  /** Send the paths as the shape's new outline. */
  private commit(next: PathState[], select: Sel | null, ordIn?: number) {
    if (!this.pos) return;
    const pos = this.pos;
    this.busy = true;
    const paths = next.map((p) => ({ w: p.w, h: p.h, cmds: p.cmds.map((c) => ({ t: c.t, pts: c.pts })), fill: p.fill, stroke: p.stroke }));
    const ok = this.ed.exec({ op: "set_shape_path", at: pos, paths } as Op, { select: { anchor: pos, focus: { p: pos.p, o: pos.o + 1 } } });
    this.busy = false;
    if (!ok) return this.render();
    // The point stays selected by its place in the outline (a line made a curve moves its point's index inside the command).
    const ord = ordIn ?? (select ? anchors(next[select.path].cmds).findIndex((r) => r.cmd === select.cmd && r.pt === select.pt) : -1);
    this.pendingSel = select && ord >= 0 ? { path: select.path, ord } : null;
    this.sel = null;
    this.sync();
  }

  /** The anchor to select once the shape is read again: which path and which anchor of it. */
  private pendingSel: { path: number; ord: number } | null = null;

  private downAnchor(e: PointerEvent, pi: number, ref: Ref) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const b = this.box();
    if (!b) return;
    this.sel = { path: pi, ...ref };
    const start = this.paths[pi].cmds;
    const path = this.paths[pi];
    let moved = false;
    track(
      e,
      (_dx, _dy, ev) => {
        moved = true;
        const to = this.inPath(b, path, ev.clientX, ev.clientY);
        this.paths[pi] = { ...path, cmds: movePoint(start, ref, to) };
        this.render();
      },
      () => {
        if (!moved) return this.render();
        this.commit(this.paths, this.sel);
      },
    );
  }

  private downHandle(e: PointerEvent, pi: number, ref: Ref) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const b = this.box();
    if (!b) return;
    const start = this.paths[pi].cmds;
    const path = this.paths[pi];
    let moved = false;
    track(
      e,
      (_dx, _dy, ev) => {
        moved = true;
        // Alt breaks the pair: the other handle stays where it is.
        const to = this.inPath(b, path, ev.clientX, ev.clientY);
        this.paths[pi] = { ...path, cmds: movePoint(start, ref, to, { handle: true, keepSmooth: !ev.altKey }) };
        this.render();
      },
      () => {
        if (moved) this.commit(this.paths, this.sel);
      },
    );
  }

  /** A press on the outline puts a new point there (and the point can be dragged straight away). */
  private downOutline(e: PointerEvent, pi: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    this.addPointAt(pi, e.clientX, e.clientY);
  }

  private addPointAt(pi: number, x: number, y: number) {
    const b = this.box();
    if (!b) return;
    const path = this.paths[pi];
    const p = this.inPath(b, path, x, y);
    const tol = (8 / (b.w || 1)) * path.w;
    const hit = nearest(path.cmds, p, Math.max(tol, (8 / (b.h || 1)) * path.h));
    if (!hit) return;
    const { cmds, added } = splitAt(path.cmds, hit.cmd, hit.t);
    const next = this.paths.map((q, i) => (i === pi ? { ...q, cmds } : q));
    this.commit(next, { path: pi, ...added });
  }

  private deleteSelected() {
    const s = this.sel;
    if (!s) return;
    const cmds = deleteAnchor(this.paths[s.path].cmds, s);
    if (!cmds) return toast(t("An outline needs at least two points."));
    this.commit(
      this.paths.map((q, i) => (i === s.path ? { ...q, cmds } : q)),
      null,
    );
  }

  private retype(type: PointType) {
    const s = this.sel;
    if (!s) return;
    const cmds = setPointType(this.paths[s.path].cmds, s, type);
    // A line made a curve changes where the point is inside its command: keep it by its place in the outline.
    const ord = anchors(this.paths[s.path].cmds).findIndex((r) => r.cmd === s.cmd && r.pt === s.pt);
    this.commit(
      this.paths.map((q, i) => (i === s.path ? { ...q, cmds } : q)),
      s,
      ord,
    );
  }

  // -- the right-click menus ---------------------------------------------------------------------

  private menu(e: MouseEvent, items: [string, () => void][]) {
    e.preventDefault();
    e.stopPropagation();
    const body = h("div", { class: "ep-menu" });
    let close = () => {};
    for (const [label, run] of items) {
      const b = h("button", { type: "button", class: "menu-item", role: "menuitem" }, label);
      b.addEventListener("click", () => {
        close();
        run();
      });
      body.append(b);
    }
    close = popover({ x: e.clientX, y: e.clientY }, body);
  }

  private menuAnchor(e: MouseEvent, pi: number, ref: Ref) {
    this.sel = { path: pi, ...ref };
    this.render();
    this.menu(e, [
      [t("Smooth point"), () => this.retype("smooth")],
      [t("Straight point"), () => this.retype("straight")],
      [t("Corner point"), () => this.retype("corner")],
      [t("Delete point"), () => this.deleteSelected()],
    ]);
  }

  private menuOutline(e: MouseEvent, pi: number) {
    const [x, y] = [e.clientX, e.clientY];
    this.menu(e, [[t("Add point"), () => this.addPointAt(pi, x, y)]]);
  }
}
