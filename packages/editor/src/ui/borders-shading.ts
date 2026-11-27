// Borders and shading for the selected paragraphs, the selected text, or in a table the selected cells or the whole
// table: which edges, how wide, what style and colour, and a fill. The preview draws the same CSS the page will.
// Text takes a box (Word's character border) and a shading colour; paragraphs take any sides; cells and tables take
// the four sides, the lines between rows and columns, and (cells) the diagonals.
import type { Border, BorderEdge, Op, ParaBorders, TableBorders } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { borderCss } from "../editor/render";
import { h, modal } from "./dom";
import { selectionTarget, type FormatTarget } from "./format-target";
import { colorPalette } from "./swatches";
import { cellSelection } from "./table-selection";
import { hex } from "../editor/color.ts";

type Side = "top" | "bottom" | "left" | "right";
/** A table's lines also run between rows and columns, and a cell can have diagonals. */
type Edge = Side | "insideH" | "insideV" | "diagDown" | "diagUp";
const SIDES: [Side, string][] = [
  ["top", msg("Top")],
  ["bottom", msg("Bottom")],
  ["left", msg("Left")],
  ["right", msg("Right")],
];
const EDGES: [Edge, string][] = [
  ...SIDES,
  ["insideH", msg("Between rows")],
  ["insideV", msg("Between columns")],
  ["diagDown", msg("Diagonal down")],
  ["diagUp", msg("Diagonal up")],
];
/** OOXML widths are eighths of a point. */
const WIDTHS: [number, string][] = [
  [4, msg("½ pt")],
  [8, msg("1 pt")],
  [12, msg("1½ pt")],
  [18, msg("2¼ pt")],
  [24, msg("3 pt")],
];
const STYLES: [string, string][] = [
  ["single", msg("Solid")],
  ["dotted", msg("Dotted")],
  ["dashed", msg("Dashed")],
  ["double", msg("Double")],
];
/** Word's own spaces between a box's text and its lines, in points. */
const SPACE: Record<Side, number> = { top: 1, bottom: 1, left: 4, right: 4 };

export type BorderTarget = "paragraph" | "text" | "cell" | "table";

/** What the dialog shows for one target. */
interface Look {
  sides: Set<Edge>;
  sz: number;
  val: string;
  color: string;
  fill: string;
}

/** The edges a target can have; a cell has the lines between cells only when several are selected. */
function edgesOf(target: BorderTarget, multi: boolean): Edge[] {
  if (target === "table") return ["top", "bottom", "left", "right", "insideH", "insideV"];
  if (target === "cell") return multi ? ["top", "bottom", "left", "right", "insideH", "insideV", "diagDown", "diagUp"] : ["top", "bottom", "left", "right", "diagDown", "diagUp"];
  return SIDES.map(([s]) => s);
}

/** The borders a table or cell draws, by edge (an edge that is absent draws nothing). */
function drawn(b: TableBorders, diagDown?: Border, diagUp?: Border): Partial<Record<Edge, Border>> {
  const out: Partial<Record<Edge, Border>> = {};
  for (const e of ["top", "bottom", "left", "right", "insideH", "insideV"] as const) {
    const x = b[e];
    if (x && x.val !== "nil" && x.val !== "none") out[e] = x;
  }
  if (diagDown && diagDown.val !== "nil" && diagDown.val !== "none") out.diagDown = diagDown;
  if (diagUp && diagUp.val !== "nil" && diagUp.val !== "none") out.diagUp = diagUp;
  return out;
}

/** Borders and shading for the selection, or for whatever `format` stands for (a style being edited: paragraph borders only). In a table the cell and the table are targets too; `start` opens on one of them. */
export function openBordersShading(ed: Editor, format: FormatTarget = selectionTarget(ed), start?: BorderTarget) {
  const f = format.state();
  const sel = format.document && ed.format()?.inTable ? cellSelection(ed) : null;
  const para = f.para;
  const run = f.run;
  const nearest = (sz: number) => WIDTHS.reduce((a, b) => (Math.abs(b[0] - sz) < Math.abs(a[0] - sz) ? b : a))[0];
  const look = (border: Border | undefined, sides: Edge[], fill: string | undefined): Look => ({
    sides: new Set(sides),
    sz: nearest(border?.sz ?? 8),
    val: STYLES.some(([v]) => v === border?.val) ? border!.val : "single",
    color: border?.color && border.color !== "auto" ? border.color.toUpperCase() : "auto",
    fill: fill?.toUpperCase() ?? "",
  });
  const old: ParaBorders = para?.borders ?? {};
  const present = SIDES.map(([s]) => s).filter((s) => old[s]);
  const oldText = run?.border && run.border.val !== "none" ? run.border : undefined;
  // The lines a cell or table draws now, by edge.
  const cellNow = sel ? drawn(sel.details.cellBorders, sel.details.diagDown, sel.details.diagUp) : {};
  const tableNow = sel ? drawn(sel.details.borders) : {};
  const firstOf = (m: Partial<Record<Edge, Border>>) => Object.values(m)[0];
  // Each target keeps its own settings while the dialog is open, so switching between them loses nothing.
  const state: Record<BorderTarget, Look> = {
    paragraph: look(present.length ? old[present[0]] : undefined, present, para?.shading),
    text: look(oldText, oldText ? SIDES.map(([s]) => s) : [], run?.shading),
    cell: look(firstOf(cellNow), Object.keys(cellNow) as Edge[], sel?.details.shading),
    table: look(firstOf(tableNow), Object.keys(tableNow) as Edge[], undefined),
  };
  const was: Record<BorderTarget, string> = { paragraph: para?.shading?.toUpperCase() ?? "", text: run?.shading?.toUpperCase() ?? "", cell: sel?.details.shading?.toUpperCase() ?? "", table: "" };
  const textSelected = format.document && !ed.collapsed() && ed.range() && ed.range()![0].p === ed.range()![1].p && !wholeParagraph(ed);
  // A person who has selected text starts on Text, as Word does; in a table the cell is what a border is for; with only a caret it is the paragraph.
  let target: BorderTarget = start && (sel || (start !== "cell" && start !== "table")) ? start : textSelected ? "text" : sel ? "cell" : "paragraph";
  let cur = state[target];
  // Styles, widths and colours this dialog doesn't offer (triple, wave, 3-D) stay as they are until one is chosen.
  let lineTouched = false;
  const theme = ed.engine?.themeColors() ?? null;

  const radio = (items: [string | number, string][], get: () => string | number, set: (v: never) => void, label: string) => {
    const group = h("div", { class: "seg", role: "radiogroup", "aria-label": label });
    for (const [v, text] of items) {
      const b = h("button", { type: "button", role: "radio" }, t(text));
      b.addEventListener("click", () => {
        lineTouched = true;
        set(v as never);
        refresh();
      });
      group.append(b);
    }
    const paint = () => items.forEach(([v], i) => {
      const b = group.children[i] as HTMLElement;
      b.classList.toggle("on", v === get());
      b.setAttribute("aria-checked", String(v === get()));
    });
    return { el: group, paint };
  };

  const applyTo = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Apply to") });
  const targets: [BorderTarget, string][] = [["paragraph", t("Paragraph")], ["text", t("Text")], ...(sel ? ([["cell", t("Cell")], ["table", t("Table")]] as [BorderTarget, string][]) : [])];
  const targetButtons = targets.map(([id, label]) => {
    const b = h("button", { type: "button", role: "radio" }, label);
    b.addEventListener("click", () => {
      state[target] = cur;
      target = id;
      cur = state[id];
      refresh();
    });
    applyTo.append(b);
    return [id, b] as const;
  });

  const position = h("div", { class: "seg bs-edges", role: "group", "aria-label": t("Position") });
  const sideButtons = EDGES.map(([side, text]) => {
    const b = h("button", { type: "button", "aria-pressed": "false", "data-edge": side }, t(text));
    b.addEventListener("click", () => {
      if (cur.sides.has(side)) cur.sides.delete(side);
      else cur.sides.add(side);
      refresh();
    });
    position.append(b);
    return [side, b] as const;
  });
  const shortcuts = h("div", { class: "seg" });
  const box = h("button", { type: "button" }, t("Box"));
  const all = h("button", { type: "button" }, t("All"));
  const none = h("button", { type: "button" }, t("None"));
  box.addEventListener("click", () => {
    for (const s of ["top", "bottom", "left", "right"] as const) cur.sides.add(s);
    refresh();
  });
  all.addEventListener("click", () => {
    for (const s of edgesOf(target, multi())) if (s !== "diagDown" && s !== "diagUp") cur.sides.add(s);
    refresh();
  });
  none.addEventListener("click", () => {
    cur.sides.clear();
    refresh();
  });
  shortcuts.append(box, all, none);
  const multi = () => !!sel?.multi;

  const width = radio(WIDTHS, () => cur.sz, (v: number) => (cur.sz = v), t("Width"));
  const style = radio(STYLES, () => cur.val, (v: string) => (cur.val = v), t("Style"));

  // The two colour fields show the same palette Word does: theme colours, standard and recent colours, More colors.
  const borderColors = colorPalette({
    theme,
    current: null,
    automatic: {
      label: t("Automatic"),
      run: () => {
        lineTouched = true;
        cur.color = "auto";
        refresh();
      },
    },
    pick: (c) => {
      lineTouched = true;
      cur.color = c.hex;
      refresh();
    },
  });
  const fills = colorPalette({
    theme,
    current: null,
    none: {
      label: t("None"),
      run: () => {
        cur.fill = "";
        refresh();
      },
    },
    pick: (c) => {
      cur.fill = c.hex;
      refresh();
    },
  });

  const sample = h("div", { class: "bs-sample" }, h("span", {}, t("Sample text")), h("span", {}, t("The border and fill show here.")));
  // A table or cell is drawn as a grid of cells: one for a single cell, four for the rest.
  const gridCells = [0, 1, 2, 3].map(() => h("div", { class: "bs-cell" }));
  const grid = h("div", { class: "bs-grid" }, ...gridCells);
  const preview = h("div", { class: "bs-preview", "aria-hidden": "true" }, sample, grid);

  function refresh() {
    for (const [id, b] of targetButtons) {
      b.classList.toggle("on", id === target);
      b.setAttribute("aria-checked", String(id === target));
    }
    // Text takes a box or none; paragraphs take any sides; cells and tables take their own set of edges.
    const text = target === "text";
    const allowed = edgesOf(target, multi());
    const inTable = target === "cell" || target === "table";
    for (const [side, b] of sideButtons) {
      b.classList.toggle("on", cur.sides.has(side));
      b.setAttribute("aria-pressed", String(cur.sides.has(side)));
      (b as HTMLButtonElement).disabled = text;
      b.hidden = !allowed.includes(side);
    }
    all.hidden = !inTable;
    // Six edges and the shortcuts don't fit on one line: the edges take it and the shortcuts go below.
    position.classList.toggle("wide", allowed.length > 4);
    width.paint();
    style.paint();
    borderColors.select(cur.color === "auto" ? null : cur.color, cur.color === "auto" ? "automatic" : undefined);
    fills.select(cur.fill || null, cur.fill ? undefined : "none");
    sample.hidden = inTable;
    grid.hidden = !inTable;
    const line = { val: cur.val, sz: cur.sz, color: cur.color, space: 0 };
    if (inTable) {
      const several = target === "table" || multi();
      grid.classList.toggle("one", !several);
      gridCells.forEach((c, i) => {
        c.hidden = !several && i > 0;
        const row = several ? Math.floor(i / 2) : 0;
        const col = several ? i % 2 : 0;
        const last = several ? 1 : 0;
        const edge = (e: Edge) => (cur.sides.has(e) ? borderCss(line) : "none");
        const css = [
          `border-top:${edge(row === 0 ? "top" : "insideH")}`,
          `border-bottom:${edge(row === last ? "bottom" : "insideH")}`,
          `border-left:${edge(col === 0 ? "left" : "insideV")}`,
          `border-right:${edge(col === last ? "right" : "insideV")}`,
        ];
        const g: string[] = [];
        const diag = (dir: string) => `linear-gradient(${dir},transparent calc(50% - 0.5px),currentColor calc(50% - 0.5px),currentColor calc(50% + 0.5px),transparent calc(50% + 0.5px))`;
        if (!several && cur.sides.has("diagDown")) g.push(diag("to top right"));
        if (!several && cur.sides.has("diagUp")) g.push(diag("to bottom right"));
        if (g.length) css.push(`background-image:${g.join(",")}`);
        // Borders drawn in the line's own colour; automatic is the page's ink.
        css.push(`border-color:${hex(cur.color) ?? "var(--paper-ink)"}`);
        if (hex(cur.fill)) css.push(`background-color:${hex(cur.fill)}`);
        c.setAttribute("style", css.join(";"));
        c.dataset.fill = cur.fill;
      });
      return;
    }
    const css: string[] = [];
    for (const [side] of SIDES) {
      if (cur.sides.has(side)) css.push(`border-${side}:${borderCss(line)}`, `padding-${side}:${SPACE[side] + 8}px`);
    }
    if (hex(cur.fill)) css.push(`background:${hex(cur.fill)}`);
    sample.setAttribute("style", css.join(";"));
  }

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog bs-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Borders and shading") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Borders and shading"))),
    h(
      "div",
      { class: "dialog-body" },
      format.document ? h("div", { class: "field" }, h("label", {}, t("Apply to")), applyTo) : null,
      h("div", { class: "field" }, h("label", {}, t("Position")), h("div", { class: "bs-row" }, position, shortcuts)),
      h("div", { class: "bs-cols" }, h("div", { class: "field" }, h("label", {}, t("Width")), width.el), h("div", { class: "field" }, h("label", {}, t("Style")), style.el)),
      h(
        "div",
        { class: "bs-cols" },
        h("div", { class: "field" }, h("label", {}, t("Border color")), borderColors.el),
        h("div", { class: "field" }, h("label", {}, t("Background color")), fills.el),
      ),
      h("div", { class: "field" }, h("label", {}, t("Preview")), preview),
    ),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  refresh();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    state[target] = cur;
    close();
    if (target === "text") {
      const l = state.text;
      const kept = oldText && !lineTouched;
      // A box round the text, or none; a fill left alone keeps its pattern and theme colours.
      const border: Border = l.sides.size ? (kept ? { ...oldText } : { val: l.val, sz: l.sz, color: l.color, space: oldText?.space ?? 1 }) : { val: "none", sz: 0, color: "auto", space: 0 };
      ed.setRun(l.fill === was.text ? { border } : { border, shading: l.fill || "none" });
    } else if (target === "paragraph") {
      const l = state.paragraph;
      const borders: ParaBorders = {};
      for (const [side] of SIDES) {
        if (!l.sides.has(side as Edge)) continue;
        // A side that was already there keeps its distance from the text, and its look unless one was chosen here.
        borders[side] = old[side] && !lineTouched ? { ...old[side]! } : { val: l.val, sz: l.sz, color: l.color, space: old[side]?.space ?? SPACE[side] };
      }
      // A fill left alone keeps its pattern and theme colours.
      format.setPara(l.fill === was.paragraph ? { borders } : { borders, shading: l.fill });
    } else if (sel) {
      const l = state[target];
      const now = target === "cell" ? cellNow : tableNow;
      // An edge that is on gets the line chosen here (or keeps its own look when none was chosen); one that was on and
      // is now off is switched off; the others are left as they were.
      const edges: Partial<Record<Edge, BorderEdge>> = {};
      for (const e of edgesOf(target, multi())) {
        const line = now[e];
        if (l.sides.has(e)) {
          if (!line || lineTouched) edges[e] = { val: l.val, sz: l.sz, color: l.color };
        } else if (line) edges[e] = { val: "none" };
      }
      const ops: Op[] = [];
      if (Object.keys(edges).length) {
        ops.push(target === "cell" ? { op: "table_cells", from: sel.from, to: sel.to, action: { type: "borders", ...edges } } : { op: "table_format", at: sel.from, borders: edges });
      }
      if (l.fill !== was[target]) ops.push(shadingOp(ed, sel.from, sel.to, target, l.fill));
      if (ops.length) ed.exec(ops);
    }
    format.done();
  });
}

/** The shading of the selected cells, or of every cell of the table. */
function shadingOp(ed: Editor, from: number, to: number, target: BorderTarget, fill: string): Op {
  const color = fill || null;
  if (target === "cell") return { op: "table_cells", from, to, action: { type: "shading", color } };
  const table = ed.root.querySelector(`[data-p="${from}"]`)?.closest("table.ft");
  const rows = table ? Array.from((table as HTMLTableElement).rows).filter((r) => !r.classList.contains("pgsp") && !r.classList.contains("hdr-repeat")) : [];
  const para = (td: Element | undefined) => Number(td?.querySelector<HTMLElement>("[data-p]")?.dataset.p ?? from);
  const first = rows[0]?.querySelector("td:not(.gap)") ?? undefined;
  const lastCells = rows[rows.length - 1]?.querySelectorAll("td:not(.gap)");
  const last = lastCells?.length ? lastCells[lastCells.length - 1] : undefined;
  return { op: "table_cells", from: para(first), to: para(last), action: { type: "shading", color } };
}

/** Whether the selection takes in the whole of its paragraph (then the paragraph is what a border is for). */
function wholeParagraph(ed: Editor): boolean {
  const r = ed.range();
  if (!r) return false;
  return r[0].o === 0 && r[1].o >= ed.engine.textRange({ p: r[1].p, o: 0 }, { p: r[1].p, o: 1_000_000 }).length;
}
