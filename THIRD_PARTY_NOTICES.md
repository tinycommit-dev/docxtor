# Third-party notices

Docxtor's own code is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE). The Docxtor app also ships the files below, which keep their own licences. They are separate data files that the app downloads when a document needs them; they are not part of Docxtor's code.

## Spelling dictionaries

The app (`web/`) serves these Hunspell dictionaries for its spelling checker (see `web/src/spelling.ts`). Each package's `license` file has the full terms.

| Language | npm package | Licence | Source |
|---|---|---|---|
| English (United States) | `dictionary-en` | MIT and BSD (SCOWL) | [wooorm/dictionaries](https://github.com/wooorm/dictionaries), from [SCOWL](http://wordlist.aspell.net/) |
| English (United Kingdom) | `dictionary-en-gb` | MIT and BSD (SCOWL) | [wooorm/dictionaries](https://github.com/wooorm/dictionaries), from [SCOWL](http://wordlist.aspell.net/) |
| Spanish | `dictionary-es` | GPL-3.0, LGPL-3.0 or MPL-1.1, at your choice | [wooorm/dictionaries](https://github.com/wooorm/dictionaries), from [RLA-ES](https://github.com/sbosio/rla-es) |
| French | `dictionary-fr` | MPL-2.0 | [wooorm/dictionaries](https://github.com/wooorm/dictionaries), from [Grammalecte](https://grammalecte.net/) |
| German | `dictionary-de` | GPL-2.0 or GPL-3.0 | [wooorm/dictionaries](https://github.com/wooorm/dictionaries), from [igerman98](https://www.j3e.de/ispell/igerman98/) by Björn Jacke |

The German dictionary is GPL. Docxtor uses it only as data, loaded at run time, and doesn't modify it. Anyone who redistributes the app with it must pass on its licence and offer its source, which the links above provide.

## LanguageTool

Nothing from LanguageTool is shipped. The optional LanguageTool extension (`web/src/languagetool.ts`, off until someone turns it on) sends text to a LanguageTool server that the user chooses, by default the public server at languagetool.org, and shows what it answers. LanguageTool is free software under the LGPL-2.1 ([languagetool.org](https://languagetool.org/)); the public server has its own terms and rate limits.

## Citation styles

The app (`web/src/csl/`) ships these CSL styles from the [Citation Style Language styles repository](https://github.com/citation-style-language/styles) (also the source of Zotero's styles). They are unchanged copies, loaded by the app only when someone chooses the style or opens a document that uses it. Docxtor's CSL processor (`crates/docxtor-core/src/csl/`) is Docxtor's own code and doesn't contain any of them. The tests in `crates/docxtor-core/tests/all/csl.rs` read the same files.

Each style is licensed under the [Creative Commons Attribution-ShareAlike 3.0 licence](https://creativecommons.org/licenses/by-sa/3.0/), as its `rights` element says: "This work is licensed under a Creative Commons Attribution-ShareAlike 3.0 License". Anyone who changes a style and shares the result must do the same under that licence.

| File | Style | Authors (from the file) |
|---|---|---|
| `chicago-notes-bibliography.csl` | Chicago Manual of Style 18th edition (notes and bibliography) | Andrew Dunning |
| `chicago-notes-bibliography-16th-edition.csl` | Chicago Manual of Style 16th edition (notes and bibliography) | Julian Onions and contributors |
| `elsevier-vancouver.csl` | Elsevier, NLM/Vancouver (citation-sequence) | Richard Karnesky |
| `nature.csl` | Nature | Michael Berkowitz, Patrick O'Brien |
| `american-medical-association.csl` | AMA Manual of Style 11th edition | Julian Onions, Christian Pietsch, Daniel W. Chan, Patrick O'Brien, Andrew Dunning |

## Libraries

| Package | Licence | Used for |
|---|---|---|
| `spellbook` (Rust crate, [helix-editor/spellbook](https://github.com/helix-editor/spellbook)) | MPL-2.0 | Reading Hunspell dictionaries, checking words and suggesting corrections: compiled, unchanged, into the spelling checker's WebAssembly (`crates/docxtor-spell`, `spell_bg.wasm` in `@docxtor/editor`). Its source is at the link, as MPL-2.0 asks of anyone who ships it. |
| `@anthropic-ai/sdk` ([anthropics/anthropic-sdk-typescript](https://github.com/anthropics/anthropic-sdk-typescript)) | Apache-2.0 (MIT before 1.0) | The assistant's Anthropic provider (`web/src/ai/providers.ts`), bundled into the app. Its `LICENSE` file has the full terms. |

The PDF writer's WOFF2 reader (`packages/editor/src/pdf/brotli.ts`) is Docxtor's own code, but `brotli-data.ts` holds data from the Brotli reference decoder: the static dictionary, the context table and the word transforms of RFC 7932. Those are MIT licensed, Copyright 2013 Google Inc.

Every other dependency that ships, with its licence text, is in the generated files: `web/public/licenses.txt` (the app, copied to `licenses.txt` in the build), `packages/core/THIRD_PARTY_LICENSES.txt`, `packages/editor/THIRD_PARTY_LICENSES.txt`, `crates/docxtor-cli/THIRD_PARTY_LICENSES.txt` and `crates/docxtor-mcp/THIRD_PARTY_LICENSES.txt`. `npm run notices` writes them and `npm run check:notices` checks them (see `scripts/notices.mjs`).
