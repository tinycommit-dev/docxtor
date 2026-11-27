// Horizontal ruler with margins, draggable indent markers for the current paragraph, and its tab stops: click the
// ruler to add one, drag one to move it, drag it off the ruler to remove it, double-click one for the Tabs dialog.
// The button at the left end picks the kind of stop a click adds (left, centre, right, decimal, bar).
import { t } from "@docxtor/extensions";
import type { FormatState, TabStop } from "@docxtor/core";
import { COLUMNS } from "./layout";
import { onUnitChange, twipsPerUnit, unitSystem } from "../units";
import { tw } from "./render";

// The unit of the rulers and of the dialogs that measure against them. Live bindings: they follow the unit setting (units.ts).
export let METRIC = unitSystem() === "cm";
export let UNIT_TW = twipsPerUnit(); // twips per cm / inch
onUnitChange(() => {
  METRIC = unitSystem() === "cm";
  UNIT_TW = twipsPerUnit();
});
export const NS = "http://www.w3.org/2000/svg";

const TRI_DOWN = `<svg viewBox="0 0 11 8"><path d="M0.5 0.5h10L5.5 7.5z" fill="currentColor"/></svg>`;
const TRI_UP = `<svg viewBox="0 0 11 8"><path d="M0.5 7.5h10L5.5 0.5z" fill="currentColor"/></svg>`;

/** The marks of the tab stops on the ruler, by the kind of stop (10 x 10, drawn with the ruler's ink). */
const TAB_MARKS: Record<string, string> = {
  left: `<svg viewBox="0 0 10 10"><path d="M3 1v7.5h6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
  center: `<svg viewBox="0 0 10 10"><path d="M5 1v7.5M1 8.5h8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>`,
  right: `<svg viewBox="0 0 10 10"><path d="M7 1v7.5H1" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
  decimal: `<svg viewBox="0 0 10 10"><path d="M4 1v7.5M0.8 8.5h6.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><circle cx="8.6" cy="5.4" r="1.1" fill="currentColor"/></svg>`,
  bar: `<svg viewBox="0 0 10 10"><path d="M5 0.5v9" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>`,
};

/** The kinds of stop the selector cycles through, with the name its tooltip gives. */
const KINDS: [string, string][] = [
  ["left", "Left tab"],
  ["center", "Center tab"],
  ["right", "Right tab"],
  ["decimal", "Decimal tab"],
  ["bar", "Bar tab"],
];

/** How the ruler reaches the document: indents and tab stops are written through these. */
export interface RulerActions {
  /** Write the tab stops the paragraphs should have (all of them, inherited ones included). */
  tabs?: (stops: TabStop[]) => void;
  /** Open the Tabs dialog. */
  openTabs?: () => void;
}

/** What the ruler needs of a section (or of the whole page setup): its width and margins, in twips. */
export interface RulerPage {
  width: number;
  /** top, right, bottom, left */
  margin: number[];
  /** Columns of the section: how many and the gap between them (twips). A page setup without sections has only the count. */
  cols?: { num: number; space: number } | number;
}

export class Ruler {
  el: HTMLElement;
  private track: HTMLElement;
  private mLeft: HTMLElement;
  private mRight: HTMLElement;
  private first: HTMLElement;
  private left: HTMLElement;
  private right: HTMLElement;
  private kindButton: HTMLButtonElement;
  private stops: HTMLElement[] = [];
  /** The kind of stop a click on the ruler adds. */
  private kind = 0;
  /** Width and margins (twips) of the section the caret is in. */
  private page: RulerPage | null = null;
  private drawn = "";
  /** What the markers and tab stops were last drawn from. */
  private placed = "";
  private fmt: FormatState | null = null;
  private dragging = false;

  constructor(
    private onIndent: (patch: { indLeft?: number; indRight?: number; indFirst?: number }) => void,
    private zoom: () => number,
    private actions: RulerActions = {},
  ) {
    this.track = document.createElement("div");
    this.track.className = "ruler-track";
    this.mLeft = this.div("ruler-margin");
    this.mRight = this.div("ruler-margin");
    this.track.append(this.mLeft, this.mRight);
    this.first = this.marker("first", TRI_DOWN, t("First-line indent"));
    this.left = this.marker("left", TRI_UP, t("Left indent"));
    this.right = this.marker("right", TRI_UP, t("Right indent"));
    this.kindButton = document.createElement("button");
    this.kindButton.type = "button";
    this.kindButton.className = "ruler-kind";
    // The ruler is for the pointer (the Paragraph and Tabs dialogs do the same from the keyboard) and hidden from screen readers: nothing in it takes focus.
    this.kindButton.tabIndex = -1;
    this.kindButton.addEventListener("click", () => {
      this.kind = (this.kind + 1) % KINDS.length;
      this.paintKind();
    });
    this.paintKind();
    this.el = document.createElement("div");
    this.el.className = "ruler";
    this.el.setAttribute("aria-hidden", "true");
    this.el.append(this.track, this.first, this.left, this.right, this.kindButton);
    this.track.addEventListener("pointerdown", (e) => this.addStop(e));
  }

  private div(cls: string) {
    const d = document.createElement("div");
    d.className = cls;
    return d;
  }

  private marker(kind: "first" | "left" | "right", svg: string, title: string) {
    const m = this.div(`ruler-marker ${kind}`);
    m.innerHTML = svg;
    m.title = title;
    m.addEventListener("pointerdown", (e) => this.drag(e, kind));
    return m;
  }

  private paintKind() {
    const [id, name] = KINDS[this.kind];
    this.kindButton.innerHTML = TAB_MARKS[id];
    this.kindButton.dataset.kind = id;
    this.kindButton.title = t("{kind}. Click to change it, then click the ruler to set one.", { kind: t(name) });
    this.kindButton.setAttribute("aria-label", this.kindButton.title);
  }

  /** Show the width and margins of the section the caret is in (the ruler follows the caret from section to section). */
  /** Called when the ruler's size changed (a page of another width), so the box that holds it can follow at once. */
  onSize: (() => void) | null = null;

  setPage(p: RulerPage) {
    this.page = p;
    const cols = typeof p.cols === "object" ? p.cols : { num: p.cols ?? 1, space: 720 };
    const n = COLUMNS ? Math.max(1, cols.num) : 1;
    const key = `${p.width}:${p.margin[1]}:${p.margin[3]}:${n}:${cols.space}`;
    if (key === this.drawn) return;
    this.drawn = key;
    const w = tw(p.width);
    this.el.style.width = `${w}px`;
    this.onSize?.();
    this.mLeft.style.cssText = `left:0;width:${tw(p.margin[3])}px`;
    this.mRight.style.cssText = `right:0;width:${tw(p.margin[1])}px`;
    this.track.querySelector("svg")?.remove();
    // The gaps between columns are shaded like the margins: the text does not go there.
    this.track.querySelectorAll(".ruler-gap").forEach((g) => g.remove());
    if (n > 1) {
      const gap = tw(cols.space);
      const colW = (w - tw(p.margin[3]) - tw(p.margin[1]) - (n - 1) * gap) / n;
      for (let c = 0; c < n - 1; c++) {
        const g = this.div("ruler-gap");
        g.style.cssText = `left:${tw(p.margin[3]) + (c + 1) * colW + c * gap}px;width:${gap}px`;
        this.track.append(g);
      }
    }
    const svg = document.createElementNS(NS, "svg");
    svg.setAttribute("viewBox", `0 0 ${w} 16`);
    svg.setAttribute("preserveAspectRatio", "none");
    const origin = tw(p.margin[3]);
    const unitPx = tw(UNIT_TW);
    const step = unitPx / (METRIC ? 4 : 8);
    const ticks: string[] = [];
    const labels: SVGTextElement[] = [];
    const sub = METRIC ? 4 : 8;
    for (let dir = -1; dir <= 1; dir += 2) {
      for (let i = dir === 1 ? 0 : 1; ; i++) {
        const x = origin + dir * i * step;
        if (x < 0 || x > w) break;
        const major = i % sub === 0;
        const half = i % (sub / 2) === 0;
        if (major && i !== 0) {
          const t = document.createElementNS(NS, "text");
          t.setAttribute("x", String(x));
          t.setAttribute("y", "11.5");
          t.setAttribute("text-anchor", "middle");
          t.setAttribute("font-size", "8.5");
          t.setAttribute("font-family", "Inter, sans-serif");
          t.setAttribute("fill", "currentColor");
          t.textContent = String(i / sub);
          labels.push(t);
        } else if (!major) {
          const len = half ? 5 : 3;
          ticks.push(`M${x.toFixed(2)} ${16 - len}V16`);
        }
      }
    }
    const path = document.createElementNS(NS, "path");
    path.setAttribute("d", ticks.join(""));
    path.setAttribute("stroke", "currentColor");
    path.setAttribute("stroke-width", "1");
    path.setAttribute("vector-effect", "non-scaling-stroke");
    svg.append(path, ...labels);
    this.track.append(svg);
    this.placed = "";
    this.place();
  }

  setFormat(f: FormatState | null) {
    if (this.dragging) return;
    this.fmt = f;
    this.place();
  }

  /** Where a position in twips from the start margin falls on the ruler (px from its left edge). */
  private xOf(twips: number, rtl: boolean): number {
    const p = this.page!;
    return rtl ? tw(p.width) - tw(p.margin[1]) - tw(twips) : tw(p.margin[3]) + tw(twips);
  }

  /** The tab stops that tabs go to, without the cleared ones and a list level's number stop (it belongs to the level, not the paragraph). */
  private currentStops(): TabStop[] {
    return (this.fmt?.para.tabs ?? []).filter((s) => s.val !== "clear" && s.val !== "num").map((s) => ({ ...s }));
  }

  private place() {
    const p = this.page;
    if (!p) return;
    const para = this.fmt?.para;
    // In a right-to-left paragraph the indents and tab stops count from the right margin.
    const rtl = !!para?.bidi;
    const start = this.xOf(para?.indLeft ?? 0, rtl);
    const first = start + (rtl ? -1 : 1) * (para?.indHanging ? -tw(para.indHanging) : tw(para?.indFirstLine));
    const end = rtl ? tw(p.margin[3]) + tw(para?.indRight) : tw(p.width) - tw(p.margin[1]) - tw(para?.indRight);
    const show = !!this.fmt && !this.fmt.inTable;
    // The same marks and stops as drawn: leave them (every keystroke comes here, and a write makes the browser lay the page out again).
    const stops = show ? this.currentStops() : [];
    const sig = `${first}|${start}|${end}|${show}|${rtl}|${stops.map((x) => `${x.val}@${x.pos}`).join(",")}`;
    if (sig === this.placed) return;
    this.placed = sig;
    for (const [m, x] of [
      [this.first, first],
      [this.left, start],
      [this.right, end],
    ] as [HTMLElement, number][]) {
      m.style.left = `${x}px`;
      m.style.display = show ? "" : "none";
    }
    for (const s of this.stops) s.remove();
    this.stops = [];
    this.kindButton.style.display = show ? "" : "none";
    if (!show) return;
    for (const stop of stops) {
      const m = this.div(`ruler-tab ${stop.val}`);
      m.innerHTML = TAB_MARKS[stop.val] ?? TAB_MARKS.left;
      m.style.left = `${this.xOf(stop.pos, rtl)}px`;
      m.dataset.pos = String(stop.pos);
      m.title = t("Tab stop. Drag to move it, drag it off the ruler to remove it, double-click for more.");
      m.addEventListener("pointerdown", (e) => this.dragStop(e, stop));
      m.addEventListener("dblclick", () => this.actions.openTabs?.());
      this.stops.push(m);
      this.el.append(m);
    }
  }

  /** The ruler's x for a pointer, in px from its left edge (unzoomed), snapped to the ruler's grid. */
  private snapX(clientX: number): number {
    const r = this.el.getBoundingClientRect();
    const snap = tw(UNIT_TW) / (METRIC ? 4 : 8);
    return Math.round((clientX - r.left) / this.zoom() / snap) * snap;
  }

  /** Twips from the start margin for an x on the ruler, or null outside the text area. */
  private posOf(x: number, rtl: boolean): number | null {
    const p = this.page!;
    const from = tw(p.margin[3]);
    const to = tw(p.width) - tw(p.margin[1]);
    if (x < from - 0.5 || x > to + 0.5) return null;
    return Math.round((rtl ? to - x : x - from) * 15);
  }

  private addStop(e: PointerEvent) {
    if (!this.page || !this.fmt || this.fmt.inTable || !this.actions.tabs) return;
    const rtl = !!this.fmt.para.bidi;
    const pos = this.posOf(this.snapX(e.clientX), rtl);
    // A click is a stop only in the text area, and not at the very start of it.
    if (pos === null || pos <= 0) return;
    e.preventDefault();
    const stop: TabStop = { val: KINDS[this.kind][0], pos };
    const stops = this.currentStops().filter((s) => s.pos !== pos);
    this.actions.tabs([...stops, stop].sort((a, b) => a.pos - b.pos));
  }

  private dragStop(e: PointerEvent, stop: TabStop) {
    if (!this.page || !this.fmt || !this.actions.tabs) return;
    e.preventDefault();
    const m = e.currentTarget as HTMLElement;
    m.setPointerCapture(e.pointerId);
    m.classList.add("drag");
    this.dragging = true;
    const rtl = !!this.fmt.para.bidi;
    const guide = this.div("ruler-guide");
    this.el.append(guide);
    const startX = e.clientX;
    let pos = stop.pos;
    let off = false;
    let moved = false;
    const move = (ev: PointerEvent) => {
      const r = this.el.getBoundingClientRect();
      // Dragging well away from the ruler, up or down, takes the stop off.
      off = ev.clientY < r.top - 20 || ev.clientY > r.bottom + 28;
      m.classList.toggle("off", off);
      const next = this.posOf(this.snapX(ev.clientX), rtl);
      if (Math.abs(ev.clientX - startX) > 2) moved = true;
      if (next !== null && next > 0) pos = next;
      m.style.left = `${this.xOf(pos, rtl)}px`;
      guide.style.left = `${this.xOf(pos, rtl)}px`;
    };
    const up = () => {
      m.removeEventListener("pointermove", move);
      m.removeEventListener("pointerup", up);
      m.classList.remove("drag", "off");
      guide.remove();
      this.dragging = false;
      this.placed = "";
      const others = this.currentStops().filter((s) => s.pos !== stop.pos && s.pos !== pos);
      if (off) this.actions.tabs?.(others);
      else if (moved && pos !== stop.pos) this.actions.tabs?.([...others, { ...stop, pos }].sort((a, b) => a.pos - b.pos));
      // Not moved: it stays as it is, so a double-click still finds it.
      else m.style.left = `${this.xOf(stop.pos, rtl)}px`;
    };
    m.addEventListener("pointermove", move);
    m.addEventListener("pointerup", up);
  }

  private drag(e: PointerEvent, kind: "first" | "left" | "right") {
    if (!this.page || !this.fmt) return;
    e.preventDefault();
    const m = e.currentTarget as HTMLElement;
    m.setPointerCapture(e.pointerId);
    m.classList.add("drag");
    this.dragging = true;
    const guide = this.div("ruler-guide");
    this.el.append(guide);
    const z = this.zoom();
    const startX = e.clientX;
    const startLeft = parseFloat(m.style.left);
    const snap = tw(UNIT_TW) / (METRIC ? 4 : 8);
    const rtl = !!this.fmt.para.bidi;
    const marginL = tw(this.page.margin[3]);
    const marginR = tw(this.page.margin[1]);
    const width = tw(this.page.width);
    let x = startLeft;
    const move = (ev: PointerEvent) => {
      x = Math.round((startLeft + (ev.clientX - startX) / z) / snap) * snap;
      x = Math.max(0, Math.min(width, x));
      m.style.left = `${x}px`;
      guide.style.left = `${x}px`;
    };
    const up = () => {
      m.removeEventListener("pointermove", move);
      m.removeEventListener("pointerup", up);
      m.classList.remove("drag");
      guide.remove();
      this.dragging = false;
      this.placed = "";
      if (Math.abs(x - startLeft) < 0.5) return;
      const toTw = (px: number) => Math.round(px * 15);
      const para = this.fmt!.para;
      // The indents are at the start and end of the line: the left and right in a left-to-right paragraph.
      const startEdge = rtl ? width - marginR : marginL;
      const along = (px: number) => (rtl ? startEdge - px : px - startEdge);
      const curStart = tw(para.indLeft);
      if (kind === "left") {
        // Moving the left indent keeps the first line's offset (like Word's lower triangle).
        this.onIndent({ indLeft: Math.max(0, toTw(along(x))) });
      } else if (kind === "first") {
        this.onIndent({ indFirst: toTw(along(x) - curStart) });
      } else {
        const endEdge = rtl ? marginL : width - marginR;
        this.onIndent({ indRight: Math.max(0, toTw(rtl ? x - endEdge : endEdge - x)) });
      }
    };
    m.addEventListener("pointermove", move);
    m.addEventListener("pointerup", up);
  }
}
