// Spelling, language detection, grammar, reading, hyphenation and the thesaurus

/** A Hunspell dictionary: the affix file and the word list, as bytes or text (UTF-8 unless the affix file says otherwise). */
export interface SpellingDictionary {
  aff: string | Uint8Array;
  dic: string | Uint8Array;
}

/** A source of Hunspell dictionaries for Docxtor's spelling checker. */
export interface SpellingDictionaryProvider {
  id: string;
  label: string;
  /**
   * The languages it can serve, as BCP 47 tags: `["en", "es"]`. A base tag covers its regions (`en` covers
   * `en-US` and `en-GB`), a tag with a region only that region. `"*"` or nothing means it may serve any
   * language and `load` decides.
   */
  languages?: string[] | "*";
  /** The dictionary for a language (`en-US`, `es`, `fr-CA`…), or null when it has none: then the next provider is asked. */
  load(lang: string): Promise<SpellingDictionary | null>;
}

/** What a language detector says about a piece of text. */
export interface DetectedLanguage {
  /** A BCP 47 tag: `en`, `fr`, `pt-BR`. */
  lang: string;
  /** How sure it is, from 0 to 1. */
  confidence: number;
}

/** Tells what language text is in, for Detect language automatically (the spelling and grammar checkers use it). */
export interface LanguageDetector {
  id: string;
  label: string;
  /** The language of a paragraph's text, or null when it can't tell. It may answer at once or later. */
  detect(text: string): DetectedLanguage | null | Promise<DetectedLanguage | null>;
}

/** One paragraph handed to a grammar checker. */
export interface GrammarParagraph {
  /** The paragraph's id; issues name it. */
  id: number;
  /** Its text as the model has it: objects (images, tabs, note marks) count as one character, U+FFFC. */
  text: string;
  /** Its language, a BCP 47 tag such as `en-US`. */
  lang: string;
}

/** A batch of paragraphs to check. */
export interface GrammarRequest {
  paragraphs: GrammarParagraph[];
}

/** What a checker found. */
export interface GrammarIssue {
  /** The id of the paragraph it is in. */
  paragraph: number;
  /** The words, as UTF-16 offsets into that paragraph's text: `[from, to)`. */
  from: number;
  to: number;
  /** What is wrong, in a sentence. */
  message: string;
  /** A few words for a tooltip or a label. */
  shortMessage?: string;
  /** Text to put in place of the words, best first. */
  replacements: string[];
  rule: { id: string; description?: string; category?: string };
  /** `grammar` (the default), `style` (a hint, drawn more softly) or `spelling` (a typo the checker found). */
  kind?: "grammar" | "style" | "spelling";
  /** The id of the checker that found it; the host fills it in. */
  source?: string;
}

// ---------------------------------------------------------------------------
// Readings
// ---------------------------------------------------------------------------

/** Something that knows how East Asian text is read (a kanji dictionary, a pinyin table, a service). */
export interface ReadingProvider {
  id: string;
  label: string;
  /** The languages it knows (`["ja"]`, `["zh-CN", "zh-TW"]`; `"*"` or nothing means any, and `reading` decides). */
  languages?: string[] | "*";
  /**
   * The reading of `text`, as the phonetic guide over it should say (hiragana for Japanese, pinyin for Chinese).
   * `null` means it has no opinion, and the next provider is asked.
   */
  reading(text: string, lang: string): string | null | Promise<string | null>;
}

// ---------------------------------------------------------------------------
// Hyphenation
// ---------------------------------------------------------------------------

/** Something that knows where a word may be hyphenated (a pattern dictionary, a rule set, a service). */
export interface Hyphenator {
  id: string;
  label: string;
  /** The languages it knows, as for dictionaries (`["en", "es"]`; `"*"` or nothing means any, and `hyphenate` decides). */
  languages?: string[] | "*";
  /**
   * Where `word` may be broken: UTF-16 offsets into it, ascending, each between the first and the last letter (a break at
   * 3 puts the first three letters before the hyphen). `null` means it has no opinion, and the next hyphenator is asked.
   */
  hyphenate(word: string, lang: string): number[] | null | Promise<number[] | null>;
}

// ---------------------------------------------------------------------------
// The thesaurus
// ---------------------------------------------------------------------------

/** One meaning of a word, with the words that mean about the same. */
export interface ThesaurusSense {
  /** Part of speech, in English and lower case: `noun`, `verb`, `adjective`, `adverb`. */
  pos?: string;
  /** A few words that tell this meaning from the others (the closest word, or a gloss). */
  meaning?: string;
  /** The word this meaning is for, when it is not the word asked about: the base word of an inflected form (`run` for `running`). */
  word?: string;
  /** Words with about the same meaning, best first. */
  synonyms: string[];
  antonyms?: string[];
}

/** What a thesaurus knows about a word. */
export interface ThesaurusEntry {
  /** The word the senses are for, when it is not the word asked about (the look-up found `run` for `running`). */
  word?: string;
  senses: ThesaurusSense[];
}

/** An entry as the host hands it on: each sense says which thesaurus it came from. */
export interface ThesaurusAnswer {
  /** The word asked about. */
  word: string;
  /** Meanings from every thesaurus that has the word, by thesaurus. */
  results: { source: string; label: string; word: string; senses: ThesaurusSense[] }[];
}

/**
 * A source of synonyms for Review, Thesaurus (and the right-click menu): a word list that ships with the extension, a
 * dictionary, a service. A thesaurus that sends the word anywhere says so in `note` and is a separate, switched off extension.
 */
export interface Thesaurus {
  id: string;
  label: string;
  /** The languages it knows, as for dictionaries (`["en"]`; `"*"` or nothing means any, and `lookup` decides). */
  languages?: string[] | "*";
  /** One plain sentence about what leaves the device. */
  note?: string;
  /** Look a word up in a language (`en-US`, `es`). `null` or no senses means it has nothing for the word. */
  lookup(word: string, lang: string): ThesaurusEntry | null | Promise<ThesaurusEntry | null>;
}

// ---------------------------------------------------------------------------
// Inspect document
// ---------------------------------------------------------------------------

/** Something that finds grammar and style problems in text. */
export interface GrammarChecker {
  id: string;
  label: string;
  /** The languages it can check (as for dictionaries); paragraphs in other languages are not sent to it. Default: all. */
  languages?: string[] | "*";
  /** Check a batch of paragraphs. Resolve with every issue found; never reject for a paragraph it can't check. */
  check(request: GrammarRequest): Promise<GrammarIssue[]>;
}

