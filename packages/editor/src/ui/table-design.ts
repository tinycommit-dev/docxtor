// The table design gallery: Word's built-in table styles drawn small, with the style options that switch parts of a
// design on or off. The engine draws the previews (`Engine.tableDesigns`), so a tile shows exactly what the design
// does to a table with these options, in this document's colours.
import { t, type TableDesignContribution } from "@docxtor/extensions";
import type { RTableLook, TableDesign } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { h, popover } from "./dom";
import { hex, hexOr } from "../editor/color.ts";

/** The style options, in the order Word lists them. */
export const LOOK_OPTIONS: { key: keyof RTableLook; title: () => string }[] = [
  { key: "firstRow", title: () => t("Header row") },
  { key: "lastRow", title: () => t("Total row") },
  { key: "bandedRows", title: () => t("Banded rows") },
  { key: "firstColumn", title: () => t("First column") },
  { key: "lastColumn", title: () => t("Last column") },
  { key: "bandedColumns", title: () => t("Banded columns") },
];

const GROUPS: { id: TableDesign["group"]; title: () => string }[] = [
  { id: "plain", title: () => t("Plain tables") },
  { id: "grid", title: () => t("Grid tables") },
  { id: "list", title: () => t("List tables") },
];

const W = 64;
const H = 50;
const PAD = 3;
/** Designs per row of the gallery: the grey one and the six accents. */
const PER_ROW = 7;

/** A design drawn as an SVG: five rows of four cells, with each cell's fill, lines and a bar for its text. */
export function drawDesign(d: Pick<TableDesign, "cells">): string {
  const rows = d.cells.length;
  const cols = d.cells[0]?.length ?? 1;
  const cw = (W - 2 * PAD) / cols;
  const rh = (H - 2 * PAD) / rows;
  let fills = "";
  let lines = "";
  let bars = "";
  d.cells.forEach((row, r) =>
    row.forEach((c, k) => {
      const x = PAD + k * cw;
      const y = PAD + r * rh;
      if (hex(c.fill)) fills += `<rect x="${x}" y="${y}" width="${cw}" height="${rh}" fill="${hex(c.fill)}"/>`;
      const line = (l: [string, number] | undefined, x1: number, y1: number, x2: number, y2: number) => {
        if (l) lines += `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${hexOr(l[0])}" stroke-width="${Math.min(2, Math.max(0.6, l[1] * 0.8))}"/>`;
      };
      line(c.top, x, y, x + cw, y);
      line(c.bottom, x, y + rh, x + cw, y + rh);
      line(c.left, x, y, x, y + rh);
      line(c.right, x + cw, y, x + cw, y + rh);
      const th = c.bold ? 2.2 : 1.3;
      const ink = c.color === "000000" ? 'class="dg-ink"' : `fill="${hexOr(c.color)}"`;
      bars += `<rect ${ink} x="${x + 2}" y="${y + rh / 2 - th / 2}" width="${cw * 0.58}" height="${th}" rx="0.6"/>`;
    }),
  );
  return `<svg viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" aria-hidden="true"><rect class="dg-paper" width="${W}" height="${H}"/>${fills}${lines}${bars}</svg>`;
}

/** What Word gives a new table, and what a table without a look starts from when it gets a design: header row, first column, banded rows. */
export const DEFAULT_LOOK: RTableLook = { firstRow: true, lastRow: false, firstColumn: true, lastColumn: false, bandedRows: true, bandedColumns: false };

export interface DesignActions {
  /** The options and style of the table the caret is in. */
  state(): { style?: string; look?: RTableLook } | null;
  pick(id: string): void;
  toggle(key: keyof RTableLook): void;
}

/**
 * Open the gallery under `anchor`. The previews follow the table's options, so turning one on or off redraws them;
 * picking a design closes the gallery and gives the table that design.
 */
export function openDesignGallery(ed: Editor, anchor: HTMLElement | { x: number; y: number }, extra: () => TableDesignContribution[], actions: DesignActions): () => void {
  // The gallery holds checkboxes as well as design tiles, which a menu may not (WAI-ARIA): the popover is a small dialog, and each grid of tiles is a menu in it.
  const root = h("div", { class: "design-gallery", role: "group", "aria-label": t("Table design"), "data-pop-role": "dialog" });
  let close = () => {};
  let picked = false;
  const draw = () => {
    const st = actions.state();
    const look: RTableLook = st?.look ?? DEFAULT_LOOK;
    const designs = ed.engine ? ed.engine.tableDesigns(look) : [];
    const had = root.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset.design : undefined;
    root.replaceChildren();
    const options = h("div", { class: "design-options", role: "group", "aria-label": t("Table style options") });
    for (const o of LOOK_OPTIONS) {
      const box = h("input", { type: "checkbox", "data-look": o.key });
      box.checked = look[o.key];
      box.addEventListener("change", () => {
        actions.toggle(o.key);
        draw();
        root.querySelector<HTMLElement>(`[data-look="${o.key}"]`)?.focus();
      });
      options.append(h("label", { class: "design-option" }, box, h("span", {}, o.title())));
    }
    root.append(options);
    const tiles: HTMLButtonElement[][] = [];
    const sections = GROUPS.map((g) => ({ title: g.title(), members: designs.filter((d) => d.group === g.id) }));
    // Designs extensions offer come after Word's own, in the sections they ask for; one the engine can't read is left out.
    const more = new Map<string, TableDesign[]>();
    for (const c of extra()) {
      try {
        const d = { ...ed.engine!.tableDesignPreview(c.xml, look), id: c.id, name: c.label };
        more.set(c.group ?? t("More designs"), [...(more.get(c.group ?? t("More designs")) ?? []), d]);
      } catch (e) {
        console.warn(`Docxtor: table design ${c.id} was left out: ${(e as Error).message ?? e}`);
      }
    }
    for (const [title, members] of more) sections.push({ title, members });
    for (const g of sections) {
      const members = g.members;
      if (!members.length) continue;
      const grid = h("div", { class: "design-grid", role: "menu", "aria-label": g.title });
      const row: HTMLButtonElement[] = [];
      for (const d of members) {
        const on = d.id === st?.style;
        const tile = h("button", { class: "design-tile", type: "button", role: "menuitemradio", "aria-checked": String(on), "aria-label": d.name, title: d.name, "data-design": d.id, tabindex: on ? 0 : -1, html: drawDesign(d) });
        tile.addEventListener("click", () => {
          picked = true;
          actions.pick(d.id);
          close();
          ed.focus();
        });
        grid.append(tile);
        row.push(tile);
      }
      tiles.push(row);
      root.append(h("div", { class: "menu-title" }, g.title), grid);
    }
    const flat = tiles.flat();
    if (flat.length && !flat.some((x) => x.tabIndex === 0)) flat[0].tabIndex = 0;
    if (had) root.querySelector<HTMLElement>(`[data-design="${CSS.escape(had)}"]`)?.focus();
    root.onkeydown = (ev) => {
      const tile = document.activeElement as HTMLButtonElement;
      const group = tiles.find((g) => g.includes(tile));
      if (!group) return;
      const i = group.indexOf(tile);
      const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: PER_ROW, ArrowUp: -PER_ROW };
      let to = -1;
      if (ev.key in step) to = i + step[ev.key];
      else if (ev.key === "Home") to = 0;
      else if (ev.key === "End") to = group.length - 1;
      else return;
      ev.preventDefault();
      if (to < 0 || to >= group.length) return;
      for (const x of flat) x.tabIndex = -1;
      group[to].tabIndex = 0;
      group[to].focus();
    };
  };
  draw();
  close = popover(anchor, root, {
    onClose: () => {
      if (!picked) ed.focus();
    },
  });
  root.querySelector<HTMLElement>('.design-tile[tabindex="0"]')?.focus();
  return close;
}
