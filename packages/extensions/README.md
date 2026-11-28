# @docxtor/extensions

The Docxtor extension API: the types an extension is written against, and the `ExtensionHost` that registers what extensions contribute (commands, keyboard shortcuts, toolbar items, panels, importers and exporters, templates, themes, AI providers and tools, storage providers, languages). It has no dependency on the DOM, so a host and its tests can run in Node.

```sh
npm install @docxtor/core @docxtor/extensions
```

```ts
import { defineExtension } from "@docxtor/extensions";

export const shout = defineExtension({
  id: "acme.shout",
  name: "Shout",
  activate(ctx) {
    ctx.commands.register(
      "acme.shout",
      () => {
        const sel = ctx.document.selection();
        if (sel?.text) ctx.document.insertMarkdown(sel.text.toUpperCase());
      },
      { title: "Make the selection upper case", category: "Format", keybinding: "Mod+Shift+U" },
    );
  },
});
```

You can pass an extension to `new DocxtorEditor({ extensions: [shout] })` from [`@docxtor/editor`](https://www.npmjs.com/package/@docxtor/editor). The editor re-exports this package, so an app that uses the editor does not need to install it separately.

The package is ES modules only. It depends on `@docxtor/core` at the same version.

## Documentation

The extensions guide lists everything an extension can contribute: https://docxtor.tinycommit.dev/docs/extensions

Docxtor is licensed under MIT or Apache-2.0, at your choice.
