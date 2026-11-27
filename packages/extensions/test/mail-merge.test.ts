// Mail merge contribution points: where recipients come from and which label sheets the Labels dialog offers.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ExtensionHost, type DocumentApi, type LabelProduct, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;

const product = (id: string, over: Partial<LabelProduct> = {}): LabelProduct => ({
  id,
  vendor: "Acme",
  name: `${id} address labels`,
  pageWidth: 8.5,
  pageHeight: 11,
  top: 0.5,
  left: 0.19,
  labelWidth: 2.63,
  labelHeight: 1,
  pitchH: 2.75,
  pitchV: 1,
  across: 3,
  down: 10,
  ...over,
});

test("data sources come and go with their extension and hand over a table", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({
    id: "acme.sheets",
    name: "Acme sheets",
    activate(ctx) {
      ctx.mailMerge.dataSources.register({
        id: "acme.sheet",
        label: "From an Acme sheet…",
        note: "Asks acme.example for the sheet you name.",
        pick: async ({ ui: u }) => {
          u.notify("picked");
          return { columns: ["Name", "City"], rows: [["Ada", "London"]], name: "Members" };
        },
      });
      ctx.mailMerge.dataSources.register({ id: "acme.none", label: "From nothing…", pick: () => null });
    },
  });
  assert.deepEqual(host.mailMergeSources.list().map((s) => s.id), ["acme.sheet", "acme.none"]);
  const [sheet, none] = host.mailMergeSources.list();
  assert.deepEqual(await sheet.pick({ ui, signal: new AbortController().signal }), { columns: ["Name", "City"], rows: [["Ada", "London"]], name: "Members" });
  // A source the person gave up on resolves with null.
  assert.equal(await none.pick({ ui, signal: new AbortController().signal }), null);
  await host.disable("acme.sheets");
  assert.equal(host.mailMergeSources.list().length, 0);
  await host.enable("acme.sheets");
  assert.equal(host.mailMergeSources.list().length, 2);
});

test("a data source needs an id, a label and a pick function", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({ id: "bad1", name: "Bad", activate: (ctx) => void ctx.mailMerge.dataSources.register({ id: "x", label: "X" } as never) });
  assert.match(String(host.list().find((x) => x.id === "bad1")?.error), /needs a pick\(\) function/);
  await host.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.mailMerge.dataSources.register({ id: "x", label: "", pick: () => null }) });
  assert.match(String(host.list().find((x) => x.id === "bad2")?.error), /needs a label/);
  await host.add({ id: "bad3", name: "Bad", activate: (ctx) => void ctx.mailMerge.dataSources.register({ id: "", label: "X", pick: () => null }) });
  assert.match(String(host.list().find((x) => x.id === "bad3")?.error), /needs an id/);
  assert.equal(host.mailMergeSources.list().length, 0);
});

test("label products are grouped by vendor, the latest of an id wins, and a bad sheet is refused", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  await host.add({
    id: "acme.labels",
    name: "Acme labels",
    activate(ctx) {
      ctx.mailMerge.labelProducts.register(product("a1"));
      ctx.mailMerge.labelProducts.register(product("b1", { vendor: "Beta", unit: "mm", pageWidth: 210, pageHeight: 297, top: 15, left: 7, labelWidth: 63.5, labelHeight: 38.1, pitchH: 66, pitchV: 38.1, across: 3, down: 7 }));
      ctx.mailMerge.labelProducts.register(product("a2"));
    },
  });
  await host.add({ id: "acme.labels2", name: "Acme labels again", activate: (ctx) => void ctx.mailMerge.labelProducts.register(product("a1", { name: "a1 renamed" })) });
  const groups = host.labelProductGroups();
  assert.deepEqual(groups.map((g) => [g.vendor, g.products.map((p) => p.name)]), [
    ["Acme", ["a1 renamed", "a2 address labels"]],
    ["Beta", ["b1 address labels"]],
  ]);
  const bad = async (id: string, over: Partial<LabelProduct>, message: RegExp) => {
    await host.add({ id, name: "Bad", activate: (ctx) => void ctx.mailMerge.labelProducts.register(product("x", over)) });
    assert.match(String(host.list().find((x) => x.id === id)?.error), message);
  };
  await bad("bad1", { pitchH: 1 }, /pitch can't be smaller/);
  await bad("bad2", { across: 0 }, /whole number of labels/);
  await bad("bad3", { labelWidth: 0 }, /greater than zero/);
  await bad("bad4", { vendor: "" }, /needs a vendor and a name/);
  assert.equal(host.labelProducts.list().length, 4);
});

test("mail senders come and go with their extension and take the messages one by one", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const seen: string[] = [];
  await host.add({
    id: "acme.mail",
    name: "Acme mail",
    activate(ctx) {
      ctx.mailMerge.mailSenders.register({
        id: "acme.smtp",
        label: "Send with Acme mail",
        note: "Sends each message through mail.acme.example.",
        formats: ["html", "text"],
        warnAbove: 100,
        limitNote: "Acme allows 100 messages a day.",
        summary: (n) => `${n} sent through Acme`,
        async send(messages, c) {
          for (const [i, m] of messages.entries()) {
            seen.push(`${m.to}|${m.subject}|${c.name}`);
            c.progress(i, m.to.startsWith("bad") ? { ok: false, error: "Refused" } : { ok: true });
          }
        },
      });
    },
  });
  const [sender] = host.mailSenders.list();
  assert.equal(sender.id, "acme.smtp");
  const outcomes: unknown[] = [];
  await sender.send(
    [
      { to: "ada@example.com", subject: "Hi", text: "Dear Ada", record: 0 },
      { to: "bad@example.com", subject: "Hi", text: "Dear Bad", record: 1 },
    ],
    { ui, name: "Letter", signal: new AbortController().signal, progress: (i, o) => outcomes.push([i, o]) },
  );
  assert.deepEqual(seen, ["ada@example.com|Hi|Letter", "bad@example.com|Hi|Letter"]);
  assert.deepEqual(outcomes, [[0, { ok: true }], [1, { ok: false, error: "Refused" }]]);
  assert.equal(sender.summary?.(2), "2 sent through Acme");
  await host.disable("acme.mail");
  assert.equal(host.mailSenders.list().length, 0);
  await host.enable("acme.mail");
  assert.equal(host.mailSenders.list().length, 1);
});

test("a mail sender needs an id, a label, a send function and at least one format", async () => {
  const host = new ExtensionHost({ document: doc, ui });
  const send = () => undefined;
  await host.add({ id: "bad1", name: "Bad", activate: (ctx) => void ctx.mailMerge.mailSenders.register({ id: "x", label: "X" } as never) });
  assert.match(String(host.list().find((x) => x.id === "bad1")?.error), /needs a send\(\) function/);
  await host.add({ id: "bad2", name: "Bad", activate: (ctx) => void ctx.mailMerge.mailSenders.register({ id: "x", label: "", send }) });
  assert.match(String(host.list().find((x) => x.id === "bad2")?.error), /needs a label/);
  await host.add({ id: "bad3", name: "Bad", activate: (ctx) => void ctx.mailMerge.mailSenders.register({ id: "", label: "X", send }) });
  assert.match(String(host.list().find((x) => x.id === "bad3")?.error), /needs an id/);
  await host.add({ id: "bad4", name: "Bad", activate: (ctx) => void ctx.mailMerge.mailSenders.register({ id: "x", label: "X", formats: [], send }) });
  assert.match(String(host.list().find((x) => x.id === "bad4")?.error), /lists no formats/);
  assert.equal(host.mailSenders.list().length, 0);
});
