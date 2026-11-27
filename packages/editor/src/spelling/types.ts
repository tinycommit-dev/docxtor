// What the spelling checker needs from its host, and the messages between the page and the worker.

import type { SpellingDictionary } from "@docxtor/extensions";

export type { SpellingDictionary };

/**
 * Where the dictionaries come from. `load` gets a language (`en-US`, `es`, `fr-CA`…) and returns the
 * dictionary to use for it, or null when it has none: then text in that language isn't checked.
 * The editor never bundles dictionaries. A host passes a provider as the `spelling` option of
 * `DocxtorEditor`, which is a shortcut for registering it with `ctx.spelling.dictionaries`; extensions can
 * register more, and the latest registered that has a language wins.
 */
export interface SpellingProvider {
  load(lang: string): Promise<SpellingDictionary | null>;
}

/** Page to worker. */
export type SpellRequest =
  | { id: number; type: "load"; key: string; aff: string | Uint8Array; dic: string | Uint8Array; extra?: readonly (readonly [string, string?])[] }
  | { id: number; type: "check"; key: string; words: string[]; frenchCapitals?: boolean }
  | { id: number; type: "suggest"; key: string; word: string };

/** Worker to page. */
export type SpellResponse = { id: number; ok: true; result?: boolean[] | string[] } | { id: number; ok: false; error: string };
