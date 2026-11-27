// A colour in a .docx is a string the file chooses (`red;background:url(http://...)` is as good a `w:color` as `FF0000`). The engine hands out
// only colours (crates/docxtor-core/tests/all/hostile_colours.rs), and the editor checks again, in one place: `hex()` and `hexOr()` in
// src/editor/color.ts, which give a colour for six hex digits and nothing for anything else.
//
// This test is the guard on the editor's side. Every place that writes `#${...}` (or `"#" + ...`) into a string has to go through that module,
// or be one of the few uses below that are not a colour going into CSS or SVG: an id the drawing code made itself, a property set through the
// CSSOM (the browser drops a value that is not a colour), or text and an input's value that a person reads. It is a guard, not a parser: a line
// is looked at, not the code around it.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const SOURCES = ["packages/editor/src", "web/src"];
/** The validator itself: the one file allowed to write `#${...}` for a colour. */
const VALIDATOR = "packages/editor/src/editor/color.ts";

function* files(dir: string): Generator<string> {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, e.name);
    if (e.isDirectory()) yield* files(path);
    else if (/\.ts$/.test(e.name) && !/\.test\.ts$/.test(e.name)) yield path;
  }
}

/** What writes a `#` and a value into a string: a template (`#${x}`), or a `"#"` that something is added to. */
const WRITES_HASH = /#\$\{|["']#["']\s*\+/;

/** Lines that are the same use everywhere, with the reason. */
const EVERYWHERE: { name: string; line: RegExp; reason: string }[] = [
  { name: "reference by id", line: /url\(#\$\{|\bhref["']?\s*[:,]\s*`#\$\{/, reason: "a reference to a gradient, clip, mask, marker or filter the drawing code made itself: its ids come from a counter (nextId, ++uid), never from the file" },
  { name: "lookup by id", line: /querySelector(?:<[^>]*>)?\(`#\$\{/, reason: "an element looked up by an id (CSS.escape'd, or one the page made), not a colour" },
  { name: "CSSOM property", line: /\.style\.[A-Za-z]+\s*=\s*(?:[^`]*[?:]\s*)?`#\$\{/, reason: "one property set through the CSSOM: a value that is not a colour is dropped by the browser, there is no string for it to end" },
];

/** The rest, one file at a time: the text a swatch is called, the value of an input, a link to a bookmark, a PDF name. */
const IN_FILE: { file: string; has: string; reason: string }[] = [
  { file: "packages/editor/src/ui/bookmarks.ts", has: "const linkTo = (name: string) => `#${name}`", reason: "a link to a bookmark by its name, not a colour" },
  { file: "packages/editor/src/ui/toolbar.ts", has: "this.ed.setLink(`#${target.name}`", reason: "a link to a bookmark by its name, not a colour" },
  { file: "packages/editor/src/pdf/writer.ts", has: '"#()<>[]{}/%".includes(c)', reason: "a PDF name's escape for one byte (#2f), not a colour" },
  { file: "packages/editor/src/ui/shape-tool.ts", has: "title: `#${c}`", reason: "the name of a swatch from the tool's own palette (a title and an aria-label are set as attributes, never parsed); its colour goes through hexOr" },
  { file: "packages/editor/src/ui/shape-tool.ts", has: 'value: `#${current ?? "4472C4"}`', reason: "the value of a colour input (a property: the control drops what is not a colour)" },
  { file: "packages/editor/src/ui/table-tool.ts", has: "title: `#${f}`", reason: "the name of a swatch from the tool's own palette; its colour goes through hexOr" },
  { file: "packages/editor/src/ui/ink-tool.ts", has: "title: `#${hex}`", reason: "the name of a swatch from the pen's palette; its colour goes through hexOr" },
  { file: "packages/editor/src/ui/page-fill.ts", has: "(c) => `#${c}`", reason: "the text a swatch is called" },
  { file: "packages/editor/src/ui/page-fill.ts", has: "value: `#${value}`", reason: "the value of a text input (a property)" },
  { file: "packages/editor/src/ui/page-fill.ts", has: "hex.value = `#${", reason: "the value of a text input (a property)" },
  { file: "packages/editor/src/ui/page-fill.ts", has: "value: solid ? `#${solid}`", reason: "the value of a text input (a property)" },
  { file: "packages/editor/src/ui/find-format.ts", has: "`#${f.color}`", reason: "the text a search is described by, shown as text" },
  { file: "packages/editor/src/ui/find-format.ts", has: "`#${run.color}`", reason: "the text a search is described by, shown as text" },
  { file: "packages/editor/src/ui/find-format.ts", has: "`#${colorVal}`", reason: "the value of a text input (a property)" },
  { file: "packages/editor/src/ui/swatches.ts", has: "(c) => `#${c}`", reason: "the text a swatch is called" },
  { file: "packages/editor/src/ui/swatches.ts", has: "`#${hex.toUpperCase()}`", reason: "the text a recent colour is called" },
  { file: "packages/editor/src/ui/swatches.ts", has: "text.textContent = ", reason: "the text a colour is called, set as text" },
  { file: "packages/editor/src/ui/swatches.ts", has: "hex.value = `#${color}`", reason: "the value of a text input (a property)" },
  { file: "packages/editor/src/ui/swatches.ts", has: "${t(name)} #${hex}", reason: "the text a swatch is called" },
  { file: "packages/editor/src/ui/chart-color.ts", has: "label: `#${hex}`", reason: "the name of a swatch from the chart's own palette (the swatch itself is drawn through hexOr)" },
  { file: "packages/editor/src/ui/control-properties.ts", has: "value: `#${", reason: "the value of a colour input (a property: the control drops what is not a colour)" },
  { file: "packages/editor/src/ui/design.ts", has: "value: `#${", reason: "the value of a colour input (a property: the control drops what is not a colour)" },
  { file: "packages/editor/src/ui/design.ts", has: "picker.value = `#${", reason: "the value of a colour input (a property)" },
  { file: "packages/editor/src/ui/watermark.ts", has: "(c) => `#${c}`", reason: "the text a swatch is called" },
  { file: "packages/editor/src/ui/page-borders.ts", has: "hex.value =", reason: "the value of a text input (a property)" },
  { file: "packages/editor/src/ui/style-preview.ts", has: "return color ? `#${color}` : t(", reason: "the text a colour is called, shown as text" },
  { file: "web/src/ui/citations/form.ts", has: "querySelector", reason: "an element looked up by an id" },
];

function findings(): { where: string; text: string }[] {
  const out: { where: string; text: string }[] = [];
  for (const dir of SOURCES) {
    for (const path of files(join(root, dir))) {
      const rel = relative(root, path);
      if (rel === VALIDATOR) continue;
      readFileSync(path, "utf8").split("\n").forEach((text, i) => {
        if (WRITES_HASH.test(text)) out.push({ where: `${rel}:${i + 1}`, text: text.trim() });
      });
    }
  }
  return out;
}

const inFile = (where: string, text: string) => IN_FILE.some((a) => where.startsWith(`${a.file}:`) && text.includes(a.has));

test("every colour written into CSS or SVG goes through hex() or hexOr()", () => {
  const bad = findings().filter((f) => !EVERYWHERE.some((e) => e.line.test(f.text)) && !inFile(f.where, f.text));
  assert.deepEqual(
    bad.map((f) => `${f.where}: ${f.text.slice(0, 160)}`),
    [],
    "these write `#${...}` into a string without hex() or hexOr() (src/editor/color.ts): a colour from a document must not reach a style string, cssText or an SVG attribute any other way. If it is not a colour from a file (an id, a name, an input's value), add it to the allow-list in this test with the reason.",
  );
});

test("each entry of the allow-list is still used", () => {
  const all = findings();
  for (const e of EVERYWHERE) assert.ok(all.some((f) => e.line.test(f.text)), `the rule "${e.name}" matches no line: take it out`);
  for (const a of IN_FILE) {
    assert.ok(all.some((f) => f.where.startsWith(`${a.file}:`) && f.text.includes(a.has)), `${a.file}: "${a.has}" matches no line: take it out`);
    assert.ok(a.reason.length > 15, `${a.file}: say why`);
  }
});

test("the guard finds what it is for", () => {
  // A line like each of the ways a colour got into a string before, and what the guard says about it.
  const wrong = [
    "el.setAttribute(\"style\", `color:#${r.color}`);",
    "const st = `background:#${c.fill}`;",
    'out.stroke = `#${line.color}`;',
    "html += `<rect fill=\"#${c.fill}\"/>`;",
    "const css = \"#\" + color;",
  ];
  for (const line of wrong) {
    assert.ok(WRITES_HASH.test(line), `${line} is seen`);
    assert.ok(!EVERYWHERE.some((e) => e.line.test(line)), `${line} is not one of the allowed uses`);
  }
  // and what it lets by
  for (const line of ["return { paint: `url(#${id})` };", 'use.setAttribute("href", `#${group.id}`);', "chip.style.background = `#${c}`;", "form.querySelector<HTMLInputElement>(`#${id}-title`)?.focus();"]) {
    assert.ok(EVERYWHERE.some((e) => e.line.test(line)), `${line} is allowed`);
  }
});

test("the validator gives a colour for six hex digits, and nothing for what a file can send", async () => {
  const { hex, hexOr } = await import("../src/editor/color.ts");
  assert.equal(hex("FF4F00"), "#FF4F00");
  assert.equal(hex("ff4f00"), "#ff4f00");
  for (const bad of ["auto", "", undefined, null, "red", "#FF4F00", "FF4F0", "FF4F000", "a;b:c)", "FF0000;background:url(http://evil.test/x)", 'FF0000" onload="x', "FF0000\n", " FF0000", "ＦＦ００００", "red;ba"]) {
    assert.equal(hex(bad as string), undefined, JSON.stringify(bad));
  }
  assert.equal(hexOr("zzzzzz"), "#000000");
  assert.equal(hexOr(undefined, "#fff"), "#fff");
});

test("a highlight is one of Word's names, not a key every object has", async () => {
  const { HIGHLIGHT, runBackground } = await import("../src/editor/charfmt.ts");
  assert.equal(HIGHLIGHT.get("yellow"), "#ffff00");
  for (const bad of ["constructor", "__proto__", "toString", "hasOwnProperty", "red;background:url(http://evil.test/x)"]) assert.equal(HIGHLIGHT.get(bad), undefined, bad);
  assert.equal(runBackground({ highlight: "constructor" } as never), undefined);
});
