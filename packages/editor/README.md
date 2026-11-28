# @docxtor/editor

A document editor for the web with real pages and lossless `.docx`: bytes in, bytes out. It needs no framework and no server. Use it as a JavaScript class or as the `<docxtor-editor>` element.

```sh
npm install @docxtor/editor
```

The editor depends on [`@docxtor/core`](https://www.npmjs.com/package/@docxtor/core) (the engine) and [`@docxtor/extensions`](https://www.npmjs.com/package/@docxtor/extensions), and re-exports both.

```ts
import { DocxtorEditor } from "@docxtor/editor";
import "@docxtor/editor/style.css";

const editor = new DocxtorEditor({ onSave: (bytes) => upload(bytes) }).mount(document.getElementById("editor")!);
await editor.open(await (await fetch("/report.docx")).arrayBuffer(), "report.docx");
const bytes = editor.save(); // a .docx
```

Or with no code at all:

```html
<docxtor-editor src="/report.docx" style="height: 80vh"></docxtor-editor>
<script type="module">
  import "@docxtor/editor";
  import "@docxtor/editor/style.css";
</script>
```

Give the element the editor mounts in a height.

## Bundlers

Vite and webpack 5 bundle everything the editor loads (the engine, its two workers and the spelling checker's WebAssembly). With a bundler that does not follow `new URL("...", import.meta.url)`, such as esbuild, copy `@docxtor/core/wasm/docxtor_bg.wasm` and `@docxtor/editor/dist/spelling/wasm/spell_bg.wasm` next to your bundle (or pass the engine's location to `DocxtorEditor.ready(url)`); the workers are an optimisation and the editor works without them.

Importing the package on a server (a server-rendered page, Node) does nothing: nothing touches the DOM until you create an editor in the browser.

The spelling checker is off until you give it dictionaries; see the Spelling section of the embedding guide. The package is ES modules only.

## Documentation

Options, methods, events, spelling, extensions and the rest: https://docxtor.tinycommit.dev/docs/embedding

Docxtor is licensed under MIT or Apache-2.0, at your choice. The licences of what the spelling checker contains are in `THIRD_PARTY_LICENSES.txt`.
