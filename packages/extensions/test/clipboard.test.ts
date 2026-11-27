// Paste converters: the registry, validation, and their removal with the extension.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { defineExtension, ExtensionHost, type DocumentApi, type UiApi } from "../src/index.ts";

const host = () => new ExtensionHost({ document: {} as DocumentApi, ui: { notify() {} } as unknown as UiApi, mac: false });

test("an extension adds a converter for its own clipboard format, and it goes when the extension does", async () => {
  const h = host();
  await h.add(
    defineExtension({
      id: "test.shout",
      name: "Shout",
      activate(ctx) {
        ctx.clipboard.converters.register({ id: "test.shout", label: "Shout", mime: "application/x-shout", priority: 5, convert: ({ data }) => ({ markdown: data.toUpperCase() }) });
      },
    }),
  );
  assert.deepEqual(
    h.pasteConverters.list().map((c) => [c.id, c.mime]),
    [["test.shout", "application/x-shout"]],
  );
  const c = h.pasteConverters.list()[0];
  const out = await c.convert({ data: "hi", get: () => undefined, types: [], engine: null as never, fetchImage: async () => null });
  assert.deepEqual(out, { markdown: "HI" });
  await h.disable("test.shout");
  assert.equal(h.pasteConverters.list().length, 0);
});

test("a converter needs an id, a format and a convert function", async () => {
  const h = host();
  let n = 0;
  const bad = (c: unknown) =>
    h.add(
      defineExtension({
        id: `test.bad${n++}`,
        name: "Bad",
        activate(ctx) {
          ctx.clipboard.converters.register(c as never);
        },
      }),
    );
  await bad({ id: "", label: "x", mime: "text/x", convert: () => null });
  await bad({ id: "a", label: "x", mime: "", convert: () => null });
  await bad({ id: "b", label: "x", mime: "text/x" });
  assert.equal(h.pasteConverters.list().length, 0);
});
