# Architecture

Docxtor has one engine — `docxtor-core`, in Rust — and several thin hosts around it: the browser editor (via WebAssembly), the CLI, and the MCP server. Every change to a document, whether a keystroke or an AI edit, is an **op** applied by the engine.

<!-- text -->
```
        ┌──────────── browser ─────────────┐   ┌── native ──┐
 input ─▶ @docxtor/editor (DOM, selection,  │   │ docxtor-cli  │
        │   layout)  │ ops (JSON) ▲ view  │   │ docxtor-mcp  │
        │            ▼            │ model │   └─────┬──────┘
        │   @docxtor/core (docxtor-wasm)──────┘        │
        └────────┬──────────────────────────┘          │
                 ▼                                      ▼
           docxtor-core: Package, Document model, Ops, Render, AI tools, Fidelity
```

## The JavaScript layers

<!-- text -->
```
@docxtor/core        packages/core        the engine (WASM) + Engine API; browsers, Node, edge
@docxtor/extensions  packages/extensions  the extension API and ExtensionHost (no DOM)
@docxtor/editor      packages/editor      DocxtorEditor, <docxtor-editor>, editing surface, in-page tools
app                web/                 files, history, addresses, AI panel, start screen
```

A lower layer never imports a higher one. The editor knows nothing about where files live: it takes bytes and gives bytes back (see [Embedding Docxtor](../embedding.md)). Every CSS rule is scoped under `.docxtor`; colour tokens live on the Docxtor root and on the shared `.docxtor-layer` that holds popovers, dialogs and toasts. Each editor owns a `Lifetime` that releases its global listeners and overlays on `destroy()`.

## packages/extensions

`ExtensionHost` activates extensions and keeps what they contribute in typed registries (commands, toolbar, menus, panels, exporters, importers, templates, themes, AI providers and tools, storage). Hosts render from the registries and re-render on their `onChange`. Every registration is a `Disposable` tied to its extension, so turning one off removes everything it added. Commands carry their shortcuts: the host matches key events by physical key, first registration wins, and text fields keep their own editing keys. See [Writing extensions](../extensions/index.md).

## web/ (the app)

- `builtins.ts` — the app's features as extensions: the navigator's panels, File/View/Extensions/Help commands with their menu places and icons (including Open recent and Download), and templates.
- `ai/` — providers (Anthropic SDK; OpenAI-compatible streaming), registered as an extension, and the agent loop, which runs the engine's AI tools plus tools extensions add, and refreshes the view after each one.
- `files/` — the Library (IndexedDB, version 3), connected folders and single files (File System Access), Recents (with pins), version history, the Trash (`trash.ts`) and crash recovery copies (`recovery.ts`, which uses a Web Lock per session to tell a crash from a normal close).
- `settings.ts`, `help.ts`, `offline.ts` — app preferences (autosave, version interval, default font), Help (topics, what's new, report a problem), and the offline layer: `offline/sw.js` is a cache-first service worker whose precache list `vite.config.ts` writes at build time, the fonts are bundled (`fonts.css`, fontsource packages), and `public/manifest.webmanifest` makes the app installable.
- `route.ts` — per-document addresses (`#/library/…`, `#/storage/<provider>/…` for storage extensions).
- `ui/` — navigator (a tab per panel), AI panel (chat, tool log, word-diff review), settings, the Extensions dialog.

## Collaboration

Real-time collaboration is not built. Ops already address content by stable block ids plus offsets, which is what a CRDT layer underneath them needs (see the [roadmap](../roadmap.md)).

## Document tabs

Google Docs' tabs are kept so Word reads the file as one document: the body stays linear, each tab after the first starts with a section break and a hidden point bookmark `_DocxtorTab_<id>`, and the list of tabs (names, emoji, nesting, the open tab) is a custom XML part in Docxtor's own namespace that Word keeps without reading. A document without that part has no tabs and gets nothing added. The engine checks after every batch that each tab still has its start (`tabs_hold`). Which tab is open is view state, one for each pane of the editor, never an op: the engine only keeps the tab the file was saved on (`Document::set_open_tab`, which the first pane calls whenever its tab changes, written into the file at save). The editor shows one tab at a time: the top-level blocks of the other tabs are marked `data-tab-hidden` and the paginator leaves them out; Print all tabs shows them all while printing.

## The rest of the architecture

- [How the engine works](engine-concepts.md): Positions, stories, undo, change tracking for rendering and the input budget.
- [Engine modules: files and the model](engine-files-and-model.md): Packages, encryption, the model, ops and `Document`, properties, fragments and paste, opening and importing, the input budget and invariants.
- [Engine modules: rendering and layout data](engine-rendering-and-layout.md): The render view model, character formatting, styles, themes, numbering, page layout, notes, tables, East Asian text, exports and accessibility.
- [Engine modules: content features](engine-content-features.md): Fields, indexes and contents, find, mail merge, citations, controls and protection, comparison, pictures, shapes, SmartArt, equations, charts and document tabs.
- [Engine modules: AI tools and fidelity](engine-ai-and-fidelity.md): DocScript, the shared tool executor, the `ai_*` modules and the fidelity score.
- [The editor package](editor.md): `packages/editor`: the editing surface, layout, rendering and the interface.
