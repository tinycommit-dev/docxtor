// The word the caret is in, or the words selected: what Thesaurus (and a dictionary) look up, and what a synonym replaces.
import type { MPos } from "@docxtor/core";
import type { Editor } from "../editor/editor";

export interface WordTarget {
  p: number;
  from: number;
  to: number;
  text: string;
}

const segmenters = new Map<string, Intl.Segmenter>();
function segmenter(lang: string) {
  let s = segmenters.get(lang);
  if (!s) {
    try {
      s = new Intl.Segmenter(lang, { granularity: "word" });
    } catch {
      s = new Intl.Segmenter(undefined, { granularity: "word" });
    }
    segmenters.set(lang, s);
  }
  return s;
}

/** Letters and numbers of a stretch of text, without the spaces and punctuation around them. */
function trimmed(text: string, from: number): { text: string; from: number } {
  const lead = text.length - text.replace(/^[^\p{L}\p{N}]+/u, "").length;
  const body = text.slice(lead).replace(/[^\p{L}\p{N}]+$/u, "");
  return { text: body, from: from + lead };
}

/** The paragraph's language, else the document's, else the browser's. */
export function languageAt(ed: Editor, p: number): string {
  return (ed.paraElement(p)?.dataset.lang || ed.pageInfo.lang || navigator.language || "en-US").replace(/_/g, "-");
}

/**
 * The selected words (inside one paragraph), or the word the caret is in or touches. Null when there is neither:
 * the caret is in space, or the selection runs over several paragraphs.
 */
export function wordAtCaret(ed: Editor): WordTarget | null {
  const engine = ed.engine;
  const r = ed.range();
  if (!engine || !r) return null;
  const [a, b] = r;
  const para = (p: number) => engine.textRange({ p, o: 0 } as MPos, { p, o: Number.MAX_SAFE_INTEGER } as MPos);
  if (a.p !== b.p) return null;
  const text = para(a.p);
  if (a.o !== b.o) {
    const t = trimmed(text.slice(a.o, b.o), a.o);
    return t.text ? { p: a.p, from: t.from, to: t.from + t.text.length, text: t.text } : null;
  }
  for (const s of segmenter(languageAt(ed, a.p)).segment(text)) {
    if (s.isWordLike && a.o >= s.index && a.o <= s.index + s.segment.length) return { p: a.p, from: s.index, to: s.index + s.segment.length, text: s.segment };
  }
  return null;
}
