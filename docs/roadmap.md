# Roadmap

Docxtor aims to be to documents what VS Code is to code: a free, open-source editor that anyone can use, host themselves, build into their own products, and extend.

Docxtor runs entirely in the browser: no server of its own, no accounts, nothing uploaded. Accounts, sharing, team storage and keeping an organisation's AI keys off the browser are what a product builds on top of Docxtor, through its extension points and APIs.

## The four pillars

1. **A free alternative to Google Docs.** A hosted Docxtor anyone can use, and the same app as static files you can serve from any web host. Documents stay lossless `.docx`.
2. **Self-hosting.** A release archive for any web host (nginx, Caddy, Netlify, S3) and a small Docker image, `docxtor-web`, for platforms that only run containers. See [Putting Docxtor online](deploy.md).
3. **Embedding.** Other products can include Docxtor as a headless engine, an editor component or an HTML element. MIT or Apache-2.0, so commercial use is free. See [Embedding Docxtor](embedding.md).
4. **Extensions and a marketplace**, so the community can cover every niche: LaTeX, `.doc` import, citations, legal clauses, translation, storage, AI providers, exporters. See [Writing extensions](extensions/index.md).

## Three layers

<!-- text -->

```
@docxtor/core     the Rust engine (WebAssembly and a TypeScript API): open, edit with ops, save, DocScript, AI tools.
                  Runs in browsers, Node, Deno and edge functions. Also the docxtor-core crate.
@docxtor/editor   the editing surface: pages, toolbar, tools, header, footer and footnote editing.
                  Bytes in, bytes out; it doesn't know where files live.
web/ (the app)    one host for the editor: Library, folders, history, start screen, the AI panel.
```

Code in a lower layer never depends on a higher one. More in [Architecture](architecture/index.md).

## Done

- **The engine and the editor as separate packages**, with one `DocxtorEditor` API and a `<docxtor-editor>` element.
- **The extension API**: commands with shortcuts and a command palette, toolbar buttons, menus, side panels, importers and exporters, templates, themes, AI providers and tools, storage, dictionaries and grammar checkers. Docxtor's own features are built on it.
- **A complete `.docx` editor**: the formatting, tables, pictures, shapes, sections, headers and footers, footnotes and endnotes, fields, references, mail merge and views people expect from Word and Google Docs, saved so Word opens the file as it was.
- **AI that edits like a coding agent edits code**: a precise view of the document and small, reviewable edits, in the app, through the `docxtor` command line and through an MCP server. See [Using AI with Docxtor](ai/index.md).

## Next

- **Comments and tracked changes.** They are kept in the file today but not shown. Next, they are shown and edited, and the AI reads and writes tracked changes, so you see exactly what it changed.
- **The packages on npm**: `@docxtor/core`, `@docxtor/extensions` and `@docxtor/editor`, with thin React and Vue wrappers, and the crate on crates.io.
- **Building blocks for products**: storage providers (a product's server, Google Drive, S3), people providers for mentions, AI through a product's own endpoint so the key never reaches the browser, mail senders for email merge, and scripts in the spirit of Google Apps Script.

## Later

- **Real-time collaboration**: a CRDT layer (compatible with Yjs) under the same ops, with the host supplying the connection. Presence, suggestions, and AI agents that join a document over MCP with their own cursor and name.
- **A marketplace**: an open registry with signed packages and permission manifests, a sandboxed runtime for extensions, and document themes that restyle a document and are written into its styles so Word shows the same.
- **More ways in**: an iframe embed over `postMessage`, Python bindings, a C interface for Go, Java, C# and Ruby, and a WebAssembly build without JavaScript.
- **Mobile apps**, native on iOS and Android. Until then the web app works at phone and tablet sizes.

Something missing, or want to help? Open an issue, or see [Contributing](../CONTRIBUTING.md).
