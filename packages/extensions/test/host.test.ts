// The extension host runs anywhere: no DOM, no editor.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { defineExtension, ExtensionHost, formatChord, parseChord, type DocumentApi, type ExtensionContext, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const key = (k: string, code: string, mods: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) => {
  let prevented = false;
  return {
    key: k,
    code,
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    ...mods,
    get defaultPrevented() {
      return prevented;
    },
    preventDefault() {
      prevented = true;
    },
  };
};

test("contributions come and go with their extension", async () => {
  const host = new ExtensionHost({ document: doc, ui, mac: true });
  let ran = 0;
  let deactivated = false;
  const ext = defineExtension({
    id: "test.hello",
    name: "Hello",
    activate(ctx) {
      ctx.commands.register("hello.say", () => ++ran, { title: "Say hello", keybinding: "Mod+Shift+H" });
      ctx.toolbar.register({ id: "hello", command: "hello.say", label: "Hi" });
      ctx.exporters.register({ id: "shout", label: "Shout", extension: "txt", mime: "text/plain", export: () => "HI" });
    },
    deactivate() {
      deactivated = true;
    },
  });
  let changes = 0;
  host.toolbar.onChange(() => changes++);
  await host.add(ext);
  assert.equal(host.toolbar.list().length, 1);
  assert.deepEqual(await host.execute("hello.say"), { status: "done", value: 1 });
  assert.equal(host.shortcut("hello.say"), "⇧⌘H");
  assert.ok(host.exporter("shout"));

  // ⌘⇧H runs the command; the same keys with an extra modifier don't.
  assert.equal(host.handleKey(key("h", "KeyH", { metaKey: true, shiftKey: true })), true);
  assert.equal(host.handleKey(key("h", "KeyH", { metaKey: true, shiftKey: true, altKey: true })), false);
  // A key the editor already handled is left alone.
  const handled = key("h", "KeyH", { metaKey: true, shiftKey: true });
  handled.preventDefault();
  assert.equal(host.handleKey(handled), false);
  assert.equal(ran, 2);
  // In a text field, ⌘Z stays with the field; other shortcuts still work.
  await host.add({ id: "test.undo", name: "Undo", activate: (ctx) => void ctx.commands.register("doc.undo", () => 0, { title: "Undo", keybinding: "Mod+Z" }) });
  assert.equal(host.handleKey(key("z", "KeyZ", { metaKey: true }), { textField: true }), false);
  assert.equal(host.handleKey(key("z", "KeyZ", { metaKey: true })), true);
  assert.equal(host.handleKey(key("h", "KeyH", { metaKey: true, shiftKey: true }), { textField: true }), true);
  ran = 2;
  await host.remove("test.undo");

  await host.disable("test.hello");
  assert.equal(deactivated, true);
  assert.equal(host.toolbar.list().length, 0);
  assert.equal(host.command("hello.say"), undefined);
  assert.equal(host.exporter("shout"), undefined);
  assert.equal(changes, 2);
  await assert.rejects(host.execute("hello.say"), /No command/);

  await host.enable("test.hello");
  assert.equal(host.commands.list().length, 1);
  assert.deepEqual(host.list().map((e) => [e.id, e.enabled]), [["test.hello", true]]);
});

test("a failing extension is reported and leaves nothing behind", async () => {
  const errors: string[] = [];
  const host = new ExtensionHost({ document: doc, ui, onError: (id) => errors.push(id) });
  await host.add({
    id: "test.broken",
    name: "Broken",
    activate(ctx) {
      ctx.commands.register("broken.cmd", () => 0, { title: "Broken" });
      throw new Error("boom");
    },
  });
  assert.deepEqual(errors, ["test.broken"]);
  assert.equal(host.list()[0].error, "boom");
  assert.equal(host.commands.list().length, 0);
});

test("command ids are unique, required extensions stay on", async () => {
  const host = new ExtensionHost({ document: doc, ui, onError: () => {} });
  await host.add({ id: "a", name: "A", activate: (ctx) => void ctx.commands.register("same", () => 0, { title: "Same" }) }, { required: true });
  await host.add({ id: "b", name: "B", activate: (ctx) => void ctx.commands.register("same", () => 0, { title: "Same" }) });
  assert.match(host.list()[1].error ?? "", /already registered/);
  await host.disable("a");
  assert.equal(host.list()[0].enabled, true);
});

test("importers match by file extension; state is kept per extension", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  let seen = 0;
  await host.add({
    id: "md",
    name: "Markdown",
    activate(ctx) {
      ctx.importers.register({ id: "md", label: "Markdown", extensions: ["md", ".markdown"], import: (b) => ({ markdown: new TextDecoder().decode(b) }) });
      ctx.state.set("count", 3);
      seen = ctx.state.get("count", 0);
    },
  });
  assert.equal(host.importerFor("Notes.MD")?.id, "md");
  assert.equal(host.importerFor("notes.markdown")?.id, "md");
  assert.equal(host.importerFor("notes.docx"), undefined);
  assert.equal(seen, 3);
});

test("importers that detect their file type are chosen by content, for shared extensions and for misnamed files", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const starts = (s: string) => (b: Uint8Array) => new TextDecoder().decode(b.slice(0, s.length)) === s;
  const bytes = (s: string) => new TextEncoder().encode(s);
  await host.add({
    id: "formats",
    name: "Formats",
    activate(ctx) {
      ctx.importers.register({ id: "flat", label: "Flat", extensions: ["xml"], detect: starts("<pkg"), import: () => ({ docx: new Uint8Array() }) });
      ctx.importers.register({ id: "wordml", label: "WordML", extensions: ["xml"], detect: starts("<w:wordDocument"), import: () => ({ docx: new Uint8Array() }) });
      ctx.importers.register({ id: "rtf", label: "Rich Text", extensions: ["rtf"], detect: starts("{\\rtf"), import: () => ({ docx: new Uint8Array() }) });
      ctx.importers.register({ id: "md", label: "Markdown", extensions: ["md"], import: () => ({ markdown: "" }) });
    },
  });
  // Without the bytes the latest importer for the extension answers.
  assert.equal(host.importerFor("a.xml")?.id, "wordml");
  // With them, the one that recognises the file does.
  assert.equal(host.importerFor("a.xml", bytes("<pkg:package/>"))?.id, "flat");
  assert.equal(host.importerFor("a.xml", bytes("<w:wordDocument/>"))?.id, "wordml");
  // A file called .docx that is really Rich Text.
  assert.equal(host.importerFor("report.docx", bytes("{\\rtf1 hi}"))?.id, "rtf");
  assert.equal(host.importerFor("report.docx", bytes("PK\u0003\u0004")), undefined);
  assert.equal(host.importerForBytes(bytes("{\\rtf1"))?.id, "rtf");
  // An importer without detect takes its extension, whatever the bytes are.
  assert.equal(host.importerFor("n.md", bytes("{\\rtf1"))?.id, "md");
  // A detect that throws counts as no.
  await host.add({
    id: "bad",
    name: "Bad",
    activate(ctx) {
      ctx.importers.register({
        id: "bad",
        label: "Bad",
        extensions: ["bad"],
        detect: () => {
          throw new Error("no");
        },
        import: () => ({ docx: new Uint8Array() }),
      });
    },
  });
  assert.equal(host.importerFor("x.bad", bytes("x")), undefined);
});

test("shortcuts parse, match by physical key, and print per platform", () => {
  assert.deepEqual(parseChord("Mod+Alt+F", false), { meta: false, ctrl: true, alt: true, shift: false, key: "f" });
  assert.equal(formatChord("Mod+Shift+P", true), "⇧⌘P");
  assert.equal(formatChord("Mod+Shift+P", false), "Ctrl+Shift+P");
  assert.equal(formatChord("Alt+/", false), "Alt+/");
  assert.throws(() => parseChord("Hyper+K", true), /Unknown modifier/);
  // Option+/ on a Mac types "÷"; the physical key still matches.
  const host = new ExtensionHost({ document: doc, ui, mac: true });
  return host
    .add({ id: "p", name: "P", activate: (ctx) => void ctx.commands.register("p", () => 0, { title: "P", keybinding: "Alt+/" }) })
    .then(() => assert.equal(host.handleKey(key("÷", "Slash", { altKey: true })), true));
});

test("language packs: translations, placeholders, plurals and fallbacks", async () => {
  const { pickLanguage, setLanguage, t, tn, currentLocale } = await import("../src/index.ts");
  const es = {
    locale: "es",
    label: "Español",
    messages: { "Find and replace": "Buscar y reemplazar", "Page {page} of {pages}": "Página {page} de {pages}", "{n} words": { one: "{n} palabra", other: "{n} palabras" } },
  };
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({ id: "lang.es", name: "Spanish", contributes: { languages: [es] }, activate() {} });
  assert.equal(pickLanguage("es-MX", host.languages())?.locale, "es");
  assert.equal(pickLanguage("de", host.languages()), null);
  setLanguage(es);
  assert.equal(currentLocale(), "es");
  assert.equal(t("Find and replace"), "Buscar y reemplazar");
  assert.equal(t("Page {page} of {pages}", { page: 2, pages: 12000 }), "Página 2 de 12.000");
  assert.equal(tn(1, "{n} word", "{n} words"), "1 palabra");
  assert.equal(tn(3, "{n} word", "{n} words"), "3 palabras");
  assert.equal(t("Untranslated"), "Untranslated");
  setLanguage(pickLanguage("en-XA", []));
  assert.match(t("Save"), /^⟦Sávé⟧$/);
  setLanguage(null);
  assert.equal(tn(1, "{n} word", "{n} words"), "1 word");
  await host.disable("lang.es");
  assert.equal(host.languages().length, 0);
});

test("menus: groups in order, submenus, disabled and checked entries", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  let ruler = true;
  await host.add({
    id: "m",
    name: "M",
    activate(ctx) {
      ctx.commands.register("a", () => "a", { title: "A" });
      ctx.commands.register("b", () => "b", { title: "B", when: () => false });
      ctx.commands.register("ruler", () => (ruler = !ruler), { title: "Show ruler", toggled: () => ruler });
      ctx.menus.register({ menu: "view", command: "ruler", group: "2_show" });
      ctx.menus.register({ menu: "view", command: "b", group: "1_first", order: 2 });
      ctx.menus.register({ menu: "view", command: "a", group: "1_first", order: 1 });
      ctx.menus.register({ menu: "view", command: "missing", group: "1_first" });
      ctx.menus.register({ menu: "view", submenu: "view/zoom", title: "Zoom" });
      ctx.menus.register({ menu: "view/zoom", command: "a", title: "Zoom in", args: [1] });
      ctx.menus.register({ menu: "view", submenu: "view/empty", title: "Empty" });
      // A submenu that contains itself is left out rather than looping.
      ctx.menus.register({ menu: "view/loop", submenu: "view/loop", title: "Loop" });
      ctx.menus.register({ menu: "view", submenu: "view/loop", title: "Loop" });
    },
  });
  const view = host.menu("view");
  assert.deepEqual(view.map((g) => g.map((e) => e.title)), [["A", "B"], ["Show ruler"], ["Zoom"]]);
  assert.deepEqual(view[0].map((e) => e.enabled), [true, false]);
  assert.equal(view[1][0].checked, true);
  await host.run(view[1][0]);
  assert.equal(host.menu("view")[1][0].checked, false);
  assert.deepEqual(await host.run(host.menu("view/zoom")[0][0]), { status: "done", value: "a" });
  // hasMenu says what menu() would, without building the entries: an empty submenu, a loop and a missing command count for nothing.
  for (const id of ["view", "view/zoom", "view/empty", "view/loop", "nowhere"]) assert.equal(host.hasMenu(id), host.menu(id).length > 0, id);
  await host.disable("m");
  assert.equal(host.menu("view").length, 0);
  assert.equal(host.hasMenu("view"), false);
});

test("icons: commands and menus name them; a later extension replaces one", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const svg = (s: string) => `<svg viewBox="0 0 24 24">${s}</svg>`;
  await host.add({
    id: "base",
    name: "Base icons",
    activate(ctx) {
      ctx.icons.register({ id: "bold", svg: svg("b") });
      ctx.commands.register("bold", () => 0, { title: "Bold", icon: "bold" });
      ctx.menus.register({ menu: "format", command: "bold" });
    },
  });
  assert.equal(host.menu("format")[0][0].icon, svg("b"));
  await host.add({ id: "pack", name: "Icon pack", activate: (ctx) => void ctx.icons.register({ id: "bold", svg: svg("B") }) });
  assert.equal(host.menu("format")[0][0].icon, svg("B"));
  await host.disable("pack");
  assert.equal(host.icon("bold"), svg("b"));
  assert.equal(host.icon(svg("x")), svg("x"));
  assert.equal(host.icon("nothing"), undefined);
  // The lookup is cached, so a new icon must show up without a stale answer.
  await host.add({ id: "more", name: "More icons", activate: (ctx) => void ctx.icons.register({ id: "italic", svg: svg("i") }) });
  assert.equal(host.icon("italic"), svg("i"));
  assert.equal(host.icon("bold"), svg("b"));
  await assert.rejects(host.add({ id: "bad", name: "Bad", activate: (ctx) => void ctx.icons.register({ id: "x", svg: "nope" }) }).then(() => { throw host.list().find((x) => x.id === "bad")?.error; }));
});

test("icon packs: the chosen pack draws over Docxtor's, what it leaves out stays Docxtor's", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const svg = (s: string) => `<svg viewBox="0 0 24 24">${s}</svg>`;
  await host.add({
    id: "base",
    name: "Base",
    activate(ctx) {
      ctx.iconPacks.register({ id: "docxtor", label: "Docxtor", icons: [{ id: "bold", svg: svg("b") }, { id: "italic", svg: svg("i") }] });
      ctx.commands.register("bold", () => 0, { title: "Bold", icon: "bold" });
      ctx.menus.register({ menu: "format", command: "bold" });
    },
  });
  assert.equal(host.iconPack, "docxtor");
  assert.deepEqual(host.listIconPacks(), [{ id: "docxtor", label: "Docxtor" }]);
  // A pack that redraws one icon only.
  await host.add({ id: "thin", name: "Thin", activate: (ctx) => void ctx.iconPacks.register({ id: "thin", label: "Thin", icons: [{ id: "bold", svg: svg("T") }] }) });
  assert.deepEqual(host.listIconPacks().map((p) => p.id), ["docxtor", "thin"]);
  // Offering a pack changes nothing until it is chosen.
  assert.equal(host.icon("bold"), svg("b"));
  let changes = 0;
  const sub = host.onDidChangeIcons(() => changes++);
  host.setIconPack("thin");
  // Told once, in a microtask, for a run of changes.
  await Promise.resolve();
  assert.equal(changes, 1);
  assert.equal(host.activeIconPack, "thin");
  assert.equal(host.icon("bold"), svg("T"));
  assert.equal(host.icon("italic"), svg("i"), "an icon the pack leaves out falls back to Docxtor's");
  assert.equal(host.menu("format")[0][0].icon, svg("T"), "menus follow the pack");
  host.setIconPack("thin");
  await Promise.resolve();
  assert.equal(changes, 1, "choosing the same pack again changes nothing");
  // An icon registered on its own still overrides the chosen pack.
  await host.add({ id: "one", name: "One", activate: (ctx) => void ctx.icons.register({ id: "bold", svg: svg("X") }) });
  assert.equal(host.icon("bold"), svg("X"));
  await host.disable("one");
  assert.equal(host.icon("bold"), svg("T"));
  // Switching back.
  host.setIconPack("docxtor");
  assert.equal(host.icon("bold"), svg("b"));
  host.setIconPack("thin");
  // Turning the pack's extension off falls back to Docxtor's, and the choice is kept for when it is back on.
  await host.disable("thin");
  assert.equal(host.iconPack, "thin");
  assert.equal(host.activeIconPack, "docxtor");
  assert.equal(host.icon("bold"), svg("b"));
  assert.deepEqual(host.listIconPacks().map((p) => p.id), ["docxtor"]);
  await host.enable("thin");
  assert.equal(host.activeIconPack, "thin");
  assert.equal(host.icon("bold"), svg("T"));
  // A pack that was never added shows Docxtor's icons.
  host.setIconPack("nowhere");
  assert.equal(host.icon("bold"), svg("b"));
  sub.dispose();
  const before = changes;
  host.setIconPack("docxtor");
  assert.equal(changes, before, "a disposed listener is not called");
});

test("icon packs: packs with the same id add to each other, the latest icon wins, bad packs are refused", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const svg = (s: string) => `<svg viewBox="0 0 24 24">${s}</svg>`;
  await host.add({ id: "a", name: "A", activate: (ctx) => void ctx.iconPacks.register({ id: "docxtor", label: "Docxtor", icons: [{ id: "bold", svg: svg("1") }, { id: "italic", svg: svg("2") }] }) });
  await host.add({ id: "b", name: "B", activate: (ctx) => void ctx.iconPacks.register({ id: "docxtor", label: "Docxtor again", icons: [{ id: "bold", svg: svg("3") }] }) });
  assert.equal(host.icon("bold"), svg("3"));
  assert.equal(host.icon("italic"), svg("2"));
  assert.deepEqual(host.listIconPacks(), [{ id: "docxtor", label: "Docxtor" }], "one entry per id, with the first label");
  let n = 0;
  const bad = async (pack: unknown) => {
    const id = `bad${n++}`;
    await host.add({ id, name: "Bad", activate: (ctx) => void ctx.iconPacks.register(pack as never) });
    return host.list().find((x) => x.id === id)?.error;
  };
  assert.match((await bad({ id: "x", label: "X", icons: [{ id: "bold", svg: "nope" }] })) ?? "", /SVG/);
  assert.match((await bad({ id: "", label: "X", icons: [] })) ?? "", /needs an id/);
  assert.match((await bad({ id: "x", label: "X" })) ?? "", /needs an id/);
  // Nothing of a refused pack was kept.
  assert.deepEqual(host.listIconPacks().map((p) => p.id), ["docxtor"]);
});

test("citation styles and source providers: registered by extensions, one style per id, loaded on demand", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  let loaded = 0;
  await host.add({
    id: "styles",
    name: "Styles",
    activate(ctx) {
      ctx.citations.styles.register({ id: "apa", label: "APA 7th edition" });
      ctx.citations.styles.register({ id: "nature", label: "Nature", csl: () => (loaded++, "<style/>"), group: "Numbered" });
      ctx.citations.styles.register({ id: "inline", label: "Inline", csl: "<style class='note'/>" });
    },
  });
  assert.deepEqual(host.citationStyleList().map((s) => s.id), ["apa", "nature", "inline"]);
  // A function is called when the CSL is asked for, and not before.
  assert.equal(loaded, 0);
  assert.equal(await host.citationStyleCsl("nature"), "<style/>");
  assert.equal(loaded, 1);
  assert.equal(await host.citationStyleCsl("inline"), "<style class='note'/>");
  assert.equal(await host.citationStyleCsl("apa"), null, "a style the engine formats itself has no CSL");
  assert.equal(await host.citationStyleCsl("missing"), null);

  // A later extension can replace a style by id; turning it off brings the first back.
  await host.add({ id: "better", name: "Better", activate: (ctx) => void ctx.citations.styles.register({ id: "nature", label: "Nature (newer)", csl: "<style/>" }) });
  assert.deepEqual(host.citationStyleList().map((s) => s.label), ["APA 7th edition", "Inline", "Nature (newer)"]);
  await host.disable("better");
  assert.deepEqual(host.citationStyleList().map((s) => s.label), ["APA 7th edition", "Nature", "Inline"]);
  await host.disable("styles");
  assert.equal(host.citationStyleList().length, 0);

  await host.add({
    id: "sources",
    name: "Sources",
    activate(ctx) {
      ctx.citations.sources.register({ id: "doi", label: "From a DOI…", resolve: async () => [{ type: "book", title: "X", authors: [], tag: "" }] });
      ctx.citations.sources.register({ id: "library", label: "From my library…", search: async () => [] });
    },
  });
  assert.deepEqual(host.citationSources.list().map((p) => p.id), ["doi", "library"]);
  await host.disable("sources");
  assert.equal(host.citationSources.list().length, 0);

  // A provider that can neither search nor resolve, or a style without a label, is refused.
  await host.add({ id: "bad1", name: "Bad", activate: (ctx) => void ctx.citations.sources.register({ id: "x", label: "X" }) });
  assert.match(String(host.list().find((x) => x.id === "bad1")?.error), /needs search\(\) or resolve\(\)/);
  await host.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.citations.styles.register({ id: "x", label: "" }) });
  assert.match(String(host.list().find((x) => x.id === "bad2")?.error), /needs an id and a label/);
});

test("document themes and style sets come and go with their extension, and bad ones are refused", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({
    id: "acme",
    name: "Acme",
    activate(ctx) {
      ctx.documentThemes.register({ id: "acme-brand", label: "Acme brand", colors: { accent1: "C00000", accent2: "00B050" }, fonts: { major: "Georgia", minor: "Verdana" }, group: "Acme" });
      ctx.documentThemes.register({ id: "acme-colors", label: "Acme colours", colors: { accent1: "112233" } });
      ctx.styleSets.register({ id: "acme-set", label: "Acme set", description: "Headings in red", styles: '<w:style w:type="paragraph" w:styleId="Heading1"/>', spacing: { after: 120 } });
    },
  });
  assert.deepEqual(host.documentThemes.list().map((d) => [d.id, d.group]), [["acme-brand", "Acme"], ["acme-colors", undefined]]);
  assert.deepEqual(host.styleSets.list().map((s) => s.id), ["acme-set"]);
  await host.disable("acme");
  assert.equal(host.documentThemes.list().length + host.styleSets.list().length, 0);
  await host.enable("acme");
  assert.equal(host.documentThemes.list().length, 2);
  // Docxtor's own style sets have no XML: the engine has them by id.
  await host.add({ id: "own", name: "Own", activate: (ctx) => void ctx.styleSets.register({ id: "word-2010", label: "Word 2010" }) });
  assert.equal(host.styleSets.list().length, 2);
  const refused = async (id: string, register: (ctx: ExtensionContext) => void, message: RegExp) => {
    await host.add({ id, name: "Bad", activate: (ctx) => void register(ctx) });
    assert.match(String(host.list().find((x) => x.id === id)?.error), message);
  };
  await refused("bad1", (c) => c.documentThemes.register({ id: "", label: "X", colors: { accent1: "112233" } }), /needs an id and a label/);
  await refused("bad2", (c) => c.documentThemes.register({ id: "x", label: "X" }), /needs colours, fonts or both/);
  await refused("bad3", (c) => c.documentThemes.register({ id: "x", label: "X", colors: { accent1: "red" } }), /not a colour/);
  await refused("bad4", (c) => c.documentThemes.register({ id: "x", label: "X", fonts: { major: "Georgia", minor: " " } }), /heading font and a body font/);
  await refused("bad5", (c) => c.styleSets.register({ id: "x", label: "X", styles: "<p/>" }), /needs its w:style elements/);
  await refused("bad6", (c) => c.styleSets.register({ id: "x", label: "" }), /needs an id and a label/);
  assert.equal(host.documentThemes.list().length, 2);
});

test("picture sources come and go with their extension, and need an id, a label and pick()", async () => {
  const host = new ExtensionHost({ document: doc, ui, mac: true });
  assert.equal(host.pictureSources.list().length, 0);
  await host.add({
    id: "photos",
    name: "Photos",
    activate(ctx) {
      ctx.pictures.sources.register({ id: "file", label: "From your computer…", pick: () => null });
      ctx.pictures.sources.register({ id: "stock", label: "From a stock library…", note: "Sends your search words to the library.", pick: async () => ({ bytes: new Uint8Array([1]), name: "tree.png", type: "image/png" }) });
    },
  });
  assert.deepEqual(host.pictureSources.list().map((p) => p.id), ["file", "stock"]);
  const stock = host.pictureSources.list()[1];
  assert.deepEqual(await stock.pick({ ui, signal: new AbortController().signal }), { bytes: new Uint8Array([1]), name: "tree.png", type: "image/png" });
  await host.disable("photos");
  assert.equal(host.pictureSources.list().length, 0);
  await host.add({ id: "bad", name: "Bad", activate: (ctx) => void ctx.pictures.sources.register({ id: "x", label: "X" } as never) });
  assert.match(String(host.list().find((x) => x.id === "bad")?.error), /needs a pick\(\) function/);
  await host.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.pictures.sources.register({ id: "x", label: "", pick: () => null }) });
  assert.match(String(host.list().find((x) => x.id === "bad2")?.error), /needs a label/);
});

test("table designs come and go with their extension, and a bad one is refused", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const xml = '<w:style w:type="table" w:styleId="AcmeTable"><w:name w:val="Acme table"/></w:style>';
  await host.add({ id: "acme", name: "Acme", activate: (ctx) => void ctx.tableDesigns.register({ id: "AcmeTable", label: "Acme table", xml, group: "Acme" }) });
  assert.deepEqual(host.tableDesigns.list().map((d) => [d.id, d.label, d.group]), [["AcmeTable", "Acme table", "Acme"]]);
  await host.disable("acme");
  assert.equal(host.tableDesigns.list().length, 0);
  await host.enable("acme");
  assert.equal(host.tableDesigns.list().length, 1);
  await host.add({ id: "bad1", name: "Bad", activate: (ctx) => void ctx.tableDesigns.register({ id: "X", label: "X", xml: "" }) });
  assert.match(String(host.list().find((x) => x.id === "bad1")?.error), /needs its w:style as XML/);
  await host.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.tableDesigns.register({ id: "", label: "X", xml }) });
  assert.match(String(host.list().find((x) => x.id === "bad2")?.error), /needs an id and a label/);
  assert.equal(host.tableDesigns.list().length, 1);
});

test("chart palettes come and go with their extension, and a bad one is refused", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({ id: "brand", name: "Brand", activate: (ctx) => void ctx.chartPalettes.register({ id: "brand", label: "Brand colors", colors: ["FF4F00", "scheme:accent2:lumMod=60000:lumOff=40000"] }) });
  assert.deepEqual(host.chartPalettes.list().map((p) => [p.id, p.colors.length]), [["brand", 2]]);
  await host.disable("brand");
  assert.equal(host.chartPalettes.list().length, 0);
  await host.enable("brand");
  assert.equal(host.chartPalettes.list().length, 1);
  await host.add({ id: "bad1", name: "Bad", activate: (ctx) => void ctx.chartPalettes.register({ id: "x", label: "X", colors: [] }) });
  assert.match(String(host.list().find((x) => x.id === "bad1")?.error), /needs a list of colors/);
  await host.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.chartPalettes.register({ id: "x", label: "X", colors: ["red"] }) });
  assert.match(String(host.list().find((x) => x.id === "bad2")?.error), /neither RRGGBB nor scheme:name/);
  await host.add({ id: "bad3", name: "Bad", activate: (ctx) => void ctx.chartPalettes.register({ id: "", label: "X", colors: ["FF0000"] }) });
  assert.match(String(host.list().find((x) => x.id === "bad3")?.error), /needs an id and a label/);
  assert.equal(host.chartPalettes.list().length, 1);
});
test("content control kinds come and go with their extension, and a bad one is refused", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({ id: "acme", name: "Acme", activate: (ctx) => void ctx.contentControls.register({ kind: "acme-rating", label: "Rating", edit: () => {} }) });
  assert.deepEqual(host.contentControls.list().map((c) => c.kind), ["acme-rating"]);
  await host.disable("acme");
  assert.equal(host.contentControls.list().length, 0);
  await host.enable("acme");
  assert.equal(host.contentControls.list().length, 1);
  await host.add({ id: "bad1", name: "Bad", activate: (ctx) => void ctx.contentControls.register({ kind: "x" }) });
  assert.match(String(host.list().find((x) => x.id === "bad1")?.error), /needs render\(\) or edit\(\)/);
  await host.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.contentControls.register({ kind: "", edit: () => {} }) });
  assert.match(String(host.list().find((x) => x.id === "bad2")?.error), /needs a kind/);
  assert.equal(host.contentControls.list().length, 1);
});

test("a command with a caution asks first, or offers Undo after; dialogs ask by id", async () => {
  const asked: { message: string; title?: string; confirm?: string; danger?: boolean }[] = [];
  const notices: { message: string; action?: { label: string; run: () => void } }[] = [];
  let answer = false;
  const ui = {
    notify: (message: string, o?: { action?: { label: string; run: () => void } }) => notices.push({ message, action: o?.action }),
    confirm: async (message: string, o: { title?: string; confirm?: string; danger?: boolean } = {}) => {
      asked.push({ message, ...o });
      return answer;
    },
  } as unknown as UiApi;
  const host = new ExtensionHost({ document: doc, ui, mac: true });
  const ran: string[] = [];
  await host.add(
    defineExtension({
      id: "test.risky",
      name: "Risky",
      activate(ctx) {
        ctx.commands.register("doc.removeHeader", (who) => void ran.push(`header ${who}`), { title: "Remove header" });
        ctx.commands.register("doc.deleteTable", () => void ran.push("table"), { title: "Delete table" });
        ctx.commands.register("doc.deleteNothing", () => false, { title: "Delete nothing" });
        ctx.commands.register("edit.undo", () => void ran.push("undo"), { title: "Undo" });
        ctx.cautions.register({ id: "doc.removeHeader", kind: "ask", title: (who) => `Remove ${who}'s header?`, impact: "It goes from every page.", confirm: "Remove header" });
        ctx.cautions.register({ id: "doc.deleteTable", kind: "undo", title: "Table deleted." });
        ctx.cautions.register({ id: "doc.deleteNothing", kind: "undo", title: "Deleted." });
        ctx.cautions.register({ id: "dict.remove", kind: "ask", title: "Remove the dictionary?", impact: "Its words go.", confirm: "Remove", permanent: true });
      },
    }),
  );
  // Said no: nothing happens. Said yes: it runs. The question names the action and says what changes.
  assert.deepEqual(await host.execute("doc.removeHeader", "Ana"), { status: "declined" });
  assert.deepEqual(ran, []);
  assert.deepEqual(asked[0], { message: "It goes from every page.", title: "Remove Ana's header?", confirm: "Remove header", danger: true });
  answer = true;
  assert.equal((await host.execute("doc.removeHeader", "Ana")).status, "done");
  assert.deepEqual(ran, ["header Ana"]);
  // Undo kind: no question, a notice with Undo that runs edit.undo; none when the command did nothing.
  await host.execute("doc.deleteTable");
  assert.equal(asked.length, 2);
  assert.equal(notices.at(-1)!.message, "Table deleted.");
  notices.at(-1)!.action!.run();
  await new Promise((r) => setTimeout(r));
  assert.deepEqual(ran, ["header Ana", "table", "undo"]);
  const before = notices.length;
  await host.execute("doc.deleteNothing");
  assert.equal(notices.length, before);
  // A dialog's button asks by id; what can't be undone says so; an unknown id has nothing to ask.
  answer = false;
  assert.equal(await host.ask("dict.remove"), false);
  assert.equal(asked.at(-1)!.message, "Its words go. This can't be undone.");
  assert.equal(await host.ask("no.such.thing"), true);
});

test("execute is the only door: a command that is not available is not asked about and not run", async () => {
  const asked: string[] = [];
  const ui = { notify() {}, confirm: async (m: string) => (asked.push(m), true) } as unknown as UiApi;
  const host = new ExtensionHost({ document: doc, ui, mac: true });
  let on = false;
  const ran: string[] = [];
  await host.add(
    defineExtension({
      id: "test.door",
      name: "Door",
      activate(ctx) {
        ctx.commands.register("door.remove", () => void ran.push("remove"), { title: "Remove", when: () => on });
        ctx.cautions.register({ id: "door.remove", kind: "ask", title: "Remove?", impact: "It goes." });
      },
    }),
  );
  assert.deepEqual(await host.execute("door.remove"), { status: "unavailable" });
  assert.deepEqual(asked, []);
  assert.deepEqual(ran, []);
  on = true;
  assert.equal((await host.execute("door.remove")).status, "done");
  assert.deepEqual(ran, ["remove"]);
});

test("the Undo offer appears only when the document changed, and only works while that step is the last", async () => {
  const notices: { message: string; action?: { label: string; run: () => void } }[] = [];
  const ui = { notify: (message: string, o?: { action?: { label: string; run: () => void } }) => notices.push({ message, action: o?.action }) } as unknown as UiApi;
  const engine = { revision: 1 };
  const host = new ExtensionHost({ document: { engine } as unknown as DocumentApi, ui, mac: true });
  const ran: string[] = [];
  await host.add(
    defineExtension({
      id: "test.undo",
      name: "Undo",
      activate(ctx) {
        ctx.commands.register("t.delete", () => void engine.revision++, { title: "Delete" });
        ctx.commands.register("t.refused", () => undefined, { title: "Refused" });
        ctx.commands.register("edit.undo", () => void ran.push("undo"), { title: "Undo" });
        ctx.cautions.register({ id: "t.delete", kind: "undo", title: "Deleted." });
        ctx.cautions.register({ id: "t.refused", kind: "undo", title: "Refused." });
      },
    }),
  );
  await host.execute("t.refused");
  assert.equal(notices.length, 0, "nothing changed, so no offer");
  await host.execute("t.delete");
  assert.equal(notices.length, 1);
  engine.revision++; // the person types
  notices[0].action!.run();
  await new Promise((r) => setTimeout(r));
  assert.deepEqual(ran, [], "Undo would take back the typing, so it does nothing");
  await host.execute("t.delete");
  notices[1].action!.run();
  await new Promise((r) => setTimeout(r));
  assert.deepEqual(ran, ["undo"]);
});
