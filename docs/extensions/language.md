# Extensions for language

Spelling and grammar, hyphenation, East Asian readings, language packs and synonyms. Part of the [extensions guide](index.md).

## Spelling and grammar

Proofing is two contribution points, so Docxtor's own checkers and outside services sit side by side and each can be turned off in the Extensions dialog. Docxtor's own are built-in extensions (`docxtor.spelling`, `docxtor.grammar`, `docxtor.grammar.rules`); in the app, the word lists are one more (`docxtor.dictionaries`) and LanguageTool another (`docxtor.languagetool`).

**Dictionaries.** `ctx.spelling.dictionaries.register({ id, label, languages, load })`. `languages` is a list of BCP 47 tags (`["en", "es"]`): a base tag covers its regions, a tag with a region only that region, and `"*"` (or nothing) leaves it to `load`. `load(lang)` returns `{ aff, dic }` (text or bytes) or `null`; null passes the language on to the next provider. The `spelling` option of `DocxtorEditor` is a shortcut that registers one provider, so embedders keep working.

**Language detectors.** `ctx.spelling.languageDetectors.register({ id, label, detect })`. `detect(text)` gets a paragraph's text (up to 600 characters) and returns `{ lang: "es", confidence: 0.9 }`, null when it can't tell, or a promise of either. Docxtor only asks about paragraphs of six words or more that are in the document's own language, and only uses an answer for another language; the paragraph is then checked with that language's dictionary and grammar checkers (left unchecked when there are none). Nothing is written to the file. `host.detectLanguage(text, sure?)` asks them as Docxtor does.

**Grammar checkers.** `ctx.grammar.register({ id, label, languages, check })`. `check` gets a batch of paragraphs, each `{ id, text, lang }` (the text as the model has it, with objects as U+FFFC and links, code and web addresses masked the same way), and resolves with issues:

| Field | |
|---|---|
| `paragraph` | the paragraph's `id` |
| `from`, `to` | UTF-16 offsets into that paragraph's text |
| `message`, `shortMessage` | what is wrong, in a sentence |
| `replacements` | text to put in place of the words, best first |
| `rule` | `{ id, description?, category? }` |
| `kind` | `grammar` (default, a wavy blue line), `style` (dotted blue) or `spelling` (a typo; drawn like a misspelling) |

Only paragraphs in a language the checker covers are sent to it. The host drops issues that don't fit the text, merges the lists of all checkers, and removes exact duplicates (same words, message and replacements). A checker that fails or takes over half a minute is left out and the others still count. A misspelled word keeps its red mark: a grammar issue on the same words is not drawn. Turning a checker's extension off clears its underlines. Results are kept per paragraph and its text, so a checker is only asked again when the text, its language or the set of checkers changes.

<!-- example: ext-grammar -->
```ts
import { defineExtension } from "@docxtor/extensions";

export default defineExtension({
  id: "acme.no-bananas",
  name: "No bananas",
  activate(ctx) {
    ctx.grammar.register({
      id: "acme.no-bananas",
      label: "No bananas",
      languages: ["en"],
      async check({ paragraphs }) {
        return paragraphs.flatMap((p) => {
          const at = p.text.indexOf("banana");
          if (at < 0) return [];
          return [{ paragraph: p.id, from: at, to: at + 6, message: "Fruit is not allowed here.", replacements: ["pear"], rule: { id: "BANANA" }, kind: "style" }];
        });
      },
    });
  },
});
```

**Each paragraph has a language.** The engine reports a paragraph's language (`RPara.lang`: its first run with text, else the paragraph mark, else its style and the styles it is based on, else the document default), the editor puts it on the paragraph as `data-lang`, and the spelling and grammar checkers use it. File, Language… with a selection sets it for those paragraphs. A paragraph in a language with no dictionary is not spell checked.

**Text that is not checked.** "Do not check spelling or grammar" is a property of text (`w:noProof`, `RunFmt.noProof`, `format_text` with `noProof: true`; File, Language… sets it on the selection). The editor puts `data-np="1"` on those runs, and the spelling and grammar checkers treat them like links and code: never asked about. A character or paragraph style can carry it too.

**Proofing options.** The built-in extension `docxtor.proofing` (Tools, Spelling and grammar, Proofing options…) keeps Word's choices between sessions: ignore words in capitals, words with numbers, and Internet and file addresses (spelling), and flag repeated words (a rule of Docxtor's grammar checker). Hide spelling errors and Hide grammar errors in this document only are the document's own settings (`w:hideSpellingErrors`, `w:hideGrammaticalErrors`, set with `set_layout_settings`): the marks are not drawn but the check panel still finds the problems. Recheck document forgets the words and rules that were ignored. The options are read by `docxtor.spelling` and `docxtor.grammar.rules` through `ProofingOptions` (`packages/editor/src/spelling/options.ts`); a checker an extension adds is not affected.

**Sending text away.** An extension that sends the document's text to a server should say so: `privacy` is one plain sentence that the Extensions dialog shows and asks about before the extension is turned on. `enabledByDefault: false` starts it off. LanguageTool in the app (`web/src/languagetool.ts`) does both, keeps its server, username and API key with `ctx.state`, and shows where the text goes in its settings (Tools, Spelling and grammar, LanguageTool settings…).

## Hyphenation

`ctx.hyphenation.register({ id, label, languages, hyphenate })` adds a hyphenator: something that knows where a word may be broken. Hyphenate manually (Tools) asks the host, `ExtensionHost.hyphenationPoints(word, lang)`, which tries the hyphenators that cover the language, latest registered first, and uses the first answer. `hyphenate(word, lang)` returns the offsets in the word where a hyphen may go (UTF-16, ascending, each between its first and last letter: 3 puts three letters before the hyphen), or null when it has no opinion, and the next hyphenator is asked. Offsets outside the word are dropped, and a hyphenator that throws is reported and skipped. Docxtor's own hyphenator is the built-in extension `docxtor.pageLayout` (`packages/editor/src/hyphenation/rules.ts`): syllable rules for English, Spanish, French, German, Italian and Portuguese. A pattern dictionary (TeX patterns), or a service, can sit beside it.

<!-- example: ext-hyphenation -->
```ts
ctx.hyphenation.register({
  id: "acme.patterns",
  label: t("Acme hyphenation patterns"),
  languages: ["en-GB"],
  hyphenate: (word) => patterns.breaks(word),
});
```

The breaks it offers become optional hyphens (U+00AD) in the text when the person agrees. Automatic hyphenation, the zone and the limit of hyphens in a row are the document's settings (`w:autoHyphenation`, `w:hyphenationZone`, `w:consecutiveHyphenLimit`, `w:doNotHyphenateCaps`); the browser does the automatic breaking on screen and follows only the first.

## Readings

`ctx.readings.register({ id, label, languages, reading })` adds a reading provider: something that knows how East Asian text is read. The Phonetic guide box (Format, Asian layout) asks the host, `ExtensionHost.readingFor(text, lang)`, when it opens on text with no guide yet and when the person clicks Suggest. The host tries the providers that cover the language, latest registered first, and uses the first answer; the box says which provider it was. `reading(text, lang)` returns the guide text, or null when it has no opinion and the next provider is asked. A provider that throws is reported and skipped. Docxtor's own provider is the built-in extension `docxtor.eastAsian` (`packages/editor/src/builtins/east-asian.ts`): it reads kana as hiragana (a katakana base gets a hiragana guide) and has no opinion about anything else. A kanji dictionary, a pinyin table or an online service sits beside it; one that sends text off the device is a separate extension, off by default, that says what it sends.

<!-- example: ext-readings -->
```ts
ctx.readings.register({
  id: "acme.kanji",
  label: t("Acme kanji dictionary"),
  languages: ["ja"],
  reading: (text) => dictionary.lookup(text) ?? null,
});
```

## Language packs

A language pack is data, not code, so Docxtor can build its interface in the right language from the first paint. Put it in `contributes`:

<!-- example: ext-language-pack -->
```ts
import { defineExtension, type LanguagePack } from "@docxtor/extensions";
import messages from "./fr.json"; // { "Find and replace": "Rechercher et remplacer", … }

export default defineExtension({
  id: "acme.french",
  name: "Français",
  contributes: { languages: [{ locale: "fr", label: "Français", messages } satisfies LanguagePack] },
  activate() {},
});
```

The keys are Docxtor's English text, and [`locales/source.json`](../../locales/source.json) lists every one of them. Placeholders like `{name}` stay as they are. A message that depends on a count can give one form per plural category: `"{n} words": { "one": "{n} mot", "other": "{n} mots" }`. Anything missing falls back to English. `node scripts/messages.mjs --pack fr.json` reports what's missing and anything that no longer exists. [`web/src/languages/es.json`](../../web/src/languages/es.json) is a complete example.

People pick a language under View, Display language, and Docxtor reloads in it. Embedders pass `locale: "fr"` (or `locale="fr"` on `<docxtor-editor>`); there's one language per page. Opening the app with `?lang=en-XA` shows every translatable string accented and in ⟦brackets⟧, which makes any untranslated text stand out.

Your own extensions' text can use the same system: `t("Count words")`, `tn(n, "{n} word", "{n} words")` and `msg()` from `@docxtor/extensions`.

## Thesaurus

`ctx.thesaurus.register({ id, label, languages, note?, lookup })` adds a source of synonyms for Tools, Thesaurus (Shift+F7) and the Synonyms… line of the right-click menu. `languages` is a list of BCP 47 tags, as for dictionaries. `lookup(word, lang)` returns `{ word?, senses }` or `null`; each sense is `{ pos?, meaning?, word?, synonyms, antonyms? }`. `pos` is `noun`, `verb`, `adjective` or `adverb`; `meaning` is a few words that tell the sense from the others (the closest word, or a gloss); `word` names the base word when the sense is for another word than the one asked about (`run` for `running`), and `word` of the entry does the same for all of its senses.

The panel asks every thesaurus that serves the paragraph's language, through `ExtensionHost.thesaurusLookup(word, lang)`, and lists the answers by source, in the order they were registered; with a single source the heading is left out. The host tidies what comes back: blank synonyms, repeats and the word itself are dropped, and a thesaurus left with nothing is not listed. One that throws is reported and skipped.

Docxtor's own source is the built-in extension `docxtor.thesaurus.english`: a list of about 1,700 groups of words (8,500 words with their meanings, 170 kB, 65 kB compressed), written for Docxtor and released with it (`packages/editor/src/thesaurus/english-data.ts`; one group a line, `pos|word,word,word`, and lines starting with `!` for opposites). It is a chunk of its own that the browser fetches the first time a word is looked up, so it costs nothing until it is used. It finds inflected forms (running, stopped, cities, bigger, happily) through their base word and gives the synonyms the same ending, with the irregular verbs. It is small on purpose; WordNet and other large lists are too big to ship as a bundled list, and a bigger source belongs in an extension of its own. The app adds Datamuse (`web/src/thesaurus-datamuse.ts`, extension `docxtor.thesaurus.datamuse`): it is off until someone turns it on, says in its `privacy` line that it sends each word to api.datamuse.com, and answers with one list for each part of speech.

<!-- example: ext-thesaurus -->
```ts
ctx.thesaurus.register({
  id: "acme.legal-terms",
  label: t("Legal terms"),
  languages: ["en"],
  lookup: (word) => (terms[word] ? { senses: [{ pos: "noun", meaning: "in contracts", synonyms: terms[word] }] } : null),
});
```
