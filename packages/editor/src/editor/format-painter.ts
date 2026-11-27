// Copy formatting and the format painter. What is copied is the text formatting at the selection and the
// formatting of its paragraph (style, alignment, indents, spacing, list). Applied to a whole paragraph, or to
// a selection that spans paragraphs, both go on; applied to part of a paragraph, only the text formatting does.
import type { FormatState, MPos, Op, ParaFmt, ParaPatch, RunPatch } from "@docxtor/core";
import type { Editor } from "./editor";
import { paraLength } from "./selection";

export interface CopiedFormat {
  run: RunPatch;
  /** The paragraph style id. */
  style: string;
  /** What the paragraph looked like at the source (style and direct formatting together). */
  para: ParaFmt;
  /** The list the paragraph was in. */
  list: { numId: number; ilvl: number } | null;
}

/** The formatting at the selection, with every text property spelled out so pasting also turns off what the source doesn't have. */
export function captureFormat(f: FormatState | null): CopiedFormat | null {
  if (!f) return null;
  const r = f.run;
  const run: RunPatch = {
    bold: !!r.bold,
    italic: !!r.italic,
    underline: r.underline ?? "none",
    strike: !!r.strike,
    caps: !!r.caps,
    smallCaps: !!r.smallCaps,
    vertAlign: r.vertAlign ?? "baseline",
    color: r.color ?? "auto",
    highlight: r.highlight ?? "none",
    ...(r.font ? { font: r.font } : {}),
    ...(r.size ? { size: r.size } : {}),
    // The rest of the character formatting: underline colour, effects, spacing, position, scale, kerning, shading, a box.
    dstrike: !!r.dstrike,
    vanish: !!r.vanish,
    outline: !!r.outline,
    shadow: !!r.shadow,
    emboss: !!r.emboss,
    imprint: !!r.imprint,
    underlineColor: r.underlineColor ?? "auto",
    spacing: r.spacing ?? 0,
    position: r.position ?? 0,
    scale: r.scale ?? 100,
    kern: r.kern ?? 0,
    em: r.em ?? "none",
    shading: r.shading ?? "none",
    border: r.border ?? { val: "none", sz: 0, color: "auto", space: 0 },
    ligatures: r.fx?.ligatures ?? "default",
    numForm: r.fx?.numForm ?? "default",
    numSpacing: r.fx?.numSpacing ?? "default",
    styleSets: r.fx?.styleSets ?? [],
    cntxtAlts: !!r.fx?.cntxtAlts,
    // A theme colour goes with its hex, as it is written.
    ...(r.colorTheme ? { themeColor: r.colorTheme.name, ...(r.colorTheme.tint ? { themeTint: r.colorTheme.tint } : {}), ...(r.colorTheme.shade ? { themeShade: r.colorTheme.shade } : {}) } : {}),
    ...(r.underlineTheme ? { underlineTheme: r.underlineTheme.name, ...(r.underlineTheme.tint ? { underlineTint: r.underlineTheme.tint } : {}), ...(r.underlineTheme.shade ? { underlineShade: r.underlineTheme.shade } : {}) } : {}),
  };
  const p = f.para;
  return { run, style: f.style ?? "", para: p, list: p.numId && f.list ? { numId: p.numId, ilvl: p.ilvl ?? 0 } : null };
}

/** The paragraph properties that differ between what a target has and what the source had. */
function difference(src: ParaFmt, now: ParaFmt): ParaPatch {
  const out: ParaPatch = {};
  const align = (a?: string) => (a === "start" ? "left" : a === "end" ? "right" : (a ?? "left"));
  if (align(src.align) !== align(now.align)) out.align = align(src.align);
  if ((src.spaceBefore ?? 0) !== (now.spaceBefore ?? 0)) out.spaceBefore = src.spaceBefore ?? 0;
  if ((src.spaceAfter ?? 0) !== (now.spaceAfter ?? 0)) out.spaceAfter = src.spaceAfter ?? 0;
  const multiple = (p: ParaFmt) => (p.line && (p.lineRule ?? "auto") === "auto" ? p.line / 240 : 1);
  if (Math.abs(multiple(src) - multiple(now)) > 0.001) out.lineMultiple = multiple(src);
  if ((src.indLeft ?? 0) !== (now.indLeft ?? 0)) out.indLeft = src.indLeft ?? 0;
  if ((src.indRight ?? 0) !== (now.indRight ?? 0)) out.indRight = src.indRight ?? 0;
  const first = (p: ParaFmt) => (p.indFirstLine ?? 0) - (p.indHanging ?? 0);
  if (first(src) !== first(now)) out.indFirst = first(src);
  if (!!src.keepNext !== !!now.keepNext) out.keepNext = !!src.keepNext;
  if (!!src.keepLines !== !!now.keepLines) out.keepLines = !!src.keepLines;
  if ((src.widowControl !== false) !== (now.widowControl !== false)) out.widowControl = src.widowControl !== false;
  if ((src.shading ?? "") !== (now.shading ?? "")) out.shading = src.shading ?? "";
  if (JSON.stringify(src.borders ?? {}) !== JSON.stringify(now.borders ?? {})) out.borders = src.borders ?? {};
  return out;
}

/** Whether a selection is one paragraph's whole text. */
function coversParagraph(ed: Editor, r: [MPos, MPos]): boolean {
  return r[0].p === r[1].p && r[0].o === 0 && r[1].o > 0 && r[1].o >= paraLength(ed.root, r[1].p);
}

/**
 * Apply copied formatting to the selection. `how` tells what a bare caret does: the command sets the text
 * formatting for the next typed text as well; the painter, clicked on a paragraph, sets paragraph formatting only.
 */
export function applyFormat(ed: Editor, c: CopiedFormat, how: "command" | "painter") {
  const r = ed.range();
  if (!r) return;
  const collapsed = ed.collapsed();
  if (ed.multi) {
    ed.setRun(c.run);
    return;
  }
  const ids = ed.selectedParagraphs();
  const paragraphs = collapsed || ids.length > 1 || coversParagraph(ed, r);
  if (collapsed && how === "command") ed.pending = { ...(ed.pending ?? {}), ...c.run };
  const first: Op[] = [];
  if (!collapsed) first.push({ op: "format_text", from: r[0], to: r[1], format: c.run });
  // Each paragraph: clear what it had, take the style and list, then whatever else differs from what the style gives.
  const perPara = paragraphs ? ids.map((id) => ({ id, inList: !!ed.engine.formatAt({ p: id, o: 0 })?.list })) : [];
  for (const { id, inList } of perPara) {
    const format: ParaPatch = { clear: true, style: c.style };
    if (c.list) format.num = [c.list.numId, c.list.ilvl];
    else if (inList) format.num = [0, 0];
    first.push({ op: "format_paragraphs", ids: [id], format });
  }
  if (!first.length) {
    ed.hooks.onSelection?.(ed.formatWithPending());
    return;
  }
  ed.exec(first, { select: ed.sel, keepExtra: true });
  const second: Op[] = [];
  for (const { id } of perPara) {
    const now = ed.engine.formatAt({ p: id, o: 0 })?.para;
    const format = now ? difference(c.para, now) : {};
    if (Object.keys(format).length) second.push({ op: "format_paragraphs", ids: [id], format });
  }
  if (second.length) ed.exec(second, { select: ed.sel, keepExtra: true, merge: true });
}

/** The format painter: pick up formatting, then drag over text to apply it. One use, or until Escape when switched on with a double-click. */
export class Painter {
  copied: CopiedFormat | null = null;
  on = false;
  sticky = false;

  constructor(private ed: Editor) {}

  bind() {
    const c = this.ed.canvas;
    c.addEventListener("mouseup", (e) => this.up(e));
    c.addEventListener("keydown", (e) => {
      if (this.on && e.key === "Escape") {
        e.stopPropagation();
        this.disarm();
      }
    });
  }

  copy() {
    this.copied = captureFormat(this.ed.formatWithPending());
  }

  /** Pick up the formatting at the selection and switch the painter on. */
  arm(sticky = false) {
    this.copy();
    if (!this.copied) return;
    this.on = true;
    this.sticky = sticky;
    this.changed();
  }

  disarm() {
    if (!this.on) return;
    this.on = false;
    this.sticky = false;
    this.changed();
  }

  /** The button or command: switch on, or off when it is on. */
  toggle(sticky = false) {
    if (this.on && !(sticky && !this.sticky)) this.disarm();
    else this.arm(sticky);
  }

  private changed() {
    this.ed.canvas.classList.toggle("painting", this.on);
    this.ed.canvas.dispatchEvent(new CustomEvent("docxtor-painter"));
  }

  private up(e: MouseEvent) {
    if (!this.on || e.button !== 0 || !this.copied || !this.ed.root.contains(e.target as Node)) return;
    // The browser finishes updating the selection after the mouse goes up.
    window.setTimeout(() => {
      if (!this.on || !this.copied) return;
      applyFormat(this.ed, this.copied, "painter");
      if (!this.sticky) this.disarm();
    }, 0);
  }
}
