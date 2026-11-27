// Draws a table from the view model: grid, merges, the borders, fill and margins the engine settled for each cell
// (table style, banding and direct formatting are already worked out), content controls around rows and cells,
// rows that start or end short, exact row heights, diagonal borders, cell spacing, and floating tables.
import { t } from "@docxtor/extensions";
import type { Border, RCell, RTable } from "@docxtor/core";
import { lineWidth } from "./borders";
import { isDark } from "./charfmt";
import { borderCss, hex, renderBlocks, tw } from "./render";
import type { RenderEnv } from "./render";
import { pageMetrics } from "./place";

const PT_PX = 96 / 72;

/** A border that says "no line". */
export function isNil(b?: Border): boolean {
  return !b || b.val === "nil" || b.val === "none";
}

function cellBorders(t: RTable, cell: RCell, r: number, c0: number, c1: number, rows: number, cols: number) {
  const tb = t.borders;
  const cb = cell.borders;
  // A side the engine settled (even as "no line") is used as it is; one it left out comes from the table.
  return {
    top: cb.top ?? (r === 0 ? tb.top : tb.insideH),
    bottom: cb.bottom ?? (r === rows - 1 ? tb.bottom : tb.insideH),
    left: cb.left ?? (c0 === 0 ? tb.left : tb.insideV),
    right: cb.right ?? (c1 >= cols ? tb.right : tb.insideV),
  };
}

/** A thin line along one diagonal of a cell, as a gradient (the 50% line of a corner to corner gradient is the other diagonal). */
function diagonal(b: Border, dir: string): string {
  const w = Math.max(1, lineWidth(b));
  const c = hex(b.color) ?? "#000";
  return `linear-gradient(${dir},transparent calc(50% - ${w / 2}px),${c} calc(50% - ${w / 2}px),${c} calc(50% + ${w / 2}px),transparent calc(50% + ${w / 2}px))`;
}

/** The name a content control around a row or cell shows. */
function controlName(name: string | undefined): string {
  return name || t("Content control");
}

export function renderTable(tbl: RTable, env: RenderEnv): HTMLElement {
  const table = document.createElement("table");
  table.className = "ft";
  table.dataset.t = String(tbl.id);
  table.dataset.id = String(tbl.id);
  if (tbl.grid.length) table.dataset.grid = tbl.grid.join(",");
  if (tbl.style) table.dataset.style = tbl.style;
  if (tbl.styleName) table.dataset.styleName = tbl.styleName;
  if (tbl.look) table.dataset.look = JSON.stringify(tbl.look);
  const gridCols = tbl.grid.length || Math.max(...tbl.rows.map((r) => (r.gridBefore ?? 0) + r.cells.reduce((a, c) => a + c.span, 0) + (r.gridAfter ?? 0)), 1);
  const total = tbl.grid.reduce((a, b) => a + b, 0);
  const st: string[] = [];
  if (tbl.widthPct && tbl.width) st.push(`width:${tbl.width / 50}%`);
  else if (total) st.push(`width:${tw(total)}px`);
  // Fixed columns keep the grid's widths. Autofit ones start from them and grow where a word needs the room.
  if (tbl.grid.length) st.push(tbl.fixed ? "table-layout:fixed" : "table-layout:auto;max-width:100%");
  if (!tbl.fixed) table.dataset.autofit = "1";
  if (tbl.spacing) st.push(`border-collapse:separate;border-spacing:${tw(tbl.spacing)}px`);
  // A right-to-left table starts at the right margin: its first column is on the right, its alignment and
  // indent count from there, and its left and right borders and cell margins swap.
  const rtl = !!tbl.bidiVisual;
  if (rtl) st.push("direction:rtl");
  if (tbl.float) {
    // A floating table is placed by the wrapper; it keeps no automatic margins of its own.
  } else if (tbl.align === "center") st.push("margin-left:auto;margin-right:auto");
  else if (rtl) {
    if (tbl.align === "right" || tbl.align === "end") st.push("margin-right:auto");
    else st.push("margin-left:auto");
    if (tbl.indent && !(tbl.align === "right" || tbl.align === "end")) st.push(`margin-right:${tw(tbl.indent)}px`);
  } else if (tbl.align === "right" || tbl.align === "end") st.push("margin-left:auto");
  else if (tbl.indent) st.push(`margin-left:${tw(tbl.indent)}px`);
  table.setAttribute("style", st.join(";"));
  if (tbl.grid.length) {
    const cg = document.createElement("colgroup");
    for (const g of tbl.grid) {
      const col = document.createElement("col");
      col.style.width = `${tw(g)}px`;
      cg.append(col);
    }
    table.append(cg);
  }
  const tbody = document.createElement("tbody");
  // Each cell's first grid column, for vertical merges (a row that starts short begins further in).
  const starts = tbl.rows.map((r) => {
    let g = r.gridBefore ?? 0;
    return r.cells.map((c) => {
      const s = g;
      g += c.span;
      return s;
    });
  });
  const gap = (n: number) => {
    const td = document.createElement("td");
    td.className = "gap";
    td.colSpan = n;
    td.contentEditable = "false";
    td.setAttribute("aria-hidden", "true");
    td.style.cssText = "padding:0;border:none;background:transparent";
    return td;
  };
  tbl.rows.forEach((row, ri) => {
    const tr = document.createElement("tr");
    const exact = row.heightRule === "exact" && !!row.height;
    if (row.height) tr.style.height = `${tw(row.height)}px`;
    if (exact) tr.dataset.exact = "1";
    if (row.header) tr.dataset.header = "1";
    if (row.cantSplit) tr.dataset.cs = "1";
    if (row.sdt !== undefined) {
      tr.dataset.sdt = "1";
      tr.dataset.sdtLabel = controlName(row.sdt);
    }
    // A row that is an item of a repeating section: the controls tool adds and removes such rows.
    if (row.control?.ctl !== undefined) {
      tr.dataset.ctlWrap = String(row.control.ctl);
      tr.dataset.ck = row.control.kind;
      if (row.section?.ctl !== undefined) {
        tr.dataset.repSection = String(row.section.ctl);
        if (row.section.fixedItems) tr.dataset.repFixed = "1";
        if (row.section.sectionTitle) tr.dataset.repTitle = row.section.sectionTitle;
      }
    }
    if (row.gridBefore) tr.append(gap(row.gridBefore));
    row.cells.forEach((cell, ci) => {
      if (cell.vmerge === "continue") return;
      const td = document.createElement("td");
      if (cell.span > 1) td.colSpan = cell.span;
      if (cell.vmerge === "restart") {
        const g = starts[ri][ci];
        let span = 1;
        for (let k = ri + 1; k < tbl.rows.length; k++) {
          const idx = starts[k].indexOf(g);
          if (idx >= 0 && tbl.rows[k].cells[idx].vmerge === "continue") span++;
          else break;
        }
        if (span > 1) td.rowSpan = span;
      }
      const g0 = starts[ri][ci];
      const b = cellBorders(tbl, cell, ri, g0, g0 + cell.span, tbl.rows.length, gridCols);
      const mar = cell.mar ?? tbl.cellMar;
      const css = [
        rtl ? `padding:${tw(mar[0])}px ${tw(mar[3])}px ${tw(mar[2])}px ${tw(mar[1])}px` : `padding:${tw(mar[0])}px ${tw(mar[1])}px ${tw(mar[2])}px ${tw(mar[3])}px`,
        `border-top:${borderCss(b.top)}`,
        `border-bottom:${borderCss(b.bottom)}`,
        `border-left:${borderCss(rtl ? b.right : b.left)}`,
        `border-right:${borderCss(rtl ? b.left : b.right)}`,
        // The text inside goes the way its own paragraphs say.
        ...(rtl ? ["direction:ltr"] : []),
      ];
      if (hex(cell.shading)) css.push(`background-color:${hex(cell.shading)}`);
      // Paragraphs with the automatic colour read white on a dark cell; the ones with a colour of their own keep it.
      if (isDark(cell.shading)) css.push("color:#ffffff");
      const lines: string[] = [];
      if (!isNil(cell.diagDown)) lines.push(diagonal(cell.diagDown!, "to top right"));
      if (!isNil(cell.diagUp)) lines.push(diagonal(cell.diagUp!, "to bottom right"));
      if (lines.length) css.push(`background-image:${lines.join(",")}`);
      const va = { center: "middle", bottom: "bottom" }[cell.valign ?? ""] ?? "top";
      css.push(`vertical-align:${va}`);
      // Text running down (lines from the right) or up (lines from the left), and text kept on one line.
      if (cell.textDir) css.push(`writing-mode:${cell.textDir === "tbRl" ? "vertical-rl" : "sideways-lr"}`);
      if (cell.noWrap) css.push("white-space:nowrap");
      td.setAttribute("style", css.join(";"));
      if (cell.sdt !== undefined) {
        td.dataset.sdt = "1";
        td.dataset.sdtLabel = controlName(cell.sdt);
      }
      const blocks = renderBlocks(cell.blocks, env);
      if (exact) {
        // An exact row height clips what does not fit, so the content sits in a box of that height.
        const box = document.createElement("div");
        box.className = "clip";
        box.style.height = `${Math.max(0, tw(row.height) - tw(mar[0]) - tw(mar[2]))}px`;
        if (va !== "top") box.style.cssText += `;display:flex;flex-direction:column;justify-content:${va === "middle" ? "center" : "flex-end"}`;
        for (const el of blocks) box.append(el);
        td.append(box);
      } else {
        for (const el of blocks) td.append(el);
      }
      tr.append(td);
    });
    if (row.gridAfter) tr.append(gap(row.gridAfter));
    // The row's control shows its name above the row's first cell.
    const first = tr.querySelector<HTMLElement>("td:not(.gap)");
    if (row.sdt !== undefined && first) first.dataset.rowSdt = controlName(row.sdt);
    tbody.append(tr);
  });
  table.append(tbody);
  return table;
}

/**
 * The block a table is drawn in. A floating table (`w:tblpPr`) floats to the left or right of the text, which flows around
 * it like around a picture, at the distance from the text the file asks for.
 */
export function tableWrap(tbl: RTable, env: RenderEnv): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "ftw";
  wrap.dataset.id = String(tbl.id);
  wrap.append(renderTable(tbl, env));
  const f = tbl.float;
  if (!f) return wrap;
  wrap.classList.add("tfloat");
  const spec = f.xAlign;
  const side = spec === "right" || spec === "outside" ? "right" : spec === "center" ? "none" : "left";
  const [l, r, tp, bt] = [tw(f.left), tw(f.right), tw(f.top), tw(f.bottom)];
  // Measured from the margin or the text column, an offset is the space before the table. From the page edge it counts the margin in.
  const x = f.x ?? 0;
  const fromPage = f.horzAnchor === "page" ? pageMetrics.marginL * PT_PX : 0;
  const lead = side === "left" && !spec ? Math.max(0, tw(x) - fromPage) : 0;
  const y = f.vertAnchor === "text" || !f.vertAnchor ? tw(f.y ?? 0) : 0;
  const s = wrap.style;
  s.cssText = `float:${side === "none" ? "none" : side};margin:${tp + y}px ${r}px ${bt}px ${l + lead}px`;
  if (side === "none") s.cssText += ";margin-left:auto;margin-right:auto;float:none";
  if (f.neverOverlap) wrap.dataset.nooverlap = "1";
  wrap.dataset.float = side;
  return wrap;
}
