// Tab stops, laid out as Word does. The browser has one tab size and no alignment, so each tab character gets its
// own width from the layout: after the lines are broken, every tab is measured (where it starts, how wide the text
// after it is) and given exactly the width that puts the next text on its stop.
//
// A paragraph with tabs carries its stops in `data-tl` (see `tabPlan`). `layoutTabs` then goes through its tabs in
// order. A tab that ends at `end` px from the start of the text area gets `tab-size: end`, because the browser's
// tab stops are the multiples of that size from the same edge, so the first one past the tab's start is `end`.
// Where that can't work (a tab that starts before the text area, as a hanging first line does, or one of no width)
// the tab is an inline box of the width it needs instead.
//
// Positions are measured from the start edge of the text area, which is the left edge in a left-to-right paragraph
// and the right edge in a right-to-left one. Stops are written in twips from the margin, so a stop is
// `pos - indent` from the text area.
import type { ParaFmt } from "@docxtor/core";

const px = (tw: number | undefined) => (tw ?? 0) / 15;

/** What a paragraph's tabs need, in CSS px: kind (l c r d), position from the margin, leader; the indents. */
export interface TabPlan {
  s: [string, number, string?][];
  /** Indent at the start edge, and the hanging part of it. */
  i: number;
  h: number;
  /** Indent at the end edge. */
  e: number;
}

const KIND: Record<string, string> = { left: "l", start: "l", num: "l", center: "c", right: "r", end: "r", decimal: "d" };

/** The stops that tabs jump to (bar stops draw a line and are not tab targets). */
export function tabPlan(fmt: ParaFmt, indStart: number, indEnd: number, hang: number): TabPlan {
  const s: TabPlan["s"] = [];
  for (const t of fmt.tabs ?? []) {
    const k = KIND[t.val];
    if (k) s.push([k, px(t.pos), t.leader]);
  }
  s.sort((a, b) => a[1] - b[1]);
  return { s, i: indStart, h: hang, e: indEnd };
}

/** Where a tab that starts at `x` (px from the margin) stops: the nearest custom stop, the hanging indent, or the default grid. */
export function nextStop(plan: TabPlan, x: number, defaultTab: number): { k: string; pos: number; lead?: string } {
  let best: { k: string; pos: number; lead?: string } | null = null;
  for (const [k, pos, lead] of plan.s) {
    if (pos > x + 0.5) {
      best = { k, pos, lead };
      break;
    }
  }
  // A hanging indent is a stop of its own, for a tab before it.
  if (plan.h > 0 && x < plan.i - 0.5 && (!best || plan.i < best.pos)) best = { k: "l", pos: plan.i };
  if (best) return best;
  const step = Math.max(defaultTab, 1);
  return { k: "l", pos: (Math.floor((x + 0.5) / step) + 1) * step };
}

/** The character that separates the whole part of a number from the decimals, as this device writes numbers. */
let decimalChar: string | undefined;
function decimal(): string {
  if (!decimalChar) {
    try {
      decimalChar = new Intl.NumberFormat().formatToParts(1.1).find((p) => p.type === "decimal")?.value ?? ".";
    } catch {
      decimalChar = ".";
    }
  }
  return decimalChar;
}

const isBreaker = (el: Element) => el.classList.contains("ftab") || el.classList.contains("fptab") || el.classList.contains("pbr") || el.classList.contains("cbr") || (el.classList.contains("s") && el.textContent === "\n");

/** The width of the first line of what is in `range`, in layout px. */
function lineWidth(range: Range, scale: number): number {
  let top = Infinity;
  let h = 0;
  const rects = Array.from(range.getClientRects()).filter((r) => r.width > 0 && r.height > 0);
  for (const r of rects) {
    if (r.top < top) {
      top = r.top;
      h = r.height;
    }
  }
  let from = Infinity;
  let to = -Infinity;
  for (const r of rects) {
    if (Math.abs(r.top - top) > h * 0.6) continue;
    from = Math.min(from, r.left);
    to = Math.max(to, r.right);
  }
  return to > from ? (to - from) / scale : 0;
}

/** The text between a tab and the next tab or break (or the end of the paragraph) as a range. */
function afterTab(p: HTMLElement, tab: HTMLElement): { range: Range; nodes: Text[] } {
  const range = document.createRange();
  range.setStartAfter(tab);
  let next: Element | null = tab.nextElementSibling;
  while (next && !isBreaker(next)) next = next.nextElementSibling;
  if (next) range.setEndBefore(next);
  else range.setEnd(p, p.childNodes.length);
  const nodes: Text[] = [];
  let n: Element | null = tab.nextElementSibling;
  while (n && n !== next) {
    const walker = document.createTreeWalker(n, NodeFilter.SHOW_TEXT);
    for (let t = walker.nextNode(); t; t = walker.nextNode()) nodes.push(t as Text);
    n = n.nextElementSibling;
  }
  return { range, nodes };
}

/** Width of the text after a tab up to its decimal point (the whole width when there is none). */
function decimalOffset(range: Range, nodes: Text[], scale: number): number {
  for (const sep of [decimal(), decimal() === "." ? "," : "."]) {
    for (const t of nodes) {
      const at = t.data.indexOf(sep);
      if (at < 0) continue;
      const r = document.createRange();
      r.setStart(range.startContainer, range.startOffset);
      r.setEnd(t, at);
      return lineWidth(r, scale);
    }
  }
  return lineWidth(range, scale);
}

interface Geometry {
  rtl: boolean;
  /** Client x of the start edge of the text area. */
  edge: number;
  scale: number;
}

const dist = (g: Geometry, clientX: number) => (g.rtl ? g.edge - clientX : clientX - g.edge) / g.scale;

/** Make one tab exactly `end - start` px wide, ending at `end` px from the text area's start edge. */
function shape(tab: HTMLElement, start: number, end: number, leader: string | undefined) {
  const width = end - start;
  const st = tab.style;
  if (width > 0.5 && (start >= 0 || Math.abs(end) < 0.5)) {
    st.display = "";
    st.width = "";
    st.tabSize = `${Math.abs(end) < 0.5 ? 100000 : end}px`;
  } else {
    // An inline box of the width: the tab stays one character, but no browser tab stop sits where it should end.
    st.tabSize = "";
    st.display = "inline-block";
    st.width = `${Math.max(width, 0)}px`;
  }
  tab.dataset.tw = String(Math.round(Math.max(width, 0) * 100) / 100);
  if (leader && leader !== "none") {
    tab.dataset.leader = leader;
    tab.classList.add("tab-fill");
  } else {
    delete tab.dataset.leader;
    tab.classList.remove("tab-fill");
  }
}

/** The ptab spec of a tab element: `alignment`, `relativeTo`, `leader`. */
function ptabSpec(tab: HTMLElement): { align: string; rel: string; leader: string } {
  const [align = "left", rel = "margin", leader = "none"] = (tab.dataset.ptab ?? "").split(":");
  return { align, rel, leader };
}

function layoutParagraph(p: HTMLElement, defaultTab: number) {
  let plan: TabPlan;
  try {
    plan = JSON.parse(p.dataset.tl ?? "");
  } catch {
    return;
  }
  const cs = getComputedStyle(p);
  const rect = p.getBoundingClientRect();
  const scale = p.offsetWidth ? rect.width / p.offsetWidth : 1;
  const rtl = cs.direction === "rtl";
  const pad = (side: "Left" | "Right") => parseFloat(cs.getPropertyValue(`padding-${side.toLowerCase()}`)) + parseFloat(cs.getPropertyValue(`border-${side.toLowerCase()}-width`));
  const startPad = rtl ? pad("Right") : pad("Left");
  const endPad = rtl ? pad("Left") : pad("Right");
  const textW = p.offsetWidth - startPad - endPad;
  // Skip the work when neither the text nor the room for it changed since the last time.
  const sig = `${Math.round(textW * 10)}|${defaultTab}|${p.textContent}`;
  if (p.dataset.tls === sig) return;
  const g: Geometry = { rtl, edge: rtl ? rect.right - startPad * scale : rect.left + startPad * scale, scale };
  const ind = plan.i;
  const dtPx = px(defaultTab);

  // A list number followed by a tab goes to the next stop, which is the hanging indent unless the number is wider.
  const lbl = p.querySelector<HTMLElement>(":scope > .lbl[data-sfx='tab']");
  if (lbl && Number(lbl.dataset.est ?? 0) > plan.h * 0.8) {
    lbl.style.minWidth = "0px";
    const r = lbl.getBoundingClientRect();
    const from = dist(g, rtl ? r.right : r.left);
    const stop = nextStop(plan, from + r.width / scale + ind, dtPx);
    lbl.style.minWidth = `${Math.max(0, stop.pos - ind - from)}px`;
  }

  const tabs = Array.from(p.querySelectorAll<HTMLElement>(":scope > .ftab, :scope > .fptab"));
  for (const tab of tabs) {
    const r = tab.getClientRects()[0];
    if (!r) continue;
    const start = dist(g, rtl ? r.right : r.left);
    const x = start + ind;
    let stop: { k: string; pos: number; lead?: string };
    if (tab.classList.contains("fptab")) {
      // An absolute position tab: the left, centre or right of the margins (or of the indents).
      const { align, rel, leader } = ptabSpec(tab);
      const full = textW + ind + plan.e;
      const [from, width] = rel === "indent" ? [ind, textW] : [0, full];
      const pos = align === "center" ? from + width / 2 : align === "right" ? from + width : from;
      stop = { k: align === "center" ? "c" : align === "right" ? "r" : "l", pos, lead: leader };
    } else stop = nextStop(plan, x, dtPx);
    let end = stop.pos - ind;
    if (stop.k !== "l") {
      const { range, nodes } = afterTab(p, tab);
      const w = stop.k === "d" ? decimalOffset(range, nodes, scale) : lineWidth(range, scale);
      end -= stop.k === "c" ? w / 2 : w;
    }
    shape(tab, start, end, stop.lead);
  }
  p.dataset.tls = sig;
}

/** Give every tab under `root` (or `root` itself) the width its stop asks for. `defaultTab` is the document's, in twips. */
export function layoutTabs(root: HTMLElement, defaultTab: number) {
  const list = root.matches(".fp[data-tl]") ? [root] : Array.from(root.querySelectorAll<HTMLElement>(".fp[data-tl]"));
  for (const p of list) {
    try {
      layoutParagraph(p, defaultTab);
    } catch (e) {
      console.error("Docxtor: tab stops could not be laid out", e);
    }
  }
}
