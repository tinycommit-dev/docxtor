// East Asian typography on the page: the paragraph switches of the Asian typography tab as CSS, two lines in one and
// text upright in vertical lines, phonetic guides (ruby), and the kinsoku lists a document sets itself.
//
// Word's rules and what the browser can do about them:
//   kinsoku            line-break: strict (the strict level), normal, or loose when the paragraph turns the rules off
//   wordWrap off       word-break: break-all (a line may break inside a Latin word)
//   topLinePunct       text-spacing-trim: trim-start
//   autoSpaceDE / DN   text-autospace (Chrome 140 and later; older browsers keep their own spacing)
//   overflowPunct      has no CSS in Chrome, so it is read and written but not drawn
import type { AsianSettings, ParaFmt, RRuby, RSeg, RunFmt } from "@docxtor/core";
import type { Piece } from "./charfmt";

/** The CSS a paragraph's Asian typography needs (empty for a paragraph that follows Word's defaults). */
export function eaParaCss(fmt: ParaFmt): string[] {
  const s: string[] = [];
  // The rules follow the document's level (the flow's `--ea-line-break`); a paragraph that turned them off is set loosely.
  if (fmt.kinsoku === false) s.push("line-break:loose");
  if (fmt.wordWrap === false) s.push("word-break:break-all");
  if (fmt.topLinePunct) s.push("text-spacing-trim:trim-start");
  const de = fmt.autoSpaceDe !== false;
  const dn = fmt.autoSpaceDn !== false;
  if (!de && !dn) s.push("text-autospace:no-autospace");
  else if (!de) s.push("text-autospace:ideograph-numeric");
  else if (!dn) s.push("text-autospace:ideograph-alpha");
  // A paragraph with a direction of its own: a box of its own, as tall as a page's text is long.
  if (fmt.textDirection === "tbRl" || fmt.textDirection === "tbRlV") s.push("writing-mode:vertical-rl", "height:30em", "overflow:hidden");
  else if (fmt.textDirection === "btLr") s.push(`writing-mode:${typeof CSS !== "undefined" && CSS.supports("writing-mode", "sideways-lr") ? "sideways-lr" : "vertical-lr"}`, "height:30em", "overflow:hidden");
  if (fmt.textAlignment) {
    const a = { top: "top", center: "middle", baseline: "baseline", bottom: "bottom" }[fmt.textAlignment];
    if (a) s.push(`--ea-valign:${a}`);
  }
  return s;
}

/** The value of `--ea-line-break` for the document's settings: `strict` for the strict level, nothing otherwise. */
export function lineBreakFor(settings: AsianSettings | undefined): string {
  return settings?.strict ? "strict" : "";
}

/** The characters of the document's own lists for the language of some text: its language's list, else the first one. */
function listFor(settings: AsianSettings | undefined, lang: string | undefined) {
  const custom = settings?.custom;
  if (!custom?.length) return undefined;
  return custom.find((c) => c.lang === lang) ?? custom.find((c) => lang && c.lang.slice(0, 2) === lang.slice(0, 2)) ?? custom[0];
}

/**
 * Apply the lists of characters a document sets itself: one that may not start a line stays with the character before
 * it, and one that may not end a line stays with the one after it. The characters that must stay together become a
 * piece of their own that does not break (the text between them stays as it was). Paragraphs that turned the rules off
 * are left alone.
 */
export function kinsokuPieces(pieces: Piece[], fmt: ParaFmt, settings: AsianSettings | undefined): Piece[] {
  if (!settings?.custom?.length || fmt.kinsoku === false) return pieces;
  const out: Piece[] = [];
  for (const piece of pieces) {
    const seg = piece.seg;
    const text = seg.x ?? "";
    const list = seg.k === "t" && seg.f.eaText ? listFor(settings, seg.f.langEa ?? guessLang(text)) : undefined;
    if (!list || (!list.before && !list.after)) {
      out.push(piece);
      continue;
    }
    const chars = [...text];
    // Which characters are glued to the one after them.
    const glued = chars.map((c, i) => i + 1 < chars.length && (list.after.includes(c) || list.before.includes(chars[i + 1])));
    if (!glued.some(Boolean)) {
      out.push(piece);
      continue;
    }
    let from = 0;
    let at = seg.o;
    while (from < chars.length) {
      let to = from;
      while (to + 1 < chars.length && glued[to]) to++;
      const part = chars.slice(from, to + 1).join("");
      const together = to > from;
      // The characters between the groups stay in one piece; a group that must not break is a piece of its own.
      const last = out[out.length - 1];
      if (!together && last && last.seg !== seg && last.cls.includes("kin-run") && last.seg.o + (last.seg.x ?? "").length === at) {
        last.seg = { ...last.seg, x: (last.seg.x ?? "") + part };
      } else {
        out.push({ seg: { ...seg, o: at, x: part }, css: [...piece.css, ...(together ? ["white-space:nowrap"] : [])], cls: [...piece.cls, ...(together ? ["kin-keep"] : ["kin-run"])] });
      }
      at += part.length;
      from = to + 1;
    }
  }
  return out;
}

/** The number of characters (code points) in a string. */
const count = (s: string) => [...s].length;

/** The brackets of two lines in one. */
const BRACKETS: Record<string, [string, string]> = {
  round: ["(", ")"],
  square: ["[", "]"],
  angle: ["<", ">"],
  curly: ["{", "}"],
};

export interface EaPiece {
  css: string[];
  cls: string[];
  attrs: Record<string, string>;
}

/** Whether a text is mostly East Asian characters, so that a grid's character pitch belongs to it. */
export function isMostlyEastAsian(text: string): boolean {
  let n = 0;
  let total = 0;
  for (const c of text) {
    total++;
    if (/[ᄀ-ᇿ⺀-⿟　-㏿㐀-䶿一-鿿가-퟿豈-﫿＀-￯]/u.test(c) || (c.codePointAt(0) ?? 0) >= 0x20000) n++;
  }
  return total > 0 && n * 2 >= total;
}

/** Hiragana for text that is all kana (katakana becomes hiragana); null for anything else, which a kana table can't read. */
export function hiraganaOf(text: string): string | null {
  const t = text.trim();
  if (!t || !/^[\u3041-\u3096\u30a1-\u30f6\u30fc\u30fb\u309d\u309e\u30fd\u30fe\s]+$/u.test(t)) return null;
  return t.replace(/[\u30a1-\u30f6]/gu, (c) => String.fromCharCode(c.charCodeAt(0) - 0x60)).replace(/\u30fd/g, "\u309d").replace(/\u30fe/g, "\u309e");
}

/** A language for East Asian text whose run names none: kana is Japanese, Hangul is Korean; Han alone is left to the browser. */
export function guessLang(text: string): string | undefined {
  if (/[぀-ヿㇰ-ㇿ]/u.test(text)) return "ja";
  if (/[ᄀ-ᇿ㄰-㆏가-퟿]/u.test(text)) return "ko";
  return undefined;
}

/**
 * What a text segment needs for its East Asian layout: two lines in one (a box half the size with the text wrapped
 * into two lines), upright text in a vertical line, and the grid's character pitch. `size` is the run's size in points.
 */
/**
 * The width of a two-lines-in-one box for `text`: half its characters to a line. Each character may also take up to a pixel more
 * than its em, where the system rounds glyph widths to whole pixels (Chromium on Linux), so the box allows a pixel a character:
 * without it, the second character of a line no longer fits and the text stacks one character to a line.
 */
export function combWidth(text: string): string {
  const n = Math.ceil(count(text) / 2);
  return `calc(${n}em + ${n}px)`;
}

export function eaSegPiece(seg: RSeg, size: number): EaPiece | null {
  if (seg.k !== "t") return null;
  const f = seg.f;
  const text = seg.x ?? "";
  const out: EaPiece = { css: [], cls: [], attrs: {} };
  const l = f.eaLayout;
  if (l?.combine && text) {
    // Two lines of half height inside one character box: as wide as half the characters, as tall as one line.
    const half = (size * 0.5).toFixed(2);
    out.cls.push("ea-comb");
    out.css.push(`font-size:${half}pt`, `width:${combWidth(text)}`);
    const b = l.combineBrackets && BRACKETS[l.combineBrackets];
    if (b) {
      out.cls.push("ea-comb-br");
      out.attrs["data-bl"] = b[0];
      out.attrs["data-brt"] = b[1];
    }
  } else if (l?.vert || l?.vertCompress) {
    // Horizontal in vertical: fitted into the line's height, or at its own width across the vertical line.
    if (l.vertCompress) out.css.push("text-combine-upright:all");
    else out.cls.push("ea-vert");
  }
  if (text && f.eaText) {
    if (f.snapToGrid !== false) out.cls.push("ea-t");
    const lang = f.langEa ?? guessLang(text);
    if (lang) out.attrs.lang = lang;
  }
  return out.css.length || out.cls.length || Object.keys(out.attrs).length ? out : null;
}

/** The draw of one phonetic guide: the base text with the guide above it. */
export function rubyAtom(seg: RSeg, fontStack: (f?: string) => string | undefined, runStyle: (f: RunFmt, base?: RunFmt) => string): HTMLElement {
  const r = seg.ruby as RRuby;
  const el = document.createElement("ruby");
  el.className = "atom ruby";
  el.dataset.align = r.align;
  const base = document.createElement("span");
  base.className = "rb";
  base.textContent = r.base;
  const rt = document.createElement("rt");
  rt.textContent = r.text;
  const own = r.rtF ? runStyle(r.rtF) : "";
  const parts = [own];
  if (r.size && !r.rtF?.size) parts.push(`font-size:${r.size}pt`);
  if (r.font && !r.rtF?.font) parts.push(`font-family:${fontStack(r.font)}`);
  const css = parts.filter(Boolean).join(";");
  if (css) rt.setAttribute("style", css);
  const lang = r.lang ?? seg.f.langEa;
  if (lang) el.lang = lang;
  el.dataset.rb = r.base;
  el.append(base, rt);
  return el;
}
