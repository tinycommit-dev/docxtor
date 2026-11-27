// The dictionaries Docxtor's spelling checker uses in the app: wooorm's Hunspell packages for English
// (US and UK), Spanish, French and German. They are kept out of the main bundle twice over: each one is
// its own file that is fetched the first time a document in that language is checked.
//
// The `@dictionaries/*` aliases are set in vite.config.ts (the packages' `exports` hide their files).
//
// Licences of the word lists (they are data files the app fetches, not code Docxtor links to): English
// MIT and BSD (SCOWL), French MPL-2.0 (Grammalecte), Spanish GPL, LGPL or MPL 1.1 at your choice
// (LibreOffice's), German GPL 2 or 3 (igerman98). Keep this list in step with the packages.
import type { SpellingProvider } from "@docxtor/editor";

type Files = () => Promise<[string, string]>;

const DICTIONARIES: Record<string, Files> = {
  "en-us": async () => [(await import("@dictionaries/en-us/index.aff?no-inline")).default, (await import("@dictionaries/en-us/index.dic?no-inline")).default],
  "en-gb": async () => [(await import("@dictionaries/en-gb/index.aff?no-inline")).default, (await import("@dictionaries/en-gb/index.dic?no-inline")).default],
  es: async () => [(await import("@dictionaries/es/index.aff?no-inline")).default, (await import("@dictionaries/es/index.dic?no-inline")).default],
  fr: async () => [(await import("@dictionaries/fr/index.aff?no-inline")).default, (await import("@dictionaries/fr/index.dic?no-inline")).default],
  de: async () => [(await import("@dictionaries/de/index.aff?no-inline")).default, (await import("@dictionaries/de/index.dic?no-inline")).default],
};

/** English as spoken where the British spelling is used. */
const BRITISH = new Set(["gb", "au", "nz", "ie", "za", "in"]);

/** Which dictionary a language tag (`en-US`, `en_GB`, `es-MX`, `fr`…) uses, or null when Docxtor has none. */
export function dictionaryFor(lang: string): string | null {
  const [base, region = ""] = lang.toLowerCase().replace(/_/g, "-").split("-");
  if (base === "en") return BRITISH.has(region) ? "en-gb" : "en-us";
  return base in DICTIONARIES ? base : null;
}

const loaded = new Map<string, Promise<{ aff: Uint8Array; dic: Uint8Array }>>();

async function fetchBytes(url: string): Promise<Uint8Array> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Couldn't load ${url}`);
  return new Uint8Array(await res.arrayBuffer());
}

export const spellingProvider: SpellingProvider = {
  async load(lang) {
    const key = dictionaryFor(lang);
    if (!key) return null;
    let p = loaded.get(key);
    if (!p) {
      p = DICTIONARIES[key]().then(async ([aff, dic]) => ({ aff: await fetchBytes(aff), dic: await fetchBytes(dic) }));
      loaded.set(key, p);
      // A failed load is tried again next time.
      p.catch(() => loaded.delete(key));
    }
    return p;
  },
};
