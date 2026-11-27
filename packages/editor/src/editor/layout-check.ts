// @ts-nocheck
// The paginator's own check for tests and development (see `Paginator.verify`): not part of the production build.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { layoutUnit, layoutUnits, measureLayoutUnit } from "./layout";
type P = Record<string, any>;
function snapshot(pg: P, kids: HTMLElement[]): string[] {
  const out: string[] = [`pages ${pg.pages} bodyEnd ${pg.bodyEnd}`];
  pg.boxes.forEach((b, k) => out.push(`box ${k} ${b.top} ${b.h} ${b.mT} ${b.mB} ${b.num} ${b.section} ${b.first}`));
  // A page with no room is a hole in the list or a 0, depending on the way the room was found: the same.
  const reserve = Array.from(pg.reserve, (v) => v ?? 0);
  while (reserve.length && !reserve[reserve.length - 1]) reserve.pop();
  out.push(`reserve ${reserve.join(",")}`);
  pg.notesByPage.forEach((l, k) => out.push(`notes ${k} ${l?.map((x) => x.id).join(",")}`));
  pg.endByPage.forEach((a, k) => a && out.push(`endnotes ${k} ${a.top} ${a.h} ${a.notes.map((x) => x.id).join(",")}`));
  out.push(`chapters ${pg.chapterAt.map((c) => (c ? `${c.label}:${c.n}` : "-")).join(",")}`);
  const range = document.createRange();
  kids.forEach((el, i) => {
    // The gap a block carries for far blocks is not part of what the pass set.
    const own = el.hasAttribute("data-fg") ? parseFloat(el.getAttribute("data-fm") ?? "0") || 0 : parseFloat(el.style.marginTop) || 0;
    if (Math.abs(own - (parseFloat(el.dataset.base ?? "0") || 0)) > layoutUnit() / 2) out.push(`margin ${i} ${layoutUnits(own)}px`);
    for (const sp of Array.from(el.querySelectorAll<HTMLElement>(".pgsp, .hdr-repeat"))) {
      range.setStart(el, 0);
      range.setEndBefore(sp);
      // A spacer's height as the browser lays it out (whole layout units): two ways of working out one length can write it
      // with different last digits (307.334px and 307.333px in Firefox, where the unit is 1/60 px), which lay out the same.
      const css = sp.style.cssText.replace(/(^|;\s*)height:\s*([\d.]+)px/, (_m, pre: string, v: string) => `${pre}height: ${layoutUnits(parseFloat(v))}px`);
      out.push(`spacer ${i} ${sp.tagName} ${sp.className} ${range.toString().length} ${css}`);
    }
    for (const t of Array.from(el.querySelectorAll<HTMLElement>("table.ft[data-cut]"))) out.push(`cut ${i} ${t.style.getPropertyValue("mask-image")}`);
  });
  return out;
}

/** For tests and development (see `verify`): lay out again from the top and throw if that is not what the incremental layout made. */
export function checkAgainstFullPass(pg: P, pages: number, reserve: number[] | null) {
  return uncounted(pg, () => checkFull(pg, pages, reserve));
}

/** The checks draw and leave out everything themselves: what they do is not counted as the bookkeeping of far pages. */
function uncounted<T>(pg: P, fn: () => T): T {
  const v = pg.farVisits;
  const l = pg.farLong;
  try {
    return fn();
  } finally {
    pg.farVisits = v;
    pg.farLong = l;
  }
}

function checkFull(pg: P, pages: number, reserve: number[] | null) {
  const kids = pg.pageBlocks();
  const at = new Map<HTMLElement, number>();
  kids.forEach((k, i) => at.set(k, i));
  const mine = snapshot(pg, kids);
  const recs = kids.map((k) => {
    const r = pg.recs.get(k);
    return r ? { ...r } : null;
  });
  const firsts = pg.firstOnPage.map((el) => at.get(el) ?? -1);
  const plan = pg.lastPlan;
  const saved = JSON.stringify(pg.stats);
  pg.layoutBody(null, reserve, true);
  Object.assign(pg.stats, JSON.parse(saved));
  const full = snapshot(pg, pg.pageBlocks());
  const problems: string[] = [];
  const a = new Set(mine);
  const b = new Set(full);
  for (const l of mine) if (!b.has(l)) problems.push(`incremental only: ${l}`);
  for (const l of full) if (!a.has(l)) problems.push(`full only: ${l}`);
  // A position read from a rectangle: exact where the page is drawn as it is laid out. Where the editor is zoomed (a transform) Firefox
  // rounds the rectangle to its unit on the screen, in coordinates from the top of the window, so a position is only known to a unit
  // of the screen, which in layout pixels is 1/zoom of one, and the same block measured with the editor lower down the page (another
  // editor put above it) differs by that much. (At zoom 1 the rectangles are exact: 1,082 positions of an editor, none differed.)
  // Chrome and Safari keep floats under a transform, so they keep the strict figure.
  const scaleNow = pg.scale();
  const tolerance = scaleNow !== 1 && !measureLayoutUnit().trunc ? Math.max(1 / 128, layoutUnit() * (1 + 1 / scaleNow)) : 1 / 128;
  kids.forEach((k, i) => {
    const x = recs[i];
    const y = pg.recs.get(k);
    if (!x || !y) {
      if (x || y) problems.push(`page map: block ${i} has a record in only one`);
      return;
    }
    for (const f of ["entryPage", "force", "page"] as const) if (x[f] !== y[f]) problems.push(`page map: block ${i} ${f} ${x[f]} against ${y[f]}`);
    for (const f of ["acc", "cx", "rel"] as const) if (Math.abs(x[f] - y[f]) > tolerance) problems.push(`page map: block ${i} ${f} ${x[f]} against ${y[f]}`);
  });
  const firstsFull = pg.firstOnPage.map((el) => at.get(el) ?? -1);
  if (firsts.join() !== firstsFull.join()) problems.push(`page map: first block on each page ${firsts.slice(0, 40).join()} against ${firstsFull.slice(0, 40).join()}`);
  // The page each block starts on (what the page map answers) is the page its rectangle is in.
  const fr = pg.flow.getBoundingClientRect();
  const sc = pg.scale();
  kids.forEach((k, i) => {
    const y = pg.recs.get(k);
    if (!y || k.dataset.fold || k.closest("[data-tab-hidden]") || k.classList.contains("cols")) return;
    const rect = pg.pageAt(pg.topOf(k, fr, sc));
    if (rect !== y.page) problems.push(`page map: block ${i} starts on page ${y.page}, its rectangle is on page ${rect}`);
  });
  if (pg.pages !== pages) problems.push(`page count ${pages} against ${pg.pages}`);
  const hint = /(?:margin|spacer|block) (\d+)/.exec(problems.find((l) => /(?:margin|spacer|page map: block) \d+/.test(l)) ?? "");
  if (hint) {
    const at0 = Number(hint[1]);
    const all = pg.pageBlocks();
    for (let j = Math.max(0, at0 - 2); j <= Math.min(all.length - 1, at0 + 1); j++) {
      const el = all[j];
      const r = pg.recs.get(el);
      problems.unshift(`block ${j}: parent ${el.parentElement?.className} ${el.tagName}.${el.className} id=${el.dataset.id} kn=${el.dataset.kn} pbb=${el.dataset.pbb} pbr=${el.dataset.pbr} height ${pg.hOf(el)} margin ${el.style.marginTop} base ${el.dataset.base} rec ${r ? `${r.entryPage},${r.page},acc ${r.acc},cx ${r.cx},rel ${r.rel}` : "-"}`);
    }
  }
  if (problems.length) throw new Error(`Docxtor: a page layout differs from one made the long way (${problems.length} differences; the layout ran ${plan}):\n${problems.slice(0, 12).join("\n")}`);
}



/** For tests and development (see `Editor.updateFar`): the blocks that are drawn are where they would be with everything drawn. */
export function checkFar(pg: P, reapply: () => void) {
  return uncounted(pg, () => checkFarInner(pg, reapply));
}

function checkFarInner(pg: P, reapply: () => void) {
  const kids = pg.pageBlocks();
  const fr = pg.flow.getBoundingClientRect();
  const sc = pg.scale();
  const before = new Map<number, number>();
  kids.forEach((k, i) => {
    if (!k.hasAttribute("data-far") && k.getClientRects().length) before.set(i, pg.topOf(k, fr, sc));
  });
  pg.farShowAll();
  const fr2 = pg.flow.getBoundingClientRect();
  const bad: string[] = [];
  let worst = 0;
  for (const [i, v] of before) {
    const now = pg.topOf(kids[i], fr2, sc);
    worst = Math.max(worst, Math.abs(now - v));
    if (Math.abs(now - v) > 0.05) bad.push(`block ${i} is at ${v} with far pages left out, at ${now} with everything drawn`);
  }
  reapply();
  if (bad.length) throw new Error(`Docxtor: the drawn stretch is not where a full layout puts it (${bad.length} blocks, at most ${worst} px):\n${bad.slice(0, 8).join("\n")}`);
}
