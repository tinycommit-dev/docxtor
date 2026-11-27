// The contribution points behind smart chips: the @ menu, people, files, drop-downs, building blocks and link previews.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { atMenuMatches, chipKind, choiceValue, defineExtension, ExtensionHost, parseChoice, type DocumentApi, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const newHost = (errors: unknown[] = []) => new ExtensionHost({ document: doc, ui, mac: true, onError: (_id, err) => errors.push(err) });

test("the @ menu merges providers by order and heading, and skips one that fails", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  const ran: string[] = [];
  await host.add(
    defineExtension({
      id: "test.dates",
      name: "Dates",
      activate: (ctx) =>
        void ctx.atMenu.register({
          id: "dates",
          label: "Dates",
          order: 20,
          items: (q) => [{ id: "today", label: "Today", run: () => void ran.push("today") }].filter((i) => atMenuMatches(q, i.label)),
        }),
    }),
  );
  await host.add(
    defineExtension({
      id: "test.people",
      name: "People",
      activate: (ctx) =>
        void ctx.atMenu.register({
          id: "people",
          label: "People",
          order: 10,
          items: () => [
            { id: "ada", label: "Ada", run: () => void ran.push("ada") },
            { id: "ghost", label: "Ghost", section: "Dates", run: () => void ran.push("ghost") },
          ],
        }),
    }),
  );
  await host.add(
    defineExtension({
      id: "test.broken",
      name: "Broken",
      activate: (ctx) =>
        void ctx.atMenu.register({
          id: "broken",
          label: "Broken",
          items: () => {
            throw new Error("no network");
          },
        }),
    }),
  );
  const sections = await host.atMenuSections("");
  assert.deepEqual(
    sections.map((s) => [s.title, s.items.map((i) => i.label)]),
    [
      ["People", ["Ada"]],
      ["Dates", ["Ghost", "Today"]],
    ],
  );
  assert.equal(errors.length, 1);
  // Ids are made unique across providers.
  assert.equal(sections[0].items[0].id, "people:ada");
  await sections[1].items[1].run();
  assert.deepEqual(ran, ["today"]);
  // A query narrows what the providers return.
  assert.deepEqual((await host.atMenuSections("tod")).map((s) => s.title), ["People", "Dates"]);
  // Turning an extension off takes its lines away.
  await host.disable("test.people");
  assert.deepEqual((await host.atMenuSections("")).map((s) => s.title), ["Dates"]);
});

test("a menu provider needs an id, a label and items()", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(defineExtension({ id: "test.bad", name: "Bad", activate: (ctx) => void ctx.atMenu.register({ id: "x", label: "X" } as never) }));
  assert.equal(host.atMenuProviders.all().length, 0);
  assert.equal(errors.length, 1);
});

test("atMenuMatches looks at the start of words, ignores case and accents, and wants every word", () => {
  assert.ok(atMenuMatches("", "Anything"));
  assert.ok(atMenuMatches("tod", "Today"));
  assert.ok(atMenuMatches("meet no", "Meeting notes"));
  assert.ok(!atMenuMatches("day", "Today"));
  assert.ok(atMenuMatches("jose", "José García"));
  assert.ok(atMenuMatches("garc", "José García"));
  assert.ok(atMenuMatches("status", "Dropdown", ["Status", "Project status"]));
  assert.ok(atMenuMatches("ada@", "Ada Lovelace <ada@example.com>"));
  assert.ok(!atMenuMatches("tod x", "Today"));
});

test("people from several providers are merged, each person once, and a failing provider is skipped", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  const kept: string[] = [];
  await host.add(
    defineExtension({
      id: "test.local",
      name: "Local",
      activate: (ctx) =>
        void ctx.people.register({
          id: "local",
          label: "On this device",
          search: (q) => [{ name: "Ada Lovelace", email: "ada@example.com" }, { name: "Grace Hopper" }].filter((p) => atMenuMatches(q, p.name)),
          remember: (p) => void kept.push(p.name),
        }),
    }),
  );
  await host.add(
    defineExtension({
      id: "test.server",
      name: "Server",
      activate: (ctx) => {
        ctx.people.register({ id: "server", label: "Server", search: async (q) => [{ name: "Ada L.", email: "ADA@example.com", avatar: "https://example.com/a.png" }, { name: "Grace Hopper" }, { name: "Alan Turing" }].filter((p) => atMenuMatches(q, p.name)) });
        ctx.people.register({
          id: "down",
          label: "Down",
          search: () => {
            throw new Error("offline");
          },
        });
      },
    }),
  );
  const all = await host.findPeople("");
  // Same email in another case, and the same name without an email, are the same person.
  assert.deepEqual(all.map((p) => p.name), ["Ada Lovelace", "Grace Hopper", "Alan Turing"]);
  assert.equal(errors.length, 1);
  assert.deepEqual((await host.findPeople("alan")).map((p) => p.name), ["Alan Turing"]);
  await host.rememberPerson({ name: "Linus", email: "l@example.com" });
  assert.deepEqual(kept, ["Linus"]);
});

test("files are merged by address", async () => {
  const host = newHost();
  await host.add(
    defineExtension({
      id: "test.files",
      name: "Files",
      activate: (ctx) => {
        ctx.files.register({ id: "a", label: "A", search: () => [{ title: "Plan", url: "https://x/a" }] });
        ctx.files.register({ id: "b", label: "B", search: () => [{ title: "Plan again", url: "https://x/a" }, { title: "Budget", url: "https://x/b" }] });
      },
    }),
  );
  assert.deepEqual((await host.findFiles("")).map((f) => f.title), ["Plan", "Budget"]);
});

test("drop-down presets and building blocks are listed, and checked when registered", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.blocks",
      name: "Blocks",
      activate: (ctx) => {
        ctx.dropdowns.register({ id: "priority", label: "Priority", options: [{ label: "Low", color: "gray" }, { label: "High", color: "red" }] });
        ctx.buildingBlocks.register({ id: "standup", label: "Standup", markdown: "## Standup\n\n{{date}}" });
      },
    }),
  );
  await host.add(defineExtension({ id: "test.empty", name: "Empty", activate: (ctx) => void ctx.dropdowns.register({ id: "empty", label: "Empty", options: [] }) }));
  await host.add(defineExtension({ id: "test.bad", name: "Bad", activate: (ctx) => void ctx.buildingBlocks.register({ id: "bad", label: "Bad" } as never) }));
  assert.deepEqual(host.dropdownPresets.list().map((d) => d.id), ["priority"]);
  assert.deepEqual(host.docBuildingBlocks.list().map((b) => b.id), ["standup"]);
  assert.equal(errors.length, 2);
});

test("a link preview comes from the latest provider that knows the page; one that fails or does not match is passed over", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(defineExtension({ id: "test.base", name: "Base", activate: (ctx) => void ctx.linkPreviews.register({ id: "base", label: "Base", preview: () => ({ title: "Base title" }) }) }));
  await host.add(
    defineExtension({
      id: "test.docs",
      name: "Docs",
      activate: (ctx) => void ctx.linkPreviews.register({ id: "docs", label: "Docs", matches: (u) => u.startsWith("https://docs.example/"), preview: () => ({ title: "A doc", site: "Docs" }) }),
    }),
  );
  await host.add(
    defineExtension({
      id: "test.broken",
      name: "Broken",
      activate: (ctx) =>
        void ctx.linkPreviews.register({
          id: "broken",
          label: "Broken",
          preview: () => {
            throw new Error("blocked");
          },
        }),
    }),
  );
  assert.equal((await host.linkPreview("https://docs.example/x"))?.title, "A doc");
  assert.equal((await host.linkPreview("https://other.example/"))?.title, "Base title");
  assert.equal(errors.length, 2);
  await host.disable("test.base");
  assert.equal(await host.linkPreview("https://other.example/"), null);
});

test("a chip's kind comes from its tag, and a drop-down choice keeps its colour in the value", () => {
  assert.equal(chipKind("docxtor:chip:person"), "person");
  assert.equal(chipKind("docxtor:chip:"), null);
  assert.equal(chipKind("other"), null);
  assert.equal(chipKind(undefined), null);
  assert.equal(choiceValue({ label: "Done", color: "green" }), "green:Done");
  assert.equal(choiceValue({ label: "Plain" }), "gray:Plain");
  assert.deepEqual(parseChoice("green:Done"), { color: "green", label: "Done" });
  assert.deepEqual(parseChoice("Step 1: plan"), { color: "gray", label: "Step 1: plan" });
  assert.deepEqual(parseChoice("red:Blocked: waiting"), { color: "red", label: "Blocked: waiting" });
});
