// Spelling and grammar: finds the problems in the paragraphs on screen and draws a wavy underline under each.
//
// Like the non-printing characters (marks.ts) the underlines sit on a layer of their own over the stage,
// outside the editable flow: nothing in the text, the selection, the layout or the model changes, and
// printing leaves the layer out. What the layer needs from outside is a spelling checker (a way to look
// words up, in the language of each paragraph) and a grammar source (a way to ask the grammar checkers
// about paragraphs); both come from built-in extensions. A host with neither has no underlines and the
// browser's own spellcheck keeps working.
//
// Work is kept small: only the paragraphs near the view are looked at (the check panel reads the rest when
// it is opened), and not while a fling goes by (`settleFling`). Reading every paragraph of a long book when
// the browser was idle cost 250 MB of a War and Peace tab while typing, saving and scrolling. A paragraph is only checked again when its text or language changes (results are
// kept per paragraph id and a hash of its text), and every distinct word is looked up once. The word being
// typed is left alone until the caret moves off it. Spelling is drawn first and at once; grammar can take
// longer (a checker may be on a server), so it is drawn when it arrives.
import type { MPos } from "@docxtor/core";
import type { GrammarIssue, GrammarParagraph } from "@docxtor/extensions";
import { addressSpans, DEFAULT_WORD_OPTIONS, hash, wordsIn, type Span, type WordOptions } from "../spelling/words";
import { paragraphsBetween } from "./marks";
import { isZeroWidth, paraIn } from "./selection";

export interface SpellChecker {
  /** The document's language, for paragraphs that name none. */
  readonly lang: string;
  /** Get the dictionary for a language ready. Resolves false when there is none: those paragraphs are not checked. */
  ready(lang: string): Promise<boolean>;
  /** For each word, whether it is spelled correctly in a language. */
  check(lang: string, words: string[]): Promise<boolean[]>;
  /** Words to leave alone in a language: the custom dictionaries in use for it and the words ignored in this document. */
  ignored(word: string, lang: string): boolean;
  /** Which kinds of word are not checked (capitals, words with numbers, addresses). All three when absent. */
  options?(): WordOptions;
}

export interface GrammarSource {
  /** The document's language, for paragraphs that name none. */
  readonly lang: string;
  /**
   * Every issue the grammar checkers find in these paragraphs. `partial` gets what is known so far as each checker
   * answers, so a slow one (on a server) does not hold back the rest.
   */
  check(paragraphs: GrammarParagraph[], partial?: (issues: GrammarIssue[]) => void): Promise<GrammarIssue[]>;
}

export interface SpellHost {
  canvas: HTMLElement;
  stage: HTMLElement;
  flow: HTMLElement;
  sheets: HTMLElement;
  /** The top-level blocks that may reach into a band of the stage, from the page map (undefined: not known, look through the flow). */
  blocksBetween?(y0: number, y1: number): HTMLElement[] | undefined;
  /** The caret (null when text is selected, or nothing is). */
  caret(): MPos | null;
  /** The browser is composing text (an input method): leave the text alone. */
  busy(): boolean;
}

/**
 * A misspelled word or a grammar or style problem. `p` is null in a header, footer or note that isn't being
 * edited (it can't be changed from there). `word` is the text it covers.
 */
export interface SpellHit {
  p: number | null;
  from: number;
  to: number;
  word: string;
  /** The language the text was checked in (spelling hits). */
  lang?: string;
  /** Spelling unless it says otherwise. */
  kind?: "spelling" | "grammar" | "style";
  /** For grammar and style: what is wrong, and what to put instead. */
  message?: string;
  shortMessage?: string;
  replacements?: string[];
  rule?: GrammarIssue["rule"];
}

interface Seg {
  node: Text;
  start: number;
  end: number;
}

/** A paragraph's text as the model has it (list labels and spacers left out, objects as one character), and where it sits in the DOM. */
interface Analysis {
  id: number;
  p: number | null;
  /** The language of its text (`data-lang`), or "" when the file gives none. */
  lang: string;
  text: string;
  segs: Seg[];
  skip: Span[];
  key: number;
}

/** The boxes of one paragraph, and what they were made from (`stamp`): the same stamp means the same boxes, and they are left alone. */
interface Mounted {
  el: HTMLElement;
  stamp: string;
  /** What the boxes look like, so boxes that came out the same are not put on the page again. */
  look: string;
  boxes: HTMLElement[];
  hits: SpellHit[];
}

interface Entry {
  key: number;
  /** The language the paragraph was checked in. */
  lang: string;
  errors: { word: string; from: number; to: number }[];
  /** A word was left out for being typed: look again next time. */
  partial: boolean;
}

interface GrammarEntry {
  key: number;
  /** The language the paragraph was checked in. */
  lang?: string;
  /** The checkers changed since: still drawn, but asked again. */
  stale?: boolean;
  issues: GrammarIssue[];
  /** Issues left out for being typed: look again once the caret is off them. */
  hidden: GrammarIssue[];
}

/** Fonts for code: words in them are not prose. */
const CODE_FONT = /mono|courier|consolas|menlo|monaco|lucida console|cousine|source code|fira code|jetbrains|inconsolata|andale/i;

/** The inline font of an element, read from its attribute: `el.style` and `el.childNodes` make an object for every element they are asked of, which the page keeps as long as the element (26 MB over War and Peace once every page was read). */
const fontOf = (el: Element) => /(?:^|;)\s*font-family\s*:([^;]*)/i.exec(el.getAttribute("style") ?? "")?.[1] ?? "";

/** Read a paragraph element: its text (offsets as in the model), the text nodes it is made of and the stretches to skip (links, fields, code fonts). */
export function analyse(el: HTMLElement): Analysis {
  const segs: Seg[] = [];
  const skip: Span[] = [];
  let text = "";
  const walk = (node: Node, skipping: boolean) => {
    for (let c = node.firstChild; c; c = c.nextSibling) {
      if (c.nodeType === Node.TEXT_NODE) {
        const data = (c as Text).data;
        segs.push({ node: c as Text, start: text.length, end: text.length + data.length });
        if (skipping && data.length) skip.push([text.length, text.length + data.length]);
        text += data;
      } else if (c.nodeType === Node.ELEMENT_NODE) {
        const e = c as HTMLElement;
        if (isZeroWidth(e)) continue;
        if (e.classList.contains("atom")) {
          // An object, a tab or a note mark: one character that is not part of a word.
          text += "￼";
          continue;
        }
        // Links, fields, code fonts and text marked "do not check spelling or grammar" (`data-np`) are not prose.
        walk(e, skipping || e.classList.contains("lnk") || !!e.getAttribute("data-fld") || e.getAttribute("data-np") === "1" || CODE_FONT.test(fontOf(e)));
      }
    }
  };
  walk(el, CODE_FONT.test(fontOf(el)));
  const pAttr = el.getAttribute("data-p");
  const p = pAttr === null ? null : Number(pAttr);
  const lang = el.getAttribute("data-lang") ?? "";
  return { id: Number(el.getAttribute("data-id") ?? pAttr ?? NaN), p, lang, text, segs, skip, key: hash(`${lang}\u0000${text}\u0000${skip.map((s) => s.join("-")).join(",")}`) };
}

/** The text a grammar checker sees: links, fields, code and web addresses are not prose, so each of their characters becomes an object character (the length stays the same). */
function proseOf(a: Analysis): string {
  const spans = [...a.skip, ...addressSpans(a.text)];
  if (!spans.length) return a.text;
  const chars = a.text.split("");
  for (const [from, to] of spans) for (let i = from; i < to; i++) chars[i] = "￼";
  return chars.join("");
}

/** The DOM position of an offset in a paragraph; `end` prefers the end of a text node over the start of the next. */
function pointAt(segs: Seg[], o: number, end: boolean): { node: Text; offset: number } | null {
  for (const s of segs) if (end ? o > s.start && o <= s.end : o >= s.start && o < s.end) return { node: s.node, offset: o - s.start };
  return null;
}

const MAX_KNOWN = 60000;
/**
 * How long after the last key the paragraph being typed in is checked and marked again (milliseconds). Checking it and
 * measuring every mark in it on each key cost more than a millisecond a key in a long paragraph; Word too waits for a pause.
 */
const TYPING_PAUSE = 500;
/** How long the check panel waits for the grammar checkers (milliseconds). */
const GRAMMAR_WAIT = 8000;
/** How long a fling's view stays put before its marks are found and drawn (milliseconds). */
const FLING_SETTLE = 150;

const sameIssues = (a: GrammarIssue[], b: GrammarIssue[]) =>
  a.length === b.length && a.every((x, i) => x.from === b[i].from && x.to === b[i].to && x.message === b[i].message && x.kind === b[i].kind && x.replacements.join("\u0000") === b[i].replacements.join("\u0000"));

/** What "ignore" remembers about a grammar issue: the rule and the words it flagged. */
const ignoreKey = (issue: GrammarIssue, word: string) => `${issue.rule.id}\u0000${word.toLowerCase()}`;

export class SpellLayer {
  readonly el: HTMLElement;
  private checker: SpellChecker | null = null;
  private grammar: GrammarSource | null = null;
  private frame = 0;
  /** Bumped by every change that makes a running pass out of date. */
  private gen = 0;
  private built: [number, number] | null = null;
  /** The stretch last drawn, kept when `built` is let go: a view clear of it is a fling. */
  private lastDrawn: [number, number] | null = null;
  private flingTimer = 0;
  private range = document.createRange();
  private cache = new Map<number, Entry>();
  /** The language of a paragraph the file gives none, by its words (Detect language automatically); null to use the document's. */
  private detect: ((text: string) => string | null) | null = null;
  private gcache = new Map<number, GrammarEntry>();
  /** Grammar requests in flight: paragraph id to the key of the text that was sent. */
  private gpending = new Map<number, number>();
  /** What is drawn now, by paragraph id: the paragraph's element, what its boxes were made from, and the boxes. */
  private mounted = new Map<number, Mounted>();
  /** The grammar problem each grammar box stands for. */
  private hitOf_ = new WeakMap<HTMLElement, SpellHit>();
  /** Bumped by everything that changes how a paragraph's boxes come out other than its text, its results and where it sits (the marks hidden, an issue ignored, the layout run again). */
  private epoch = 0;
  /** A number for each results object, to tell in a stamp whether a paragraph has new ones. */
  private ids = new WeakMap<object, number>();
  private nextId = 1;
  /** Grammar issues the person chose to ignore in this document. */
  private ignoredIssues = new Set<string>();
  private known = new Map<string, boolean>();
  /** The paragraph in which the word under the caret is being typed. */
  private held: { p: number } | null = null;
  private pendingHold = false;
  /** While keys come quickly: the paragraph being typed in, left unmarked until the pause (see `TYPING_PAUSE`). */
  private typing: number | null = null;
  private typingTimer = 0;
  /** The height the paragraph being typed in had at the last look (-1: not looked at yet). While it stays the same nothing else on the page moved, so a key needs no pass. */
  private typingHeight = -1;
  /** The frame asked for draws again (`invalidate`), rather than only looking whether the typed paragraph changed height (`typingMoved`). */
  private frameFull = false;
  /** What each paragraph element said when it was last read. The editor draws a changed paragraph again as a new element, so an element's text only changes while an input method is composing in it. */
  private readCache = new WeakMap<HTMLElement, Analysis>();
  /** Bumped when cached spelling results are thrown away. */
  private version = 0;
  /** Bumped when cached grammar results are thrown away. */
  private gversion = 0;
  /** Called when the spelling checker was switched on or off. */
  onActive: ((active: boolean) => void) | null = null;

  constructor(private host: SpellHost) {
    this.el = document.createElement("div");
    this.el.className = "sp-layer";
    this.el.setAttribute("aria-hidden", "true");
    host.stage.append(this.el);
  }

  /** The spelling checker is on. */
  get active(): boolean {
    return !!this.checker;
  }

  /** Grammar is on (a source is set; whether any checker covers the language is for the host to say). */
  get grammarActive(): boolean {
    return !!this.grammar;
  }

  private get on(): boolean {
    return !!this.checker || !!this.grammar;
  }

  /** Check spelling with `checker`, or stop checking it (null). Everything found about spelling so far is forgotten. */
  setChecker(checker: SpellChecker | null) {
    const was = !!this.checker;
    this.checker = checker;
    this.clearSpelling();
    if (!this.on) this.unmountAll();
    if (was !== !!checker) this.onActive?.(!!checker);
  }

  /** Tell paragraphs without a language of their own by their words (null: they are in the document's language). */
  setDetector(detect: ((text: string) => string | null) | null) {
    this.detect = detect;
    this.languagesChanged();
  }

  /** A paragraph's language was found (or the way of finding it changed): those whose language is now different are checked again. */
  languagesChanged() {
    // The results are checked against the language again by `resolve`, and a paragraph that has new ones is drawn again; the others keep their boxes (the detector answers once for each text typed, so this comes after nearly every keystroke).
    this.built = null;
    if (this.on) this.invalidate(true);
  }

  /**
   * The language a paragraph is checked in. One marked with another language than the document's keeps it; the rest
   * (the file fills a paragraph's language down from the document's default) are in the language their words are
   * in, when that can be told, else in their own or the document's.
   */
  private langOfText(a: Analysis, fallback: string): string {
    const base = (l: string) => l.toLowerCase().split(/[-_]/)[0];
    if (a.lang && base(a.lang) !== base(fallback)) return a.lang;
    return this.detect?.(a.text) || a.lang || fallback;
  }

  /** Spelling or grammar marks are not drawn (they are still found, so the check panel works). Set from the document's settings. */
  private hidden = { spelling: false, grammar: false };

  setHidden(spelling: boolean, grammar: boolean) {
    if (this.hidden.spelling === spelling && this.hidden.grammar === grammar) return;
    this.hidden = { spelling, grammar };
    this.built = null;
    this.epoch++;
    if (this.on) this.invalidate();
  }

  /** Check grammar with `source`, or stop (null). Everything found about grammar so far is forgotten. */
  setGrammar(source: GrammarSource | null) {
    this.grammar = source;
    this.clearGrammar();
    if (!this.on) this.unmountAll();
  }

  private unmountAll() {
    this.el.replaceChildren();
    this.mounted.clear();
  }

  /**
   * The grammar checkers changed (one was turned on or off, or changed what it finds): ask them all again. What was
   * found stays on the page until the new answers come, so the marks that are still right do not blink.
   */
  grammarChanged() {
    for (const e of this.gcache.values()) e.stale = true;
    this.gpending.clear();
    this.gversion++;
    this.built = null;
    this.epoch++;
    if (this.on) this.invalidate();
  }

  /** Forget what was found (a new document, a new language) and look again. */
  clear() {
    // A new document: the word that was being typed is not in it, and nothing of it was drawn yet.
    this.held = null;
    this.pendingHold = false;
    this.lastDrawn = null;
    this.clearSpelling();
    this.clearGrammar();
  }

  private clearSpelling() {
    this.cache.clear();
    this.known.clear();
    this.version++;
    // The word being typed stays held: the checker is often set (its dictionary loaded, an option changed) while the
    // person is in the middle of a word, and forgetting that would underline the word before the caret leaves it.
    this.built = null;
    this.epoch++;
    if (this.checker || this.grammar) this.invalidate();
  }

  private clearGrammar() {
    this.gcache.clear();
    this.gpending.clear();
    this.gversion++;
    this.built = null;
    this.epoch++;
    if (this.checker || this.grammar) this.invalidate();
  }

  /** The words to leave alone changed: look at the words again, not up in the dictionary again. */
  refilter() {
    this.cache.clear();
    this.version++;
    if (this.on) this.invalidate();
  }

  /** Leave a grammar issue alone in this document: the same rule on the same words. */
  ignoreGrammar(hit: SpellHit) {
    if (hit.rule) this.ignoredIssues.add(ignoreKey({ rule: hit.rule } as GrammarIssue, hit.word));
    this.invalidate();
  }

  /** A new document: nothing is ignored in it yet. */
  resetIgnoredGrammar() {
    this.ignoredIssues.clear();
  }

  destroy() {
    window.cancelAnimationFrame(this.frame);
    window.clearTimeout(this.typingTimer);
    window.clearTimeout(this.flingTimer);
    this.checker = null;
    this.grammar = null;
    this.gen++;
    this.el.remove();
  }

  /**
   * The text changed, moved or was zoomed: draw again in the next frame. Every box is made again, since anything may have
   * moved; `soft` (the caret moved, the view scrolled) leaves the boxes of paragraphs that are as they were.
   */
  invalidate(soft = false) {
    if (!this.on) return;
    this.gen++;
    this.built = null;
    if (!soft) this.epoch++;
    this.frameFull = true;
    this.schedule();
  }

  /**
   * A key in the paragraph being typed in, which waits for the pause: the other paragraphs' marks stay where they are
   * unless it changed height (a line came or went, so what follows it moved). Then they are drawn again.
   */
  private typingMoved() {
    this.schedule();
  }

  private schedule() {
    if (this.frame) return;
    this.frame = window.requestAnimationFrame(() => {
      this.frame = 0;
      const full = this.frameFull;
      this.frameFull = false;
      if (!full && this.typingStill()) return;
      if (!full) this.gen++;
      void this.cycle(this.gen);
    });
  }

  /** Whether the paragraph being typed in has the height it had at the last look. */
  private typingStill(): boolean {
    const el = this.typing === null ? null : paraIn(this.host.flow, this.typing);
    const h = el ? el.offsetHeight : -1;
    const still = h >= 0 && h === this.typingHeight;
    this.typingHeight = h;
    return still;
  }

  /**
   * The pages were laid out again. While keys are being typed a layout from the changed block (every key, in a document under 400
   * blocks) moves the other paragraphs only when the one typed in changed height, which `typingMoved` checks: measuring every word's
   * lines again and putting every box back on each key cost Safari about 120 ms a key on 161 paragraphs. Any other layout draws
   * everything again, since words may have moved inside a paragraph whose box stayed where it was.
   */
  relaid(whole: boolean) {
    if (!this.on) return;
    if (!whole && this.typing !== null) return this.typingMoved();
    this.invalidate();
  }

  /** The user typed: the word at the caret is left alone until the caret leaves it. */
  typed() {
    // Noted even before a checker is set, so a dictionary that loads while a word is being typed leaves that word alone.
    this.pendingHold = true;
    if (!this.on) return;
    // The paragraph's text is about to move (in a long document the pages are laid out again after a pause): its lines wait for the next pass rather than sit where the text no longer is.
    const p = this.host.caret()?.p;
    if (p !== undefined) this.unmount(p);
    // Its marks come back once the keys pause: checked and measured once, not on every key.
    if ((p ?? null) !== this.typing) this.typingHeight = -1;
    this.typing = p ?? null;
    window.clearTimeout(this.typingTimer);
    this.typingTimer = window.setTimeout(() => {
      this.typing = null;
      this.invalidate(true);
    }, TYPING_PAUSE);
    // The other paragraphs' marks move only when this one changed height.
    this.typingMoved();
  }

  /** The caret moved. */
  selectionMoved() {
    if (!this.on || !this.held) return;
    const c = this.host.caret();
    if (!c || c.p !== this.held.p) this.held = null;
    // Typing: the paragraph waits for the pause, and the others need drawing again only when it changed height.
    if (c && c.p === this.typing) return this.typingMoved();
    // Within the paragraph, the word shows again once the caret is off it.
    this.invalidate(true);
  }

  /** The view scrolled: draw again when it has left the drawn stretch. */
  scrolled() {
    if (!this.on) return;
    const v = this.view();
    const h = v.bottom - v.top;
    // A fling (the view jumped clear of what was drawn, as a scroll bar dragged or a fast wheel does): the pages it passes are not
    // looked at. Finding and drawing the marks of every page going by allocated hundreds of megabytes over a long book (docs/performance/index.md,
    // Memory of a tab); the marks come once the view stays put for a moment. Scrolling a little at a time keeps them as it goes.
    if (this.flingTimer || (this.lastDrawn && (v.top >= this.lastDrawn[1] || v.bottom <= this.lastDrawn[0]))) {
      this.settleFling();
      return;
    }
    if (this.frame && this.frameFull) return;
    if (this.built && v.top > this.built[0] + h * 0.1 && v.bottom < this.built[1] - h * 0.1) return;
    this.invalidate(true);
  }

  /**
   * A fling settles once the view has stayed put for a moment and across two frames after it: a frame that takes longer than the
   * moment (a long document drawing the pages it scrolls to) is not the view stopping.
   */
  private settleFling() {
    window.clearTimeout(this.flingTimer);
    this.flingTimer = window.setTimeout(() => {
      const top = this.view().top;
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          if (!this.flingTimer) return;
          if (this.view().top !== top) return this.settleFling();
          this.flingTimer = 0;
          this.invalidate(true);
        }),
      );
    }, FLING_SETTLE);
  }

  private view() {
    const c = this.host.canvas.getBoundingClientRect();
    const s = this.host.stage.getBoundingClientRect();
    const sc = s.width / (this.host.stage.offsetWidth || 1) || 1;
    return { top: (c.top - s.top) / sc, bottom: (c.bottom - s.top) / sc, s, sc };
  }

  // ---------------------------------------------------------------------------
  // Looking words up
  // ---------------------------------------------------------------------------

  private read(el: HTMLElement): Analysis {
    if (this.host.busy()) return analyse(el);
    let a = this.readCache.get(el);
    if (!a) this.readCache.set(el, (a = analyse(el)));
    return a;
  }

  /** Work out the misspelled words of paragraphs that have no valid result yet. Resolves when they are in the cache. */
  private async resolve(list: Analysis[]): Promise<boolean> {
    const checker = this.checker;
    // Without a spelling checker there is nothing to look up (grammar may still be on).
    if (!checker) return true;
    const version = this.version;
    const caret = this.host.caret();
    // Typing a space (or anything but a letter) ends the word being typed.
    if (this.held && (!caret || caret.p !== this.held.p)) this.held = null;
    const langOf = (a: Analysis) => this.langOfText(a, checker.lang);
    const todo: { a: Analysis; lang: string; words: ReturnType<typeof wordsIn>; skipped: boolean }[] = [];
    const misses = list.filter((a) => {
      const e = this.cache.get(a.id);
      return !(e && e.key === a.key && !e.partial && e.lang === langOf(a));
    });
    // A paragraph in a language without a dictionary is left unchecked.
    const usable = new Map<string, boolean>();
    if (misses.length) {
      await Promise.all([...new Set(misses.map(langOf))].map(async (l) => usable.set(l, await checker.ready(l))));
      if (checker !== this.checker || version !== this.version) return false;
    }
    const need = new Map<string, Set<string>>();
    let touching = false;
    for (const a of misses) {
      const lang = langOf(a);
      if (!usable.get(lang)) {
        this.cache.set(a.id, { key: a.key, lang, errors: [], partial: false });
        continue;
      }
      const words = wordsIn(a.text, lang, a.skip, checker.options?.() ?? DEFAULT_WORD_OPTIONS);
      const hold = !!this.held && !!caret && a.p === this.held.p;
      let skipped = false;
      const keep = words.filter((w) => {
        if (hold && caret!.o >= w.from && caret!.o <= w.to) {
          skipped = true;
          touching = true;
          return false;
        }
        return true;
      });
      for (const w of keep) {
        if (this.known.has(this.wordKey(lang, w.word)) || checker.ignored(w.word, lang)) continue;
        if (!need.has(lang)) need.set(lang, new Set());
        need.get(lang)!.add(w.word);
      }
      todo.push({ a, lang, words: keep, skipped });
    }
    if (this.held && !touching && caret && list.some((a) => a.p === this.held!.p)) this.held = null;
    if (need.size) {
      if (this.known.size > MAX_KNOWN) this.known.clear();
      for (const [lang, set] of need) {
        const asked = [...set];
        let ok: boolean[];
        try {
          ok = await checker.check(lang, asked);
        } catch {
          return false;
        }
        if (checker !== this.checker || version !== this.version) return false;
        asked.forEach((w, i) => this.known.set(this.wordKey(lang, w), ok[i]));
      }
    }
    for (const { a, lang, words, skipped } of todo) {
      const errors = words.filter((w) => this.known.get(this.wordKey(lang, w.word)) === false && !checker.ignored(w.word, lang)).map(({ word, from, to }) => ({ word, from, to }));
      this.cache.set(a.id, { key: a.key, lang, errors, partial: skipped });
    }
    return true;
  }

  private wordKey(lang: string, word: string) {
    return `${lang.toLowerCase()}\u0000${word}`;
  }

  /**
   * Ask the grammar checkers about paragraphs that have no valid result yet. Resolves when their answers are in
   * the cache, and draws again then (a checker can take a while, and spelling is not held up by it).
   */
  private async resolveGrammar(list: Analysis[]): Promise<void> {
    const source = this.grammar;
    if (!source) return;
    const gversion = this.gversion;
    const asked: Analysis[] = [];
    const paragraphs: GrammarParagraph[] = [];
    const caret = this.host.caret();
    const typedIn = (a: Analysis) => !!this.held && !!caret && a.p === this.held.p;
    for (const a of list) {
      const e = this.gcache.get(a.id);
      // Done, unless issues were held back for being typed and the caret has left them.
      if (e && e.key === a.key && !e.stale && (e.lang === undefined || e.lang === this.langOfText(a, source.lang)) && (!e.hidden.length || (typedIn(a) && e.hidden.some((i) => caret!.o >= i.from && caret!.o <= i.to)))) continue;
      if (this.gpending.get(a.id) === a.key) continue;
      const text = proseOf(a);
      if (text.replace(/￼/g, "").trim().length < 2) {
        this.gcache.set(a.id, { key: a.key, lang: this.langOfText(a, source.lang), issues: [], hidden: [] });
        continue;
      }
      asked.push(a);
      paragraphs.push({ id: a.id, text, lang: this.langOfText(a, source.lang) });
      this.gpending.set(a.id, a.key);
    }
    if (!paragraphs.length) return;
    // Put what the checkers found into the cache. A checker may have been turned on or off, or the results thrown
    // away, while they worked: then the answer is out of date and is dropped.
    const apply = (found: GrammarIssue[]) => {
      if (source !== this.grammar || gversion !== this.gversion) return;
      const now = this.host.caret();
      let changed = false;
      for (const a of asked) {
        const issues = found.filter((i) => i.paragraph === a.id);
        // The word being typed is left alone, as in spelling.
        const hold = !!this.held && !!now && a.p === this.held.p;
        const hidden = hold ? issues.filter((i) => now!.o >= i.from && now!.o <= i.to) : [];
        const shown = issues.filter((i) => !hidden.includes(i));
        // Most of the time a paragraph still has nothing wrong with it: then there is nothing to draw again. (Marks are
        // only on the page for an answer about this very text.)
        const before = this.gcache.get(a.id);
        const drawn = before && before.key === a.key ? before.issues : [];
        if (shown.length ? !sameIssues(drawn, shown) || !before || before.key !== a.key : drawn.length > 0) changed = true;
        // The language it was asked in (it may have been found to be another since).
        this.gcache.set(a.id, { key: a.key, lang: paragraphs[asked.indexOf(a)].lang, issues: shown, hidden });
      }
      // Only the paragraphs that have new results are drawn again (their boxes were made from the old ones).
      if (changed) this.invalidate(true);
    };
    let found: GrammarIssue[] = [];
    try {
      found = await source.check(paragraphs, apply);
    } catch {
      /* a checker that fails adds nothing */
    }
    for (const a of asked) if (this.gpending.get(a.id) === a.key) this.gpending.delete(a.id);
    apply(found);
  }

  // ---------------------------------------------------------------------------
  // Drawing
  // ---------------------------------------------------------------------------

  /** How many passes over the paragraphs in view have started (read by tests: a key that changes no height starts none). */
  passes = 0;

  private async cycle(gen: number) {
    // While a fling settles nothing is looked at (the pages drawn as it goes ask for passes too); its timer asks for one at the end.
    if (!this.on || this.host.busy() || this.flingTimer) return;
    this.passes++;
    if (this.pendingHold) {
      this.pendingHold = false;
      const c = this.host.caret();
      this.held = c ? { p: c.p } : null;
    }
    const v = this.view();
    const h = v.bottom - v.top;
    const y0 = Math.max(0, v.top - h * 0.5);
    const y1 = v.bottom + h * 0.5;
    const items: { el: HTMLElement; a: Analysis }[] = [];
    for (const el of paragraphsBetween(this.host.flow, this.host.sheets, y0, y1, this.host.blocksBetween)) {
      // The paragraph being typed in waits for the pause (its marks were taken off by `typed`).
      if (this.typing !== null && el.dataset.p === String(this.typing)) continue;
      items.push({ el, a: this.read(el) });
    }
    const done = await this.resolve(items.map((i) => i.a));
    // Something changed while the words were being looked up: that change asked for a pass of its own.
    if (!done || gen !== this.gen || !this.on) return;
    this.draw(items, v, y0, y1);
    void this.resolveGrammar(items.map((i) => i.a));
  }

  /** Whether a grammar issue is shown: not ignored, and not on a word that is also misspelled (the spelling mark wins). */
  private shown(issue: GrammarIssue, a: Analysis, spelling: Entry["errors"]): boolean {
    if (this.ignoredIssues.has(ignoreKey(issue, a.text.slice(issue.from, issue.to)))) return false;
    return !spelling.some((s) => issue.from < s.to && issue.to > s.from);
  }

  private draw(items: { el: HTMLElement; a: Analysis }[], v: ReturnType<SpellLayer["view"]>, y0: number, y1: number) {
    // Three passes, so the page is laid out at most once: everything is read first (where each paragraph is, the lines of each word), and only then the boxes go on. A write between two reads makes the browser lay the page out again for the second.
    const present = new Set<number>();
    const todo: { a: Analysis; el: HTMLElement; stamp: string; e: Entry | undefined; spelling: Entry["errors"]; issues: GrammarIssue[] }[] = [];
    for (const item of items) {
      let a = item.a;
      const e = this.cache.get(a.id);
      const g = this.gcache.get(a.id);
      // "Hide spelling errors in this document only" and its grammar twin: found, but not drawn.
      const spelling = e && e.key === a.key && !this.hidden.spelling ? e.errors : [];
      let issues = g && g.key === a.key ? g.issues.filter((i) => this.shown(i, a, spelling) && !(i.kind === "spelling" ? this.hidden.spelling : this.hidden.grammar)) : [];
      if (!spelling.length && !issues.length) continue;
      // Pagination splits a paragraph's text nodes where a page or column breaks: read the nodes again when they moved.
      if (a.segs.some((s) => !s.node.isConnected || s.node.data.length !== s.end - s.start)) {
        a = analyse(item.el);
        this.readCache.set(item.el, a);
        if (e && e.key !== a.key) continue;
        if (g && g.key !== a.key) issues = [];
      }
      present.add(a.id);
      // A paragraph whose text, results and place are what its boxes were made from keeps them: measuring every word's lines again is what a keystroke must not do for the paragraphs it did not touch.
      const r = item.el.getBoundingClientRect();
      const at = [(r.left - v.s.left) / v.sc, (r.top - v.s.top) / v.sc, r.width / v.sc, r.height / v.sc].map((n) => n.toFixed(1)).join(",");
      const stamp = `${a.key}|${this.idOf(e)}|${this.idOf(g)}|${this.epoch}|${at}`;
      const was = this.mounted.get(a.id);
      if (was && was.stamp === stamp && was.el === item.el && was.boxes.every((b) => b.isConnected)) continue;
      todo.push({ a, el: item.el, stamp, e, spelling, issues });
    }
    const made: { id: number; m: Mounted }[] = [];
    for (const { a, el, stamp, e, spelling, issues } of todo) {
      const boxes: HTMLElement[] = [];
      const hits: SpellHit[] = [];
      /** What the boxes say, made from the numbers and not read back from the elements (reading a thousand style attributes costs more than measuring the words). */
      const look: string[] = [];
      const box = (from: number, to: number, cls: string, extra: string, set: (el: HTMLElement) => void, hit?: SpellHit) => {
        const start = pointAt(a.segs, from, false);
        const end = pointAt(a.segs, to, true);
        if (!start || !end) return;
        this.range.setStart(start.node, start.offset);
        this.range.setEnd(end.node, end.offset);
        for (const r of Array.from(this.range.getClientRects())) {
          if (r.width < 1 || r.height < 1) continue;
          const b = document.createElement("div");
          b.className = cls;
          b.dataset.from = String(from);
          b.dataset.to = String(to);
          if (a.p !== null) b.dataset.p = String(a.p);
          set(b);
          const css = `left:${((r.left - v.s.left) / v.sc).toFixed(1)}px;top:${((r.top - v.s.top) / v.sc).toFixed(1)}px;width:${(r.width / v.sc).toFixed(1)}px;height:${(r.height / v.sc).toFixed(1)}px`;
          b.style.cssText = css;
          boxes.push(b);
          look.push(`${cls}|${from}|${to}|${a.p}|${extra}|${css}`);
          if (hit) this.hitOf_.set(b, hit);
        }
      };
      for (const err of spelling) {
        box(err.from, err.to, "sp-u", `${err.word}|${e!.lang}`, (b) => {
          b.dataset.word = err.word;
          b.dataset.lang = e!.lang;
        });
      }
      for (const issue of issues) {
        const hit = this.hitOf(a, issue);
        hits.push(hit);
        box(
          issue.from,
          issue.to,
          "sp-g",
          `${hit.word}|${hit.kind}`,
          (b) => {
            b.dataset.word = hit.word;
            b.dataset.kind = hit.kind;
          },
          hit,
        );
      }
      const sig = look.join("\n");
      const was = this.mounted.get(a.id);
      // The same boxes again (its results were set again, say, with nothing different): leave the ones on the page.
      if (was && was.look === sig && was.el === el && was.boxes.every((b) => b.isConnected)) this.mounted.set(a.id, { ...was, stamp });
      else made.push({ id: a.id, m: { el, stamp, look: sig, boxes, hits } });
    }
    // Now the writes: what left the stretch or has nothing to mark any more, then what changed.
    const gone = [...this.mounted.keys()].filter((id) => !present.has(id));
    const leaving = gone.reduce((n, id) => n + this.mounted.get(id)!.boxes.length, 0);
    if (leaving > 300) {
      // The view moved a screen or more: taking thousands of boxes off one by one costs a layout invalidation each, so the layer is set in one write.
      for (const id of gone) this.mounted.delete(id);
      for (const { id } of made) this.mounted.delete(id);
      this.el.replaceChildren(...[...this.mounted.values()].flatMap((m) => m.boxes), ...made.flatMap((x) => x.m.boxes));
      for (const { id, m } of made) this.mounted.set(id, m);
    } else {
      for (const id of gone) this.unmount(id);
      for (const { id, m } of made) {
        this.unmount(id);
        this.el.append(...m.boxes);
        this.mounted.set(id, m);
      }
    }
    this.built = [y0, y1];
    this.lastDrawn = [y0, y1];
  }

  /** Take a paragraph's boxes off the layer. */
  private unmount(id: number) {
    const m = this.mounted.get(id);
    if (!m) return;
    for (const b of m.boxes) b.remove();
    this.mounted.delete(id);
  }

  /** A number for a results object that is the same for the same object (0 for none). */
  private idOf(o: object | undefined): number {
    if (!o) return 0;
    let n = this.ids.get(o);
    if (!n) this.ids.set(o, (n = this.nextId++));
    return n;
  }

  private hitOf(a: Analysis, issue: GrammarIssue): SpellHit {
    return {
      p: a.p,
      from: issue.from,
      to: issue.to,
      word: a.text.slice(issue.from, issue.to),
      kind: issue.kind === "spelling" ? "spelling" : issue.kind === "style" ? "style" : "grammar",
      message: issue.message,
      shortMessage: issue.shortMessage,
      replacements: issue.replacements,
      rule: issue.rule,
    };
  }

  // ---------------------------------------------------------------------------
  // Asking about the errors
  // ---------------------------------------------------------------------------

  private boxAtPoint(cls: string, x: number, y: number): HTMLElement | null {
    for (const u of Array.from(this.el.children) as HTMLElement[]) {
      if (!u.classList.contains(cls)) continue;
      const r = u.getBoundingClientRect();
      if (x >= r.left - 1 && x <= r.right + 1 && y >= r.top - 1 && y <= r.bottom + 1) return u;
    }
    return null;
  }

  /** The misspelled word drawn under a screen point, if any. */
  errorAtPoint(x: number, y: number): SpellHit | null {
    const u = this.boxAtPoint("sp-u", x, y);
    return u ? { p: u.dataset.p === undefined ? null : Number(u.dataset.p), from: Number(u.dataset.from), to: Number(u.dataset.to), word: u.dataset.word ?? "", lang: u.dataset.lang } : null;
  }

  /** The grammar or style problem drawn under a screen point, if any. */
  grammarAtPoint(x: number, y: number): SpellHit | null {
    const u = this.boxAtPoint("sp-g", x, y);
    return u ? (this.hitOf_.get(u) ?? null) : null;
  }

  /** The misspelled word the caret is in or touches, if any. */
  errorAtCaret(): SpellHit | null {
    const c = this.host.caret();
    if (!c || !this.checker) return null;
    const e = this.cache.get(c.p);
    const hit = e?.errors.find((x) => c.o >= x.from && c.o <= x.to);
    return hit ? { p: c.p, ...hit, lang: e!.lang } : null;
  }

  /** The grammar or style problem the caret is in or touches, if any. */
  grammarAtCaret(): SpellHit | null {
    const c = this.host.caret();
    if (!c || !this.grammar) return null;
    for (const m of this.mounted.values()) {
      const hit = m.hits.find((h) => h.p === c.p && c.o >= h.from && c.o <= h.to);
      if (hit) return hit;
    }
    return null;
  }

  /**
   * Every misspelled word and grammar or style problem in the body (and a header, footer or note being edited),
   * in reading order. Looks at the whole document first, and waits for the grammar checkers.
   */
  async allErrors(): Promise<SpellHit[]> {
    if (!this.on) return [];
    this.held = null;
    this.pendingHold = false;
    const els = Array.from(this.host.flow.querySelectorAll<HTMLElement>(".fp[data-p]"));
    const story = this.host.sheets.querySelectorAll<HTMLElement>(".story-edit .fp[data-p]");
    const all = [...els, ...Array.from(story)];
    const list = all.map((el) => this.read(el));
    for (let i = 0; i < list.length; i += 200) if (!(await this.resolve(list.slice(i, i + 200)))) return [];
    // A checker on a server can be slow or away: wait a few seconds for it, and use what has come in by then.
    if (this.grammar) {
      for (let i = 0; i < list.length; i += 200) await Promise.race([this.resolveGrammar(list.slice(i, i + 200)), new Promise((done) => setTimeout(done, GRAMMAR_WAIT))]);
    }
    const out: SpellHit[] = [];
    for (const a of list) {
      if (a.p === null) continue;
      const e = this.cache.get(a.id);
      const g = this.gcache.get(a.id);
      const spelling = e && e.key === a.key ? e.errors : [];
      const hits: SpellHit[] = spelling.map((x) => ({ p: a.p, ...x, lang: e!.lang, kind: "spelling" as const }));
      if (g && g.key === a.key) for (const i of g.issues) if (this.shown(i, a, spelling)) hits.push(this.hitOf(a, i));
      out.push(...hits.sort((x, y) => x.from - y.from || (x.kind === "spelling" ? -1 : 0) - (y.kind === "spelling" ? -1 : 0)));
    }
    return out;
  }
}
