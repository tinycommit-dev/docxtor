// A section's document grid (`w:docGrid`): lines sit on a grid of a fixed pitch, and with a character grid East Asian
// characters take a fixed width. The paragraphs are drawn without the grid; before each layout the ones in a section
// that has one get their line height rounded up to whole grid lines and, with a character grid, the letter spacing that
// makes a character as wide as the grid says. A paragraph that turned grid snapping off (`w:snapToGrid`), or has an
// exact line spacing, keeps its own spacing.
import type { DocGrid, SectionInfo } from "@docxtor/core";

const PT_PX = 96 / 72;
const TWIP_PX = 1 / 15;

/** The width of one character in px for a grid: the base font size plus what the grid adds (4096ths of a point). */
export function charPitch(grid: NonNullable<SectionInfo["docGrid"]>, base: number | undefined): number | undefined {
  if (grid.type !== "linesAndChars" && grid.type !== "snapToChars") return undefined;
  return ((base ?? 10.5) + (grid.charSpace ?? 0) / 4096) * PT_PX;
}

/** The characters per line and lines per page a grid gives a text area (points). */
export function gridCounts(grid: DocGrid | undefined, base: number, width: number, height: number): { chars: number; lines: number } {
  const pitch = base + (grid?.charSpace ?? 0) / 4096;
  const lineTw = grid?.linePitch ?? 360;
  return { chars: Math.max(1, Math.round(width / Math.max(1, pitch))), lines: Math.max(1, Math.round(height / (lineTw / 20))) };
}

/** The grid for a number of characters per line and lines per page in a text area (points). */
export function gridFor(type: DocGrid["type"], base: number, width: number, height: number, chars: number, lines: number): DocGrid {
  if (type === "default") return { type };
  const grid: DocGrid = { type, linePitch: Math.max(1, Math.min(31680, Math.round((height / Math.max(1, lines)) * 20))) };
  if (type !== "lines") grid.charSpace = Math.round((width / Math.max(1, chars) - base) * 4096);
  return grid;
}

function paragraphs(block: HTMLElement): HTMLElement[] {
  return block.classList.contains("fp") ? [block] : Array.from(block.querySelectorAll<HTMLElement>(".fp"));
}

/** Put the grid on the paragraphs of the blocks (`infos[i]` is the section of `blocks[i]`), and take it off the others. Returns whether any block has a grid. */
export function applyDocGrid(blocks: HTMLElement[], infos: SectionInfo[]): boolean {
  let any = false;
  blocks.forEach((block, i) => {
    const grid = infos[i]?.docGrid;
    const pitch = grid?.linePitch ? grid.linePitch * TWIP_PX : 0;
    for (const p of paragraphs(block)) {
      const on = !!grid && grid.type !== "default" && p.dataset.sg !== "0" && p.dataset.lx !== "1";
      if (on && pitch > 0) {
        any = true;
        if (p.dataset.lh0 === undefined) p.dataset.lh0 = p.style.lineHeight;
        const own = p.dataset.lh0;
        const font = parseFloat(getComputedStyle(p).fontSize) || 14;
        const natural = /^[\d.]+$/.test(own) ? parseFloat(own) * font : parseFloat(own) || font * 1.2;
        p.style.lineHeight = `${(pitch * Math.max(1, Math.ceil(natural / pitch - 0.01))).toFixed(2)}px`;
        const w = charPitch(grid!, infos[i].gridBase);
        if (w) p.style.setProperty("--ea-ls", `calc(${w.toFixed(2)}px - 1em)`);
        else p.style.removeProperty("--ea-ls");
        p.dataset.gr = "1";
      } else if (p.dataset.gr) {
        p.style.lineHeight = p.dataset.lh0 ?? "";
        p.style.removeProperty("--ea-ls");
        delete p.dataset.gr;
        delete p.dataset.lh0;
      }
    }
  });
  return any;
}
