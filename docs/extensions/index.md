# Writing extensions

An extension is a JavaScript object with an id, a name and an `activate(ctx)` function. Inside `activate` it registers what it adds: commands, toolbar buttons, panels, file formats, templates, themes, AI providers and tools, or places to store documents. Docxtor's own features are built this way. Find and replace, the command palette, the Word/Markdown/text formats, the navigator, the AI providers and the templates are all extensions, and you can see them under Extensions, Manage extensions.

The API lives in `@docxtor/extensions` (`packages/extensions`). It has no dependency on the editor, so the same extension works in the Docxtor app and in any product that embeds Docxtor.

Pick your own prefix for ids (`acme.shout`). Docxtor's own extensions have ids that start with `docxtor.`. They are what `disabledExtensions` takes.

<!-- example: ext-shout -->
```ts
import { defineExtension } from "@docxtor/extensions";

export default defineExtension({
  id: "acme.shout",
  name: "Shout",
  description: "Upper-case the selection.",
  activate(ctx) {
    ctx.commands.register(
      "acme.shout",
      () => {
        const sel = ctx.document.selection();
        if (sel?.text) ctx.document.insertMarkdown(sel.text.toUpperCase());
      },
      { title: "Make the selection upper case", category: "Format", keybinding: "Mod+Shift+U" },
    );
    ctx.toolbar.register({ id: "acme.shout", command: "acme.shout", label: "AA", group: "text" });
  },
});
```

Two complete examples are in [`web/examples/extensions`](../../web/examples/extensions): `title-case.ts` (a command, a toolbar button and a shortcut) and `word-goal.ts` (a panel, saved settings and document events).

## Installing one

In the Docxtor app, add it to the list in [`web/src/extensions.ts`](../../web/src/extensions.ts). It then shows up under Extensions, Manage extensions, where it can be turned off.

In an embedded editor, pass it when you create the editor, or add it later:

<!-- example: extensions-use -->
```ts
const editor = new DocxtorEditor({ extensions: [shout] });
await editor.use(anotherOne);
document.querySelector("docxtor-editor")?.editor?.use(shout);
```

Extensions run with the same access as the page, so only install code you trust. A sandboxed runtime and a registry with permission manifests are on the [roadmap](../roadmap.md).

## The context

| | |
|---|---|
| `ctx.document` | The open document: `selection()`, `selections()`, `select()`, `selectRanges()`, `exec(ops)`, `insertMarkdown()`, `tool(name, input)`, `markdown()`, `text()`, `format()`, `reveal()`, `focus()`, the `engine`, and the events `onDidChange`, `onDidChangeSelection` and `onDidOpen`. |
| `ctx.ui` | `notify`, `confirm`, `prompt`, and `pick` (a searchable list, like the command palette). |
| `ctx.state` | `get(key, fallback)` and `set(key, value)`: settings that last between sessions, kept separately for each extension. |
| `ctx.commands` | `register(id, run, options)` and `execute(id, ...args)`. `execute` is the only way to run a command: it answers `{ status: "done", value }`, `{ status: "declined" }` when the person said no to its question, or `{ status: "unavailable" }` when the command's `when` says no (a read-only document, for example). |
| `ctx.subscriptions` | Push anything with a `dispose()` here. All registrations add themselves. |

Everything an extension registers is removed when it's turned off, and `deactivate()` is called if the extension has one.

### Find and replace, Go to

Find and replace, Go to, Select text with similar formatting and the next and previous footnote are built-in commands (`find.open`, `find.next`, `find.previous`, `find.results`, `edit.goto`, `edit.selectSimilar`, `references.nextFootnote` and its three kin), so an extension can run them, rebind them or put them in a menu. The engine does the searching, so the same options work in the AI tools (`search`, `replace_text`) and the CLI: `Engine.find(text, options)` returns every match with its context, and the `replace_text` op takes the same options plus the formatting to put on. The list of results lives in the find bar; the app moves it into the navigator as the "Find in" panel with `DocxtorEditor.dockFindResults({ el, show })` (`null` puts it back), and an embedding page can do the same with a panel of its own.

### More than one selection

Edit, Select all matching text selects every place the selected text occurs. Browsers keep one selection, so Docxtor keeps the rest itself: the real selection (the one the toolbar shows) stays the first range and the others are drawn beside it in the same colour.

<!-- example: ext-selections -->
```ts
const all = ctx.document.selections(); // every range in document order: { from, to, text }[]
ctx.document.selection();              // only the real one
ctx.document.selectRanges([{ from: a, to: b }, { from: c, to: d }]); // the first becomes the real selection
```

- `selection()` stays the real selection, so extensions written for one range keep working, and the assistant's commands use it too.
- Bold, italic, underline, size, colour, links, clear formatting, styles, alignment, lists and indents apply to every range (paragraph commands to every paragraph a range touches). Delete, Backspace, typing, paste, copy and cut work on all of them; each is one undo step. After typing or deleting, each range carries on as a caret, so typing goes on at all of them.
- Anything else (inserting a table, an image, a break) acts on the real selection and ends the others. So do undo, redo and `exec()` from an extension: the other ranges point at text those edits can move.
- A click, an arrow key, Escape, `select()` or a new document ends it. `onDidChangeSelection` fires when it starts.
- `selectRanges` drops a range that overlaps an earlier one. A range may cross paragraphs, but Select all matching text only offers text inside one.

### Editing goes through ops

`ctx.document.exec()` takes the same ops the AI tools, the CLI and the MCP server use (`insert_text`, `delete`, `set_run`, `insert_markdown`, `replace_text` and the rest; see `crates/docxtor-core/src/ops.rs`). One call is one undo step, the change shows up in version history, and anything Docxtor doesn't model is kept. For larger edits, `ctx.document.tool("replace_text", { find, replace, all: true })` runs one of the [AI tools](../ai/index.md) directly.

### Character formatting

Character formatting is `format_text` ops with a `RunPatch` (see `@docxtor/core`): bold, italic, `underline` (any of Word's eighteen styles) with `underlineColor`, `color` with `themeColor`, `themeTint` and `themeShade` for a theme colour, `highlight`, `shading`, `border`, `position`, `scale`, `spacing`, `kern`, `outline`, `shadow`, `emboss`, `imprint`, `vanish` (hidden), `em`, `fitText`, `ligatures`, `numForm`, `numSpacing`, `styleSets`, `cntxtAlts` and `style` (a character style, which can also be a linked paragraph style). `change_case` changes the text itself and `set_default_format` writes the document defaults. The built-in `docxtor.character` extension adds the commands `format.font` (the Font dialog), `format.case.sentence`, `format.case.lower`, `format.case.upper`, `format.case.title`, `format.case.toggle`, `format.cycleCase` (Shift+F3), `format.clearCharStyle` and `view.hiddenText`; an extension can run them, or bind and place them again.

## Testing

The host runs without a browser, so you can test an extension's commands in Node with a stub document. [`packages/extensions/test/host.test.ts`](../../packages/extensions/test/host.test.ts) shows how.

## The rest of this guide

- [Extension contribution points](contribution-points.md): Every contribution point in one table, then menus and shortcuts.
- [Extensions for files and review](files.md): Opening files, PDF files, Inspect document, and Compare and combine.
- [Extensions for design and content](document-look.md): Cover pages, smart chips, themes and style sets, picture sources, icon packs, shared lists and quick parts.
- [Extensions for mail merge, citations and fields](merge-citations-fields.md): Mail merge, citation styles and sources, and fields.
- [Extensions for language](language.md): Spelling and grammar, hyphenation, readings, language packs and the thesaurus.
