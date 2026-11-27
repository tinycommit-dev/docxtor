// Shapes and text boxes: selecting one (a frame with resize and rotation handles and a strip of fill, outline
// and wrapping), moving it by dragging or the arrow keys, typing in its text, and inserting new ones. Every
// change is an op (`set_shape`, `insert_shape`, `insert_text_box`, `delete`) applied through the editor, so
// undo, the assistant and the CLI see the same thing.
//
// The text inside a shape is a story of ordinary paragraphs. While it is being edited its element is a
// contenteditable island inside the page (see `editing` in editor/shapes.ts, which keeps it editable when the
// paragraph that holds the shape is drawn again after every keystroke).
import { msg, t, tn } from "@docxtor/extensions";
import type { Engine, LineSpec, MPos, Op, RShape } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { frameOf, offsetsAt } from "../editor/float-place";
import { pageMetrics } from "../editor/place";
import { presetPaths } from "../editor/presets";
import { caretRangeAt, domToModel, paraLength } from "../editor/selection";
import { editing, textParas } from "../editor/shapes";
import { behindAt, openArrangeMenu } from "./arrange";
import { ARRANGE_ICONS } from "./arrange-icons";
import { h, overlayLayer, popover, promptDialog, toast } from "./dom";
import { I } from "./icons";
import { ObjectFrame, track, type Handle } from "./object-frame";
import { toolPreview, type GeometryTool } from "./geometry-icons";
import { SHAPE_ICONS } from "./shape-icons";
import { TEXT_COLORS } from "./toolbar";
import { hex, hexOr } from "../editor/color.ts";

const PT_PX = 96 / 72;
const RAD = Math.PI / 180;

const WRAPS: [string, string, string][] = [
  ["inline", msg("In line"), msg("Moves with the text, like a character")],
  ["square", msg("Wrap"), msg("Text flows around the shape")],
  ["topBottom", msg("Break"), msg("Text above and below only")],
  ["behind", msg("Behind"), msg("Behind the text")],
  ["front", msg("Front"), msg("In front of the text")],
];

const WEIGHTS = [0.25, 0.5, 0.75, 1, 1.5, 2.25, 3, 4.5, 6];
const DASH_CHOICES: [string, string][] = [
  ["solid", msg("Solid")],
  ["sysDot", msg("Dotted")],
  ["dash", msg("Dashed")],
  ["lgDash", msg("Long dashes")],
  ["dashDot", msg("Dash and dot")],
];

/** Shapes the picker offers, by group: the Word preset, its name, and what to draw it with when inserted. */
export interface ShapeChoice {
  geometry: string;
  name: string;
  /** Filled when inserted; lines and arrows are not. */
  line?: LineSpec;
  fill?: boolean;
  /** The size it is inserted at, in points (1.5 in by 1 in unless it says). */
  w?: number;
  h?: number;
  /** A tool to draw it with instead of inserting a fixed shape (curves, freeforms, connectors); see `onTool`. */
  tool?: GeometryTool;
  /** The tool draws an arrow at the end. */
  arrow?: boolean;
}

const SHAPE_FILL = "4472C4";
const SHAPE_LINE = "2F528F";

export const SHAPE_GROUPS: { title: string; shapes: ShapeChoice[] }[] = [
  {
    title: msg("Lines and arrows"),
    shapes: [
      { geometry: "line", name: msg("Line"), line: { color: SHAPE_FILL, width_pt: 1.5 }, h: 0 },
      { geometry: "straightConnector1", name: msg("Arrow"), line: { color: SHAPE_FILL, width_pt: 1.5, tail_arrow: "triangle" }, h: 0 },
      { geometry: "straightConnector1", name: msg("Double arrow"), line: { color: SHAPE_FILL, width_pt: 1.5, head_arrow: "triangle", tail_arrow: "triangle" }, h: 0 },
      { geometry: "straightConnector1", name: msg("Straight connector"), tool: "straight" },
      { geometry: "straightConnector1", name: msg("Straight arrow connector"), tool: "straight", arrow: true },
      { geometry: "bentConnector3", name: msg("Elbow connector"), tool: "elbow" },
      { geometry: "bentConnector3", name: msg("Elbow arrow connector"), tool: "elbow", arrow: true },
      { geometry: "curvedConnector3", name: msg("Curved connector"), tool: "curved" },
      { geometry: "curvedConnector3", name: msg("Curved arrow connector"), tool: "curved", arrow: true },
      { geometry: "custom", name: msg("Curve"), tool: "curve" },
      { geometry: "custom", name: msg("Freeform"), tool: "freeform" },
      { geometry: "custom", name: msg("Scribble"), tool: "scribble" },
    ],
  },
  {
    title: msg("Rectangles"),
    shapes: [
      { geometry: "rect", name: msg("Rectangle"), fill: true },
      { geometry: "roundRect", name: msg("Rounded rectangle"), fill: true },
      { geometry: "snip1Rect", name: msg("Rectangle with a cut corner"), fill: true },
      { geometry: "round2SameRect", name: msg("Rectangle with rounded top"), fill: true },
    ],
  },
  {
    title: msg("Basic shapes"),
    shapes: [
      { geometry: "ellipse", name: msg("Oval"), fill: true },
      { geometry: "triangle", name: msg("Triangle"), fill: true },
      { geometry: "rtTriangle", name: msg("Right triangle"), fill: true },
      { geometry: "diamond", name: msg("Diamond"), fill: true },
      { geometry: "parallelogram", name: msg("Parallelogram"), fill: true },
      { geometry: "trapezoid", name: msg("Trapezoid"), fill: true },
      { geometry: "pentagon", name: msg("Pentagon"), fill: true },
      { geometry: "hexagon", name: msg("Hexagon"), fill: true },
      { geometry: "octagon", name: msg("Octagon"), fill: true },
      { geometry: "plus", name: msg("Cross"), fill: true },
      { geometry: "heart", name: msg("Heart"), fill: true },
      { geometry: "cloud", name: msg("Cloud"), fill: true },
      { geometry: "star4", name: msg("4-point star"), fill: true },
      { geometry: "star5", name: msg("5-point star"), fill: true },
      { geometry: "star6", name: msg("6-point star"), fill: true },
      { geometry: "donut", name: msg("Ring"), fill: true },
      { geometry: "can", name: msg("Can"), fill: true },
      { geometry: "cube", name: msg("Cube"), fill: true },
      { geometry: "foldedCorner", name: msg("Folded corner"), fill: true },
      { geometry: "frame", name: msg("Frame"), fill: true },
    ],
  },
  {
    title: msg("Block arrows"),
    shapes: [
      { geometry: "rightArrow", name: msg("Right arrow"), fill: true },
      { geometry: "leftArrow", name: msg("Left arrow"), fill: true },
      { geometry: "upArrow", name: msg("Up arrow"), fill: true },
      { geometry: "downArrow", name: msg("Down arrow"), fill: true },
      { geometry: "leftRightArrow", name: msg("Left and right arrow"), fill: true },
      { geometry: "chevron", name: msg("Chevron"), fill: true },
      { geometry: "homePlate", name: msg("Pentagon arrow"), fill: true },
    ],
  },
  {
    title: msg("Callouts"),
    shapes: [
      { geometry: "wedgeRectCallout", name: msg("Rectangular callout"), fill: true },
      { geometry: "wedgeRoundRectCallout", name: msg("Rounded callout"), fill: true },
      { geometry: "wedgeEllipseCallout", name: msg("Oval callout"), fill: true },
    ],
  },
  {
    title: msg("Flowchart"),
    shapes: [
      { geometry: "flowChartProcess", name: msg("Process"), fill: true },
      { geometry: "flowChartDecision", name: msg("Decision"), fill: true },
      { geometry: "flowChartTerminator", name: msg("Start or end"), fill: true },
      { geometry: "flowChartInputOutput", name: msg("Data"), fill: true },
      { geometry: "flowChartPredefinedProcess", name: msg("Predefined process"), fill: true },
      { geometry: "flowChartDocument", name: msg("Document"), fill: true },
      { geometry: "flowChartConnector", name: msg("Connector"), fill: true },
      { geometry: "flowChartAlternateProcess", name: msg("Alternate process"), fill: true },
    ],
  },
];

/** A small drawing of a preset, for the picker. */
function preview(geometry: string, line?: LineSpec): SVGElement {
  const w = 30;
  const h = 22;
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", `-2 -2 ${w + 4} ${h + 4}`);
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", "shape-preview");
  for (const p of presetPaths(geometry, w, h, undefined)) {
    const path = document.createElementNS(ns, "path");
    path.setAttribute("d", p.d);
    path.setAttribute("fill", p.fill ? "currentColor" : "none");
    path.setAttribute("fill-opacity", "0.16");
    path.setAttribute("fill-rule", "evenodd");
    path.setAttribute("stroke", p.stroke ? "currentColor" : "none");
    path.setAttribute("stroke-width", "1.5");
    path.setAttribute("stroke-linejoin", "round");
    path.setAttribute("stroke-linecap", "round");
    svg.append(path);
  }
  // Arrow heads of a line, at its two ends.
  const ends = typeof line === "object" ? line : {};
  const arrows: [number[], number[], string | undefined][] = [
    [[w, h], [0, 0], ends.tail_arrow],
    [[0, 0], [w, h], ends.head_arrow],
  ];
  for (const [tip, from, kind] of arrows) {
    if (!kind) continue;
    const a = Math.atan2(tip[1] - from[1], tip[0] - from[0]);
    const corner = (len: number, side: number) => `${tip[0] - Math.cos(a) * len - Math.sin(a) * side},${tip[1] - Math.sin(a) * len + Math.cos(a) * side}`;
    const head = document.createElementNS(ns, "path");
    head.setAttribute("d", `M${tip[0]},${tip[1]} L${corner(8, 4)} L${corner(8, -4)} Z`);
    head.setAttribute("fill", "currentColor");
    svg.append(head);
  }
  return svg;
}

/** The ids of every paragraph of text in a shape, the text of its members included. */
function allTextParas(s: RShape): number[] {
  return [...textParas(s), ...(s.children ?? []).flatMap(allTextParas)];
}

/** The colours to pick from (and none, and a custom one) in a popover at an element. */
export function pickColor(anchor: HTMLElement, title: string, current: string | undefined, none: string | null, pick: (color: string | null) => void) {
  const grid = h("div", { class: "swatches" });
  const close = popover(anchor, h("div", {}, h("div", { class: "menu-title" }, title), grid));
  const body = grid.parentElement!;
  for (const c of TEXT_COLORS) {
    const on = current?.toUpperCase() === c;
    const s = h("button", { class: on ? "swatch on" : "swatch", type: "button", title: `#${c}`, "aria-label": `#${c}`, "aria-pressed": String(on), style: `background:${hexOr(c)}` });
    s.addEventListener("click", () => {
      close();
      pick(c);
    });
    grid.append(s);
  }
  const noneBtn = none ? h("button", { class: "menu-item", type: "button" }, none) : null;
  noneBtn?.addEventListener("click", () => {
    close();
    pick(null);
  });
  const custom = h("input", { type: "color", value: `#${current ?? "4472C4"}`, "aria-label": t("Custom color") });
  custom.addEventListener("change", () => {
    close();
    pick(custom.value.slice(1).toUpperCase());
  });
  body.append(...(noneBtn ? [noneBtn] : []), h("label", { class: "menu-item custom-color" }, h("span", { class: "menu-label" }, t("Custom color")), custom));
}

export class ShapeTool {
  private frame: ObjectFrame;
  /** Outlines of the other shapes in a selection of several. */
  private outlines: ObjectFrame[] = [];
  private strip: HTMLElement;
  private readout: HTMLElement;
  private target: MPos | null = null;
  /** The member of the selected group or canvas that is selected (its index, then its member's index...); empty for the shape itself. */
  private member: number[] = [];
  /** The other shapes selected with Shift, besides `target`. */
  private others: MPos[] = [];
  /** The text element being edited, if the caret is inside the shape. */
  private textEl: HTMLElement | null = null;
  /** A drag, resize or rotation is under way. */
  private busy = false;
  private lastNudge = -1e9;

  constructor(private ed: Editor) {
    this.frame = new ObjectFrame((e, k) => (k === "rot" ? this.rotate(e) : this.resize(e, k)), { rotate: true, onMove: (e) => this.dragEdge(e) });
    this.readout = h("span", { class: "readout obj-size" });
    this.strip = h("div", { class: "obj-strip shape-strip", style: "display:none", role: "toolbar", "aria-label": t("Shape") });
    overlayLayer().append(ed.life.own(this.frame.el), ed.life.own(this.strip));
    // On the stage, not the flow: a header or footer being edited is not in the flow, and its shapes are selected too.
    ed.life.on(ed.stageElement, "mousedown", (e) => this.onDown(e as MouseEvent), { capture: true });
    ed.life.on(ed.stageElement, "dblclick", (e) => this.onDouble(e as MouseEvent));
    ed.canvas.addEventListener("keydown", (e) => this.onKey(e), true);
    ed.canvas.addEventListener("scroll", () => this.place(), { passive: true });
    ed.life.on(window, "resize", () => this.place());
    ed.life.on(document, "selectionchange", () => {
      if (this.busy) return;
      const s = ed.sel;
      if (!s) return;
      // Selecting just the shape from the keyboard (Shift+Arrow over it) selects it as a click does.
      if (!this.target) {
        const o = Math.min(s.anchor.o, s.focus.o);
        if (s.anchor.p === s.focus.p && Math.abs(s.anchor.o - s.focus.o) === 1 && this.atomAt(s.anchor.p, o)) {
          this.target = { p: s.anchor.p, o };
          this.member = [];
          this.show();
        }
        return;
      }
      const tg = this.target;
      const onAtom = s.anchor.p === tg.p && Math.min(s.anchor.o, s.focus.o) === tg.o && Math.max(s.anchor.o, s.focus.o) === tg.o + 1;
      if (this.textEl) {
        const paras = allTextParas(this.model() ?? ({} as RShape));
        if (!this.textEl.isConnected || paras.includes(s.focus.p)) return;
        // A selection that starts in the text stays in it: Safari's double-click on the last word of a box selects up to the shape around
        // the text, which would read as a selection outside the box and end the editing. Its end is brought back to the end of the text.
        const dom = window.getSelection();
        if (paras.includes(s.anchor.p) && dom?.anchorNode && this.textEl.contains(dom.anchorNode)) {
          dom.extend(this.textEl, this.textEl.childNodes.length);
          return;
        }
        this.leaveText(false);
      }
      if (!onAtom) this.hide();
    });
  }

  private get engine(): Engine {
    return this.ed.engine;
  }

  private atomAt(p: number, o: number): HTMLElement | null {
    return this.ed.paraElement(p)?.querySelector<HTMLElement>(`:scope > .atom.shape[data-o="${o}"]`) ?? null;
  }

  private atom(): HTMLElement | null {
    return this.target ? this.atomAt(this.target.p, this.target.o) : null;
  }

  /** The element of what is selected: the shape, or its member. */
  private subjectEl(): HTMLElement | null {
    const a = this.atom();
    if (!a || !this.member.length) return a;
    return a.querySelector<HTMLElement>(`.shape-child[data-m="${this.member.join(".")}"]`);
  }

  /** The shape as the engine reads it (a group with all its members). */
  private model(): RShape | null {
    if (!this.target) return null;
    return this.shapeAt(this.target);
  }

  private shapeAt(pos: MPos): RShape | null {
    try {
      return this.engine.shapeAt(pos);
    } catch {
      return null;
    }
  }

  /** What is selected: the shape, or its member. */
  private subject(): RShape | null {
    let s = this.model();
    for (const i of this.member) s = s?.children?.[i] ?? null;
    return s;
  }

  /** A shape is selected (clicked, right-clicked or edited). */
  get selected(): boolean {
    return !!this.atom();
  }

  /** The caret is in the shape's text. */
  get typing(): boolean {
    return !!this.textEl;
  }

  /** The selected shape has a text frame to type in. */
  get hasText(): boolean {
    return !!this.subject()?.text;
  }

  /** The wrapping of the selected shape (`inline`, `square`, ...). */
  get wrap(): string | null {
    return this.model()?.wrap ?? null;
  }

  /** The position of the selected shape's object. */
  get targetPos(): MPos | null {
    return this.atom() ? this.target : null;
  }

  /** Several shapes are selected. */
  get multiple(): boolean {
    return this.others.length > 0;
  }

  /** The positions of the shapes that are selected (the one the frame is on first); empty when none is. */
  get positions(): MPos[] {
    return this.atom() ? this.selection() : [];
  }

  /** Every shape that is selected, the one the frame is on first. */
  private selection(): MPos[] {
    return this.target ? [this.target, ...this.others] : [];
  }

  /** The selected shapes can be grouped: two or more, all floating. */
  get canGroup(): boolean {
    if (!this.others.length || this.ed.readOnly) return false;
    return this.selection().every((p) => {
      const s = this.shapeAt(p);
      return !!s && s.kind === "shape" && s.wrap !== "inline";
    });
  }

  /** The selected shape is a group that can be taken apart. */
  get canUngroup(): boolean {
    const s = this.model();
    return !this.ed.readOnly && !this.others.length && !this.member.length && s?.kind === "group" && !s.flipH && !s.flipV;
  }

  /** The selected shape floats over the text, so it has a place in the stacking order. */
  get floating(): boolean {
    return !this.member.length && this.selection().every((p) => this.shapeAt(p)?.wrap !== "inline");
  }

  // -- the Format shape panel ------------------------------------------------------------------

  private watchers = new Set<() => void>();
  /** Opens the Format shape panel (set by the extension that owns it). */
  openFormat: (() => void) | null = null;
  /** Starts the tool a picker entry names (curves, freeforms, connectors; set by the extension that owns them). */
  onTool: ((tool: GeometryTool, arrow: boolean) => void) | null = null;
  /** Buttons that other tools add to the strip over a selected shape. */
  stripExtras: ((shape: RShape) => HTMLElement[])[] = [];

  /** Hide the frame (its handles and outline) while another tool works on the shape; the strip stays. */
  suspendFrame(on: boolean, who = "tool") {
    if (on) this.suspenders.add(who);
    else this.suspenders.delete(who);
    this.frame.el.classList.toggle("suspended", this.suspenders.size > 0);
  }
  private suspenders = new Set<string>();

  private flipButton(axis: "h" | "v"): HTMLButtonElement {
    const on = axis === "h" ? this.subject()?.flipH : this.subject()?.flipV;
    const label = axis === "h" ? t("Flip horizontal") : t("Flip vertical");
    const b = this.iconButton(label, axis === "h" ? SHAPE_ICONS.flipHorizontal : SHAPE_ICONS.flipVertical, () => this.flip(axis));
    b.setAttribute("aria-pressed", String(!!on));
    b.classList.toggle("on", !!on);
    return b;
  }

  /** Run `fn` whenever the selection or the selected shape changes (the Format shape panel follows it). Returns a way to stop. */
  onSelection(fn: () => void): () => void {
    this.watchers.add(fn);
    return () => this.watchers.delete(fn);
  }

  private notify() {
    for (const fn of this.watchers) fn();
  }

  /** The selected shape, or the member of a group that is selected, as the engine reads it; null with nothing selected. */
  get current(): RShape | null {
    return this.atom() ? this.subject() : null;
  }

  /** The shape that holds the selection (the group, when one of its members is selected). */
  get whole(): RShape | null {
    return this.atom() ? this.model() : null;
  }

  /** A member of a group or canvas is selected, not the shape itself. */
  get isMember(): boolean {
    return this.member.length > 0;
  }

  /** Where the selected shape or member is, in points counted the way its position is stored; null if it has no place to measure. */
  get measured(): { x: number; y: number } | null {
    const s = this.subject();
    const a = this.subjectEl();
    return s && a ? this.position(s, a) : null;
  }

  /** Change the selected shape (or member, or every shape of a selection of several) with a `set_shape` patch. One undo step, or merged into the one before with `merge`. */
  patch(patch: Record<string, unknown>, opts: { merge?: boolean } = {}) {
    if (!this.atom() || this.ed.readOnly) return;
    // With several shapes selected the change is for all of them.
    if (this.member.length || !this.others.length) return this.apply(patch, opts);
    const tg = this.target!;
    this.ed.exec(
      this.selection().map((p) => ({ op: "set_shape", at: p, ...patch }) as Op),
      { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } }, merge: opts.merge },
    );
    this.show();
  }

  /** Mirror the selected shapes left to right or top to bottom. */
  flip(axis: "h" | "v") {
    const all = this.member.length ? [this.target!] : this.selection();
    if (!all.length || this.ed.readOnly) return;
    const member = this.member.length ? { member: this.member } : {};
    const tg = this.target!;
    const ops = all.map((p) => {
      const s = this.member.length ? this.subject() : this.shapeAt(p);
      return { op: "set_shape", at: p, ...(axis === "h" ? { flip_h: !s?.flipH } : { flip_v: !s?.flipV }), ...member } as Op;
    });
    this.ed.exec(ops, { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } } });
    this.show();
  }

  /** Turn the selected shapes a quarter turn (90 is to the right). */
  rotateBy(deg: number) {
    const all = this.member.length ? [this.target!] : this.selection();
    if (!all.length || this.ed.readOnly) return;
    const member = this.member.length ? { member: this.member } : {};
    const tg = this.target!;
    const ops = all.map((p) => {
      const s = this.member.length ? this.subject() : this.shapeAt(p);
      return { op: "set_shape", at: p, rot: (((s?.rot ?? 0) + deg) % 360 + 360) % 360, ...member } as Op;
    });
    this.ed.exec(ops, { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } } });
    this.show();
  }

  // -- selecting -----------------------------------------------------------------------------

  /** The shape that is behind the text at a point where there is no text to click on, if any. */
  private behindAt(e: MouseEvent): HTMLElement | null {
    const atom = behindAt(this.ed.root, e);
    return atom?.classList.contains("shape") ? atom : null;
  }

  /** The members under the pointer inside a group, outermost first, as paths. */
  private chain(atom: HTMLElement, target: HTMLElement): number[][] {
    const out: number[][] = [];
    for (let el = target.closest<HTMLElement>(".shape-child"); el && atom.contains(el); el = el.parentElement?.closest<HTMLElement>(".shape-child") ?? null) {
      out.unshift((el.dataset.m ?? "").split(".").map(Number));
    }
    return out;
  }

  private onDown(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const root = this.ed.root;
    if (!root.contains(target)) return;
    if (this.textEl) {
      // The caret is in the text: clicks in it are the browser's own; a click anywhere else leaves it.
      if (this.textEl.contains(target)) return;
      this.leaveText(false);
    }
    let atom = target.closest<HTMLElement>(".atom.shape");
    if (atom && !root.contains(atom)) return;
    if (!atom && !target.closest(".atom")) atom = this.behindAt(e);
    if (!atom) return;
    const para = atom.closest<HTMLElement>("[data-p]");
    if (!para) return;
    e.preventDefault();
    const pos = { p: Number(para.dataset.p), o: Number(atom.dataset.o) };
    // Shift adds a shape to the selection, or takes it out.
    if (e.shiftKey && e.button === 0 && this.target && !this.ed.readOnly) {
      this.toggle(pos);
      return;
    }
    // A right-click on one of several selected shapes keeps them all selected: the menu works on the lot.
    if (e.button === 2 && this.others.length && this.selection().some((s) => s.p === pos.p && s.o === pos.o)) return;
    const was = this.target;
    const sameShape = !!was && was.p === pos.p && was.o === pos.o && !this.others.length;
    const before = this.member;
    if (sameShape) {
      // The first click selects the group, the next one a member of it, the next a member of that.
      const chain = this.chain(atom, target);
      const eq = (a: number[], b: number[]) => a.length === b.length && a.every((v, i) => v === b[i]);
      if (!chain.length) this.member = [];
      else if (!before.length) this.member = chain[0];
      else {
        const at = chain.findIndex((c) => eq(c, before));
        this.member = at >= 0 ? (chain[at + 1] ?? before) : chain[Math.min(before.length - 1, chain.length - 1)];
      }
    } else {
      this.target = pos;
      this.member = [];
      this.others = [];
    }
    // A click in the text of a shape that is already selected puts the caret there.
    const text = target.closest<HTMLElement>(".shape-text");
    if (sameShape && this.member.join(".") === before.join(".") && e.button === 0 && text && text.parentElement === this.subjectEl() && this.subject()?.text && !this.ed.readOnly) {
      this.enterText({ x: e.clientX, y: e.clientY });
      return;
    }
    this.ed.focusRoot();
    this.ed.select({ anchor: this.target!, focus: { p: this.target!.p, o: this.target!.o + 1 } });
    this.show();
    if (e.button === 0 && !this.ed.readOnly && (this.member.length || atom.dataset.wrap !== "inline")) this.dragBody(e);
  }

  /** Add a shape to the selection, or take it out of it. */
  private toggle(pos: MPos) {
    const same = (a: MPos) => a.p === pos.p && a.o === pos.o;
    if (!this.atomAt(pos.p, pos.o)) return;
    this.member = [];
    if (this.target && same(this.target)) {
      if (this.others.length) this.target = this.others.shift()!;
    } else if (this.others.some(same)) this.others = this.others.filter((p) => !same(p));
    else this.others.push(pos);
    this.ed.select({ anchor: this.target!, focus: { p: this.target!.p, o: this.target!.o + 1 } });
    this.show();
  }

  private onDouble(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!this.ed.root.contains(target)) return;
    const atom = target.closest<HTMLElement>(".atom.shape") ?? (target.closest(".atom") ? null : this.behindAt(e));
    if (!atom || this.ed.readOnly || !this.atom()) return;
    e.preventDefault();
    // A SmartArt graphic is drawn, not edited.
    if (this.model()?.kind === "diagram") return toast(t("Edit SmartArt in Word."));
    this.enterText({ x: e.clientX, y: e.clientY });
  }

  /** Select the shape at a position, as a click on it does; false if there is none. */
  selectAt(p: number, o: number): boolean {
    const atom = this.atomAt(p, o);
    if (!atom) return false;
    this.ed.ensureDrawn(atom);
    atom.scrollIntoView({ block: "center", inline: "nearest" });
    this.target = { p, o };
    this.member = [];
    this.others = [];
    this.ed.focusRoot();
    this.ed.select({ anchor: { p, o }, focus: { p, o: o + 1 } });
    this.show();
    return true;
  }

  /** Select several shapes (the first is the one the frame is on). */
  selectShapes(list: MPos[]): boolean {
    const found = list.filter((p) => this.atomAt(p.p, p.o));
    if (!found.length) return false;
    const [first, ...rest] = found;
    this.selectAt(first.p, first.o);
    this.others = rest;
    this.show();
    return true;
  }

  /** Re-attach after the document re-renders (the shape's element is replaced). */
  refresh() {
    if (!this.target) return;
    const atom = this.atom();
    if (!atom) {
      this.textEl = null;
      editing.clear();
      return this.hide();
    }
    this.others = this.others.filter((p) => this.atomAt(p.p, p.o));
    if (this.member.length && !this.subjectEl()) this.member = [];
    if (this.textEl) {
      const box = this.subjectEl()?.querySelector<HTMLElement>(":scope > .shape-text[data-editing]");
      if (!box) {
        this.textEl = null;
        editing.clear();
      } else {
        this.textEl = box;
        // The paragraph was drawn again: the caret goes back into the new text.
        if (!box.contains(document.activeElement)) {
          box.focus({ preventScroll: true });
          if (this.ed.sel) this.ed.select(this.ed.sel);
        }
      }
    }
    this.show();
  }

  private show() {
    const a = this.subjectEl();
    if (!a) return this.hide();
    this.frame.el.style.display = "";
    // Several shapes, or the caret in the text: the frame is only an outline (the strip would cover the text).
    this.frame.setHandles(!this.textEl && !this.others.length);
    this.strip.style.display = this.textEl ? "none" : "";
    if (!this.textEl) this.buildStrip();
    this.place();
    this.notify();
  }

  hide() {
    if (this.textEl) this.leaveText(false);
    this.target = null;
    this.member = [];
    this.others = [];
    this.frame.hide();
    for (const o of this.outlines) o.hide();
    this.strip.style.display = "none";
    this.notify();
  }

  /** The total turn of an element: its own and that of the groups around it. */
  private turn(el: HTMLElement, from = 0): number {
    let rot = 0;
    for (let e: HTMLElement | null = el; e; e = e.parentElement) {
      rot += Number(e.dataset.rot) || 0;
      if (e.classList.contains("atom")) break;
    }
    return rot - from;
  }

  /** The element's own box on screen (before rotation), in viewport pixels. */
  private box(a: HTMLElement) {
    const r = a.getBoundingClientRect();
    const rot = this.turn(a);
    if (!rot) return { left: r.left, top: r.top, width: r.width, height: r.height, rot: 0 };
    const z = this.ed.zoom;
    const w = Number(a.dataset.w) * PT_PX * z;
    const hh = Number(a.dataset.h) * PT_PX * z;
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    return { left: cx - w / 2, top: cy - hh / 2, width: w, height: hh, rot };
  }

  private place() {
    const a = this.subjectEl();
    if (!a || this.frame.el.style.display === "none") return;
    const b = this.box(a);
    const r = a.getBoundingClientRect();
    const c = this.ed.canvas.getBoundingClientRect();
    const visible = r.bottom > c.top && r.top < c.bottom;
    // A line is a hair thick: its frame is at least big enough to grab.
    const pad = Math.max(0, 8 - b.height) / 2;
    this.frame.place({ ...b, top: b.top - pad, height: b.height + pad * 2 }, visible);
    this.placeOutlines(visible);
    // Measured at the left edge, where it has all the room it can use.
    this.strip.style.left = "0px";
    const sw = this.strip.offsetWidth || 420;
    const sh = this.strip.offsetHeight || 36;
    // Above the shape and its rotation handle; below the shape when there is no room.
    const above = Math.min(r.top, this.frame.el.querySelector(".rot")?.getBoundingClientRect().top ?? r.top);
    let top = above - sh - 10 < c.top + 4 ? r.bottom + 10 : above - sh - 10;
    // In a header or footer the strip of the story is up there: this one goes below it.
    if (this.ed.story) top = Math.max(r.bottom + 10, (document.querySelector(".story-strip")?.getBoundingClientRect().bottom ?? 0) + 8);
    this.strip.style.left = `${Math.max(8, Math.min(window.innerWidth - sw - 8, r.left + r.width / 2 - sw / 2))}px`;
    this.strip.style.top = `${Math.max(4, top)}px`;
    this.strip.style.display = visible && !this.textEl ? "" : "none";
    const sub = this.subject();
    this.readout.textContent = this.sizeText(Number(a.dataset.w) || sub?.w || 0, Number(a.dataset.h) || sub?.h || 0, Number(a.dataset.rot) || 0);
  }

  /** The outlines around the other shapes of a selection of several. */
  private placeOutlines(visible: boolean) {
    while (this.outlines.length < this.others.length) {
      const f = new ObjectFrame(() => {}, {});
      f.setHandles(false);
      overlayLayer().append(this.ed.life.own(f.el));
      this.outlines.push(f);
    }
    this.outlines.forEach((f, i) => {
      const a = this.others[i] ? this.atomAt(this.others[i].p, this.others[i].o) : null;
      if (!a) return f.hide();
      f.setHandles(false);
      f.place(this.box(a), visible);
    });
  }

  private sizeText(w: number, hh: number, rot: number) {
    return `${Math.round(w)} × ${Math.round(hh)} pt${rot ? `, ${Math.round(rot)}°` : ""}`;
  }

  // -- changing ------------------------------------------------------------------------------

  private apply(patch: Record<string, unknown>, opts: { merge?: boolean } = {}) {
    if (!this.target) return;
    const tg = this.target;
    const member = this.member.length ? { member: this.member } : {};
    this.ed.exec({ op: "set_shape", at: tg, ...patch, ...member }, { select: { anchor: { p: tg.p, o: tg.o }, focus: { p: tg.p, o: tg.o + 1 } }, merge: opts.merge });
    this.target = tg;
    this.show();
  }

  /** Change the selected shape's fill (hex, or null for none). */
  setFill(color: string | null) {
    this.apply({ fill: color ?? "none" });
  }

  /** Change the selected shape's outline: a colour (hex), null for none, and the parts to change. */
  setLine(color: string | null | undefined, patch: { width_pt?: number; dash?: string } = {}) {
    const s = this.subject();
    if (color === null) return this.apply({ line: "none" });
    const line: Record<string, unknown> = { ...patch };
    if (color) line.color = color;
    // An outline that wasn't there starts as a plain line in a visible weight.
    if (!s?.line) {
      line.color ??= SHAPE_LINE;
      line.width_pt ??= 1;
    }
    this.apply({ line });
  }

  setWrap(wrap: string) {
    // The wrapping belongs to the whole shape, also when a member of it is selected.
    this.member = [];
    this.apply({ wrap });
  }

  /** Ask for the selected shape's description for screen readers. */
  async editAlt() {
    const tg = this.target;
    const s = this.subject();
    if (!tg || !s) return;
    const member = this.member;
    const v = await promptDialog({ title: t("Alt text"), label: t("Describe the shape for screen readers"), value: s.alt ?? "", ok: t("Save") });
    if (v !== null) {
      this.target = tg;
      this.member = member;
      this.apply({ alt: v });
    }
  }

  /** Remove the selected shape (or every shape of a selection of several). False when nothing was removed. */
  remove(): boolean {
    const all = this.selection();
    if (!all.length) return false;
    this.hide();
    const ops = [...all].sort((a, b) => b.p - a.p || b.o - a.o).map((p) => ({ op: "delete", from: p, to: { p: p.p, o: p.o + 1 } }) as Op);
    const done = !!this.ed.exec(ops, { select: all[0] });
    this.ed.focus();
    return done;
  }

  /** Open the fill colours at the selected shape. */
  pickFill(anchor?: HTMLElement) {
    pickColor(anchor ?? this.frame.el, t("Fill"), this.subject()?.fill?.color, t("No fill"), (c) => this.setFill(c));
  }

  /** Open the outline colours at the selected shape. */
  pickOutline(anchor?: HTMLElement) {
    pickColor(anchor ?? this.frame.el, t("Outline"), this.subject()?.line?.color, t("No outline"), (c) => this.setLine(c));
  }

  // -- arranging -------------------------------------------------------------------------------

  /** Move the selected shapes in the stacking order. */
  order(how: "front" | "back" | "forward" | "backward") {
    const all = this.selection();
    if (!all.length || this.ed.readOnly) return;
    const z = (p: MPos) => this.shapeAt(p)?.z ?? 0;
    // To the front or forward: the lowest first, so they keep their order among themselves; the other way round for back.
    const sorted = [...all].sort((a, b) => (how === "front" || how === "forward" ? z(a) - z(b) : z(b) - z(a)));
    const tg = this.target!;
    this.ed.exec(
      sorted.map((p) => ({ op: "set_object_order", at: p, order: how }) as Op),
      { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } } },
    );
    this.show();
  }

  /** Put the selected shapes behind the text or in front of it. */
  setBehind(behind: boolean) {
    const all = this.selection();
    if (!all.length || this.ed.readOnly) return;
    const tg = this.target!;
    this.ed.exec(
      all.map((p) => ({ op: "set_behind_text", at: p, behind }) as Op),
      { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } } },
    );
    this.show();
  }

  /** Where a shape's top-left corner is, in points from what its position counts from. */
  private offsetsOf(pos: MPos, frameOf_: { relX?: string; relY?: string }): { x: number; y: number } | null {
    const a = this.atomAt(pos.p, pos.o);
    const para = a?.closest<HTMLElement>("[data-p]");
    if (!a || !para) return null;
    const b = this.box(a);
    const frame = frameOf(this.ed.floatHost, para, frameOf_.relX, frameOf_.relY);
    return offsetsAt(this.ed.floatHost, frame, b);
  }

  /** Join the selected shapes into one. */
  group() {
    if (!this.canGroup) return;
    const all = this.selection();
    const first = this.shapeAt(all[0])!;
    // Each shape's corner, counted from what the first one's position counts from.
    const firstPara = this.atomAt(all[0].p, all[0].o)?.closest<HTMLElement>("[data-p]");
    if (!firstPara) return;
    const frame = frameOf(this.ed.floatHost, firstPara, first.relX, first.relY);
    const offsets = all.map((p) => {
      const a = this.atomAt(p.p, p.o);
      const o = a ? offsetsAt(this.ed.floatHost, frame, this.box(a)) : { x: 0, y: 0 };
      return [Math.round(o.x * 10) / 10, Math.round(o.y * 10) / 10];
    });
    const first0 = all[0];
    this.hide();
    const res = this.ed.exec({ op: "group_shapes", at: all, offsets }, { select: first0 });
    const caret = res?.[0]?.caret;
    if (caret) this.selectAt(first0.p, caret.o - 1);
  }

  /** Take the selected group apart; the shapes it held are then selected. */
  ungroup() {
    if (!this.canUngroup || !this.target) return;
    const tg = this.target;
    const s = this.model()!;
    const o = this.offsetsOf(tg, s.wrap === "inline" ? {} : { relX: s.relX, relY: s.relY });
    this.hide();
    const op: Record<string, unknown> = { op: "ungroup_shape", at: tg };
    if (o) op.offset = [Math.round(o.x * 10) / 10, Math.round(o.y * 10) / 10];
    const res = this.ed.exec(op as Op, { select: tg });
    const caret = res?.[0]?.caret;
    if (caret) this.selectShapes(Array.from({ length: caret.o - tg.o }, (_, i) => ({ p: tg.p, o: tg.o + i })));
  }

  // -- the strip -----------------------------------------------------------------------------

  private chip(label: string, color: string | undefined, open: (anchor: HTMLElement) => void): HTMLButtonElement {
    const chip = h("span", { class: color ? "chip" : "chip none", style: hex(color) ? `background:${hex(color)}` : undefined });
    const b = h("button", { type: "button", class: "tb color-chip", title: label, "aria-label": label }, chip, h("span", {}, label));
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => open(b));
    return b;
  }

  private iconButton(label: string, icon: string, run: () => void, disabled = false): HTMLButtonElement {
    const b = h("button", { class: "tb", type: "button", title: label, "aria-label": label, disabled });
    b.innerHTML = icon;
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", run);
    return b;
  }

  /** The Arrange button: stacking order, behind and in front of the text, group and ungroup. */
  private arrangeButton(): HTMLButtonElement {
    const b = h("button", { class: "tb", type: "button", title: t("Arrange"), "aria-label": t("Arrange"), "aria-haspopup": "menu" });
    b.innerHTML = ARRANGE_ICONS.bringToFront;
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () =>
      openArrangeMenu(b, {
        floating: this.floating,
        behind: this.wrap === "behind",
        order: (how) => this.order(how),
        setBehind: (on) => this.setBehind(on),
        group: this.canGroup ? () => this.group() : undefined,
        ungroup: this.canUngroup ? () => this.ungroup() : undefined,
      }),
    );
    return b;
  }

  private buildStrip() {
    const top = this.model();
    const s = this.subject();
    if (!top || !s) return;
    if (this.others.length) {
      // Several shapes: group them, order them, delete them.
      const group = h("button", { class: "tb text-tb", type: "button", title: t("Group"), "aria-label": t("Group"), disabled: !this.canGroup }, t("Group"));
      group.addEventListener("mousedown", (e) => e.preventDefault());
      group.addEventListener("click", () => this.group());
      const del = this.iconButton(t("Delete shapes"), I.close, () => void this.ed.command("shape.delete"));
      const count = this.selection().length;
      this.strip.replaceChildren(h("span", { class: "readout obj-size" }, tn(count, "{n} shape selected", "{n} shapes selected")), group, this.arrangeButton(), del);
      return;
    }
    const fill = this.chip(t("Fill"), s.fill?.color, (b) => this.pickFill(b));
    const outline = this.chip(t("Outline"), s.line?.color, (b) => this.pickOutline(b));
    // A group or canvas is only a frame for its members: their colours aren't its own.
    if (s.kind !== "shape") for (const b of [fill, outline]) b.disabled = true;
    const weight = h("select", { class: "tb-select", "aria-label": t("Outline weight"), title: t("Outline weight"), disabled: !s.line });
    const w = s.line?.widthPt ?? 1;
    const weights = WEIGHTS.includes(w) ? WEIGHTS : [...WEIGHTS, w].sort((a, b) => a - b);
    for (const v of weights) weight.append(h("option", { value: String(v), selected: v === w }, `${v} pt`));
    weight.addEventListener("change", () => this.setLine(undefined, { width_pt: Number(weight.value) }));
    const dash = h("select", { class: "tb-select", "aria-label": t("Outline style"), title: t("Outline style"), disabled: !s.line });
    const cur = s.line?.dash ?? "solid";
    const known = DASH_CHOICES.some(([id]) => id === cur);
    for (const [id, label] of known ? DASH_CHOICES : [...DASH_CHOICES, [cur, cur] as [string, string]]) dash.append(h("option", { value: id, selected: id === cur }, t(label)));
    dash.addEventListener("change", () => this.setLine(undefined, { dash: dash.value }));
    const wrap = top.wrap || "inline";
    const wraps = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Wrapping") });
    for (const [id, label, title] of WRAPS) {
      const b = h("button", { type: "button", class: id === wrap || (id === "square" && wrap === "tight") ? "on" : "", title: t(title) }, t(label));
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => this.setWrap(id));
      wraps.append(b);
    }
    const alt = h("button", { class: "tb", type: "button", title: t("Alt text"), "aria-label": t("Alt text") }, t("Alt"));
    alt.addEventListener("click", () => void this.editAlt());
    const del = h("button", { class: "tb", type: "button", title: t("Delete shape"), "aria-label": t("Delete shape") });
    del.innerHTML = I.close;
    del.addEventListener("click", () => void this.ed.command("shape.delete"));
    for (const el of [weight, dash]) el.addEventListener("mousedown", (e) => e.stopPropagation());
    const parts: HTMLElement[] = [fill, outline, weight, dash];
    parts.push(this.flipButton("h"), this.flipButton("v"));
    // The wrapping, the order and deleting are the whole shape's.
    if (!this.member.length) parts.push(wraps, this.arrangeButton());
    if (this.canUngroup) parts.push(this.iconButton(t("Ungroup"), ARRANGE_ICONS.ungroup, () => this.ungroup()));
    for (const extra of this.stripExtras) parts.push(...extra(s));
    parts.push(this.readout, alt);
    if (this.openFormat) parts.push(this.iconButton(t("Format shape"), SHAPE_ICONS.formatShape, () => this.openFormat?.()));
    if (!this.member.length) parts.push(del);
    this.strip.replaceChildren(...parts);
  }

  // -- moving, resizing, rotating --------------------------------------------------------------

  /**
   * Where the shape is, in the terms its position is stored in: points from what its position counts from (the
   * page, the margins, the column or the paragraph) and a member's from the corner of its group. Measured from what
   * is on screen, so a shape placed by alignment or by wrapping alone starts from where it really is.
   */
  private position(s: RShape, a: HTMLElement): { x: number; y: number } | null {
    if (this.member.length) return { x: Math.round((s.childX ?? 0) * 10) / 10, y: Math.round((s.childY ?? 0) * 10) / 10 };
    const para = a.closest<HTMLElement>("[data-p]");
    if (!para) return null;
    // A shape drawn out of the text flow is placed from the page (see float-place.ts); a float keeps the paragraph's edge.
    if (a.dataset.abs) {
      const frame = frameOf(this.ed.floatHost, para, s.relX, s.relY);
      const o = offsetsAt(this.ed.floatHost, frame, this.box(a));
      return { x: Math.round(o.x * 10) / 10, y: Math.round(o.y * 10) / 10 };
    }
    const z = this.ed.zoom;
    const b = this.box(a);
    const pr = para.getBoundingClientRect();
    let x = (b.left - pr.left) / z / PT_PX;
    let y = (b.top - pr.top) / z / PT_PX;
    if (s.relX === "page") x += pageMetrics.marginL;
    if (s.relY === "page" || s.relY === "margin") {
      const stage = this.ed.stageElement.getBoundingClientRect();
      const sy = (b.top - stage.top) / z;
      let k = 0;
      while (k < this.ed.pages - 1 && sy >= this.ed.pageBox(k + 1).top) k++;
      const page = this.ed.pageBox(k);
      y = (sy - page.top - (s.relY === "margin" ? page.mT : 0)) / PT_PX;
    }
    return { x: Math.round(x * 10) / 10, y: Math.round(y * 10) / 10 };
  }

  /** What a drag moves: the selected shapes (or the member), where each starts. */
  private dragSet(): { pos: MPos; el: HTMLElement; start: { x: number; y: number }; frame: ObjectFrame }[] {
    const out: { pos: MPos; el: HTMLElement; start: { x: number; y: number }; frame: ObjectFrame }[] = [];
    if (this.member.length) {
      const s = this.subject();
      const el = this.subjectEl();
      const start = s && el ? this.position(s, el) : null;
      if (s && el && start && this.target) out.push({ pos: this.target, el, start, frame: this.frame });
      return out;
    }
    this.selection().forEach((pos, i) => {
      const s = this.shapeAt(pos);
      const el = this.atomAt(pos.p, pos.o);
      const start = s && el && s.wrap !== "inline" ? this.position(s, el) : null;
      if (s && el && start) out.push({ pos, el, start, frame: i === 0 ? this.frame : this.outlines[i - 1] });
    });
    return out;
  }

  /** Drag the shape itself: the frame follows the pointer, and the shape lands where it is let go. */
  private dragBody(e: MouseEvent) {
    // A shape in line with the text can't be moved; it is left out.
    const set = this.dragSet();
    if (!set.length) return;
    const boxes = set.map((d) => this.box(d.el));
    this.busy = true;
    track(
      e,
      (dx, dy) => {
        set.forEach((d, i) => d.frame.place({ ...boxes[i], left: boxes[i].left + dx, top: boxes[i].top + dy }));
        this.strip.style.display = "none";
      },
      (moved, ev) => {
        this.busy = false;
        if (!moved) return this.show();
        const z = this.ed.zoom;
        let dx = (ev.clientX - e.clientX) / z / PT_PX;
        let dy = (ev.clientY - e.clientY) / z / PT_PX;
        if (this.member.length) {
          // A member moves in its group's own directions, which are turned when the group is.
          const el = set[0].el;
          const turn = -this.turn(el, Number(el.dataset.rot) || 0) * RAD;
          [dx, dy] = [dx * Math.cos(turn) - dy * Math.sin(turn), dx * Math.sin(turn) + dy * Math.cos(turn)];
        }
        const member = this.member.length ? { member: this.member } : {};
        const tg = this.target!;
        this.ed.exec(
          set.map((d) => ({ op: "set_shape", at: d.pos, x: Math.round((d.start.x + dx) * 10) / 10, y: Math.round((d.start.y + dy) * 10) / 10, ...member }) as Op),
          { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } } },
        );
        this.show();
      },
    );
  }

  /** Drag from the frame's edge: the same move as dragging the shape. */
  private dragEdge(e: PointerEvent) {
    if (this.ed.readOnly || (!this.member.length && this.model()?.wrap === "inline")) return;
    e.preventDefault();
    e.stopPropagation();
    this.dragBody(e);
  }

  private resize(e: PointerEvent, k: Handle) {
    const s = this.subject();
    const a = this.subjectEl();
    if (!s || !a || !this.target) return;
    e.preventDefault();
    e.stopPropagation();
    this.busy = true;
    const z = this.ed.zoom;
    const w0 = s.w;
    const h0 = s.h;
    const rotDeg = this.turn(a);
    const rot = rotDeg * RAD;
    const cos = Math.cos(rot);
    const sin = Math.sin(rot);
    const sx = k.includes("e") ? 1 : k.includes("w") ? -1 : 0;
    const sy = k.includes("s") ? 1 : k.includes("n") ? -1 : 0;
    const top = this.model();
    const start = !rotDeg && (this.member.length || (top?.wrap !== "inline" && !top?.align)) && (this.member.length || s.x !== undefined) ? this.position(s, a) : null;
    let w = w0;
    let hh = h0;
    track(
      e,
      (dx, dy, ev) => {
        // The pointer's travel along the shape's own axes.
        const lx = (dx * cos + dy * sin) / z / PT_PX;
        const ly = (-dx * sin + dy * cos) / z / PT_PX;
        w = sx ? Math.max(4, w0 + sx * lx) : w0;
        hh = sy ? Math.max(h0 === 0 ? 0 : 4, h0 + sy * ly) : h0;
        if (sx && sy && ev.shiftKey && w0 > 0 && h0 > 0) {
          const f = Math.max(w / w0, hh / h0);
          w = w0 * f;
          hh = h0 * f;
        }
        a.style.width = `${w * PT_PX}px`;
        a.style.setProperty(s.autoFit && s.text && !this.member.length ? "min-height" : "height", `${Math.max(hh, 0.5) * PT_PX}px`);
        a.dataset.w = String(w);
        a.dataset.h = String(hh);
        this.place();
      },
      (moved) => {
        this.busy = false;
        if (!moved) return this.show();
        const patch: Record<string, unknown> = { w: Math.round(w * 10) / 10, h: Math.round(hh * 10) / 10 };
        // Pulling the top or left edge moves a shape that is placed by offsets, so the far edge stays put.
        if (start) {
          if (sx < 0) patch.x = Math.round((start.x - (w - w0)) * 10) / 10;
          if (sy < 0) patch.y = Math.round((start.y - (hh - h0)) * 10) / 10;
        }
        this.apply(patch);
      },
    );
  }

  private rotate(e: PointerEvent) {
    const s = this.subject();
    const a = this.subjectEl();
    if (!s || !a || !this.target) return;
    e.preventDefault();
    e.stopPropagation();
    this.busy = true;
    const r = a.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    const rot0 = s.rot ?? 0;
    const from = Math.atan2(e.clientY - cy, e.clientX - cx);
    let rot = rot0;
    track(
      e,
      (_dx, _dy, ev) => {
        rot = rot0 + (Math.atan2(ev.clientY - cy, ev.clientX - cx) - from) / RAD;
        if (ev.shiftKey) rot = Math.round(rot / 15) * 15;
        rot = ((Math.round(rot * 10) / 10) % 360 + 360) % 360;
        a.style.transform = `rotate(${rot}deg)`;
        a.dataset.rot = String(rot);
        this.place();
      },
      (moved) => {
        this.busy = false;
        if (!moved) return this.show();
        this.apply({ rot });
      },
    );
  }

  /** Move the selected shapes by arrow keys (a point; ten with Shift). */
  private nudge(dx: number, dy: number) {
    const set = this.dragSet();
    if (!set.length) return;
    const now = performance.now();
    const merge = now - this.lastNudge < 800;
    this.lastNudge = now;
    const member = this.member.length ? { member: this.member } : {};
    const tg = this.target!;
    this.ed.exec(
      set.map((d) => ({ op: "set_shape", at: d.pos, x: d.start.x + dx, y: d.start.y + dy, ...member }) as Op),
      { select: { anchor: tg, focus: { p: tg.p, o: tg.o + 1 } }, merge },
    );
    this.show();
  }

  // -- the text inside -------------------------------------------------------------------------

  /** Put the caret in the selected shape's text (at a point on screen, else the end). */
  enterText(at?: { x: number; y: number }): boolean {
    const a = this.subjectEl();
    const s = this.subject();
    if (!a || !s?.text || this.ed.readOnly) return false;
    const ids = textParas(s);
    if (!ids.length) return false;
    editing.clear();
    for (const id of ids) editing.add(id);
    const box = a.querySelector<HTMLElement>(":scope > .shape-text");
    if (!box) return false;
    box.contentEditable = "true";
    box.dataset.editing = "1";
    box.spellcheck = true;
    this.textEl = box;
    // WordArt shows its plain text while it is edited.
    a.classList.add("warp-off");
    box.focus({ preventScroll: true });
    let caret: MPos | null = null;
    if (at) {
      const range = caretRangeAt(at.x, at.y);
      if (range && box.contains(range.startContainer)) caret = domToModel(this.ed.root, range.startContainer, range.startOffset);
    }
    if (!caret) {
      const last = ids[ids.length - 1];
      caret = { p: last, o: paraLength(this.ed.root, last) };
    }
    this.ed.select({ anchor: caret, focus: caret });
    this.ed.emitSelection();
    this.show();
    return true;
  }

  /** Leave the text; the shape stays selected unless the caret has gone somewhere else. */
  private leaveText(selectShape: boolean) {
    editing.clear();
    const box = this.textEl;
    this.textEl = null;
    if (box) {
      box.contentEditable = "false";
      delete box.dataset.editing;
      box.parentElement?.classList.remove("warp-off");
    }
    if (selectShape && this.target) {
      const tg = this.target;
      this.ed.focusRoot();
      this.ed.select({ anchor: tg, focus: { p: tg.p, o: tg.o + 1 } });
      this.show();
    }
  }

  private onKey(e: KeyboardEvent) {
    if (!this.target || this.busy || e.isComposing) return;
    if (this.textEl) {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        this.leaveText(true);
      } else if ((e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "a") {
        // Select all in a text box is the text of the box, not the document.
        e.preventDefault();
        e.stopPropagation();
        window.getSelection()?.selectAllChildren(this.textEl);
      }
      return;
    }
    if (!this.atom() || (e.target as HTMLElement).closest("input,select,textarea")) return;
    const plain = !e.metaKey && !e.ctrlKey && !e.altKey;
    const s = this.subject();
    if (!s) return;
    const stop = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (e.key === "Enter" && plain) {
      stop();
      this.enterText();
    } else if (e.key === "Escape") {
      stop();
      const tg = this.target;
      if (this.member.length) {
        // Back out one step: from a member to the group.
        this.member = this.member.slice(0, -1);
        this.show();
      } else if (this.others.length) {
        this.others = [];
        this.show();
      } else {
        this.hide();
        this.ed.select({ p: tg.p, o: tg.o + 1 });
      }
    } else if ((e.key === "Backspace" || e.key === "Delete") && this.member.length) {
      // A member can't be deleted from its group on its own yet; the key must not take the whole group with it.
      stop();
    } else if (plain && e.key.startsWith("Arrow") && (this.member.length || s.wrap !== "inline")) {
      stop();
      const d = e.shiftKey ? 10 : 1;
      this.nudge(e.key === "ArrowLeft" ? -d : e.key === "ArrowRight" ? d : 0, e.key === "ArrowUp" ? -d : e.key === "ArrowDown" ? d : 0);
    } else if (plain && e.key.length === 1 && !this.ed.readOnly) {
      // Typing with a shape selected types in it (a shape without text keeps the character from replacing it).
      if (s.text) this.enterText();
      else stop();
    }
  }

  // -- inserting -------------------------------------------------------------------------------

  /** Where an inserted object goes: at the caret, or after the shape when the caret is in a shape's text. */
  private insertionPoint(): MPos | null {
    if (this.ed.story) {
      this.ed.hooks.onError?.(t("Shapes and text boxes can only go in the main text."));
      return null;
    }
    const r = this.ed.range();
    if (!r) return null;
    const host = this.ed.paraElement(r[0].p)?.closest<HTMLElement>(".atom.shape");
    if (host) {
      const para = host.closest<HTMLElement>("[data-p]");
      if (para) return { p: Number(para.dataset.p), o: Number(host.dataset.o) + 1 };
    }
    return r[0];
  }

  /** A text box, 2.5 by 1 inches, at the caret, with the caret in it. */
  insertTextBox() {
    if (this.ed.readOnly) return;
    const at = this.insertionPoint();
    if (!at) return;
    this.leaveSelection();
    const res = this.ed.exec({ op: "insert_text_box", at, w: 180, h: 72, text: "" });
    const caret = res?.[0]?.caret;
    if (!caret) return;
    const box = this.ed.paraElement(caret.p)?.closest<HTMLElement>(".atom.shape");
    const para = box?.parentElement?.closest<HTMLElement>("[data-p]");
    if (!box || !para) return;
    this.target = { p: Number(para.dataset.p), o: Number(box.dataset.o) };
    this.enterText();
  }

  /** A shape from the picker, 1.5 by 1 inches (a line 1.5 inches long), at the caret and selected. */
  insertShape(choice: ShapeChoice) {
    if (this.ed.readOnly) return;
    // A tool that draws the shape (curve, freeform, connectors) takes over from here.
    if (choice.tool && this.onTool) return this.onTool(choice.tool, !!choice.arrow);
    const at = this.insertionPoint();
    if (!at) return;
    this.leaveSelection();
    const op: Record<string, unknown> = { op: "insert_shape", at, geometry: choice.geometry, w: choice.w ?? 108, h: choice.h ?? 72 };
    if (choice.fill) {
      op.fill = SHAPE_FILL;
      op.line = { color: SHAPE_LINE, width_pt: 1 };
    } else op.fill = "none";
    if (choice.line) op.line = choice.line;
    if (!this.ed.exec(op as Op)) return;
    this.selectAt(at.p, at.o);
  }

  private leaveSelection() {
    if (this.textEl) this.leaveText(false);
    this.hide();
  }

  /** The picker: every shape Docxtor draws, with a small drawing of each, under its heading. */
  openPicker(anchor?: HTMLElement | { x: number; y: number }) {
    if (this.ed.readOnly) return;
    const body = h("div", { class: "shape-picker" });
    let close = () => {};
    for (const g of SHAPE_GROUPS) {
      const grid = h("div", { class: "shape-grid" });
      for (const s of g.shapes) {
        const name = t(s.name);
        const b = h("button", { type: "button", class: "shape-choice", title: name, "aria-label": name });
        b.append(s.tool ? toolPreview(s.tool, !!s.arrow) : preview(s.geometry, s.line));
        b.addEventListener("click", () => {
          close();
          this.ed.focus();
          this.insertShape(s);
        });
        grid.append(b);
      }
      body.append(h("div", { class: "menu-title" }, t(g.title)), grid);
    }
    const at = anchor ?? this.caretPoint();
    close = popover(at, body, { onClose: () => this.ed.focus() });
    body.querySelector<HTMLElement>(".shape-choice")?.focus();
  }

  private caretPoint(): { x: number; y: number } {
    const s = window.getSelection();
    const range = s?.rangeCount ? s.getRangeAt(0) : null;
    const r = range ? (range.getClientRects()[0] ?? range.getBoundingClientRect()) : null;
    if (r && (r.width || r.height)) return { x: r.left, y: r.bottom };
    const p = this.ed.sel ? this.ed.paraElement(this.ed.sel.focus.p) : null;
    const b = (p ?? this.ed.root).getBoundingClientRect();
    return { x: b.left + 8, y: Math.min(b.bottom, b.top + 24) };
  }
}
