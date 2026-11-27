// The stop bar of a gradient fill: a strip showing the gradient with one handle per stop. Drag a handle to move its stop, press
// the arrow keys on it to nudge it, press Delete to remove it, click the strip to add a stop. Moves are reported once, when
// the drag ends; the strip itself follows the pointer while it is down.
import { t } from "@docxtor/extensions";
import type { RStop } from "@docxtor/core";
import { h } from "./dom";
import { hexOr } from "../editor/color.ts";

/** A stop's colour with its transparency, as CSS. */
export function stopCss(s: RStop): string {
  const a = s.alpha ?? 1;
  const css = hexOr(s.color);
  if (a >= 1) return css;
  const n = parseInt(css.slice(1), 16);
  return `rgb(${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255} / ${a})`;
}

/** The CSS of a horizontal strip showing the stops. */
export function stripCss(stops: RStop[]): string {
  const list = stops.length === 1 ? [stops[0], stops[0]] : stops;
  return `linear-gradient(90deg, ${list.map((s) => `${stopCss(s)} ${s.pos}%`).join(", ")})`;
}

/** The colour a new stop at `pos` starts with: what the gradient is there. */
export function colorAt(stops: RStop[], pos: number): RStop {
  const sorted = [...stops].sort((a, b) => a.pos - b.pos);
  if (!sorted.length) return { pos, color: "FFFFFF" };
  const after = sorted.find((s) => s.pos >= pos) ?? sorted[sorted.length - 1];
  const before = [...sorted].reverse().find((s) => s.pos <= pos) ?? sorted[0];
  const span = after.pos - before.pos;
  const k = span <= 0 ? 0 : (pos - before.pos) / span;
  const mix = (a: number, b: number) => Math.round(a + (b - a) * k);
  const chan = (hex: string, i: number) => parseInt(hex.slice(i, i + 2), 16);
  const hex = [0, 2, 4].map((i) => mix(chan(before.color, i), chan(after.color, i)).toString(16).padStart(2, "0")).join("").toUpperCase();
  const alpha = (before.alpha ?? 1) + ((after.alpha ?? 1) - (before.alpha ?? 1)) * k;
  return { pos, color: hex, ...(alpha < 1 ? { alpha: Math.round(alpha * 1000) / 1000 } : {}) };
}

export interface GradientBar {
  el: HTMLElement;
  /** Show these stops (sorted by position) with the stop at `selected` marked. */
  set(stops: RStop[], selected: number): void;
}

export function gradientBar(o: {
  onSelect(i: number): void;
  /** A stop was moved; `stops` are sorted again and `index` is where the moved stop is now. */
  onMove(stops: RStop[], index: number): void;
  onAdd(pos: number): void;
  onRemove(i: number): void;
}): GradientBar {
  let stops: RStop[] = [];
  let sel = 0;
  const strip = h("div", { class: "gr-strip" });
  const handles = h("div", { class: "gr-handles" });
  const el = h("div", { class: "gr-bar", role: "group", "aria-label": t("Gradient stops") }, strip, handles);

  const posAt = (clientX: number) => {
    const r = strip.getBoundingClientRect();
    return Math.round(Math.min(100, Math.max(0, ((clientX - r.left) / r.width) * 100)));
  };

  const label = (i: number) => t("Stop {n} of {count}, at {pos} percent", { n: i + 1, count: stops.length, pos: stops[i].pos });

  const build = (i: number) => {
    const b = h("button", { type: "button", class: "gr-stop" }, h("span", {}));
    b.addEventListener("click", () => o.onSelect(i));
    b.addEventListener("keydown", (e) => {
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        o.onRemove(i);
      } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        const d = (e.key === "ArrowLeft" ? -1 : 1) * (e.shiftKey ? 10 : 1);
        commit(stops.map((x, j) => (j === i ? { ...x, pos: Math.min(100, Math.max(0, x.pos + d)) } : x)), i);
      }
    });
    b.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      e.stopPropagation();
      b.focus();
      o.onSelect(i);
      drag(i);
    });
    return b;
  };

  // The handles are kept while the number of stops stays the same, so a handle under the pointer is not replaced mid-drag.
  const paint = () => {
    strip.style.background = stops.length ? stripCss(stops) : "";
    const had = handles.contains(document.activeElement);
    if (handles.children.length !== stops.length) {
      handles.replaceChildren(...stops.map((_, i) => build(i)));
      if (had) (handles.children[sel] as HTMLElement | undefined)?.focus();
    }
    stops.forEach((s, i) => {
      const b = handles.children[i] as HTMLElement;
      b.style.left = `${s.pos}%`;
      b.classList.toggle("on", i === sel);
      b.setAttribute("aria-pressed", String(i === sel));
      b.setAttribute("aria-label", label(i));
      (b.firstElementChild as HTMLElement).style.background = stopCss({ ...s, alpha: 1 });
    });
  };

  const commit = (next: RStop[], i: number) => {
    const moved = next[i];
    const sorted = [...next].sort((a, b) => a.pos - b.pos);
    o.onMove(sorted, sorted.indexOf(moved));
  };

  const drag = (i: number) => {
    let pos = stops[i].pos;
    let moved = false;
    const onMove = (ev: PointerEvent) => {
      const p = posAt(ev.clientX);
      if (p === pos) return;
      pos = p;
      moved = true;
      (handles.children[i] as HTMLElement).style.left = `${p}%`;
      strip.style.background = stripCss(stops.map((s, j) => (j === i ? { ...s, pos: p } : s)).sort((a, b) => a.pos - b.pos));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      if (moved) commit(stops.map((s, j) => (j === i ? { ...s, pos } : s)), i);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  };

  strip.addEventListener("click", (e) => o.onAdd(posAt(e.clientX)));

  return {
    el,
    set(next, selected) {
      stops = next;
      sel = Math.min(Math.max(0, selected), Math.max(0, next.length - 1));
      paint();
    },
  };
}
