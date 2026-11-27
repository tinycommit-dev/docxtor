// Typing substitutions ("autoformat"): smart quotes, capital letters at the start of a sentence, links,
// Markdown shortcuts and symbol replacements such as (c) to ©. The rules only read a little text before
// the caret and answer with ops; the editor applies them as a step of their own, right after the
// character that triggered them, so one undo (or Backspace) puts back what was typed.
// Nothing here touches the DOM or the engine: the editor hands over what the rules may look at.
import type { FormatState, MPos, Op, RunPatch } from "@docxtor/core";

/** How far back from the caret the rules look. Keeps a keystroke cheap in long paragraphs. */
export const HORIZON = 120;

export interface Substitution {
  /** The text that is typed. */
  find: string;
  /** What it becomes. */
  with: string;
}

export interface AutoformatSettings {
  smartQuotes: boolean;
  capitalize: boolean;
  links: boolean;
  markdown: boolean;
  substitutions: boolean;
  replacements: Substitution[];
  /**
   * Abbreviations that end in a period without ending a sentence ("dept", "approx"), so the word after one is not
   * capitalised. They add to the built-in list ("etc", "e.g", "Dr"...). Written without the period.
   */
  exceptions: string[];
}

export const DEFAULT_REPLACEMENTS: readonly Substitution[] = [
  { find: "(c)", with: "©" },
  { find: "(r)", with: "®" },
  { find: "(tm)", with: "™" },
  { find: "--", with: "—" },
  { find: "->", with: "→" },
  { find: "<-", with: "←" },
  { find: "=>", with: "⇒" },
  { find: "...", with: "…" },
  { find: "1/2", with: "½" },
  { find: "1/4", with: "¼" },
  { find: "3/4", with: "¾" },
  { find: "!=", with: "≠" },
  { find: "<=", with: "≤" },
  { find: ">=", with: "≥" },
  { find: "+-", with: "±" },
];

export const DEFAULT_AUTOFORMAT: AutoformatSettings = {
  smartQuotes: true,
  capitalize: true,
  links: true,
  markdown: true,
  substitutions: true,
  replacements: DEFAULT_REPLACEMENTS.map((r) => ({ ...r })),
  exceptions: [],
};

/** An abbreviation as stored: trimmed, without its closing period and without spaces. */
export function cleanException(raw: string): string {
  return raw.trim().replace(/\.+$/, "").replace(/\s+/g, "");
}

/** Settings from whatever was saved (anything missing or malformed falls back to the defaults). */
export function normalizeSettings(raw: unknown): AutoformatSettings {
  const o = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const flag = (k: keyof AutoformatSettings) => (typeof o[k] === "boolean" ? (o[k] as boolean) : true);
  const list = Array.isArray(o.replacements)
    ? (o.replacements as unknown[]).flatMap((r) => {
        const x = r as Partial<Substitution> | null;
        return x && typeof x.find === "string" && x.find !== "" && typeof x.with === "string" ? [{ find: x.find, with: x.with }] : [];
      })
    : DEFAULT_REPLACEMENTS.map((r) => ({ ...r }));
  return {
    smartQuotes: flag("smartQuotes"),
    capitalize: flag("capitalize"),
    links: flag("links"),
    markdown: flag("markdown"),
    substitutions: flag("substitutions"),
    replacements: list,
    exceptions: Array.isArray(o.exceptions) ? [...new Set((o.exceptions as unknown[]).flatMap((x) => (typeof x === "string" && cleanException(x) ? [cleanException(x)] : [])))] : [],
  };
}

/** What the rules may look at. Positions are in the paragraph `p`, UTF-16 offsets as everywhere in the model. */
export interface AutoContext {
  p: number;
  /** The caret, after the character that was just typed. */
  end: number;
  /** The last `HORIZON` characters of the paragraph before the caret; for a typed character, it ends with it. */
  text: string;
  /** The document's default language, e.g. `de-DE`. */
  lang?: string;
  /** Format at the caret (asked for only by rules that apply). */
  state(): FormatState | null;
  /** Format of the character at index `i` of the paragraph. */
  formatAt(i: number): FormatState | null;
  /** The ops that give a paragraph a list style (numbering carries on from the list above). */
  listOps(id: number, style: string): Op[];
}

export interface AutoPlan {
  /** Applied as one undo step. */
  ops: Op[];
  /** Where the caret goes; null leaves it to the engine (a horizontal line puts it on the paragraph after). */
  caret: MPos | null;
  /** Formatting for what is typed next (so a closed `**bold**` doesn't carry on in bold). */
  pending?: RunPatch;
  /** The rule made the Enter key's paragraph break itself (`---` and Enter): don't split again. */
  consumesEnter?: boolean;
}

// -- helpers ------------------------------------------------------------------------------------

const LETTER_OR_DIGIT = /[\p{L}\p{N}]/u;
const isAlnum = (c: string) => LETTER_OR_DIGIT.test(c);

/** Replace `[from, to)` with `text`, keeping the formatting of the text it replaces. Do these from the end of the paragraph backwards. */
function replaceOps(p: number, from: number, to: number, text: string): Op[] {
  return [
    { op: "insert_text", at: { p, o: to }, text },
    { op: "delete", from: { p, o: from }, to: { p, o: to } },
  ];
}

const MONO = /consolas|courier|monospace|\bmono\b|menlo|monaco|source code|fira (code|mono)|jetbrains|cousine|lucida console|dejavu sans mono|sf mono|ubuntu mono/i;
const CODE_STYLE = /code|preformatted|source|listing|verbatim|typewriter/i;

/** A paragraph in a code style, or typed in a monospace font: no substitutions there. */
function inCode(ctx: AutoContext): boolean {
  const st = ctx.state();
  if (!st) return false;
  return CODE_STYLE.test(st.style ?? "") || CODE_STYLE.test(st.para.style ?? "") || MONO.test(st.run.font ?? "");
}

// -- smart quotes ---------------------------------------------------------------------------------

interface Quotes {
  dOpen: string;
  dClose: string;
  sOpen: string;
  sClose: string;
}
const NNBSP = " ";

/** The quotation marks for a document language: German and French have their own; Spanish and everything else use the English ones. */
export function quotesFor(lang?: string): Quotes {
  const l = (lang ?? "").toLowerCase();
  if (l.startsWith("de")) return { dOpen: "„", dClose: "“", sOpen: "‚", sClose: "‘" };
  if (l.startsWith("fr")) return { dOpen: `«${NNBSP}`, dClose: `${NNBSP}»`, sOpen: "‘", sClose: "’" };
  return { dOpen: "“", dClose: "”", sOpen: "‘", sClose: "’" };
}

function quoteRule(ctx: AutoContext): AutoPlan | null {
  const ch = ctx.text.slice(-1);
  if (ch !== '"' && ch !== "'") return null;
  const prev = ctx.text.slice(-2, -1);
  // After a space, an opening bracket, a dash or another opening quote (or at the start): opening.
  const opening = prev === "" ? ctx.end === 1 : /[\s([{<—–‘“‚„«‹]/.test(prev);
  const q = quotesFor(ctx.lang);
  let text: string;
  if (ch === '"') text = opening ? q.dOpen : q.dClose;
  else if (opening) text = q.sOpen;
  // After a letter or digit it closes a single quote that is open (German ‚nein‘); otherwise it is an apostrophe (don't).
  else text = !isAlnum(prev) || ctx.text.lastIndexOf(q.sOpen) > ctx.text.lastIndexOf(q.sClose) ? q.sClose : "’";
  return { ops: replaceOps(ctx.p, ctx.end - 1, ctx.end, text), caret: { p: ctx.p, o: ctx.end - 1 + text.length } };
}

// -- substitutions --------------------------------------------------------------------------------

const SYMBOLS_ONLY = /^[^\p{L}\p{N}]+$/u;

function substitutionRule(ctx: AutoContext, list: Substitution[]): AutoPlan | null {
  const { text, p, end } = ctx;
  const ch = text.slice(-1);
  const edit = (from: number, to: number, rep: string): AutoPlan => ({
    ops: replaceOps(p, from, to, rep),
    caret: { p, o: end + rep.length - (to - from) },
  });
  const dash = list.find((r) => r.find === "--");
  if (isAlnum(ch)) {
    // word--word: the dash turns into an em dash when the next word starts.
    if (dash && /[\p{L}\p{N}]--[\p{L}\p{N}]$/u.test(text.slice(-4))) return edit(end - 3, end - 1, dash.with);
    return null;
  }
  // word -- word: when the space after the dashes is typed.
  if (dash && ch === " " && /\S --$/.test(text.slice(-5, -1))) return edit(end - 3, end - 1, dash.with);
  const before = text.slice(0, -1);
  const lower = before.toLowerCase();
  // Longest trigger first, so (tm) can't lose to a shorter one.
  const sorted = [...list].filter((r) => r.find !== "" && r.find !== "--").sort((a, b) => b.find.length - a.find.length);
  for (const r of sorted) {
    if (SYMBOLS_ONLY.test(r.find)) {
      // Symbols such as -> or ... turn as soon as they are complete.
      if (text.endsWith(r.find)) return edit(end - r.find.length, end, r.with);
      continue;
    }
    // Others (the letters and digits of (c) and 1/2) wait for a space or punctuation after them.
    if (!lower.endsWith(r.find.toLowerCase())) continue;
    const start = before.length - r.find.length;
    if (LETTER_OR_DIGIT.test(r.find[0]) && start > 0 && /[\p{L}\p{N}/]/u.test(before[start - 1])) continue;
    if (start === 0 && end - text.length > 0) continue;
    return edit(end - 1 - r.find.length, end - 1, r.with);
  }
  return null;
}

// -- capital letters ------------------------------------------------------------------------------

const ABBREVIATIONS = ["e\\.g", "i\\.e", "etc", "vs", "cf", "mr", "mrs", "ms", "mx", "dr", "prof", "sr", "jr", "st", "mt", "no", "fig", "inc", "ltd", "co", "approx", "esp", "al", "ca"];
const patterns = new Map<string, RegExp>();

/** The built-in abbreviations with the person's own, as one pattern that matches the end of the text before a period's space. */
function abbreviationPattern(extra: readonly string[]): RegExp {
  const key = extra.join("\n");
  let re = patterns.get(key);
  if (!re) {
    const own = extra.map((x) => x.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
    re = new RegExp(`(?:^|[\\s(])(?:${[...ABBREVIATIONS, ...own].join("|")})\\.$`, "i");
    patterns.set(key, re);
  }
  return re;
}
const INITIAL_OR_NUMBER = /(?:^|[\s(])(?:\p{L}|\d+)\.$/u;

/** The word before the caret, if it is one to capitalise: a sentence's first word, or a lone "i". */
function capitalRule(ctx: AutoContext, trigger: boolean, exceptions: readonly string[] = []): AutoPlan | null {
  const { text, p, end } = ctx;
  // `text` includes the character that completed the word; Enter completes it with none.
  const before = trigger ? text.slice(0, -1) : text;
  const chunkAt = before.search(/\S*$/);
  const chunk = before.slice(chunkAt);
  const lead = /^[(["'“‘«¿¡]*/.exec(chunk)![0];
  const word = chunk.slice(lead.length);
  if (!/^\p{L}[\p{L}\p{M}'’]*$/u.test(word)) return null;
  // The window can start in the middle of a word: only trust a word that starts after something.
  const windowStart = end - text.length;
  if (chunkAt === 0 && windowStart > 0) return null;
  const first = word[0];
  const upper = first.toLocaleUpperCase();
  if (upper === first) return null;
  // iPhone, eBay, macOS: capitals inside a word are on purpose.
  if (/\p{Lu}/u.test(word.slice(1))) return null;
  const lone = /^i(?:['’](?:m|ll|ve|d))?$/.test(word);
  let ok = lone && !lead.includes("(");
  if (!ok) {
    const prefix = before.slice(0, chunkAt);
    if (prefix === "") ok = windowStart === 0;
    else {
      const m = /([.!?])["'”’)\]]*\s+$/.exec(prefix);
      if (m) {
        const head = prefix.slice(0, m.index + 1);
        ok = !(m[1] === "." && (abbreviationPattern(exceptions).test(head) || INITIAL_OR_NUMBER.test(head)));
      }
    }
  }
  if (!ok) return null;
  const at = windowStart + chunkAt + lead.length;
  return { ops: replaceOps(p, at, at + 1, upper), caret: { p, o: end } };
}

// -- links ---------------------------------------------------------------------------------------

const URL_RE = /^https?:\/\/[^\s/$.?#][^\s]*$/i;
const WWW_RE = /^www\.[\p{L}\p{N}-]+(?:\.[\p{L}\p{N}-]+)+(?:[/?#]\S*)?$/iu;
const EMAIL_RE = /^[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}$/;

function linkRule(ctx: AutoContext, trigger: boolean): AutoPlan | null {
  const { text, p, end } = ctx;
  const before = trigger ? text.slice(0, -1) : text;
  const chunkAt = before.search(/\S*$/);
  let chunk = before.slice(chunkAt);
  if (chunk.length < 5) return null;
  const lead = /^[(<["'“‘]*/.exec(chunk)![0].length;
  chunk = chunk.slice(lead).replace(/[.,;:!?)\]}'"”’>]+$/, "");
  let url: string | null = null;
  if (URL_RE.test(chunk)) url = chunk;
  else if (WWW_RE.test(chunk)) url = `https://${chunk}`;
  else if (EMAIL_RE.test(chunk)) url = `mailto:${chunk}`;
  if (!url) return null;
  const windowStart = end - text.length;
  if (chunkAt === 0 && windowStart > 0) return null;
  const from = windowStart + chunkAt + lead;
  if (ctx.formatAt(from)?.link) return null;
  return { ops: [{ op: "set_link", from: { p, o: from }, to: { p, o: from + chunk.length }, url }], caret: { p, o: end } };
}

// -- Markdown --------------------------------------------------------------------------------------

function blockRule(ctx: AutoContext): AutoPlan | null {
  const { text, p, end } = ctx;
  if (text.length !== end || text.slice(-1) !== " " || end > 9) return null;
  let m: RegExpExecArray | null;
  const clear: Op = { op: "delete", from: { p, o: 0 }, to: { p, o: end } };
  const done = (...ops: Op[]): AutoPlan => ({ ops: [clear, ...ops], caret: { p, o: 0 } });
  if (ctx.state()?.list) return null;
  if ((m = /^(#{1,6}) $/.exec(text))) return done({ op: "set_style", ids: [p], style: `Heading ${m[1].length}` });
  if (text === "> ") return done({ op: "set_style", ids: [p], style: "Quote" });
  if (text === "- " || text === "* ") return done(...ctx.listOps(p, "disc"));
  if ((m = /^\d{1,3}([.)]) $/.exec(text))) return done(...ctx.listOps(p, m[1] === ")" ? "paren" : "decimal"));
  if (text === "[] " || text === "[ ] ") return done(...ctx.listOps(p, "checklist"));
  return null;
}

interface Inline {
  re: RegExp;
  marker: number;
  patch: RunPatch;
  /** What to type next so the span doesn't go on. */
  reset: (base: FormatState["run"]) => RunPatch;
}

const INLINE: Record<string, Inline[]> = {
  "*": [
    { re: /(^|[^\p{L}\p{N}*])\*\*(?=\S)([^*]*?[^\s*])\*\*$/u, marker: 2, patch: { bold: true }, reset: (b) => ({ bold: !!b.bold }) },
    { re: /(^|[^\p{L}\p{N}*])\*(?=[^\s*])([^*]*?[^\s*])\*$/u, marker: 1, patch: { italic: true }, reset: (b) => ({ italic: !!b.italic }) },
  ],
  _: [{ re: /(^|[^\p{L}\p{N}_])_(?=[^\s_])([^_]*?[^\s_])_$/u, marker: 1, patch: { italic: true }, reset: (b) => ({ italic: !!b.italic }) }],
  "~": [{ re: /(^|[^~])~~(?=\S)([^~]*?[^\s~])~~$/u, marker: 2, patch: { strike: true }, reset: (b) => ({ strike: !!b.strike }) }],
  "`": [{ re: /(^|[^`])`(?=[^\s`])([^`]*?[^\s`])`$/u, marker: 1, patch: { font: "Consolas" }, reset: (b) => ({ font: b.font ?? "" }) }],
};

function inlineRule(ctx: AutoContext): AutoPlan | null {
  const { text, p, end } = ctx;
  const rules = INLINE[text.slice(-1)];
  if (!rules) return null;
  const windowStart = end - text.length;
  for (const r of rules) {
    const m = r.re.exec(text);
    if (!m) continue;
    const open = windowStart + m.index + m[1].length;
    const inner = m[2].length;
    const base = ctx.formatAt(open)?.run ?? {};
    return {
      ops: [
        { op: "delete", from: { p, o: end - r.marker }, to: { p, o: end } },
        { op: "delete", from: { p, o: open }, to: { p, o: open + r.marker } },
        { op: "format_text", from: { p, o: open }, to: { p, o: open + inner }, format: r.patch },
      ],
      caret: { p, o: open + inner },
      pending: r.reset(base),
    };
  }
  return null;
}

// -- entry points -----------------------------------------------------------------------------------

/** Triggers that complete a word for the capital-letter rule. */
const WORD_END = new Set([" ", ",", ";", ":", "!", "?"]);

/**
 * The plan for a character that was just typed (the document already has it, and `ctx.text` ends with it), or null.
 * The first rule that applies wins, so one keystroke makes at most one extra undo step.
 */
export function autoformatTyped(ctx: AutoContext, s: AutoformatSettings): AutoPlan | null {
  const ch = ctx.text.slice(-1);
  if (!ch) return null;
  let plan: AutoPlan | null = null;
  if (s.markdown) plan = blockRule(ctx) ?? inlineRule(ctx);
  if (!plan && s.links && ch === " ") plan = linkRule(ctx, true);
  if (!plan && s.smartQuotes) plan = quoteRule(ctx);
  if (!plan && s.substitutions) plan = substitutionRule(ctx, s.replacements);
  if (!plan && s.capitalize && WORD_END.has(ch)) plan = capitalRule(ctx, true, s.exceptions);
  // Not in code: a code paragraph, or text in a monospace font.
  return plan && !inCode(ctx) ? plan : null;
}

/** The plan for Enter at the end of `ctx.text`: `---` becomes a line; a link or a capital letter is done before the break. */
export function autoformatEnter(ctx: AutoContext, s: AutoformatSettings): AutoPlan | null {
  const { text, p, end } = ctx;
  let plan: AutoPlan | null = null;
  if (s.markdown && text === "---" && end === 3 && !ctx.state()?.list) {
    plan = {
      ops: [{ op: "delete", from: { p, o: 0 }, to: { p, o: 3 } }, { op: "insert_horizontal_line", at: { p, o: 0 } }],
      caret: null,
      consumesEnter: true,
    };
  }
  if (!plan && s.links) plan = linkRule(ctx, false);
  if (!plan && s.capitalize) plan = capitalRule(ctx, false, s.exceptions);
  return plan && !inCode(ctx) ? plan : null;
}
