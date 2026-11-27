// Snippet stores and source libraries: registered by extensions, checked, and removed with their extension.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ExtensionHost, type DocumentApi, type Snippet, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;

const snippet = (id: string, name: string): Snippet => ({ id, name, gallery: "AutoText", fragment: { version: 1 } as Snippet["fragment"], text: name, created: "2026-10-14T00:00:00Z" });

test("snippet stores come and go with their extension, and list and save through the store", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const kept: Snippet[] = [snippet("a", "Sign-off")];
  await host.add({
    id: "acme.snippets",
    name: "Acme snippets",
    activate(ctx) {
      ctx.snippets.stores.register({ id: "acme.mem", label: "Memory", list: () => [...kept], save: (all) => void kept.splice(0, kept.length, ...all) });
    },
  });
  const [store] = host.snippetStores.list();
  assert.equal(store.id, "acme.mem");
  assert.equal((await store.list()).length, 1);
  await store.save([...(await store.list()), snippet("b", "Address")]);
  assert.deepEqual(kept.map((s) => s.name), ["Sign-off", "Address"]);
  await host.disable("acme.snippets");
  assert.equal(host.snippetStores.list().length, 0);
});

test("source libraries are checked and removed with their extension", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({
    id: "acme.libs",
    name: "Acme libraries",
    activate(ctx) {
      ctx.citations.libraries.register({ id: "acme.shared", label: "Team list", note: "Kept on the team drive.", list: () => [], save: () => {} });
    },
  });
  assert.deepEqual(host.sourceLibraries.list().map((l) => l.label), ["Team list"]);
  await host.disable("acme.libs");
  assert.equal(host.sourceLibraries.list().length, 0);

  // A store or library that can't list and save, or has no label, is refused.
  const errors: string[] = [];
  const quiet = new ExtensionHost({ document: doc, ui, onError: (_id: string, e: unknown) => void errors.push(String((e as Error).message ?? e)) } as never);
  await quiet.add({ id: "bad1", name: "Bad", activate: (ctx) => void ctx.snippets.stores.register({ id: "x", label: "X" } as never) });
  await quiet.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.citations.libraries.register({ id: "y", label: "", list: () => [], save: () => {} }) });
  assert.equal(quiet.snippetStores.list().length, 0);
  assert.equal(quiet.sourceLibraries.list().length, 0);
});
