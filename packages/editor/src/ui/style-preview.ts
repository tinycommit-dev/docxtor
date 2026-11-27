// How a style looks, for the gallery, the Styles panel and the Modify style dialog: the resolved formatting the engine
// reports, patches laid over it while a style is being edited, CSS for a tile, and the plain-language description of
// a style ("Font: Calibri, 11 pt, Bold").
import type { ParaFmt, ParaPatch, RunFmt, RunPatch } from "@docxtor/core";
import { msg, t, tn } from "@docxtor/extensions";
import { fontStack, runStyle } from "../editor/render";

// Word's built-in style names, so they are translated (listed here for the extractor; `styleTitle` looks them up).
const BUILTIN_NAMES = [
  msg("Normal"), msg("Title"), msg("Subtitle"), msg("Heading 1"), msg("Heading 2"), msg("Heading 3"), msg("Heading 4"), msg("Heading 5"),
  msg("Heading 6"), msg("Heading 7"), msg("Heading 8"), msg("Heading 9"), msg("Quote"), msg("Intense Quote"), msg("Emphasis"), msg("Strong"),
  msg("Subtle Emphasis"), msg("Intense Emphasis"), msg("Subtle Reference"), msg("Intense Reference"), msg("Book Title"), msg("No Spacing"),
  msg("List Paragraph"), msg("Caption"), msg("Header"), msg("Footer"), msg("Hyperlink"), msg("TOC Heading"), msg("Footnote Text"),
  msg("Footnote Reference"), msg("Endnote Text"), msg("Endnote Reference"), msg("Table of Figures"), msg("Index Heading"), msg("Bibliography"),
  msg("Body Text"), msg("Default Paragraph Font"),
];

/** The name a style is shown with: Word's built-in names (written `heading 1` in the file) with capitals and translated, a person's own as they wrote them. */
export function styleTitle(name: string, custom: boolean | undefined): string {
  if (custom) return name;
  const shown = name
    .split(" ")
    .map((w) => (w === "toc" ? "TOC" : w.charAt(0).toUpperCase() + w.slice(1)))
    .join(" ");
  return BUILTIN_NAMES.includes(shown) ? t(shown) : shown;
}

const hex = (c: string | undefined) => (c ? c.replace(/^#/, "").toUpperCase() : undefined);

/** `base` with a run patch laid over it, as the engine would write it (a rough copy of its effect, for previews). */
export function applyRun(base: RunFmt, p: RunPatch | undefined): RunFmt {
  if (!p) return base;
  const r: RunFmt = { ...base };
  if (p.clear) return {};
  if (p.bold !== undefined) r.bold = p.bold;
  if (p.italic !== undefined) r.italic = p.italic;
  if (p.underline !== undefined) r.underline = p.underline === "none" || p.underline === "" ? undefined : p.underline;
  if (p.strike !== undefined) r.strike = p.strike;
  if (p.caps !== undefined) r.caps = p.caps;
  if (p.smallCaps !== undefined) r.smallCaps = p.smallCaps;
  if (p.font !== undefined) r.font = p.font || undefined;
  if (p.size !== undefined) r.size = p.size;
  if (p.color !== undefined) r.color = p.color === "auto" || p.color === "" ? undefined : hex(p.color);
  if (p.highlight !== undefined) r.highlight = p.highlight === "none" ? undefined : p.highlight;
  if (p.vertAlign !== undefined) r.vertAlign = p.vertAlign === "baseline" ? undefined : p.vertAlign;
  return r;
}

/** `base` with a paragraph patch laid over it (a rough copy of the engine's effect, for previews). */
export function applyPara(base: ParaFmt, p: ParaPatch | undefined): ParaFmt {
  if (!p) return base;
  if (p.clear) return {};
  const r: ParaFmt = { ...base };
  if (p.align !== undefined) r.align = p.align || undefined;
  if (p.spaceBefore !== undefined) (r.spaceBefore = p.spaceBefore), delete r.spaceBeforeLines, delete r.beforeAuto;
  if (p.spaceAfter !== undefined) (r.spaceAfter = p.spaceAfter), delete r.spaceAfterLines, delete r.afterAuto;
  if (p.beforeAuto !== undefined) r.beforeAuto = p.beforeAuto;
  if (p.afterAuto !== undefined) r.afterAuto = p.afterAuto;
  if (p.lineMultiple !== undefined) (r.line = Math.round(p.lineMultiple * 240)), (r.lineRule = "auto");
  if (p.line !== undefined) (r.line = p.line), (r.lineRule = p.lineRule ?? "auto");
  if (p.indLeft !== undefined) (r.indLeft = p.indLeft), delete r.indLeftChars;
  if (p.indRight !== undefined) (r.indRight = p.indRight), delete r.indRightChars;
  if (p.indFirst !== undefined) {
    delete r.indFirstLineChars;
    delete r.indHangingChars;
    r.indFirstLine = p.indFirst > 0 ? p.indFirst : undefined;
    r.indHanging = p.indFirst < 0 ? -p.indFirst : undefined;
  }
  for (const k of ["keepNext", "keepLines", "pageBreakBefore", "widowControl", "suppressLineNumbers", "suppressAutoHyphens", "contextualSpacing", "mirrorIndents", "bidi"] as const) {
    if (p[k] !== undefined) r[k] = p[k];
  }
  if (p.outlineLvl !== undefined) r.outlineLvl = p.outlineLvl < 0 ? undefined : p.outlineLvl;
  if (p.tabs !== undefined) r.tabs = p.tabs.filter((s) => s.val !== "clear");
  if (p.shading !== undefined) r.shading = p.shading || undefined;
  if (p.shd !== undefined) r.shd = p.shd.val === "nil" ? undefined : p.shd;
  if (p.borders !== undefined) r.borders = p.borders;
  if (p.num !== undefined) (r.numId = p.num[0] || undefined), (r.ilvl = p.num[1]);
  return r;
}

/** Merge patches: what is set later wins. */
export function mergePatch<T extends object>(base: T | undefined, more: T | undefined): T | undefined {
  if (!more) return base;
  return { ...(base ?? {}), ...more };
}

/** The look of a style as CSS on a white tile: its font, weight, colour and so on, with the size held to what fits a line. */
export function tileCss(run: RunFmt, para: ParaFmt, o: { min?: number; max?: number } = {}): string {
  const css = [runStyle({ ...run, size: undefined, vertAlign: undefined })];
  const family = fontStack(run.font);
  if (family && !css[0].includes("font-family")) css.push(`font-family:${family}`);
  const pt = run.size ?? 11;
  css.push(`font-size:clamp(${o.min ?? 11}px, ${pt}pt, ${o.max ?? 19}px)`);
  const align = { center: "center", right: "right", both: "justify", end: "right" }[para.align ?? ""];
  if (align) css.push(`text-align:${align}`);
  return css.filter(Boolean).join(";");
}

const points = (twips: number | undefined) => (twips === undefined ? "" : `${Math.round((twips / 20) * 100) / 100}`);

/** The colours Word's palette names, so a description can say "Font color: Red" and not only a number. */
export function describeColor(color: string | undefined): string {
  return color ? `#${color}` : t("Automatic");
}

/**
 * What a style does, in a few short clauses (Word shows the same under the dialog's preview). `run` and `para` are the
 * resolved look: the document defaults, the styles it builds on and the style itself.
 */
export function describeStyle(run: RunFmt, para: ParaFmt, o: { basedOn?: string; next?: string; kind?: string } = {}): string {
  const bits: string[] = [];
  const font = [run.font, run.size ? t("{n} pt", { n: String(run.size) }) : "", run.bold ? t("Bold") : "", run.italic ? t("Italic") : "", run.underline ? t("Underline") : "", run.strike ? t("Strikethrough") : "", run.caps ? t("All caps") : "", run.smallCaps ? t("Small caps") : ""].filter(Boolean);
  if (font.length) bits.push(t("Font: {list}", { list: font.join(", ") }));
  if (run.color) bits.push(t("Font color: {color}", { color: describeColor(run.color) }));
  const aligns: Record<string, string> = { center: t("Centered"), right: t("Right"), end: t("Right"), both: t("Justified"), distribute: t("Distributed") };
  if (para.align && aligns[para.align]) bits.push(t("Alignment: {align}", { align: aligns[para.align] }));
  if (para.spaceBefore !== undefined || para.spaceAfter !== undefined) {
    bits.push(t("Space before: {before} pt, after: {after} pt", { before: points(para.spaceBefore) || "0", after: points(para.spaceAfter) || "0" }));
  }
  if (para.line) {
    const exact = para.lineRule === "exact" || para.lineRule === "atLeast";
    bits.push(exact ? t("Line spacing: {rule} {n} pt", { rule: para.lineRule === "exact" ? t("exactly") : t("at least"), n: points(para.line) }) : t("Line spacing: {n} lines", { n: String(Math.round((para.line / 240) * 100) / 100) }));
  }
  if (para.indLeft) bits.push(t("Left indent: {n} pt", { n: points(para.indLeft) }));
  if (para.indRight) bits.push(t("Right indent: {n} pt", { n: points(para.indRight) }));
  if (para.indFirstLine) bits.push(t("First line: {n} pt", { n: points(para.indFirstLine) }));
  if (para.indHanging) bits.push(t("Hanging: {n} pt", { n: points(para.indHanging) }));
  if (para.keepNext) bits.push(t("Keep with next"));
  if (para.keepLines) bits.push(t("Keep lines together"));
  if (para.pageBreakBefore) bits.push(t("Page break before"));
  if (para.outlineLvl !== undefined && para.outlineLvl < 9) bits.push(t("Outline level {n}", { n: String(para.outlineLvl + 1) }));
  if (para.numId) bits.push(t("In a list"));
  if (para.borders && (para.borders.top || para.borders.bottom || para.borders.left || para.borders.right)) bits.push(t("Borders"));
  if (para.shading) bits.push(t("Shading: #{color}", { color: para.shading }));
  if (para.tabs?.length) bits.push(tn(para.tabs.length, "{n} tab stop", "{n} tab stops"));
  if (o.basedOn) bits.push(t("Based on: {style}", { style: o.basedOn }));
  if (o.next) bits.push(t("Following style: {style}", { style: o.next }));
  return bits.join(", ");
}
