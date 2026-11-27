// Turns the engine's resolved view model into DOM. Every paragraph element carries
// `data-p` (its id); every segment carries `data-o` (its UTF-16 offset). Selection
// mapping (selection.ts) relies on that structure.
import { t } from "@docxtor/extensions";
import { fontCategory, standInsFor, wantFont } from "../fonts";
import { mathNode } from "./math";
import { hex } from "./color.ts";
import type { Engine } from "@docxtor/core";
import type { AsianSettings, Border, ParaFmt, RBlock, RImage, RPara, RSeg, RunFmt } from "@docxtor/core";
import { chartLabel, chartTable, drawChart } from "../chart/draw";
import { registerChart } from "../chart/registry";
import { fillChartEx } from "../chart/chartex-atom";import { applyFrame } from "./frames";
import { floatData, placeCss } from "./place";
import { pictureNode, placeholder } from "./picture";
import { shapeAtom } from "./shapes";
import { borderEdge, borderShorthand, type Layer } from "./borders";
import { shdPaint } from "./shading";
import { decorationCss, HIGHLIGHT, inkOf, isDark, prepareSegs, runBackground, runExtraCss, type Piece } from "./charfmt";
import { tabPlan } from "./tabs";
import { tableWrap } from "./table-render";
import { outlineLevel } from "./folds";
import { controlName, decorateControls, formAtom } from "./controls-render";
import { eaParaCss, eaSegPiece, kinsokuPieces, rubyAtom } from "./east-asian";

/** twips → CSS px */
export const tw = (v: number | undefined) => (v ?? 0) / 15;
const PT_PX = 96 / 72;

// Metric-compatible open fonts keep line breaks close to Word's when the original font is missing.
const FONT_STACKS: Record<string, string> = {
  calibri: "Calibri, Carlito, 'Segoe UI', sans-serif",
  "calibri light": "'Calibri Light', Carlito, 'Segoe UI', sans-serif",
  cambria: "Cambria, Caladea, Georgia, serif",
  arial: "Arial, Arimo, 'Liberation Sans', Helvetica, sans-serif",
  helvetica: "Helvetica, Arial, Arimo, sans-serif",
  "times new roman": "'Times New Roman', Tinos, 'Liberation Serif', Times, serif",
  times: "Times, 'Times New Roman', Tinos, serif",
  "courier new": "'Courier New', Cousine, 'Liberation Mono', monospace",
  georgia: "Georgia, Gelasio, serif",
  consolas: "Consolas, Cousine, 'SF Mono', monospace",
  aptos: "Aptos, 'Segoe UI', Arimo, sans-serif",
  "aptos display": "'Aptos Display', Aptos, 'Segoe UI', Arimo, sans-serif",
  "segoe ui": "'Segoe UI', system-ui, Arimo, sans-serif",
  verdana: "Verdana, 'DejaVu Sans', sans-serif",
  tahoma: "Tahoma, Verdana, sans-serif",
  garamond: "Garamond, 'EB Garamond', serif",
  "book antiqua": "'Book Antiqua', Palatino, serif",
};

const GENERIC = { sans: "sans-serif", serif: "serif", mono: "monospace", display: "sans-serif", handwriting: "cursive" } as const;

/**
 * A font name as a CSS string: the quote it is written in, the backslash that would escape that quote and the line breaks that end a
 * string are left out, so a name from a file cannot end the string and add declarations of its own (`x'; background: url(...)`).
 */
const cssFontName = (font: string) => `'${font.replace(/['"\\\n\r\f<>]/g, "")}'`;

export function fontStack(font?: string): string | undefined {
  if (!font) return undefined;
  const k = font.toLowerCase();
  const name = cssFontName(font);
  // A font an extension offers is on the page once something asks for it (see `wantFont`), and so are the ones it offers in place of
  // this one, which come right after it.
  void wantFont(font);
  const instead = standInsFor(font);
  for (const f of instead) void wantFont(f);
  const extra = instead.map(cssFontName);
  // Own keys only: a font named `constructor` is not a stack that Object has.
  const known = Object.hasOwn(FONT_STACKS, k) ? FONT_STACKS[k] : undefined;
  if (known) return extra.length ? known.replace(/^[^,]+/, (first) => [first, ...extra].join(", ")) : known;
  const cat = fontCategory(font);
  const serif = /serif|times|garamond|georgia|book|roman|minion|caslon|baskerville/i.test(font) && !/sans/i.test(font);
  return [name, ...extra, cat ? GENERIC[cat] : serif ? "serif" : "sans-serif"].join(", ");
}

/**
 * Load the faces (plain, bold, italic, bold italic) of these font families before a document's first layout, waiting at most `ms`. Docxtor's
 * stand-ins for Word's fonts (Carlito for Calibri and the others) are web fonts the browser loads only when a layout first needs them, and
 * a long book's open keeps the page busy until its first layout is done: its bold and italic faces arrived after it, and every page was laid
 * out again (600 ms in War and Peace). Families with no web font behind them cost nothing.
 */
export async function loadFaces(families: readonly string[], ms = 400): Promise<void> {
  const fonts = typeof document === "undefined" ? undefined : document.fonts;
  if (!fonts?.load || !families.length) return;
  const loads: Promise<unknown>[] = [];
  // The stylesheets of fonts extensions offer come first: a face loads only once its rule is on the page.
  const end = Date.now() + ms;
  const sheets = families.flatMap((f) => [f, ...standInsFor(f)]).map((f) => wantFont(f));
  await Promise.race([Promise.all(sheets), new Promise((done) => setTimeout(done, ms))]);
  ms = Math.max(0, end - Date.now());
  for (const family of families) {
    const stack = fontStack(family);
    for (const face of ["400", "700", "italic 400", "italic 700"]) loads.push(fonts.load(`${face} 16px ${stack}`).catch(() => undefined));
  }
  await Promise.race([Promise.all(loads), new Promise((done) => setTimeout(done, ms))]);
}

/** Per-font ratio of "single" line height to font size (approximates Word). */
function lineFactor(font?: string): number {
  const k = (font ?? "").toLowerCase();
  if (k.startsWith("calibri") || k.startsWith("carlito")) return 1.22;
  if (k.startsWith("cambria")) return 1.17;
  if (k.startsWith("aptos")) return 1.2;
  if (k.includes("times")) return 1.15;
  if (k.startsWith("georgia")) return 1.14;
  if (k.startsWith("arial") || k.startsWith("helvetica")) return 1.15;
  return 1.17;
}

export { hex };

export function runStyle(f: RunFmt, base?: RunFmt): string {
  const s: string[] = [];
  // East Asian text takes the run's East Asian font.
  const family = f.eaText && f.fontEa ? f.fontEa : f.font;
  if (family && family !== base?.font) s.push(`font-family:${fontStack(family)}`);
  if (f.size && f.size !== base?.size) s.push(`font-size:${f.size}pt`);
  if (f.bold) s.push("font-weight:700");
  else if (base?.bold) s.push("font-weight:400");
  if (f.italic) s.push("font-style:italic");
  s.push(...decorationCss(f));
  const color = hex(f.color);
  if (color && f.color !== base?.color) s.push(`color:${color}`);
  // Automatic colour is black, or white on a dark background; a run that asks for it over a style's colour gets it.
  else if (!color && (f.color === "auto" ? hex(base?.color) : false)) s.push(`color:${inkOf(f)}`);
  else if (!color && isDark(runBackground(f))) s.push("color:#ffffff");
  // A highlight is one of Word's names or a plain colour; anything else in the file (`red;background:url(...)`) is not a colour.
  const highlight = f.highlight && f.highlight !== "none" ? (HIGHLIGHT.get(f.highlight) ?? hex(f.highlight.replace("#", ""))) : undefined;
  if (highlight) s.push(`background-color:${highlight}`);
  else if (!(f.highlight && f.highlight !== "none")) {
    const paint = shdPaint(f.shd, f.shading);
    if (paint.color) s.push(`background-color:${paint.color}`);
    if (paint.image) s.push(`background-image:${paint.image}`, `background-size:${paint.size}`);
  }
  if (f.vertAlign === "superscript") s.push("vertical-align:super;font-size:0.65em");
  if (f.vertAlign === "subscript") s.push("vertical-align:sub;font-size:0.65em");
  if (f.caps) s.push("text-transform:uppercase");
  if (f.smallCaps) s.push("font-variant-caps:small-caps");
  if (f.spacing) s.push(`letter-spacing:${f.spacing / 20}pt`);
  s.push(...runExtraCss(f, base));
  // A run marked right to left reads right to left whatever its characters are.
  if (f.rtl) s.push("direction:rtl;unicode-bidi:embed");
  return s.join(";");
}

export function borderCss(b?: Border): string {
  if (!b || b.val === "nil" || b.val === "none") return "none";
  return borderShorthand(b);
}

export interface RenderEnv {
  engine: Engine;
  /** The document's East Asian line-breaking rules (the lists of characters it sets itself). */
  asian?: AsianSettings;
  /** true for header/footer content (not editable, ids are not addressable) */
  readonly?: boolean;
  /** The person has asked for this document's pictures linked at web addresses to be fetched (they are not until then). */
  linkedPictures?: boolean;
  /** Ask for them: the placeholder's "Show linked pictures" button calls this. */
  showLinkedPictures?: () => void;
}

/**
 * Space before and after a paragraph in twips. Automatic spacing is the 14 points Word uses for HTML; spacing in
 * lines is that many lines of the paragraph's own font, and wins over twips when a file gives both.
 */
export function paraSpace(fmt: ParaFmt, mark: RunFmt): { before: number; after: number } {
  const line = (mark.size ?? 11) * 20 * lineFactor(mark.font);
  const one = (auto: boolean | undefined, lines: number | undefined, twips: number | undefined) =>
    auto ? 280 : lines !== undefined ? Math.round((lines / 100) * line) : twips ?? 0;
  return { before: one(fmt.beforeAuto, fmt.spaceBeforeLines, fmt.spaceBefore), after: one(fmt.afterAuto, fmt.spaceAfterLines, fmt.spaceAfter) };
}

/** Word adds the previous paragraph's space-after to the next one's space-before (no collapsing). */
export function spaceAfter(b?: RBlock): number {
  return b?.t === "p" ? paraSpace(b.fmt, b.mark).after : 0;
}

export interface Spacing {
  /** Space-after (twips) of the previous block, 0 if none. */
  prevAfter: number;
  /** Contextual spacing with the previous paragraph of the same style. */
  contextual: boolean;
  /** The previous paragraph's space-after is automatic: with automatic spacing the two collapse, as they do in HTML. */
  prevAuto?: boolean;
  /** Last block in its container (a table cell keeps its final space-after). */
  last: boolean;
  /** Part of one bordered box with the paragraph before / after it (see `sameBox`). */
  joinPrev?: boolean;
  joinNext?: boolean;
}

/** Which neighbours a bordered paragraph shares its box with: "p" (before), "n" (after), both or neither. */
/**
 * What a block shows the blocks beside it: all `renderBlock` reads of a neighbour (its kind, and a paragraph's style, paragraph
 * formatting and mark, for spacing, contextual spacing, shared borders and frames). Two blocks with the same face draw their
 * neighbours the same; keep this in step with `renderBlockInner`.
 */
export function faceOf(b?: RBlock): string | undefined {
  return b?.t === "p" ? JSON.stringify([b.style, b.fmt, b.mark]) : b?.t;
}

export function boxKey(prev: RBlock | undefined, b: RBlock, next: RBlock | undefined): string {
  return `${sameBox(prev, b) ? "p" : ""}${sameBox(b, next) ? "n" : ""}`;
}

/**
 * Word draws consecutive paragraphs with identical borders and indents as one box: the top border
 * only above the first, the bottom border only below the last, nothing between them.
 */
export function sameBox(a?: RBlock, b?: RBlock): boolean {
  if (a?.t !== "p" || b?.t !== "p") return false;
  const x = a.fmt;
  const y = b.fmt;
  if (!x.borders || !y.borders) return false;
  if ((x.indLeft ?? 0) !== (y.indLeft ?? 0) || (x.indRight ?? 0) !== (y.indRight ?? 0)) return false;
  let any = false;
  for (const side of ["top", "bottom", "left", "right"] as const) {
    const p = x.borders[side];
    const q = y.borders[side];
    if (!p && !q) continue;
    if (!p || !q || p.val !== q.val || p.sz !== q.sz || p.color !== q.color || p.space !== q.space) return false;
    any = true;
  }
  const same = (p?: Border, q?: Border) => (!p && !q) || (!!p && !!q && p.val === q.val && p.sz === q.sz && p.color === q.color && p.space === q.space);
  if (!same(x.borders.between, y.borders.between) || !same(x.borders.bar, y.borders.bar)) return false;
  return any;
}

/**
 * A paragraph's indents in CSS px: at the start and end edge (left and right, swapped in a right-to-left
 * paragraph), the hanging part and the first line's. Indents in characters are that many of the paragraph's font size.
 */
export function paraIndents(fmt: ParaFmt, mark: RunFmt): { start: number; end: number; hang: number; first: number } {
  const unit = (mark.size ?? 11) * PT_PX;
  const len = (chars: number | undefined, twips: number | undefined) => (chars !== undefined ? (chars / 100) * unit : tw(twips));
  return {
    start: len(fmt.indLeftChars, fmt.indLeft),
    end: len(fmt.indRightChars, fmt.indRight),
    hang: len(fmt.indHangingChars, fmt.indHanging),
    first: len(fmt.indFirstLineChars, fmt.indFirstLine),
  };
}

export function paraCss(fmt: ParaFmt, mark: RunFmt, sp: Spacing = { prevAfter: 0, contextual: false, last: false }): string {
  const s: string[] = [];
  const { before, after } = paraSpace(fmt, mark);
  // All spacing lives in margin-top so CSS margin collapsing never applies (and floats can
  // wrap across paragraphs, which a flex layout would prevent).
  // Inside a bordered box the space between paragraphs is padding, so the box has no gaps.
  // Automatic spacing collapses with its neighbour's, as in HTML.
  const auto = fmt.beforeAuto || sp.prevAuto;
  const gap = sp.contextual ? 0 : tw(auto ? Math.max(before, sp.prevAfter) : before + sp.prevAfter);
  s.push(`margin-top:${sp.joinPrev ? 0 : gap}px`);
  s.push(`margin-bottom:${sp.last ? tw(after) : 0}px`);
  const size = mark.size ?? 11;
  const factor = lineFactor(mark.font);
  const rule = fmt.lineRule ?? "auto";
  if (fmt.line && rule === "auto") s.push(`line-height:${((fmt.line / 240) * factor).toFixed(3)}`);
  else if (fmt.line && rule === "exact") s.push(`line-height:${tw(fmt.line)}px`);
  else if (fmt.line && rule === "atLeast") s.push(`line-height:max(${factor}em, ${tw(fmt.line)}px)`);
  else s.push(`line-height:${factor}`);
  // Indents are at the start and end of the line: the left and right in a left-to-right paragraph, swapped in a right-to-left one.
  const rtl = !!fmt.bidi;
  const ind = paraIndents(fmt, mark);
  const left = rtl ? ind.end : ind.start;
  const right = rtl ? ind.start : ind.end;
  if (left) s.push(`padding-left:${left}px`);
  if (right) s.push(`padding-right:${right}px`);
  if (ind.hang) s.push(`text-indent:${-ind.hang}px`);
  else if (ind.first) s.push(`text-indent:${ind.first}px`);
  // `left` and `right` in a file mean the start and the end of the line, so they swap in a right-to-left paragraph.
  const align = { left: "start", start: "start", center: "center", right: "end", end: "end", both: "justify", distribute: "justify" }[fmt.align ?? ""];
  if (align) s.push(`text-align:${align}`);
  // Distributed text spreads the last line too.
  if (fmt.align === "distribute") s.push("text-align-last:justify;text-justify:inter-character");
  if (rtl) s.push("direction:rtl");
  s.push(`font-family:${fontStack(mark.font) ?? "inherit"}`);
  s.push(`font-size:${size}pt`);
  if (hex(mark.color)) s.push(`color:${hex(mark.color)}`);
  if (mark.bold) s.push("font-weight:700");
  if (mark.italic) s.push("font-style:italic");
  const layers: Layer[] = [];
  const b = fmt.borders;
  let padTop = 0;
  // How far each side's border reaches out of the text (its space and width), and the border's own width.
  const reach = new Map<string, number>();
  const bw = new Map<string, number>();
  if (b) {

    for (const side of ["top", "bottom", "left", "right"] as const) {
      const bd = b[side];
      if (!bd) continue;
      const joined = (side === "top" && sp.joinPrev) || (side === "bottom" && sp.joinNext);
      if (joined) {
        // Paragraphs in one box have a line between them only when the file asks for one (the between border).
        if (side === "top") padTop = gap;
        continue;
      }
      const edge = borderEdge(side, bd);
      reach.set(side, edge.width + bd.space * PT_PX);
      bw.set(side, edge.width);
      s.push(edge.css);
      if (edge.layer) layers.push(edge.layer);
      const space = bd.space * PT_PX;
      if (side === "left" || side === "right") {
        // Word draws the border outside the text: it sits `space` (and its own width) beyond the indented
        // text edge, so the text stays where it is without a border.
        s.push(`padding-${side}:${space + (side === "left" ? left : right)}px`);
        s.push(`margin-${side}:${-(space + edge.width)}px`);
      } else s.push(`padding-${side}:${space}px`);
    }
    if (sp.joinPrev && b.between) {
      const edge = borderEdge("top", b.between);
      s.push(edge.css);
      if (edge.layer) layers.push(edge.layer);
      padTop = gap + b.between.space * PT_PX;
    }
    if (padTop) s.push(`padding-top:${padTop}px`);
    else if (sp.joinPrev && !b.top) s.push(`padding-top:${gap}px`);
    if (b.bar) {
      // The bar border: a line along the outside of the text, drawn by the paragraph's ::before.
      const w = borderEdge("left", b.bar).width;
      const x = (b.left ? -(bw.get("left") ?? 0) : left) - b.bar.space * PT_PX - w;
      s.push(`--bar:${borderShorthand(b.bar)};--bar-x:${x}px`);
    }
  }
  // The shading: its colour, then its pattern; both sit under the painted border layers and the bar tab stops.
  const paint = shdPaint(fmt.shd, fmt.shading);
  if (paint.color) s.push(`background-color:${paint.color}`);
  // Automatic text colour turns white on a dark fill.
  if (!hex(mark.color) && isDark(paint.color)) s.push("color:#ffffff");
  const paintLayers: Layer[] = [...layers];
  const inkCss = hex(mark.color) ?? "#000";
  for (const t of fmt.tabs ?? []) {
    if (t.val !== "bar") continue;
    // From the margin: the left or right edge of the box, which a border on that side pushes out.
    const x = tw(t.pos) + (reach.get(rtl ? "right" : "left") ?? 0);
    paintLayers.push({ image: `linear-gradient(${inkCss},${inkCss})`, size: "1px 100%", position: `${rtl ? "right" : "left"} ${x}px top`, repeat: "no-repeat" });
  }
  if (paint.image) paintLayers.push({ image: paint.image, size: paint.size!, position: "0 0", repeat: "repeat" });
  s.push(...eaParaCss(fmt));
  if (paintLayers.length) {
    s.push(`background-image:${paintLayers.map((l) => l.image).join(",")}`);
    s.push(`background-size:${paintLayers.map((l) => l.size).join(",")}`);
    s.push(`background-position:${paintLayers.map((l) => l.position).join(",")}`);
    s.push(`background-repeat:${paintLayers.map((l) => l.repeat).join(",")}`);
    s.push("background-origin:border-box");
  }
  return s.join(";");
}

/** Size and placement of a picture or chart: how it wraps, and where a floating one sits. */
function placeObject(el: HTMLElement, im: RImage) {
  el.dataset.w = String(im.w);
  el.dataset.h = String(im.h);
  el.dataset.wrap = im.wrap || "inline";
  if (im.align) el.dataset.align = im.align;
  const spec = { wrap: im.wrap, align: im.align, alignY: im.alignY, x: im.x, y: im.y, relX: im.relX, relY: im.relY, z: im.z, w: im.w, dist: im.place?.dist };
  const place = placeCss(spec);
  if (place) el.dataset.place = place;
  floatData(el, { ...spec, w: im.w, h: im.h });
  // Room for what turns or sticks out of the picture: Word keeps it clear of the text around a picture in line.
  const ee = im.pic?.extent;
  const box = el.querySelector<HTMLElement>(".pic");
  if (ee && box && im.wrap === "inline") box.style.margin = `${ee[1] * PT_PX}px ${ee[2] * PT_PX}px ${ee[3] * PT_PX}px ${ee[0] * PT_PX}px`;
}

function segEl(seg: RSeg, base: RunFmt, env: RenderEnv, piece?: Piece): HTMLElement {
  // A shape's text has its own formatting: the run it sits in doesn't reach into it.
  // Two lines in one, text upright in a vertical line, the grid's character pitch, the language of East Asian text.
  const ea = seg.k === "t" ? eaSegPiece(seg, seg.f.size ?? base.size ?? 11) : null;
  const style = seg.k === "shape" ? "" : [runStyle(seg.f, base), ...(piece?.css ?? []), ...(ea?.css ?? [])].filter(Boolean).join(";");
  let el: HTMLElement;
  switch (seg.k) {
    case "t":
    case "tab":
    case "br": {
      el = document.createElement("span");
      el.className = seg.k === "tab" ? "s ftab" : "s";
      el.textContent = seg.k === "t" ? seg.x ?? "" : seg.k === "tab" ? "\t" : "\n";
      break;
    }
    case "img": {
      const im = seg.img!;
      el = document.createElement("span");
      el.className = `atom img wrap-${im.wrap || "inline"}`;
      // A picture that is only linked has nothing in the file to show: a placeholder with the file's name, unless the
      // link is an address the browser can fetch.
      // A link to a web address is fetched only after the person asks (the address learns who opened the document).
      const web = (u?: string) => !!u && /^https?:/i.test(u);
      const link = im.pic?.link ?? (web(im.src) ? im.src : undefined);
      const show = web(link) && !env.linkedPictures ? env.showLinkedPictures : undefined;
      const url = link ? (web(link) && env.linkedPictures ? link : "") : env.engine.mediaUrl(im.pic?.svg ?? im.src);
      if (!url) el.append(placeholder(im, link ?? im.src, !link, show));
      else {
        const node = pictureNode(im, url);
        if (link) node.querySelector("img")?.addEventListener("error", () => node.replaceWith(placeholder(im, link)));
        el.append(node);
      }
      placeObject(el, im);
      break;
    }
    case "chart": {
      // A chart sits in the page like a picture: same size, wrapping and placement, drawn from its data.
      const im = seg.img!;
      el = document.createElement("span");
      el.className = `atom img chart wrap-${im.wrap || "inline"}`;
      if (seg.chartex) {
        // The new chart types (waterfall, histogram, treemap...) have their own model and drawing.
        fillChartEx(el, seg.chartex, im);
        placeObject(el, im);
        break;
      }
      el.append(drawChart(seg.chart!, im.w, im.h), chartTable(seg.chart!));
      el.dataset.chart = seg.chart!.kind;
      el.dataset.alt = im.alt ?? chartLabel(seg.chart!);
      placeObject(el, im);
      registerChart(el, seg.chart!);
      break;
    }
    case "math": {
      // One position in the paragraph, like an image; the engine's MathML is rebuilt from an allow-list.
      const m = seg.math!;
      el = document.createElement("span");
      el.className = m.display ? "atom math display" : "atom math";
      el.dataset.latex = m.latex;
      if (m.display) el.dataset.jc = m.jc ?? "center";
      const node = mathNode(m.mathml);
      if (node) el.append(node);
      else el.textContent = m.latex;
      el.title = t("Equation. Double-click to edit.");
      break;
    }
    case "shape": {
      el = shapeAtom(seg.shape!, env, (blocks) => renderBlocks(blocks, env));
      break;
    }
    case "ruby": {
      // A phonetic guide: one position in the paragraph, its base text with the guide above it.
      el = rubyAtom(seg, fontStack, runStyle);
      el.title = t("Phonetic guide. Double-click to edit.");
      break;
    }
    case "sym": {
      // One character of a symbol font, drawn as the Unicode character that looks like it. It takes the place the original did.
      el = document.createElement("span");
      el.className = "atom sym";
      el.textContent = seg.x ?? "";
      break;
    }
    case "val": {
      // A page number or part of today's date that Word keeps as an element: one place, its text filled in by the layout.
      el = document.createElement("span");
      el.className = "atom val";
      el.textContent = seg.x ?? "";
      break;
    }
    case "eq": {
      // An EQ field, drawn as an equation. It has no text of its own, so it takes no place in the paragraph.
      el = document.createElement("span");
      el.className = "ghost eq";
      el.contentEditable = "false";
      const node = mathNode(seg.math!.mathml);
      if (node) el.append(node);
      else el.textContent = seg.math!.latex;
      el.title = t("Equation field. Show field codes to edit it.");
      el.dataset.o = String(seg.o);
      if (seg.fi) el.dataset.fi = seg.fi;
      if (style) el.setAttribute("style", style);
      return el;
    }
    case "ff": {
      // A legacy form field: a check box, drawn as a box that is checked or not. It has no text, so it takes no place.
      el = document.createElement("span");
      el.className = "ghost ff";
      el.contentEditable = "false";
      el.dataset.ff = seg.label ?? "checkbox";
      el.dataset.checked = seg.x === "1" ? "1" : "0";
      el.dataset.o = String(seg.o);
      // Which form field of its paragraph it is: the controls tool ticks it with an op on that field.
      if (seg.fi !== undefined) el.dataset.frm = seg.fi;
      el.dataset.fk = "checkbox";
      el.setAttribute("role", "checkbox");
      el.setAttribute("aria-checked", String(seg.x === "1"));
      el.setAttribute("aria-label", t("Check box"));
      if (style) el.setAttribute("style", style);
      return el;
    }
    case "ghost": {
      el = document.createElement("span");
      el.className = `ghost ${seg.label ?? ""}`;
      el.contentEditable = "false";
      el.textContent = seg.x ?? "";
      el.title = t("Tracked deletion. Docxtor keeps it as it is.");
      break;
    }
    case "note": {
      // Footnote reference (or a note's own mark, which has no id).
      el = document.createElement("span");
      el.className = "atom note";
      el.textContent = seg.label ?? "";
      // A custom mark follows in the text: that is the visible mark, so this one shows nothing.
      if (!seg.label) el.classList.add("custom");
      if (seg.x) el.dataset.note = seg.x;
      if (seg.note === "endnote") {
        el.classList.add("endnote");
        el.dataset.nk = "endnote";
      }
      el.title = seg.x ? (seg.note === "endnote" ? t("Endnote. Double-click to edit.") : t("Footnote. Double-click to edit.")) : "";
      break;
    }
    case "hrule": {
      // A horizontal line: a block across the column (or part of it), in its own colour.
      const r = seg.hrule!;
      el = document.createElement("span");
      el.className = "atom hrule";
      el.title = t("Horizontal line");
      // It sits in the middle of its line, which keeps the paragraph's height; a narrower one is placed by its alignment.
      const width = r.pct !== undefined ? `${r.pct}%` : `${r.w! * PT_PX}px`;
      const shift = r.align === "left" ? "0px" : `calc((100% - ${width})${r.align === "right" ? "" : " / 2"})`;
      el.dataset.place = [`width:${width}`, `height:${Math.max(1, r.h * PT_PX)}px`, `background:${hex(r.color) ?? "#A0A0A0"}`, `margin-inline-start:${shift}`].join(";");
      break;
    }
    case "pbr": {
      el = document.createElement("span");
      el.className = "atom pbr";
      el.title = t("Page break");
      break;
    }
    case "cbr": {
      // One character, like the line break it replaces. In a one-column section Word treats it as a page break.
      el = document.createElement("span");
      el.className = "atom cbr";
      el.title = t("Column break");
      break;
    }
    default: {
      el = document.createElement("span");
      const label = seg.label ?? "";
      if (label.startsWith("^")) {
        el.className = "atom note";
        el.textContent = label.slice(1);
      } else if (label === "\t") {
        // An absolute position tab (`ptab:alignment:relativeTo:leader`) or a tab object.
        el.className = seg.x?.startsWith("ptab:") ? "atom fptab" : "atom";
        if (seg.x?.startsWith("ptab:")) el.dataset.ptab = seg.x.slice(5);
        el.textContent = "\t";
      } else {
        el.className = "atom obj";
        el.textContent = label || t("Object");
        el.title = t("Docxtor keeps this object intact when saving.");
      }
    }
  }
  if (ea) {
    el.classList.add(...ea.cls);
    for (const [k, v] of Object.entries(ea.attrs)) el.setAttribute(k, v);
  }
  if (el.classList.contains("atom")) el.contentEditable = "false";
  if (seg.k === "ghost") return el;
  el.dataset.o = String(seg.o);
  // A hidden object stays in the text as its one character but is not drawn or printed.
  if (seg.hidden) el.classList.add("obj-hidden");
  if (style) el.setAttribute("style", style);
  if (el.dataset.place) el.setAttribute("style", `${style ? style + ";" : ""}${el.dataset.place}`);
  if (seg.link) {
    el.classList.add("lnk");
    el.dataset.href = seg.link;
    const internal = seg.link.startsWith("#");
    el.title = internal ? t("⌘-click or Ctrl-click to go there") : `${seg.link}\n${t("⌘-click or Ctrl-click to open")}`;
    // Word shows only styled links in colour. Docxtor also colours unstyled web links, but not links
    // within the document (a table of contents, cross-references), which read as plain text in Word.
    if (!seg.f.color && !internal) el.classList.add("lnk-default");
  }
  if (seg.fld) el.dataset.fld = seg.fld;
  if (seg.fi) el.dataset.fi = seg.fi;
  // Text in a field's code (the result of a field nested in its instruction) keeps its place but isn't shown.
  if (seg.hid) el.classList.add("fhid");
  // A citation is one unit: it is outlined on hover, and a click selects all of it (editor.ts).
  if (seg.cite?.length) {
    el.classList.add("cite");
    el.dataset.cite = seg.cite.join(" ");
    el.title = t("Citation. Click to edit it.");
  }
  if (seg.ins) el.classList.add("ins");
  if (piece?.cls.length) el.classList.add(...piece.cls);
  // Hidden text is not drawn unless the document shows it (View, Show hidden text).
  if (seg.f.vanish && seg.k !== "shape") el.classList.add("hid");
  // "Do not check spelling or grammar": the checkers leave this text alone.
  if (seg.f.noProof) el.dataset.np = "1";
  return el;
}

/** A rough width of a list label in px, to tell which labels might outgrow the hanging indent. */
function labelWidth(p: RPara): number {
  const l = p.list;
  return l ? Math.round(l.text.length * 0.55 * (l.size ?? p.mark.size ?? 11) * PT_PX) : 0;
}

export function renderPara(p: RPara, env: RenderEnv, sp?: Spacing): HTMLElement {
  const el = document.createElement("div");
  el.className = "fp";
  if (!env.readonly) el.dataset.p = String(p.id);
  el.dataset.id = String(p.id);
  if (p.style) el.dataset.style = p.style;
  // Headings carry their outline level (1 to 9): the folding arrows and the Outline view read it.
  const outline = outlineLevel(p);
  if (outline) el.dataset.ol = String(outline);
  // The language of its text, for the spelling and grammar checkers.
  if (p.lang) el.dataset.lang = p.lang;
  el.setAttribute("style", paraCss(p.fmt, p.mark, sp));
  if (p.fmt.pageBreakBefore) el.dataset.pbb = "1";
  if (p.fmt.keepNext) el.dataset.kn = "1";
  if (p.fmt.keepLines) el.dataset.kl = "1";
  if (p.fmt.widowControl === false) el.dataset.wc = "0";
  if (p.fmt.suppressLineNumbers) el.dataset.nln = "1";
  if (p.fmt.suppressAutoHyphens) el.dataset.nh = "1";
  if (p.fmt.snapToGrid === false) el.dataset.sg = "0";
  if (p.fmt.line && p.fmt.lineRule === "exact") el.dataset.lx = "1";
  if (p.sectBreak && p.sectBreak !== "continuous") el.dataset.sect = p.sectBreak;
  // Every kind of section break (the non-printing marks name it).
  if (p.sectBreak) el.dataset.sb = p.sectBreak;
  if (p.list) {
    const l = p.list;
    // The list and level, so the ruler can move every item of a level together.
    el.dataset.ln = `${l.numId}:${l.level}`;
    const lbl = document.createElement("span");
    lbl.className = "lbl";
    lbl.contentEditable = "false";
    if (l.checklist) {
      // On screen the box is drawn (the glyph's font may not exist on this device); the .docx keeps ☐ and ☑. avoid-ai-design-ignore: I2 (document characters, not interface icons)
      const box = document.createElement("span");
      box.className = "box";
      if (l.checked) box.innerHTML = '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 8.6 3 3 6-6.6" vector-effect="non-scaling-stroke"/></svg>';
      lbl.append(box);
    } else if (l.pic) {
      // A picture bullet: the picture at the size the list gives it.
      const img = document.createElement("img");
      img.className = "lbl-pic";
      img.alt = "";
      img.src = env.engine.mediaUrl(l.pic.src);
      img.style.width = `${l.pic.w}pt`;
      img.style.height = `${l.pic.h}pt`;
      lbl.append(img);
    } else lbl.textContent = l.text;
    if (l.checklist) {
      // The box ticks with a click (see Editor); a screen reader hears a checkbox.
      lbl.setAttribute("role", "checkbox");
      lbl.setAttribute("aria-checked", String(!!l.checked));
      lbl.setAttribute("aria-label", t("Done"));
      lbl.classList.add("lbl-check");
    }
    const hang = paraIndents(p.fmt, p.mark).hang;
    const st: string[] = [];
    // The label's text sits in a box of its own, so a highlight or an underline covers the numeral and not the
    // space that follows it. A label right- or centre-aligned in Word sits on the number position (where the
    // hanging indent starts), not after it: a zero-width outer box there, with the label shifted back by all of
    // its width or half of it.
    const placed = !!l.align && !l.checklist && !l.pic;
    if (!l.checklist && !l.pic) {
      const inner = document.createElement("span");
      inner.className = "lbl-in";
      const marks = runStyle({ italic: l.italic, underline: l.underline, strike: l.strike, highlight: l.highlight, caps: l.caps, smallCaps: l.smallCaps });
      const facing = [marks, l.italic === false ? "font-style:normal" : ""];
      if (placed) facing.push(l.align === "right" ? "transform:translateX(-100%)" : "transform:translateX(-50%)");
      inner.setAttribute("style", facing.filter(Boolean).join(";"));
      inner.append(...Array.from(lbl.childNodes));
      lbl.append(inner);
    }
    if (placed) {
      lbl.classList.add("lbl-placed");
      if (l.suffix === "tab") st.push(`margin-right:${Math.max(hang, 0)}px`);
    } else if (l.suffix === "tab") {
      st.push(`min-width:${Math.max(hang, 0)}px`);
      // The tab layout widens a label that is wider than the hanging indent (see tabs.ts).
      lbl.dataset.sfx = "tab";
      lbl.dataset.est = String(labelWidth(p));
    }
    if (l.suffix === "space") st.push("padding-right:0.3em");
    const own = runStyle({ font: l.font, size: l.size, color: l.color, bold: l.bold });
    if (own) st.push(own);
    // A level can turn off what the paragraph's style turns on.
    if (l.bold === false) st.push("font-weight:400");
    lbl.setAttribute("style", st.join(";"));
    el.append(lbl);
  }
  if (p.bookmarks?.length) el.dataset.bm = p.bookmarks.join(" ");
  // A horizontal line: an empty paragraph whose bottom border is the rule.
  if (p.hr) el.dataset.hr = "1";
  // Tabs are measured after layout (see tabs.ts): the paragraph carries the stops it needs for that.
  const ind = paraIndents(p.fmt, p.mark);
  const hasTab = p.segs.some((s) => s.k === "tab" || (s.k === "obj" && !!s.x?.startsWith("ptab:")));
  const labelTab = !!p.list && p.list.suffix === "tab" && !p.list.checklist && labelWidth(p) > ind.hang * 0.8;
  if (hasTab || labelTab) el.dataset.tl = JSON.stringify(tabPlan(p.fmt, p.fmt.bidi ? ind.end : ind.start, p.fmt.bidi ? ind.start : ind.end, ind.hang));
  if (p.fmt.mirrorIndents) el.dataset.mi = "1";
  if (p.fmt.borders?.bar) el.dataset.bar = "1";
  let hasPbr = false;
  let hasCbr = false;
  for (const piece of kinsokuPieces(prepareSegs(p.segs, p.mark, fontStack), p.fmt, env.asian)) {
    const seg = piece.seg;
    // A drop-down form field has no text: it is drawn at its place and takes no position.
    if (seg.k === "form") {
      el.append(formAtom(seg, p.forms));
      continue;
    }
    el.append(segEl(seg, p.mark, env, piece));
    if (seg.k === "pbr") hasPbr = true;
    if (seg.k === "cbr") hasCbr = true;
  }
  // The paginator treats a column break like a page break (there is one column); the classes stay apart
  // so multi-column layout can tell them apart later.
  if (hasPbr || hasCbr) el.dataset.pbr = "1";
  decorateControls(el, p);
  if (hasCbr) el.dataset.cbr = "1";
  // A paragraph of hidden text only keeps its line, as in Word.
  const onlyHidden = p.segs.length > 0 && p.segs.every((x) => x.f.vanish && x.k === "t");
  if (p.len === 0 || onlyHidden || p.segs[p.segs.length - 1]?.k === "br") {
    // Keep empty lines (and a trailing line break) visible and caret-able.
    const br = document.createElement("br");
    br.dataset.ph = "1";
    el.append(br);
  }
  return el;
}

export function renderBlock(b: RBlock, env: RenderEnv, prev?: RBlock, last = false, next?: RBlock): HTMLElement {
  return recordBase(renderBlockInner(b, env, prev, last, next));
}

function renderBlockInner(b: RBlock, env: RenderEnv, prev?: RBlock, last = false, next?: RBlock): HTMLElement {
  switch (b.t) {
    case "p": {
      const contextual = !!b.fmt.contextualSpacing && prev?.t === "p" && prev.style === b.style;
      const el = renderPara(b, env, { prevAfter: spaceAfter(prev), prevAuto: prev?.t === "p" && !!prev.fmt.afterAuto, contextual, last, joinPrev: sameBox(prev, b), joinNext: sameBox(b, next) });
      if (b.fmt.borders) el.dataset.bj = boxKey(prev, b, next);
      // A frame or drop cap floats beside the text after it, and starts level with it.
      if (b.fmt.frame) {
        // A drop cap spans as many lines as the text beside it has: measure by that paragraph's own line height.
        const auto = next?.t === "p" && !!next.fmt.line && (next.fmt.lineRule ?? "auto") === "auto";
        const pitch = next?.t === "p" ? ((next.mark.size ?? 11) * PT_PX * lineFactor(next.mark.font) * (auto ? next.fmt.line! / 240 : 1)) : 0;
        const exact = next?.t === "p" && next.fmt.line && next.fmt.lineRule === "exact" ? tw(next.fmt.line) : pitch;
        applyFrame(el, b, next?.t === "p" ? tw(paraSpace(next.fmt, next.mark).before + spaceAfter(prev)) : 0, exact);
      }
      return el;
    }
    case "tbl": {
      return tableWrap(b, env);
    }
    case "c": {
      const el = document.createElement("div");
      el.className = "fc";
      el.dataset.id = String(b.id);
      if (b.gallery) el.dataset.gallery = b.gallery;
      if (b.control && b.control.kind !== "bibliography" && b.control.kind !== "docPart") {
        // A block-level content control: the controls tool finds it by these.
        el.classList.add("fc-ctl");
        el.dataset.ctlId = String(b.id);
        el.dataset.ck = b.control.kind;
        if (b.control.placeholder) el.classList.add("ctl-ph");
        if (b.control.lock) el.classList.add("ctl-locked");
        if (hex(b.control.color)) el.style.setProperty("--ctl-color", hex(b.control.color)!);
        if (!b.label) {
          const chip = document.createElement("div");
          chip.className = "fc-label";
          chip.contentEditable = "false";
          chip.textContent = controlName(b.control);
          el.append(chip);
        }
      }
      if (b.label) {
        const chip = document.createElement("div");
        chip.className = "fc-label";
        chip.contentEditable = "false";
        chip.textContent = b.gallery === "Bibliographies" ? t("Bibliography") : b.label;
        el.append(chip);
      }
      for (const c of renderBlocks(b.blocks, env)) el.append(c);
      return el;
    }
    case "x": {
      const el = document.createElement("div");
      el.className = "fx";
      el.contentEditable = "false";
      el.dataset.id = String(b.id);
      el.textContent = b.label ? t("{what}, kept as is", { what: b.label }) : t("Content Docxtor can't show yet, kept as is");
      el.title = t("Docxtor can't display this yet, but keeps it unchanged when saving.");
      return el;
    }
  }
}

/** Base margins of a rendered block, recorded so pagination can always reset them exactly. */
function recordBase(el: HTMLElement): HTMLElement {
  el.dataset.base = el.style.marginTop || "0px";
  // Side margins a block has of its own (a bordered paragraph reaches outside its text): the pages add
  // their section's margins to them when sections differ in width (see the paginator).
  if (el.style.marginLeft) el.dataset.bl = String(parseFloat(el.style.marginLeft) || 0);
  if (el.style.marginRight) el.dataset.br = String(parseFloat(el.style.marginRight) || 0);
  return el;
}

export function renderBlocks(blocks: RBlock[], env: RenderEnv): HTMLElement[] {
  // A frame floats beside the text: the blocks around it are each other's neighbours as if it were not there.
  const isFrame = (b?: RBlock) => b?.t === "p" && !!b.fmt.frame;
  const prevReal = (i: number) => {
    let j = i - 1;
    while (j >= 0 && isFrame(blocks[j])) j--;
    return blocks[j];
  };
  const nextReal = (i: number) => {
    let j = i + 1;
    while (j < blocks.length && isFrame(blocks[j])) j++;
    return blocks[j];
  };
  return blocks.map((b, i) => renderBlock(b, env, prevReal(i), nextReal(i) === undefined, nextReal(i)));
}
