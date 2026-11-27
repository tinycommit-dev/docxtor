// Field types: reading and writing instructions, the registry (latest wins, order by group) and evaluators.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { buildInstruction, defineExtension, ExtensionHost, parseInstruction, readValues, type DocumentApi, type FieldCall, type FieldType, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const newHost = (errors: unknown[] = []) => new ExtensionHost({ document: doc, ui, mac: true, onError: (_id, err) => errors.push(err) });

const date: FieldType = {
  name: "DATE",
  category: "Date and time",
  options: [
    { id: "picture", label: "Format", kind: "text", write: "date" },
    { id: "upper", label: "Capitals", kind: "toggle", write: "format", default: false },
  ],
};

test("an instruction is taken apart", () => {
  const p = parseInstruction(' DATE \\@ "dddd, MMMM d" \\* Upper \\* MERGEFORMAT ');
  assert.equal(p.code, "DATE");
  assert.equal(p.datePicture, "dddd, MMMM d");
  assert.deepEqual(p.formats, ["Upper", "MERGEFORMAT"]);
  assert.deepEqual(p.args, []);

  const r = parseInstruction(" REF _Ref123 \\h \\r ");
  assert.equal(r.code, "REF");
  assert.deepEqual(r.args, ["_Ref123"]);
  assert.deepEqual(r.switches, { h: true, r: true });

  const h = parseInstruction('hyperlink "https://example.com/a b" \\l "top" \\o "A tip"');
  assert.equal(h.code, "HYPERLINK");
  assert.deepEqual(h.args, ["https://example.com/a b"]);
  assert.deepEqual(h.switches, { l: "top", o: "A tip" });

  const f = parseInstruction(' =SUM(ABOVE) \\# "0.00" ');
  assert.equal(f.code, "=");
  assert.deepEqual(f.args, ["SUM(ABOVE)"]);
  assert.equal(f.numberPicture, "0.00");

  const s = parseInstruction(" SEQ Figure \\r 3 \\* ARABIC ");
  assert.deepEqual(s.switches, { r: "3" });
  assert.deepEqual(s.args, ["Figure"]);
});

test("a form is written as an instruction", () => {
  assert.equal(buildInstruction(date, { picture: "MMMM d, yyyy" }), 'DATE \\@ "MMMM d, yyyy"');
  const ref: FieldType = {
    name: "REF",
    category: "Links and references",
    options: [
      { id: "bookmark", label: "Bookmark", kind: "text", write: "arg" },
      { id: "link", label: "Link", kind: "toggle", write: "switch", switch: "h" },
      { id: "number", label: "Number", kind: "choice", write: "switch", switch: "r" },
      { id: "case", label: "Case", kind: "choice", write: "format", choices: [{ value: "Upper", label: "Upper" }, { value: "Lower", label: "Lower" }] },
      { id: "level", label: "Level", kind: "text", write: "switch-value", switch: "d" },
    ],
  };
  assert.equal(buildInstruction(ref, { bookmark: "My mark", link: true, case: "Upper", level: "-" }), 'REF "My mark" \\h \\* Upper \\d -');
  assert.equal(buildInstruction(ref, { bookmark: "A", link: false }), "REF A");
  // Pictures are always in quotes, as Word writes them.
  assert.equal(buildInstruction(date, { picture: "yyyy" }), 'DATE \\@ "yyyy"');
  // Values it wrote can be read back.
  const back = readValues(ref, 'REF "My mark" \\h \\* Lower');
  assert.deepEqual([back.bookmark, back.link, back.case, back.number], ["My mark", true, "Lower", false]);
});

test("the latest field type with a name wins, and the list follows the dialog's groups", async () => {
  const host = newHost();
  await host.add(
    defineExtension({
      id: "test.fields",
      name: "Fields",
      activate(ctx) {
        ctx.fields.register({ name: "AUTHOR", category: "Document information" });
        ctx.fields.register(date);
        ctx.fields.register({ name: "ODDONE", category: "Mine" });
        ctx.fields.register({ name: "PAGE", category: "Numbering" });
        ctx.fields.register({ name: "Date", category: "Date and time", title: "Newer date" });
      },
    }),
  );
  assert.equal(host.fieldType("date")?.title, "Newer date");
  assert.deepEqual(host.fieldTypeList().map((f) => f.name), ["Date", "AUTHOR", "PAGE", "ODDONE"]);
});

test("a field type needs a name and a group", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.bad",
      name: "Bad",
      activate(ctx) {
        assert.throws(() => ctx.fields.register({ name: "", category: "x" }));
        assert.throws(() => ctx.fields.register({ name: "has space", category: "x" }));
        assert.throws(() => ctx.fields.register({ name: "OK", category: "" }));
        assert.throws(() => ctx.fields.register({ name: "OK", category: "x", evaluate: 5 as unknown as FieldType["evaluate"] }));
      },
    }),
  );
  assert.equal(host.fieldTypes.list().length, 0);
});

test("an extension's evaluator works out a field; failures are reported and leave the result alone", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.eval",
      name: "Eval",
      activate(ctx) {
        ctx.fields.register({ name: "SHOUT", category: "Document automation", evaluate: (c) => c.args.join(" ").toUpperCase() });
        ctx.fields.register({ name: "LATER", category: "Document automation", evaluate: async () => "done" });
        ctx.fields.register({ name: "BROKEN", category: "Document automation", evaluate: () => { throw new Error("no"); } });
        ctx.fields.register({ name: "SILENT", category: "Document automation", evaluate: () => null });
        ctx.fields.register({ name: "PLAIN", category: "Document automation" });
      },
    }),
  );
  const call = (code: string, args: string[] = []): FieldCall => ({ instr: ` ${code} `, code, args, switches: {}, formats: [], engine: null as never, now: new Date(0) });
  assert.equal(await host.evaluateField(call("SHOUT", ["a", "b"])), "A B");
  assert.equal(await host.evaluateField(call("LATER")), "done");
  assert.equal(await host.evaluateField(call("BROKEN")), null);
  assert.equal(errors.length, 1);
  assert.equal(await host.evaluateField(call("SILENT")), null);
  assert.equal(await host.evaluateField(call("PLAIN")), null);
  assert.equal(await host.evaluateField(call("NOPE")), null);
  assert.ok(host.evaluatesField("shout") && !host.evaluatesField("PLAIN"));
});

test("turning the extension off takes its field types away", async () => {
  const host = newHost();
  await host.add(defineExtension({ id: "test.off", name: "Off", activate: (ctx) => void ctx.fields.register({ name: "GONE", category: "Numbering" }) }));
  assert.ok(host.fieldType("GONE"));
  await host.disable("test.off");
  assert.equal(host.fieldType("GONE"), undefined);
});
