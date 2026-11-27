// View modes: the registry, validation and the pure helpers. Runs in Node.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { columnWidth, defineExtension, ExtensionHost, sortViews, type DocumentApi, type UiApi, type ViewMode } from "../src/index.ts";

const host = () => new ExtensionHost({ document: {} as DocumentApi, ui: { notify() {} } as unknown as UiApi, mac: true });

test("views come and go with their extension", async () => {
  const h = host();
  let changes = 0;
  h.views.onChange(() => changes++);
  await h.add(
    defineExtension({
      id: "test.typewriter",
      name: "Typewriter",
      activate(ctx) {
        ctx.views.register({ id: "typewriter", title: "Typewriter", layout: "continuous", width: "window", maxWidth: 600, className: "typewriter" });
      },
    }),
  );
  assert.deepEqual(
    h.views.list().map((v) => v.id),
    ["typewriter"],
  );
  assert.ok(changes > 0);
  assert.equal(h.views.list()[0].layout, "continuous");
  await h.remove("test.typewriter");
  assert.equal(h.views.list().length, 0);
});

test("a view needs an id and a title, and enter() has to be a function", async () => {
  const h = host();
  const refused = async (id: string, view: unknown, message: RegExp) => {
    await h.add({ id, name: "Bad", activate: (ctx) => void ctx.views.register(view as ViewMode) });
    assert.match(String(h.list().find((x) => x.id === id)?.error), message);
  };
  await refused("bad1", { title: "No id" }, /needs an id/);
  await refused("bad2", { id: "x" }, /needs a title/);
  await refused("bad3", { id: "y", title: "Y", enter: "not a function" }, /enter\(\) to be a function/);
  assert.equal(h.views.list().length, 0);
});

test("views are offered by order, then by when they were registered", () => {
  const list = [{ id: "c" }, { id: "a", order: 10 }, { id: "b", order: 10 }, { id: "d", order: 5 }];
  assert.deepEqual(
    sortViews(list).map((v) => v.id),
    ["d", "a", "b", "c"],
  );
  // The input is left as it was.
  assert.equal(list[0].id, "c");
});

test("a continuous layout is as wide as the page's text, or fits the window up to its limit", () => {
  assert.equal(columnWidth({}, 624, 1200), 624);
  assert.equal(columnWidth({ width: "page" }, 624, 300), 624);
  assert.equal(columnWidth({ width: "window" }, 624, 1200), 760);
  assert.equal(columnWidth({ width: "window", maxWidth: 900 }, 624, 1200), 900);
  assert.equal(columnWidth({ width: "window" }, 624, 500.7), 500);
  assert.equal(columnWidth({ width: "window" }, 624, 100), 240);
});
