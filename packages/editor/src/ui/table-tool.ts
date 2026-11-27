// Table editing: column resize handles, cell-range selection, and a contextual strip
// (rows, columns, merge/split, fill, alignment, borders, header row).
import { msg, t, type TableDesignContribution } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import type { Op, RTableLook } from "@docxtor/core";
import { paraIn } from "../editor/selection";
import { h, overlayLayer, popover } from "./dom";
import { DEFAULT_LOOK, openDesignGallery } from "./table-design";
import { hexOr } from "../editor/color.ts";

const FILLS = ["F2F2F2", "D9D9D9", "DEEAF6", "E2EFD9", "FFF2CC", "FBE4D5", "EDE2F6", "1F2937"];

interface Grid {
  /** For every <td>: its grid rectangle. */
  cells: Map<HTMLTableCellElement, { r0: number; c0: number; r1: number; c1: number }>;
}

/** Map a rendered table's cells to grid rectangles (handles colspan/rowspan). */
function gridOf(table: HTMLTableElement): Grid {
  const cells = new Map<HTMLTableCellElement, { r0: number; c0: number; r1: number; c1: number }>();
  const taken: boolean[][] = [];
  let r = 0;
  for (const tr of Array.from(table.rows)) {
    if (tr.classList.contains("pgsp") || tr.classList.contains("hdr-repeat")) continue;
    taken[r] ??= [];
    let c = 0;
    for (const td of Array.from(tr.cells)) {
      while (taken[r][c]) c++;
      // A gap where a row starts or ends short is no cell.
      if (td.classList.contains("gap")) {
        c += td.colSpan;
        continue;
      }
      const rect = { r0: r, c0: c, r1: r + td.rowSpan - 1, c1: c + td.colSpan - 1 };
      for (let rr = rect.r0; rr <= rect.r1; rr++) for (let cc = rect.c0; cc <= rect.c1; cc++) (taken[rr] ??= [])[cc] = true;
      cells.set(td, rect);
      c += td.colSpan;
    }
    r++;
  }
  return { cells };
}

export class TableTool {
  private strip: HTMLElement;
  private handles: HTMLElement;
  private hoverTable: HTMLTableElement | null = null;
  private active: HTMLTableElement | null = null;
  private range: { a: number; b: number } | null = null;
  private dragging = false;

  /** `designs` lists the table designs extensions offer besides Word's own. */
  constructor(private ed: Editor, private designs: () => TableDesignContribution[] = () => []) {
    this.strip = h("div", { class: "tbl-strip", style: "display:none", role: "toolbar", "aria-label": t("Table") });
    this.handles = h("div", { class: "col-handles" });
    overlayLayer().append(ed.life.own(this.strip), ed.life.own(this.handles));
    ed.flow.addEventListener("mousemove", (e) => this.onMove(e));
    ed.flow.addEventListener("mouseleave", (e) => {
      // Moving onto a resize handle (which sits outside the flow) must not remove it.
      if (this.dragging || this.handles.contains(e.relatedTarget as Node)) return;
      this.hideHandles();
    });
    this.handles.addEventListener("mouseleave", (e) => {
      if (!this.dragging && !this.ed.flow.contains(e.relatedTarget as Node)) this.hideHandles();
    });
    ed.canvas.addEventListener("scroll", () => {
      this.hideHandles();
      this.place();
    }, { passive: true });
    ed.life.on(document, "selectionchange", () => requestAnimationFrame(() => this.sync()));
  }

  /** Re-evaluate after document changes. */
  refresh() {
    this.sync();
  }

  private tableOfPara(id: number): HTMLTableElement | null {
    return paraIn(this.ed.flow, id)?.closest("table.ft") ?? null;
  }

  /** The cells and table box marked as selected now. */
  private marked: [Element, string][] = [];

  private sync() {
    if (this.dragging) return;
    const s = this.ed.sel;
    // What this tool marked is taken back by what it remembers: searching the page for it costs the length of the document, on every keystroke.
    for (const [el, cls] of this.marked) el.classList.remove(cls);
    this.marked = [];
    if (!s || document.activeElement !== this.ed.flow) return this.hideStrip();
    const ta = this.tableOfPara(s.anchor.p);
    const tf = this.tableOfPara(s.focus.p);
    if (!ta || ta !== tf) {
      this.range = null;
      return this.hideStrip();
    }
    this.active = ta;
    const tdA = paraIn(this.ed.flow, s.anchor.p)?.closest("td");
    const tdF = paraIn(this.ed.flow, s.focus.p)?.closest("td");
    this.range = { a: s.anchor.p, b: s.focus.p };
    if (tdA && tdF && tdA !== tdF && tdA.closest("table") === ta && tdF.closest("table") === ta) {
      const g = gridOf(ta);
      const ra = g.cells.get(tdA as HTMLTableCellElement)!;
      const rf = g.cells.get(tdF as HTMLTableCellElement)!;
      const r0 = Math.min(ra.r0, rf.r0);
      const r1 = Math.max(ra.r1, rf.r1);
      const c0 = Math.min(ra.c0, rf.c0);
      const c1 = Math.max(ra.c1, rf.c1);
      for (const [td, r] of g.cells) {
        if (r.r0 <= r1 && r.r1 >= r0 && r.c0 <= c1 && r.c1 >= c0) {
          td.classList.add("cell-sel");
          this.marked.push([td, "cell-sel"]);
        }
      }
      const box = ta.closest(".ftw");
      if (box) {
        box.classList.add("cells-selected");
        this.marked.push([box, "cells-selected"]);
      }
    }
    this.showStrip();
  }

  private hideStrip() {
    this.strip.style.display = "none";
    this.active = null;
  }

  private multi(): boolean {
    return !!this.active?.querySelector(".cell-sel");
  }

  // ---- actions, for commands (the strip, menus and the right-click menu). They read the state the
  // last selection change left, since a menu takes the focus while it's open.

  /** Several cells are selected, so they can be merged. */
  canMerge(): boolean {
    return !!this.range && this.multi();
  }

  /** The cell at the caret spans more than one row or column. */
  canSplit(): boolean {
    const td = this.range ? paraIn(this.ed.flow, this.range.a)?.closest("td") : null;
    return !!td && ((td as HTMLTableCellElement).colSpan > 1 || (td as HTMLTableCellElement).rowSpan > 1);
  }

  merge() {
    if (this.canMerge()) this.exec({ op: "table_cells", from: this.range!.a, to: this.range!.b, action: { type: "merge" } });
  }

  split() {
    if (this.canSplit()) this.exec({ op: "table_cells", from: this.range!.a, to: this.range!.b, action: { type: "split" } });
  }

  /** The table repeats its first row on every page. */
  headerRow(): boolean {
    return !!this.active?.querySelector("tr[data-header]");
  }

  toggleHeaderRow() {
    const p = this.anyPara();
    if (p !== null) this.exec({ op: "table_props", at: p, header: !this.headerRow() });
  }

  distributeColumns() {
    this.distribute();
  }

  // ---- table designs (see ui/table-design.ts)

  /** A paragraph in the table the caret was last in. */
  private tablePara(): number | null {
    return this.range?.a ?? this.anyPara();
  }

  private tableOf(p: number): HTMLTableElement | null {
    return paraIn(this.ed.flow, p)?.closest("table.ft") ?? null;
  }

  /** The table style of the table at paragraph `at` and which parts of it show. */
  private designAt(at: number): { style?: string; styleName?: string; look?: RTableLook } {
    const table = this.tableOf(at);
    return { style: table?.dataset.style, styleName: table?.dataset.styleName, look: table?.dataset.look ? (JSON.parse(table.dataset.look) as RTableLook) : undefined };
  }

  /** The table style the caret's table has and which parts of it show. */
  design(): { style?: string; styleName?: string; look?: RTableLook } | null {
    const p = this.tablePara();
    return p === null || !this.tableOf(p) ? null : this.designAt(p);
  }

  /** The style options of the caret's table (Word's defaults for one that has none). */
  look(): RTableLook | undefined {
    const d = this.design();
    return d ? (d.look ?? DEFAULT_LOOK) : undefined;
  }

  /** Give the table a design (a built-in one is added to the document the first time). */
  pickDesign(id: string, at = this.tablePara()) {
    if (at === null) return;
    // A design an extension offers carries its definition, so the document can have it.
    const own = this.designs().find((d) => d.id === id);
    this.exec(own ? { op: "table_design", at, definition: own.xml } : { op: "table_design", at, style: id });
  }

  /** Switch one of the table style options on or off. */
  toggleLook(key: keyof RTableLook, at = this.tablePara()) {
    if (at === null) return;
    this.exec({ op: "table_design", at, look: { [key]: !(this.designAt(at).look ?? DEFAULT_LOOK)[key] } });
  }

  /** Open the design gallery under the strip's Design button, or at the table. */
  openDesigns(anchor?: HTMLElement) {
    const at = this.tablePara();
    const table = at === null ? null : this.tableOf(at);
    if (at === null || !table) return;
    const r = table.getBoundingClientRect();
    const button = anchor ?? this.strip.querySelector<HTMLElement>(".design-btn");
    const where = button?.isConnected && this.strip.style.display !== "none" ? button : { x: Math.max(8, r.left), y: Math.max(8, r.top) };
    openDesignGallery(this.ed, where, this.designs, {
      state: () => this.designAt(at),
      pick: (id) => this.pickDesign(id, at),
      toggle: (key) => this.toggleLook(key, at),
    });
  }

  private exec(op: Op) {
    this.ed.exec(op);
    this.ed.focus();
  }

  private anyPara(): number | null {
    const p = this.active?.querySelector<HTMLElement>("[data-p]");
    return p ? Number(p.dataset.p) : null;
  }

  private showStrip() {
    const tbl = this.active;
    if (!tbl || !this.range) return;
    const td = paraIn(this.ed.flow, this.range.a)?.closest("td");
    const merged = !!td && ((td as HTMLTableCellElement).colSpan > 1 || (td as HTMLTableCellElement).rowSpan > 1);
    const btn = (label: string, title: string, fn: (b: HTMLButtonElement) => void, disabled = false, extra = "") => {
      const b = h("button", { class: `tb ${extra}`.trim(), type: "button", title, disabled }, label);
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => fn(b));
      return b;
    };
    // The strip's buttons are the table commands: they check, ask and offer Undo as the menus do.
    const action = (id: string) => () => void this.ed.command(id).then(() => this.ed.focus(), () => undefined);
    const sep = () => h("div", { class: "sep" });
    const header = !!tbl.querySelector("tr[data-header]");
    this.strip.replaceChildren(
      h("span", { class: "lbl-eng strip-label" }, t("Table")),
      btn(t("Row above"), t("Insert row above"), action("table.rowAbove")),
      btn(t("Row below"), t("Insert row below"), action("table.rowBelow")),
      btn(t("Column left"), t("Insert column left"), action("table.columnLeft")),
      btn(t("Column right"), t("Insert column right"), action("table.columnRight")),
      sep(),
      btn(t("Merge"), t("Merge selected cells"), () => this.merge(), !this.multi()),
      btn(t("Split"), t("Split merged cell"), () => this.split(), !merged),
      sep(),
      btn(`${t("Fill")} ▾`, t("Cell fill"), (b) => this.fillMenu(b)),
      btn(`${t("Align")} ▾`, t("Vertical alignment"), (b) => this.alignMenu(b)),
      btn(`${t("Borders")} ▾`, t("Table borders"), (b) => this.bordersMenu(b)),
      btn(`${t("Design")} ▾`, t("Table design"), (b) => this.openDesigns(b), false, "design-btn"),
      btn(header ? `${t("Header row")} ✓` : t("Header row"), t("Repeat the first row on every page"), () => this.toggleHeaderRow()),
      btn(t("Even"), t("Distribute columns evenly"), () => this.distribute()),
      sep(),
      btn(`${t("Delete")} ▾`, t("Delete rows, columns or the table"), (b) => this.deleteMenu(b)),
    );
    this.strip.style.display = "";
    this.place();
  }

  private place() {
    const tbl = this.active;
    if (!tbl || this.strip.style.display === "none") return;
    const r = tbl.getBoundingClientRect();
    const c = this.ed.canvas.getBoundingClientRect();
    if (r.bottom < c.top || r.top > c.bottom) {
      this.strip.style.visibility = "hidden";
      return;
    }
    this.strip.style.visibility = "";
    const sw = this.strip.offsetWidth;
    const top = Math.max(c.top + 4, r.top - 40);
    this.strip.style.top = `${top}px`;
    this.strip.style.left = `${Math.max(8, Math.min(window.innerWidth - sw - 8, r.left))}px`;
  }

  private menu(anchor: HTMLElement, items: [string, () => void][]) {
    const list = h("div", {});
    const close = popover(anchor, list);
    for (const [label, fn] of items) {
      const b = h("button", { class: "menu-item", type: "button" }, t(label));
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => {
        close();
        fn();
      });
      list.append(b);
    }
  }

  private fillMenu(anchor: HTMLElement) {
    const grid = h("div", { class: "swatches" });
    const close = popover(anchor, h("div", {}, h("div", { class: "menu-title" }, t("Cell fill")), grid));
    const run = (color: string | null) => {
      close();
      this.exec({ op: "table_cells", from: this.range!.a, to: this.range!.b, action: { type: "shading", color } });
    };
    for (const f of FILLS) {
      const s = h("button", { class: "swatch", type: "button", title: `#${f}`, style: `background:${hexOr(f)}` });
      s.addEventListener("mousedown", (e) => e.preventDefault());
      s.addEventListener("click", () => run(f));
      grid.append(s);
    }
    const none = h("button", { class: "menu-item", type: "button" }, t("No fill"));
    none.addEventListener("mousedown", (e) => e.preventDefault());
    none.addEventListener("click", () => run(null));
    grid.parentElement!.append(none);
  }

  private alignMenu(anchor: HTMLElement) {
    const v = (value: string) => () => this.exec({ op: "table_cells", from: this.range!.a, to: this.range!.b, action: { type: "valign", value } });
    this.menu(anchor, [
      [msg("Top"), v("top")],
      [msg("Middle"), v("middle")],
      [msg("Bottom"), v("bottom")],
    ]);
  }

  private bordersMenu(anchor: HTMLElement) {
    const p = this.anyPara();
    const set = (borders: string) => () => p !== null && this.exec({ op: "table_props", at: p, borders });
    this.menu(anchor, [
      [msg("All borders"), set("grid")],
      [msg("Outside only"), set("outer")],
      [msg("Horizontal lines"), set("horizontal")],
      [msg("No borders"), set("none")],
    ]);
  }

  private deleteMenu(anchor: HTMLElement) {
    const run = (id: string) => () => void this.ed.command(id).then(() => this.ed.focus(), () => undefined);
    this.menu(anchor, [
      [msg("Delete row"), run("table.deleteRow")],
      [msg("Delete column"), run("table.deleteColumn")],
      [msg("Delete table"), run("table.deleteTable")],
    ]);
  }

  private gridWidths(tbl: HTMLTableElement): number[] {
    return (tbl.dataset.grid ?? "").split(",").filter(Boolean).map(Number);
  }

  private distribute() {
    const tbl = this.active;
    const p = this.anyPara();
    if (!tbl || p === null) return;
    const w = this.gridWidths(tbl);
    if (!w.length) return;
    const total = w.reduce((a, b) => a + b, 0);
    const each = Math.floor(total / w.length);
    this.exec({ op: "table_props", at: p, widths: w.map((_, i) => (i === w.length - 1 ? total - each * (w.length - 1) : each)) });
  }

  // ---- column resize ------------------------------------------------------

  private onMove(e: MouseEvent) {
    if (this.dragging) return;
    const tbl = (e.target as HTMLElement).closest?.("table.ft") as HTMLTableElement | null;
    if (!tbl || tbl.closest(".hf")) return this.hideHandles();
    if (tbl === this.hoverTable) return;
    this.hoverTable = tbl;
    this.showHandles(tbl);
  }

  private hideHandles() {
    this.hoverTable = null;
    this.handles.replaceChildren();
  }

  private showHandles(tbl: HTMLTableElement) {
    const widths = this.gridWidths(tbl);
    if (!widths.length) return this.hideHandles();
    const r = tbl.getBoundingClientRect();
    const z = this.ed.zoom;
    this.handles.replaceChildren();
    let x = 0;
    widths.forEach((w, k) => {
      x += w / 15;
      const hd = h("div", { class: "col-handle", title: t("Drag to resize column") });
      hd.style.cssText = `left:${r.left + x * z - 4}px;top:${r.top}px;height:${r.height}px`;
      hd.addEventListener("pointerdown", (ev) => this.drag(ev, tbl, k, widths));
      this.handles.append(hd);
    });
  }

  private drag(e: PointerEvent, tbl: HTMLTableElement, k: number, widths: number[]) {
    e.preventDefault();
    const hd = e.currentTarget as HTMLElement;
    hd.setPointerCapture(e.pointerId);
    hd.classList.add("drag");
    this.dragging = true;
    const z = this.ed.zoom;
    const sx = e.clientX;
    const left0 = parseFloat(hd.style.left);
    const min = 360;
    let next = widths.slice();
    const move = (ev: PointerEvent) => {
      let d = Math.round(((ev.clientX - sx) / z) * 15);
      next = widths.slice();
      if (k < widths.length - 1) {
        d = Math.max(min - widths[k], Math.min(widths[k + 1] - min, d));
        next[k] += d;
        next[k + 1] -= d;
      } else {
        d = Math.max(min - widths[k], d);
        next[k] += d;
      }
      hd.style.left = `${left0 + (d / 15) * z}px`;
      const cols = tbl.querySelectorAll("col");
      next.forEach((w, i) => cols[i] && ((cols[i] as HTMLElement).style.width = `${w / 15}px`));
      if (k === widths.length - 1) tbl.style.width = `${next.reduce((a, b) => a + b, 0) / 15}px`;
    };
    const up = () => {
      hd.removeEventListener("pointermove", move);
      hd.removeEventListener("pointerup", up);
      hd.classList.remove("drag");
      this.dragging = false;
      this.hideHandles();
      const p = tbl.querySelector<HTMLElement>("[data-p]");
      if (p && next.some((w, i) => w !== widths[i])) this.ed.exec({ op: "table_props", at: Number(p.dataset.p), widths: next });
    };
    hd.addEventListener("pointermove", move);
    hd.addEventListener("pointerup", up);
  }
}
