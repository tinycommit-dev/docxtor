// Template stores, settings, and the shortcuts a person chooses: registered by extensions, checked, and removed with them.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ExtensionHost, parseChord, specFromEvent, type DocumentApi, type TemplateInfo, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const key = (k: string, code: string, mods: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) => {
  let prevented = false;
  return { key: k, code, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods, get defaultPrevented() { return prevented; }, preventDefault() { prevented = true; } };
};

test("template stores list beside the built-in templates, make a document, and come and go with their extension", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const kept = new Map<string, { info: TemplateInfo; bytes: Uint8Array }>();
  await host.add({
    id: "docxtor.starters",
    name: "Starters",
    activate(ctx) {
      ctx.templates.register({ id: "letter", name: "Letter", description: "A letter", create: () => ({ markdown: "Dear" }) });
    },
  });
  await host.add({
    id: "acme.templates",
    name: "Acme templates",
    activate(ctx) {
      ctx.templates.stores.register({
        id: "acme.mem",
        label: "Team drive",
        list: () => [...kept.values()].map((k) => k.info),
        read: (id) => kept.get(id)!.bytes,
        save: (info, bytes) => {
          const made = { id: `t${kept.size + 1}`, name: info.name, description: info.description, created: "2026-10-14T00:00:00Z" };
          kept.set(made.id, { info: made, bytes });
          return made;
        },
        remove: (id) => void kept.delete(id),
      });
    },
  });
  const [store] = host.templateStores.list();
  const saved = await store.save({ name: "Invoice", description: "Monthly" }, new Uint8Array([1, 2, 3]));
  assert.equal(saved.name, "Invoice");

  const all = await host.templateEntries();
  assert.deepEqual(all.map((t) => [t.name, t.source]), [["Letter", ""], ["Invoice", "Team drive"]]);
  assert.equal(all[1].id, "acme.mem:t1");
  assert.deepEqual(await all[1].create(), { docx: new Uint8Array([1, 2, 3]) });
  assert.deepEqual(await all[0].create(), { markdown: "Dear" });
  // Only a template a store keeps can be removed from the dialog.
  assert.equal(all[0].remove, undefined);
  await all[1].remove!();
  assert.equal((await host.templateEntries()).length, 1);

  await host.disable("acme.templates");
  assert.equal(host.templateStores.list().length, 0);
});

test("a template store that fails is skipped; one that cannot list, read, save and remove is refused", async () => {
  const errors: string[] = [];
  const host = new ExtensionHost({ document: doc, ui, onError: (_id: string, e: unknown) => void errors.push(String((e as Error).message ?? e)) } as never);
  await host.add({
    id: "acme.broken",
    name: "Broken",
    activate(ctx) {
      ctx.templates.stores.register({ id: "b", label: "Broken", list: () => { throw new Error("offline"); }, read: () => new Uint8Array(), save: () => ({ id: "x", name: "x", created: "" }), remove: () => {} });
    },
  });
  await host.add({ id: "acme.bad", name: "Bad", activate: (ctx) => void ctx.templates.stores.register({ id: "c", label: "C", list: () => [] } as never) });
  assert.deepEqual(await host.templateEntries(), []);
  assert.ok(errors.some((e) => /offline/.test(e)));
  assert.ok(errors.some((e) => /needs list\(\), read\(\), save\(\) and remove\(\)/.test(e)));
  assert.equal(host.templateStores.list().length, 1);
});

test("settings are checked, listed in order and removed with their extension", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  let interval = 5;
  await host.add({
    id: "acme.settings",
    name: "Acme settings",
    activate(ctx) {
      ctx.settings.register({ id: "acme.interval", label: "Autosave", kind: "number", unit: "seconds", min: 1, max: 60, get: () => interval, set: (v) => void (interval = Number(v)) });
      ctx.settings.register({ id: "acme.mode", label: "Mode", kind: "choice", choices: [{ value: "a", label: "A" }], get: () => "a", set: () => {} });
    },
  });
  assert.deepEqual(host.settings.list().map((s) => s.id), ["acme.interval", "acme.mode"]);
  host.settings.list()[0].set(30);
  assert.equal(interval, 30);
  await host.disable("acme.settings");
  assert.equal(host.settings.list().length, 0);

  const errors: string[] = [];
  const quiet = new ExtensionHost({ document: doc, ui, onError: (_id: string, e: unknown) => void errors.push(String((e as Error).message ?? e)) } as never);
  await quiet.add({
    id: "bad",
    name: "Bad",
    activate(ctx) {
      for (const s of [{ id: "a", label: "", kind: "toggle" }, { id: "b", label: "B", kind: "dial" }, { id: "c", label: "C", kind: "choice" }, { id: "d", label: "D", kind: "toggle" }]) {
        try {
          ctx.settings.register({ ...s, get: s.id === "d" ? undefined : () => 1, set: () => {} } as never);
        } catch (e) {
          errors.push(String((e as Error).message));
        }
      }
    },
  });
  assert.equal(quiet.settings.list().length, 0);
  assert.equal(errors.length, 4);
});

test("a shortcut the person sets replaces the default, takes the key from the command that had it, and can be undone", async () => {
  const host = new ExtensionHost({ document: doc, ui, mac: true });
  const ran: string[] = [];
  await host.add({
    id: "t",
    name: "T",
    activate(ctx) {
      ctx.commands.register("a", () => void ran.push("a"), { title: "A", keybinding: "Mod+Shift+K" });
      ctx.commands.register("b", () => void ran.push("b"), { title: "B", keybinding: ["Mod+J", "Mod+Alt+J"] });
      ctx.commands.register("c", () => void ran.push("c"), { title: "C" });
    },
  });
  let changes = 0;
  host.onShortcutsChange(() => changes++);
  assert.equal(host.shortcut("c"), "");
  host.setShortcuts("c", ["Mod+Shift+K"]);
  assert.equal(host.shortcut("c"), "⇧⌘K");
  // The key moved from A to C.
  assert.deepEqual(host.shortcutSpecs("a"), []);
  assert.equal(host.shortcut("a"), "");
  assert.equal(host.handleKey(key("k", "KeyK", { metaKey: true, shiftKey: true })), true);
  assert.deepEqual(ran, ["c"]);
  // B keeps its other shortcut; setting one replaces both defaults.
  host.setShortcuts("b", ["Mod+Alt+B"]);
  // The old key does nothing now: it is swallowed and runs no command, rather than being left to the browser.
  assert.equal(host.handleKey(key("j", "KeyJ", { metaKey: true })), true);
  assert.deepEqual(ran, ["c"]);
  assert.equal(host.handleKey(key("b", "KeyB", { metaKey: true, altKey: true })), true);
  assert.deepEqual(ran, ["c", "b"]);
  assert.deepEqual(host.shortcutUsers("Cmd+Alt+b").map((c) => c.id), ["b"]);
  assert.deepEqual(host.shortcutUsers("Mod+Alt+B", "b"), []);
  // Saved choices round trip, and a bad one is dropped.
  const saved = JSON.parse(JSON.stringify(host.shortcutOverrides));
  host.setShortcuts("c", null);
  host.setShortcuts("b", null);
  host.setShortcuts("a", null);
  assert.deepEqual(host.shortcutOverrides, { });
  assert.equal(host.shortcut("a"), "⇧⌘K");
  host.loadShortcutOverrides({ ...saved, nope: ["Mod+Q"], bad: ["Hyper+K"] });
  assert.equal(host.shortcut("c"), "⇧⌘K");
  assert.equal(host.shortcut("b"), "⌥⌘B");
  assert.ok(changes >= 5);
  // An empty list takes a command's shortcuts away.
  host.setShortcuts("b", []);
  assert.equal(host.handleKey(key("b", "KeyB", { metaKey: true, altKey: true })), false);
  // A key a command came with, taken away and given to nothing, is swallowed (the browser must not do its own thing with it).
  const freed = key("j", "KeyJ", { metaKey: true });
  assert.equal(host.handleKey(freed), true);
  assert.equal(freed.defaultPrevented, true);
  assert.equal(ran.length, 2, "and runs nothing");
  assert.throws(() => host.setShortcuts("a", ["Hyper+K"]), /Unknown modifier/);
  assert.throws(() => host.setShortcuts("zz", ["Mod+K"]), /No command/);
});

test("a key taken from a command comes back when the command that took it is reset, instead of doing nothing", async () => {
  const host = new ExtensionHost({ document: doc, ui, mac: true });
  const ran: string[] = [];
  await host.add({
    id: "t",
    name: "T",
    activate(ctx) {
      ctx.commands.register("bold", () => void ran.push("bold"), { title: "Bold", keybinding: "Mod+B" });
      ctx.commands.register("mark", () => void ran.push("mark"), { title: "Mark" });
      ctx.commands.register("other", () => void ran.push("other"), { title: "Other" });
    },
  });
  const press = () => host.handleKey(key("b", "KeyB", { metaKey: true }));
  // Mark takes Mod+B from Bold, then is reset: Bold has it again, with no override left behind.
  host.setShortcuts("mark", ["Mod+B"]);
  assert.deepEqual(host.shortcutSpecs("bold"), []);
  assert.equal(press(), true);
  assert.deepEqual(ran, ["mark"]);
  host.setShortcuts("mark", null);
  assert.deepEqual(host.shortcutSpecs("bold"), ["Mod+B"]);
  assert.deepEqual(host.shortcutOverrides, {});
  assert.equal(press(), true);
  assert.deepEqual(ran, ["mark", "bold"]);
  // The same when the command picks another key instead.
  host.setShortcuts("mark", ["Mod+B"]);
  host.setShortcuts("mark", ["Mod+Alt+M"]);
  assert.deepEqual(host.shortcutSpecs("bold"), ["Mod+B"]);
  // A key given on to a third command goes back to the first when the last one lets go.
  host.setShortcuts("mark", ["Mod+B"]);
  host.setShortcuts("other", ["Mod+B"]);
  assert.deepEqual(host.shortcutSpecs("mark"), []);
  host.setShortcuts("other", null);
  assert.deepEqual(host.shortcutSpecs("bold"), ["Mod+B"]);
  assert.deepEqual(host.shortcutSpecs("mark"), []);
  // A key the person cleared on purpose stays free (and does nothing), even after a command used and dropped it.
  host.setShortcuts("bold", []);
  host.setShortcuts("mark", ["Mod+B"]);
  host.setShortcuts("mark", null);
  assert.deepEqual(host.shortcutSpecs("bold"), []);
  ran.length = 0;
  assert.equal(press(), true);
  assert.deepEqual(ran, []);
});

test("a key taken from a command still comes back after the choices are saved and loaded again", async () => {
  const make = async () => {
    const host = new ExtensionHost({ document: doc, ui, mac: true });
    await host.add({
      id: "t",
      name: "T",
      activate(ctx) {
        ctx.commands.register("bold", () => undefined, { title: "Bold", keybinding: "Mod+B" });
        ctx.commands.register("mark", () => undefined, { title: "Mark" });
      },
    });
    return host;
  };
  const first = await make();
  first.setShortcuts("mark", ["Mod+B"]);
  const overrides = JSON.parse(JSON.stringify(first.shortcutOverrides));
  const taken = JSON.parse(JSON.stringify(first.shortcutTaken));
  assert.equal(taken.length, 1);
  // A new page: what was saved is loaded, then Mark is reset.
  const second = await make();
  second.loadShortcutOverrides(overrides, taken);
  assert.deepEqual(second.shortcutSpecs("bold"), []);
  second.setShortcuts("mark", null);
  assert.deepEqual(second.shortcutSpecs("bold"), ["Mod+B"]);
  assert.deepEqual(second.shortcutOverrides, {});
  assert.equal(second.handleKey(key("b", "KeyB", { metaKey: true })), true);
  assert.deepEqual(second.shortcutTaken, []);
  // A record the saved choices no longer match, or one that is malformed, is dropped.
  const third = await make();
  third.loadShortcutOverrides({ bold: [] }, [...taken, { chord: 1 } as never]);
  assert.deepEqual(third.shortcutTaken, []);
});

test("a key press becomes a shortcut the host reads back", () => {
  const k = (key: string, code: string, mods: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) => specFromEvent(key_(key, code, mods), true);
  const key_ = (key: string, code: string, mods: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>>) => ({ key, code, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods });
  assert.equal(k("k", "KeyK", { metaKey: true, shiftKey: true }), "Mod+Shift+K");
  // Option+/ types a division sign on a Mac; the physical key is what counts.
  assert.equal(k("÷", "Slash", { altKey: true }), "Alt+/");
  assert.equal(k("F5", "F5"), "F5");
  assert.equal(k("ArrowUp", "ArrowUp", { altKey: true }), "Alt+Up");
  assert.equal(k("k", "KeyK", { ctrlKey: true, metaKey: true }), "Mod+Ctrl+K");
  // Plain typing and a modifier on its own are not shortcuts.
  assert.equal(k("k", "KeyK"), null);
  assert.equal(k("Shift", "ShiftLeft", { shiftKey: true }), null);
  assert.equal(k("Meta", "MetaLeft", { metaKey: true }), null);
  assert.equal(specFromEvent(key_("k", "KeyK", { ctrlKey: true }), false), "Mod+K");
  for (const spec of ["Mod+Shift+K", "Alt+/", "F5", "Alt+Up", "Mod+Ctrl+K", "Mod+Space"]) assert.doesNotThrow(() => parseChord(spec, true), spec);
});
