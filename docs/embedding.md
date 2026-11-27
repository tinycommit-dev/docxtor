# Embedding Docxtor

You can run Docxtor inside your own product in three ways: as an HTML element, as a JavaScript component, or as a headless engine with no UI. Docxtor is MIT / Apache-2.0, so commercial use is free. It needs no server either; your app decides where documents are stored.

> **The packages are not on npm yet.** `@docxtor/core`, `@docxtor/extensions` and `@docxtor/editor` live in this repository (`packages/`). What is released is the repository, the hosted app, the self-hosting archive and the CLI and MCP downloads, so `npm install @docxtor/editor` does not work. Use the code from a checkout:
>
> 1. `git clone` the repository, then `npm install && npm run wasm` (the engine as WebAssembly; see [CONTRIBUTING.md](../CONTRIBUTING.md) for what you need first).
> 2. `npm install` links the three packages into `node_modules/@docxtor/`, so code inside the checkout (a page under `web/`, a script) imports `@docxtor/core`, `@docxtor/extensions` and `@docxtor/editor` by those names. They export TypeScript source (`src/index.ts`), not built JavaScript: Vite bundles it, and Node 22.18 or later runs it by stripping the types.
> 3. Or skip the packages: `npm run build` makes the whole app as static files in `web/dist`, and you host it yourself ([Putting Docxtor online](deploy.md)).
>
> Every example below that is a program is run by `npm test` or by the browser spec `e2e/docs-examples.spec.ts`, against these workspace packages. [`web/examples/embed.html`](../web/examples/embed.html) puts two editors on one page; run `npm run dev` and open `/examples/embed.html` to try it.

## 1. The `<docxtor-editor>` element

The element works with any framework, or with none.

<!-- example: embed-element -->
```html
<docxtor-editor id="doc" src="/files/contract.docx" theme="system" style="height: 80vh"></docxtor-editor>
<script type="module">
  import "@docxtor/editor";
  import "@docxtor/editor/style.css";

  const doc = document.getElementById("doc");
  doc.addEventListener("docxtor-save", (e) => fetch("/files/contract.docx", { method: "PUT", body: e.detail.bytes }));
  doc.addEventListener("docxtor-change", () => console.log("edited"));
  // Or read the bytes whenever you like, once the document has opened (before that, save() throws):
  doc.addEventListener("docxtor-ready", () => console.log(`opened, ${doc.save().length} bytes`));
</script>
```

| Attribute | Meaning |
|---|---|
| `src` | URL of a `.docx` to open |
| `name` | File name. Without it, the last part of the URL |
| `theme` | `light`, `dark` or `system` (default) |
| `icon-pack` | `docxtor` (default), `docxtor-light`, or the id of a pack an extension contributed |
| `locale` | Interface language, such as `es`, when a language pack for it is installed (read when the element connects) |
| `readonly` | Read, select and copy only |
| `no-toolbar` | Hide the formatting toolbar |
| `fonts` | Your own font stylesheet URL, or `none` if the page already loads the fonts |

| Event | `detail` |
|---|---|
| `docxtor-ready` | (document opened) |
| `docxtor-change` | (document edited) |
| `docxtor-selection` | `{ format }`: formatting under the caret |
| `docxtor-save` | `{ bytes }`: ⌘S / Ctrl-S inside the editor |
| `docxtor-error` | `{ message }` |

The element has three methods, `load(bytes, name?)`, `save()` and `saveInSteps()` (the same bytes as a promise, made off the main thread), and an `editor` property that gives you the `DocxtorEditor` behind it.

## 2. The `DocxtorEditor` component

Use the component when you want control over layout and behaviour.

<!-- example: embed-component -->
```ts
import { DocxtorEditor } from "@docxtor/editor";
import "@docxtor/editor/style.css";

const editor = new DocxtorEditor({
  onSave: (bytes) => upload(bytes), // ⌘S inside the editor
  ask: (prompt) => myAssistant.open(prompt), // optional: adds "Ask" to the toolbar and selection bubble
}).mount(document.getElementById("editor")!, { theme: "system" });

await editor.open(await (await fetch("/report.docx")).arrayBuffer(), "report.docx");
editor.on("change", () => markDirty());
const bytes = editor.save();
```

`mount(el)` turns `el` into a Docxtor root with the toolbar above the pages; give `el` a height. If you'd rather arrange things yourself, skip `mount()` and place `editor.menuBarEl`, `editor.toolbarEl` and `editor.surface` wherever you like, inside an element with the `docxtor` class. The Docxtor app does this to fit its navigator and assistant between them.

| Options | |
|---|---|
| `toolbar` | `false` to hide the formatting toolbar |
| `toolbarAsk` | `false` to leave the "Ask" button out of the toolbar when your page has its own way to open the assistant |
| `menuBar` | `true` to add the menu bar (File, Edit, View, Insert, Format, Tools, Extensions, Help) above the toolbar |
| `readOnly` | Start read-only |
| `autoformat` | `false` to turn off typing substitutions (smart quotes, automatic capitals and links, Markdown shortcuts, `(c)` to ©); on by default, and people choose which ones under Tools, Preferences |
| `ask(prompt?)` | Hook up your own AI assistant |
| `onSave(bytes)` | Handle ⌘S / Ctrl-S inside the editor |
| `onError(message)` | Show errors your way (default: a small toast) |
| `openAddress(url)` | Offered every address a document opens (a link, a chip's Open button) first: return true when you opened it yourself, for example an address of your own documents. Otherwise web and email addresses open in a new tab and other kinds (`javascript:`, `file:` and the like) are refused; the editor never changes your page's address |
| `fonts` | Font stylesheet URL, or `false` if you load the fonts yourself |
| `spelling` | Dictionaries for Docxtor's own spelling checker (see [Spelling](#spelling)); a shortcut for registering one provider with `ctx.spelling.dictionaries`. Without a dictionary for the document's language the browser's own spellcheck keeps working. `false` leaves out Docxtor's spelling and grammar checkers altogether |
| `iconPack` | The icon pack to start with (`docxtor` by default, `docxtor-light`, or one an extension contributed; see [Icon packs](extensions/document-look.md#icon-packs)) |
| `locale` | Interface language, such as `es` or `pt-BR`, taken from the language packs in `extensions`. One language per page |
| `openNewDocument(bytes, name)` | Receives a document the editor made, such as the result of a mail merge. Without it the `.docx` is downloaded |
| `extensionState` | Where extensions keep their settings (default: in memory) |
| `extensions` | [Extensions](extensions/index.md) to add: commands, toolbar buttons, formats, themes… |
| `userGroups` | Which of Word's editing groups the user counts as (`editors`, `contributors`, `owners`...), for the parts of a protected document that only some people may edit. Parts for `everyone` always count |
| `disabledExtensions` | Ids of extensions, built-in ones included, to start turned off |

| Methods | |
|---|---|
| `open(bytes, name?, password?)` / `create(name?)` | Open a `.docx` (or OpenDocument text, Rich Text, a web page, an old `.doc`, Word 2003 XML, Flat OPC, Markdown, plain text, or any format an extension imports), or start a blank one with Word's styles. A damaged `.docx` is repaired and Strict Open XML read as the standard kind; `editor.openNotice` says what happened (`{ title, notes }`, in the person's language), and a file that is not a document rejects with an `OpenError` whose `code` and `detail` say what it is (see [Opening files](extensions/files.md#opening-files)). A password-protected `.docx` rejects with an `EncryptedError` until you pass its `password`, and with a `WrongPasswordError` when it is wrong (see [Password-protected documents](#password-protected-documents)) |
| `setPassword(password \| null)`, `hasPassword` | Protect the open document with a password, or take the protection off; `save()` and `onSave` then give protected bytes |
| `export(id)` | The document as `docx`, `markdown`, `text`, `odt`, `rtf`, `html`, `epub`, or a format an extension adds: `{ name, mime, bytes }` |
| `load(engine)` | Show an `Engine` you manage yourself |
| `save()` | The document as `.docx` bytes |
| `saveInSteps()` | The same bytes, as a promise: made in steps with the compressing in a worker, so a long document does not hold the page and the engine never builds the whole file at once. ⌘S / Ctrl-S (`onSave`) uses it |
| `exec(ops)` | Apply edit ops (the language the AI tools and CLI use) as one undoable step |
| `command(id)` | Run a command: `format.bold`, `find.open`, `palette.show`, or one an extension added |
| `use(extension)` | Add an extension after creating the editor |
| `extensions` | The extension host: what's installed and everything it contributes |
| `undo()`, `redo()`, `focus()`, `find()`, `pageSetup()` | |
| `protection`, `protectionStatus` | What the open document protects (`w:documentProtection`, Mark as final, the editable ranges), and the same in words for a status bar. The engine refuses an edit outside what is allowed, so every edit path is held to it; the editor shows a strip above the pages, tints the editable ranges and opens a marked-as-final document read-only |
| `setReadOnly(on)`, `setTheme(theme)` | `setReadOnly` is the host's own reason to be read-only; `setTheme` also takes the id of a theme an extension contributed |
| `lock(reason, { scope? })`, `lockReasons()`, `readOnly` | Read-only is a set of reasons: the document is read-only while any is held, and `lock` returns the function that lets go of its own. `scope: "document"` reasons end when another document is loaded; the default `"host"` stays. While the window is locked the engine refuses every edit (`engine.access()` is `"view"`): the assistant, extensions and scripts get "This document is read-only." A view's own lock (Reading, Line focus) stops its pane only; in a split window the engine is view only when every pane is locked. Opening a tab still works in a read-only document |
| `setView(id)`, `view`, `views()` | The layout: `print` (pages, the default), `pageless`, `reading`, `outline`, or a view an extension registered with `ctx.views`. `setView` returns false for an id that isn't there. A view with `readOnly` locks the text while it is on |
| `setCompact(on)`, `compact` | Compact mode: the menu bar folds into its Menu button and the toolbar is tighter |
| `editor.editor.zoomer`, `editor.editor.setZoom(z)`, `editor.editor.fitWidth()`, `ZoomControl` | Zoom (on the `Editor` behind the `DocxtorEditor`): a factor (1 is 100%), `zoomer.setMode("width" \| "page" \| "pages" \| "auto", across?)`, and `new ZoomControl(fe).el`, a status-bar piece with zoom out, a slider, zoom in and the presets. Ctrl or ⌘ with the wheel and pinches zoom the editing area by themselves |
| `setIconPack(id)`, `iconPacks()`, `currentIconPack` | Switch the icons of the menus, toolbar and context menu to a pack |
| `on(event, fn)` | `change`, `selection`, `layout`, `story`, `open`, `theme`, `view`, `compact`, `split`, `protection`; returns an unsubscribe function |
| `destroy()` | Remove the editor and every listener and overlay it created |

### Spelling

Browsers don't let a page read their spellchecker's suggestions, so Docxtor has a checker of its own: wavy underlines under misspelled words, corrections at the top of the right-click menu, and Tools, Spelling and grammar (a check panel that steps through the errors, the Show spelling suggestions switch and a personal dictionary). It checks each paragraph in its own language (the paragraph's, else the document's default in its page setup). Grammar needs no dictionaries: Docxtor's own rules for English, Spanish, French and German run in the page, and extensions can add checkers (see [Spelling and grammar](extensions/language.md#spelling-and-grammar) in the extension guide). With `<docxtor-editor>` the checkers are off until you set `el.spelling`.

The editor doesn't bundle dictionaries, because they are large (about 0.5 to 1.4 MB each) and not every product wants them. Pass a provider that returns Hunspell `.aff` and `.dic` files for a language, as bytes or text:

<!-- example: embed-spelling -->
```ts
import { DocxtorEditor, type SpellingProvider } from "@docxtor/editor";

const spelling: SpellingProvider = {
  // `lang` is the document's language: "en-US", "es", "fr-CA"…
  async load(lang) {
    const dir = { en: "en_US", es: "es_ES", fr: "fr_FR", de: "de_DE" }[lang.slice(0, 2).toLowerCase()];
    if (!dir) return null; // no dictionary: this document isn't checked
    const get = async (ext: string) => new Uint8Array(await (await fetch(`/dictionaries/${dir}/index.${ext}`)).arrayBuffer());
    return { aff: await get("aff"), dic: await get("dic") };
  },
};
new DocxtorEditor({ spelling });
```

`load` is called once per language, the first time text in it is checked, so a language you don't serve costs nothing. Returning `null` means "no dictionary": text in that language isn't checked, Tools says so for the document's language, and the browser's spellcheck stays on. An extension can add or replace dictionaries with `ctx.spelling.dictionaries`; the latest registered one that has a language wins. Wooorm's `dictionary-en`, `dictionary-es` and the other `dictionary-*` npm packages hold ready-made files; the Docxtor app serves five of them (`web/src/spelling.ts`). Mind their licences: the English ones are MIT and BSD, French is MPL-2.0, Spanish is GPL, LGPL or MPL (your choice) and German is GPL.

The words are looked up with [spellbook](https://github.com/helix-editor/spellbook), a Rust reading of Hunspell dictionaries, compiled to WebAssembly (`crates/docxtor-spell`; MPL-2.0, see THIRD_PARTY_NOTICES.md). It reads a dictionary without listing every form of every word, so English takes about 2 MB of memory and French about 5, and it builds one in about 12 ms, on the page: its code (`spell_bg.wasm`, 306 KB, 109 KB gzipped, and a small script) is loaded the first time a document needs checking, through `new URL(…, import.meta.url)` and a dynamic import, which Vite and webpack 5 bundle. Under a strict Content Security Policy it needs what the engine needs: `'wasm-unsafe-eval'` in `script-src`. The engine itself is compiled in a small worker of its own that closes after (`compile-worker.ts`, started with `new URL(…, import.meta.url)`), so the page keeps none of the memory compiling takes (about 27 MB); a policy that blocks it (`worker-src`) only makes the engine compile on the page instead. With `<docxtor-editor>`, set the property before the element is connected: `el.spelling = provider`.

While Docxtor's checker is on, the browser's own underlines are turned off (`spellcheck="false"` on the pages), so there is only one set. They come back when the checker is off or has no dictionary for the document's language. Words in capitals, addresses, words with digits, links, fields and runs in a code font aren't checked.

### Citations

Citing sources needs nothing but the engine, so it is in the editor. Insert, Citation and Insert, Bibliography are commands (`insert.citation`, `insert.bibliography`), clicking a citation in the text opens a popover (its sources, the page, Omit author and Omit year, Remove citation), and Backspace or Delete next to a citation takes all of it. Without anything else, Insert, Citation offers a plain list of the document's sources, and the sources themselves come in through the engine (`add_source`, `Engine.parseBibtex`) or the AI tools.

The Citations panel (a form for each kind of source, a lookup by DOI, BibTeX import and export) is a Docxtor app extension, `docxtor.citations` in `web/src/ui/citations`, not part of the editor, because a DOI lookup asks doi.org and a .bib file is opened from disk. When it is installed, Insert, Citation opens it and the popover offers "Edit source". A host that wants its own source manager provides the commands `citations.find` (open your list, ready to cite) and `citations.editSource` (takes a source tag) and gets the same behaviour.

### Password-protected documents

Word's "Encrypt with password" turns a `.docx` into a different kind of file (an OLE compound file, not a zip). Docxtor reads and writes both ways Word does it, in Rust, so the same code runs in the editor, in Node and in the CLI.

<!-- example: embed-password -->
```ts
import { EncryptedError, WrongPasswordError } from "@docxtor/editor"; // or @docxtor/core

try {
  await editor.open(bytes, "contract.docx");
} catch (e) {
  if (e instanceof EncryptedError) await editor.open(bytes, "contract.docx", await askForPassword());
  else throw e; // a WrongPasswordError means ask again
}
editor.setPassword("new password"); // saves protected from now on; editor.setPassword(null) removes it
```

A document opened with a password is saved protected with the same password. The password and the keys made from it live in memory only; `save()` on a protected document costs no more than on a plain one after the first (the first save after `setPassword` makes the keys, 100000 rounds of hashing, which takes a moment). `isEncrypted(bytes)` tells a protected file from a zip, `engine.savePlain()` gives the zip itself, and `Engine.plain(bytes, password)` decrypts bytes without opening them. `<docxtor-editor>`'s `load(bytes, name?, password?)` takes the password too.

## 3. The headless engine: `@docxtor/core`

The engine opens, edits and saves `.docx` files with no UI, in browsers, Node, Deno or edge functions. It suits filling templates, generating reports and running AI edits in the background.

<!-- example: embed-headless -->
```ts
import { readFile, writeFile } from "node:fs/promises";
import { Engine, loadEngine, toolDefinitions } from "@docxtor/core";

await loadEngine(await readFile("node_modules/@docxtor/core/wasm/docxtor_bg.wasm")); // in browsers: loadEngine()
const doc = Engine.open(new Uint8Array(await readFile("contract.docx")), "contract.docx");
doc.apply({ op: "replace_text", find: "{{client}}", replace: "Acme Ltd", all: true });
await writeFile("contract-acme.docx", doc.save());
await writeFile("contract-acme.md", doc.markdown()); // or doc.text()
await writeFile("contract-acme.odt", doc.exportOdt()); // exportRtf(), exportHtml(), exportEpub()
```

`Engine.open` reads the other formats too, and `engine.openReport` (`format`, `converted`, `notes` as codes) says what was repaired or left out; `Engine.convert(bytes, name)` returns the .docx bytes and the report, and `Engine.sniff(bytes)` says what a file is from its content. `Engine.fromMarkdown(markdown, name)` goes the other way and builds a .docx with Word's styles. `Engine.open(bytes, name, password)` opens a password-protected file (see [Password-protected documents](#password-protected-documents)).

The AI tools behind the assistant and the MCP server work in-process too. `toolDefinitions()` returns the tool schemas for any model's tool-calling loop, and `doc.raw.callTool(name, jsonArgs)` runs one. [Using AI with Docxtor](ai/index.md) describes the tools.

Outside JavaScript, the same engine is available as a Rust crate (`crates/docxtor-core`), the `docxtor` CLI and the MCP server.

## Theming

Every colour is a CSS variable on the Docxtor root (`.docxtor`), so you can override them to match your brand:

<!-- example: embed-theming -->
```css
#editor.docxtor {
  --signal: #2563eb; /* the AI mark, the active field frame */
  --selection: rgb(37 99 235 / 0.2); /* selected text */
  --canvas: #f1f5f9; /* behind the pages */
  --button: #0f172a; /* primary buttons */
}
```

[DESIGN.md](../DESIGN.md) and the top of `packages/editor/src/styles/tokens.css` list every variable and what it's for. Setting `data-theme="light" | "dark"` on the root forces a theme; without it, Docxtor follows the system setting.

## Living alongside your page

Every Docxtor rule is scoped under `.docxtor`, so Docxtor never restyles your page. Going the other way, the common element-level rules a page might set (button text transforms, margins, letter spacing) are reset inside Docxtor. Full isolation through Shadow DOM is planned.

Popovers, dialogs and toasts render into a single `.docxtor-layer` element appended to `<body>`, which carries Docxtor's tokens and theme.

The engine's WebAssembly file is large, because it holds the whole .docx engine: 12.6 MB as `npm run wasm:ship` builds it (4.1 MB gzipped, 2.7 MB brotli; this is the build the hosted app, the self-host archive and the Docker image serve, see [Putting Docxtor online](deploy.md)), or 14.5 MB (4.4 MB gzipped, 2.8 MB brotli) as `npm run wasm` builds it (the quicker everyday build). Sizes are measured with gzip level 9 and brotli quality 11, as `npm run perf` does (`perf/lib/sizes.mts`). It loads the first time it's needed, from next to the module. To serve it from your own CDN, or under a strict Content Security Policy (which needs `'wasm-unsafe-eval'`), pass a URL or the bytes to `loadEngine()` or `DocxtorEditor.ready()`.

Fonts embedded in a document (Word's "Embed fonts in the file") are used automatically. Extensions can add more families to the font menu with the `fonts` contribution point. When a document uses a font the device can't show and that has no look-alike with the same letter widths, a strip above the pages says so and offers to replace it (the `docxtor.missing-fonts` built-in; turn it off with `disabledExtensions`).

By default the editor loads Inter and faces that match Word's metrics (Carlito for Calibri, Caladea for Cambria, and so on) from Google Fonts, so lines break where Word breaks them. Pass `fonts: "/your/fonts.css"` to self-host them, or `fonts: false` if you load them yourself.

You can put several editors on one page and they won't interfere with each other; `destroy()` releases everything an editor created.

Files, version history, addresses and the built-in AI assistant belong to the Docxtor app and aren't part of the embedded editor. Your product supplies the storage and, if you want one, the assistant (through `ask`). If your product gives its users the AI, keep the model key on your server and not in the page: [Giving your users the AI without a key in the page](ai/your-users.md#giving-your-users-the-ai-without-a-key-in-the-page) has two example proxies, for Node and for a Cloudflare Worker.
