// The pattern fills Word offers: 48 presets, each an 8 by 8 bitmap in a foreground and a background colour. The grid draws
// each preset as a small tile and marks the current one, with the same bitmaps the shapes on the page are drawn with
// (editor/shape-fill.ts); ECMA-376 names the presets but does not publish their pixels.
import { msg, t } from "@docxtor/extensions";
import { patternBit } from "../editor/shape-fill";
import { h } from "./dom";

/** The presets in Word's order, with the name Word shows for each. */
export const PATTERNS: [string, string][] = [
  ["pct5", msg("5%")], ["pct10", msg("10%")], ["pct20", msg("20%")], ["pct25", msg("25%")], ["pct30", msg("30%")], ["pct40", msg("40%")],
  ["pct50", msg("50%")], ["pct60", msg("60%")], ["pct70", msg("70%")], ["pct75", msg("75%")], ["pct80", msg("80%")], ["pct90", msg("90%")],
  ["ltDnDiag", msg("Light downward diagonal")], ["ltUpDiag", msg("Light upward diagonal")], ["dkDnDiag", msg("Dark downward diagonal")], ["dkUpDiag", msg("Dark upward diagonal")],
  ["wdDnDiag", msg("Wide downward diagonal")], ["wdUpDiag", msg("Wide upward diagonal")], ["ltVert", msg("Light vertical")], ["ltHorz", msg("Light horizontal")],
  ["narVert", msg("Narrow vertical")], ["narHorz", msg("Narrow horizontal")], ["dkVert", msg("Dark vertical")], ["dkHorz", msg("Dark horizontal")],
  ["dashDnDiag", msg("Dashed downward diagonal")], ["dashUpDiag", msg("Dashed upward diagonal")], ["dashHorz", msg("Dashed horizontal")], ["dashVert", msg("Dashed vertical")],
  ["smConfetti", msg("Small confetti")], ["lgConfetti", msg("Large confetti")], ["zigZag", msg("Zig zag")], ["wave", msg("Wave")],
  ["diagBrick", msg("Diagonal brick")], ["horzBrick", msg("Horizontal brick")], ["weave", msg("Weave")], ["plaid", msg("Plaid")],
  ["divot", msg("Divot")], ["dotGrid", msg("Dotted grid")], ["dotDmnd", msg("Dotted diamond")], ["shingle", msg("Shingle")],
  ["trellis", msg("Trellis")], ["sphere", msg("Sphere")], ["smGrid", msg("Small grid")], ["lgGrid", msg("Large grid")],
  ["smCheck", msg("Small checker board")], ["lgCheck", msg("Large checker board")], ["openDmnd", msg("Outlined diamond")], ["solidDmnd", msg("Solid diamond")],
];

/** The tile as 8 by 8 pixels in two colours, drawn on a canvas. */
export function drawPattern(canvas: HTMLCanvasElement, preset: string, fg: string, bg: string) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const img = ctx.createImageData(8, 8);
  const rgb = (hex: string) => [parseInt(hex.slice(0, 2), 16) || 0, parseInt(hex.slice(2, 4), 16) || 0, parseInt(hex.slice(4, 6), 16) || 0];
  const [f, b] = [rgb(fg), rgb(bg)];
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      const c = patternBit(preset, x, y) ? f : b;
      img.data.set([c[0], c[1], c[2], 255], (y * 8 + x) * 4);
    }
  }
  ctx.putImageData(img, 0, 0);
}

export interface PatternGrid {
  el: HTMLElement;
  /** Show the tiles in these colours and mark `preset` as the current one. */
  set(preset: string | undefined, fg: string, bg: string): void;
}

export function patternGrid(commit: (preset: string) => void): PatternGrid {
  const el = h("div", { class: "pat-grid", role: "radiogroup", "aria-label": t("Pattern") });
  const tiles = PATTERNS.map(([id, name]) => {
    const canvas = h("canvas", { width: 8, height: 8, "aria-hidden": "true" });
    const b = h("button", { type: "button", role: "radio", "aria-checked": "false", class: "pat-tile", title: t(name), "aria-label": t(name), "data-pattern": id }, canvas);
    b.addEventListener("click", () => commit(id));
    el.append(b);
    return { id, canvas, b };
  });
  el.addEventListener("keydown", (e) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    const at = tiles.findIndex((x) => x.b === document.activeElement);
    if (!step || at < 0) return;
    e.preventDefault();
    const next = tiles[Math.min(tiles.length - 1, Math.max(0, at + step))];
    next.b.focus();
    commit(next.id);
  });
  return {
    el,
    set(preset, fg, bg) {
      for (const tile of tiles) {
        drawPattern(tile.canvas, tile.id, fg, bg);
        const on = tile.id === preset;
        tile.b.classList.toggle("on", on);
        tile.b.setAttribute("aria-checked", String(on));
        // One stop in the Tab order: the current tile, else the first.
        tile.b.tabIndex = on || (!preset && tile.id === "pct5") ? 0 : -1;
      }
    },
  };
}
