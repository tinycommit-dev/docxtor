# @docxtor/core

The Docxtor engine: opens, edits and saves `.docx` files without losing what it does not understand. It is Rust compiled to WebAssembly, with a small TypeScript API on top, and it runs in browsers, Node, Deno and edge runtimes. There is no UI in this package; the editor is [`@docxtor/editor`](https://www.npmjs.com/package/@docxtor/editor).

```sh
npm install @docxtor/core
```

Every edit is a serializable op, so the same edits work from code, from an AI tool call and from the command line.

## In Node

Node cannot fetch the engine by URL, so pass it the bytes of the `.wasm` file the package ships:

```js
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { Engine, loadEngine } from "@docxtor/core";

await loadEngine(await readFile(fileURLToPath(import.meta.resolve("@docxtor/core/wasm/docxtor_bg.wasm"))));

const doc = Engine.open(new Uint8Array(await readFile("report.docx")), "report.docx");
doc.apply({ op: "replace_text", find: "2025", replace: "2026", all: true });
const bytes = doc.save(); // a .docx, Uint8Array
doc.dispose();
```

## In a browser

With Vite, webpack 5 or another bundler that follows `new URL("...", import.meta.url)`, call `loadEngine()` with no argument: the engine and the workers that compile and zip it off the page are found and bundled for you. Without such a bundler (esbuild, a plain `<script type="module">`), copy `node_modules/@docxtor/core/wasm/docxtor_bg.wasm` to your site and pass its URL: `loadEngine("/docxtor_bg.wasm")`. The workers are an optimisation; when they cannot start, the engine compiles and zips on the page.

The package is ES modules only and has no dependencies. It needs Node 20 or later outside the browser.

## Documentation

How to embed Docxtor, the op language, the AI tools and the extension API are in the documentation: https://docxtor.tinycommit.dev/docs/embedding

Docxtor is licensed under MIT or Apache-2.0, at your choice. The licences of what the engine contains are in `THIRD_PARTY_LICENSES.txt`.
