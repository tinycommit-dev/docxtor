// Character formatting beyond bold and italic, as CSS: every underline style and colour, raised and lowered text,
// character scale, kerning, outline, shadow, emboss and imprint, text borders, patterned shading, emphasis marks,
// fit text, Word 2010 text effects and OpenType settings, and Word's automatic text colour.
//
// `runExtraCss` gives the declarations for one run. Some formatting needs more than one element: underline of
// words only (spaces are left out), character scale (an inline box per word, because CSS cannot stretch an inline
// box) and fit text (spacing worked out from the measured width). `prepareSegs` turns the engine's segments into
// the pieces to draw, with the classes and styles each needs.
import type { Border, RSeg, RunFmt, TextFx } from "@docxtor/core";
import { borderShorthand } from "./borders.ts";
import { shdPaint } from "./shading.ts";
import { hex, hexOr } from "./color.ts";

const PX_PT = 96 / 72;

/** How CSS draws each of Word's underline styles (`w:u`). Styles CSS has no line for are drawn as the nearest one. */
export const UNDERLINE_CSS: Record<string, { style: "solid" | "double" | "dotted" | "dashed" | "wavy"; heavy?: boolean }> = {
  single: { style: "solid" },
  words: { style: "solid" },
  double: { style: "double" },
  thick: { style: "solid", heavy: true },
  dotted: { style: "dotted" },
  dottedHeavy: { style: "dotted", heavy: true },
  dash: { style: "dashed" },
  dashedHeavy: { style: "dashed", heavy: true },
  dashLong: { style: "dashed" },
  dashLongHeavy: { style: "dashed", heavy: true },
  dotDash: { style: "dashed" },
  dashDotHeavy: { style: "dashed", heavy: true },
  dotDotDash: { style: "dashed" },
  dashDotDotHeavy: { style: "dashed", heavy: true },
  wave: { style: "wavy" },
  wavyHeavy: { style: "wavy", heavy: true },
  wavyDouble: { style: "wavy", heavy: true },
};

/** The underline styles Word offers, in the order of its list. */
export const UNDERLINE_CHOICES: [string, string][] = [
  ["none", "(none)"],
  ["single", "Single"],
  ["words", "Words only"],
  ["double", "Double"],
  ["thick", "Thick"],
  ["dotted", "Dotted"],
  ["dottedHeavy", "Thick dotted"],
  ["dash", "Dashed"],
  ["dashedHeavy", "Thick dashed"],
  ["dashLong", "Long dashed"],
  ["dashLongHeavy", "Thick long dashed"],
  ["dotDash", "Dot dash"],
  ["dashDotHeavy", "Thick dot dash"],
  ["dotDotDash", "Dot dot dash"],
  ["dashDotDotHeavy", "Thick dot dot dash"],
  ["wave", "Wavy"],
  ["wavyHeavy", "Thick wavy"],
  ["wavyDouble", "Double wavy"],
];


/** The names Word gives highlight colours, with the colour each shows. */
const HIGHLIGHT_COLORS: Record<string, string> = {
  yellow: "#ffff00", green: "#00ff00", cyan: "#00ffff", magenta: "#ff00ff", blue: "#0000ff", red: "#ff0000",
  darkBlue: "#000080", darkCyan: "#008080", darkGreen: "#008000", darkMagenta: "#800080", darkRed: "#800000",
  darkYellow: "#808000", darkGray: "#808080", lightGray: "#c0c0c0", black: "#000000", white: "#ffffff",
};

/** Word's highlight names as CSS colours. A name from a file that is not one of them (`constructor`, `red;background:url(...)`) is none. */
export const HIGHLIGHT = {
  get(name: string): string | undefined {
    return Object.hasOwn(HIGHLIGHT_COLORS, name) ? HIGHLIGHT_COLORS[name] : undefined;
  },
};

/** Relative luminance of a #rrggbb colour (0 black, 1 white), as WCAG counts it. */
export function luminance(hex: string): number {
  const c = hex.replace("#", "");
  const ch = [0, 2, 4].map((i) => {
    const v = parseInt(c.slice(i, i + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
}

/** Whether black text is hard to read on this colour, so Word's automatic colour turns white. */
export function isDark(hex?: string): boolean {
  if (!hex || !/^#?[0-9a-f]{6}$/i.test(hex)) return false;
  return luminance(hex) < 0.179;
}

/** The background a run is drawn on, if it has one of its own (highlight, then shading). */
export function runBackground(f: RunFmt): string | undefined {
  if (f.highlight && f.highlight !== "none") return HIGHLIGHT.get(f.highlight);
  const c = shdPaint(f.shd, f.shading).color;
  return c;
}

/** The colour the text is drawn in: its own, or Word's automatic colour (black, or white on a dark background). */
export function inkOf(f: RunFmt, behind?: string): string {
  const own = hex(f.color);
  if (own) return own;
  return isDark(runBackground(f) ?? behind) ? "#ffffff" : "#000000";
}

/** The text decoration declarations for underline, strike-through and double strike-through. */
export function decorationCss(f: RunFmt): string[] {
  const underline = f.underline && f.underline !== "none" ? f.underline : undefined;
  const lines: string[] = [];
  if (underline) lines.push("underline");
  if (f.strike || f.dstrike) lines.push("line-through");
  if (!lines.length) return [];
  const u = underline ? (UNDERLINE_CSS[underline] ?? { style: "solid" as const }) : undefined;
  const out = [`text-decoration-line:${lines.join(" ")}`];
  // One style and colour serve both lines; a double strike-through wins over the underline's style.
  const style = f.dstrike ? "double" : (u?.style ?? "solid");
  if (style !== "solid") out.push(`text-decoration-style:${style}`);
  if (u?.heavy) out.push("text-decoration-thickness:max(2px,0.09em)");
  const color = f.underlineColor;
  const line = hex(color);
  if (underline && line && !f.strike && !f.dstrike) out.push(`text-decoration-color:${line}`);
  return out;
}

/** CSS for an OpenType setting from `w14:`. */
function openType(fx: TextFx): string[] {
  const out: string[] = [];
  if (fx.ligatures) {
    const l = fx.ligatures;
    if (l === "none") out.push("font-variant-ligatures:none");
    else if (l !== "default") {
      const v: string[] = [];
      if (l === "all" || l.includes("standard")) v.push("common-ligatures");
      if (l === "all" || l.includes("ontextual")) v.push("contextual");
      if (l === "all" || l.includes("istorical")) v.push("historical-ligatures");
      if (l === "all" || l.includes("iscretional")) v.push("discretionary-ligatures");
      if (v.length) out.push(`font-variant-ligatures:${v.join(" ")}`);
    }
  }
  const nums: string[] = [];
  if (fx.numForm === "lining") nums.push("lining-nums");
  if (fx.numForm === "oldStyle") nums.push("oldstyle-nums");
  if (fx.numSpacing === "proportional") nums.push("proportional-nums");
  if (fx.numSpacing === "tabular") nums.push("tabular-nums");
  if (nums.length) out.push(`font-variant-numeric:${nums.join(" ")}`);
  const feats: string[] = [];
  for (const s of fx.styleSets ?? []) feats.push(`"ss${String(s).padStart(2, "0")}" 1`);
  if (fx.cntxtAlts) feats.push('"calt" 1');
  if (feats.length) out.push(`font-feature-settings:${feats.join(",")}`);
  return out;
}

const rgba = (c: { hex?: string; alpha?: number } | undefined, fallback = "#000000") => {
  const css = hex(c?.hex) ?? hexOr(fallback);
  const a = c?.alpha ?? 1;
  const n = (i: number) => parseInt(css.slice(i, i + 2), 16);
  return a >= 0.999 ? css : `rgba(${n(1)},${n(3)},${n(5)},${a.toFixed(2)})`;
};

/** The extra declarations a run needs beyond bold, italic, size, font, colour and the simple decorations. */
export function runExtraCss(f: RunFmt, base?: RunFmt): string[] {
  const s: string[] = [];
  // Raised and lowered text moves the glyphs without changing the line (vertAlign does change it).
  if (f.position) s.push(`position:relative;top:${-f.position / 2}pt`);
  if (f.kern !== undefined) {
    const size = (f.size ?? base?.size ?? 11) * 2;
    s.push(`font-kerning:${f.kern > 0 && size >= f.kern ? "normal" : "none"}`);
  }
  const shadows: string[] = [];
  const fx = f.fx;
  let color: string | undefined;
  let stroke: string | undefined;
  // Outline: hollow letters with a line in the text colour.
  if (f.outline) {
    stroke = `0.7px ${inkOf(f)}`;
    color = "transparent";
  }
  if (f.shadow) shadows.push("0.06em 0.06em 0.03em rgba(0,0,0,0.45)");
  if (f.emboss) shadows.push("-1px -1px 0 rgba(255,255,255,0.9)", "1px 1px 0 rgba(0,0,0,0.5)");
  if (f.imprint) shadows.push("1px 1px 0 rgba(255,255,255,0.9)", "-1px -1px 0 rgba(0,0,0,0.5)");
  if (fx) {
    if (fx.glow && fx.glow.radius > 0) {
      const c = rgba(fx.glow.color, "#ffc000");
      const r = fx.glow.radius * PX_PT;
      shadows.push(`0 0 ${r}px ${c}`, `0 0 ${r / 2}px ${c}`);
    }
    if (fx.shadow) {
      const a = (fx.shadow.dir * Math.PI) / 180;
      const d = fx.shadow.dist * PX_PT;
      shadows.push(`${(Math.cos(a) * d).toFixed(2)}px ${(Math.sin(a) * d).toFixed(2)}px ${(fx.shadow.blur * PX_PT).toFixed(2)}px ${rgba(fx.shadow.color, "#000000")}`);
    }
    if (fx.outline && fx.outline.width > 0) {
      stroke = `${(fx.outline.width * PX_PT).toFixed(2)}px ${rgba(fx.outline.color, inkOf(f))}`;
    }
    const fill = fx.fill;
    if (fill?.solid) color = rgba(fill.solid);
    else if (fill?.stops?.length) {
      const stops = fill.stops.map((g) => `${rgba(g.color)} ${(g.pos * 100).toFixed(0)}%`).join(",");
      s.push(`background-image:linear-gradient(${((fill.angle ?? 0) + 90).toFixed(0)}deg,${stops})`, "-webkit-background-clip:text;background-clip:text", "-webkit-text-fill-color:transparent");
      color = undefined;
    }
    s.push(...openType(fx));
  }
  if (color) s.push(`color:${color}`);
  if (stroke) s.push(`-webkit-text-stroke:${stroke}`);
  if (shadows.length) s.push(`text-shadow:${shadows.join(",")}`);
  if (f.em && f.em !== "none") {
    const style = f.em === "comma" ? "filled sesame" : f.em === "circle" ? "open circle" : "filled dot";
    s.push(`text-emphasis:${style}`, `text-emphasis-position:${f.em === "underDot" ? "under right" : "over right"}`);
  }
  return s;
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------

/** What one drawn piece needs on top of its run's own style. */
export interface Piece {
  seg: RSeg;
  css: string[];
  cls: string[];
}

type FontStack = (font?: string) => string | undefined;

let canvas: CanvasRenderingContext2D | null | undefined;

/** The width of text in px as the run draws it, measured with the browser's own font metrics. */
function measure(text: string, f: RunFmt, base: RunFmt | undefined, stack: FontStack): number {
  if (canvas === undefined) canvas = typeof document === "undefined" ? null : document.createElement("canvas").getContext("2d");
  if (!canvas) return text.length * 6;
  const size = (f.size ?? base?.size ?? 11) * PX_PT * (f.vertAlign === "superscript" || f.vertAlign === "subscript" ? 0.65 : 1);
  canvas.font = `${f.italic ? "italic " : ""}${f.bold ? 700 : 400} ${size}px ${stack(f.font ?? base?.font) ?? "sans-serif"}`;
  const shown = f.caps ? text.toUpperCase() : text;
  return canvas.measureText(shown).width + ((f.spacing ?? 0) / 15) * [...shown].length;
}

const isSpace = (s: string) => /^\s+$/.test(s);

/** Split text into words and runs of white space, each with its UTF-16 offset in the text. */
function tokens(text: string): { x: string; o: number }[] {
  const out: { x: string; o: number }[] = [];
  let o = 0;
  for (const part of text.split(/(\s+)/)) {
    if (part) out.push({ x: part, o });
    o += part.length;
  }
  return out;
}

const borderKey = (b?: Border) => (b && b.val !== "none" ? JSON.stringify(b) : "");

/** The segments to draw: some are cut into words or given measured spacing, and runs with a border are grouped. */
export function prepareSegs(segs: RSeg[], mark: RunFmt, stack: FontStack): Piece[] {
  const pieces: Piece[] = [];
  // Runs fitted together into one width (w:fitText with the same id) share the spacing that makes them fit.
  const fit = new Map<number, number>();
  for (let i = 0; i < segs.length; ) {
    const f = segs[i].f;
    if (segs[i].k !== "t" || !f.fitText || f.fitText <= 0 || f.fitTextId === undefined) {
      i++;
      continue;
    }
    let j = i;
    let natural = 0;
    let chars = 0;
    while (j < segs.length && segs[j].k === "t" && segs[j].f.fitTextId === f.fitTextId && segs[j].f.fitText === f.fitText) {
      const text = segs[j].x ?? "";
      natural += measure(text, { ...segs[j].f, spacing: 0 }, mark, stack);
      chars += [...text].length;
      j++;
    }
    if (chars) fit.set(f.fitTextId, Math.round(((f.fitText / 15 - natural) / chars) * 15));
    i = j;
  }
  for (const seg of segs) {
    let s = seg;
    if (seg.k === "t" && seg.f.fitTextId !== undefined && fit.has(seg.f.fitTextId)) s = { ...seg, f: { ...seg.f, spacing: fit.get(seg.f.fitTextId) } };
    const scale = s.f.scale && s.f.scale !== 100 && s.k === "t" ? s.f.scale / 100 : 1;
    const words = s.k === "t" && s.f.underline === "words";
    if (s.k !== "t" || (!words && scale === 1)) {
      pieces.push({ seg: s, css: [], cls: [] });
      continue;
    }
    // One box per word, so lines can still break between words. The spaces between them are plain text that is
    // as wide as the scale makes them.
    const space = scale === 1 ? 0 : measure(" ", { ...s.f, spacing: 0 }, mark, stack);
    for (const tk of tokens(s.x ?? "")) {
      const blank = isSpace(tk.x);
      const f = words ? { ...s.f, underline: blank ? "none" : "single" } : s.f;
      const piece: Piece = { seg: { ...s, o: s.o + tk.o, x: tk.x, f }, css: [], cls: [] };
      if (scale !== 1) {
        if (blank) piece.css.push(`letter-spacing:${((scale - 1) * space).toFixed(2)}px`);
        else {
          const w = measure(tk.x, s.f, mark, stack);
          piece.css.push("display:inline-block", `transform:scaleX(${scale})`, "transform-origin:0 50%", `margin-right:${((scale - 1) * w).toFixed(2)}px`);
        }
      }
      pieces.push(piece);
    }
  }
  // A box around text: one box for neighbouring runs with the same border, so the line is not cut between them.
  for (let i = 0; i < pieces.length; i++) {
    const b = pieces[i].seg.f.border;
    const key = borderKey(b);
    if (!key) continue;
    const first = i === 0 || borderKey(pieces[i - 1].seg.f.border) !== key;
    const last = i === pieces.length - 1 || borderKey(pieces[i + 1].seg.f.border) !== key;
    const edge = borderShorthand(b!);
    const pad = (b!.space || 0) * PX_PT;
    const css = pieces[i].css;
    css.push(`border-top:${edge}`, `border-bottom:${edge}`);
    if (first) css.push(`border-left:${edge}`, `padding-left:${pad}px`);
    if (last) css.push(`border-right:${edge}`, `padding-right:${pad}px`);
    pieces[i].cls.push("bdr");
  }
  return pieces;
}
