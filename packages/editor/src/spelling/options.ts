// What the spelling and grammar checkers leave alone, shared by the two built-in extensions that check
// (Spelling and Grammar rules) and the one that offers the choices (Proofing options). The choices are Word's:
// File, Options, Proofing, "When correcting spelling in Microsoft Office programs".
import type { Disposable } from "@docxtor/extensions";
import { DEFAULT_WORD_OPTIONS, type WordOptions } from "./words";

export interface ProofingSettings extends WordOptions {
  /** Flag the same word twice in a row ("the the"). A grammar rule, so it follows the grammar checkers' language. */
  repeatedWords: boolean;
  /** Flag words that are often mixed up (their and there, its and it's, then and than). English grammar rules. */
  confusedWords: boolean;
  /** Check a paragraph the file gives no language in the language its words are in. */
  detectLanguage: boolean;
  /** French words in capitals must keep their accents (ECOLE is wrong, ÉCOLE right). Off: capitals may drop them. */
  frenchAccentedCapitals: boolean;
  /** Suggest corrections from the main dictionary only, not from the custom dictionaries. */
  mainDictionaryOnly: boolean;
  /** Show readability statistics when the spelling and grammar check is done. */
  showReadability: boolean;
}

export const DEFAULT_PROOFING: ProofingSettings = {
  ...DEFAULT_WORD_OPTIONS,
  repeatedWords: true,
  confusedWords: true,
  detectLanguage: true,
  frenchAccentedCapitals: false,
  mainDictionaryOnly: false,
  showReadability: false,
};

const KEYS = Object.keys(DEFAULT_PROOFING) as (keyof ProofingSettings)[];

export class ProofingOptions {
  private value: ProofingSettings = { ...DEFAULT_PROOFING };
  private listeners = new Set<() => void>();

  get(): Readonly<ProofingSettings> {
    return this.value;
  }

  /** The part the spelling checker reads. */
  words(): WordOptions {
    const { ignoreUppercase, ignoreNumbers, ignoreAddresses } = this.value;
    return { ignoreUppercase, ignoreNumbers, ignoreAddresses };
  }

  /** Take in what was saved (anything that isn't a yes or no is left at its default). */
  load(saved: unknown) {
    const next = { ...DEFAULT_PROOFING };
    if (saved && typeof saved === "object") for (const k of KEYS) if (typeof (saved as Record<string, unknown>)[k] === "boolean") next[k] = (saved as Record<string, boolean>)[k];
    const changed = KEYS.some((k) => next[k] !== this.value[k]);
    this.value = next;
    if (changed) for (const fn of this.listeners) fn();
  }

  set(patch: Partial<ProofingSettings>) {
    const next = { ...this.value };
    let changed = false;
    for (const k of KEYS) {
      const v = patch[k];
      if (typeof v === "boolean" && v !== next[k]) {
        next[k] = v;
        changed = true;
      }
    }
    if (!changed) return;
    this.value = next;
    for (const fn of this.listeners) fn();
  }

  onChange(fn: () => void): Disposable {
    this.listeners.add(fn);
    return { dispose: () => void this.listeners.delete(fn) };
  }
}
