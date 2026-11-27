// @docxtor/core runs outside the browser: open, edit, save, check fidelity, use the AI tools.
import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { EncryptedError, isEncrypted, OpenError, WrongPasswordError, Engine, fidelityReport, loadEngine, packageAsDocument, packageAsTemplate, packageKind, parseBibtex, parseCslJson, toolDefinitions } from "../src/index.ts";
import type { InsertChartOp, SetChartOp } from "../src/chart-types.ts";
import type { Op } from "../src/types.ts";

const here = new URL(".", import.meta.url);
await loadEngine(await readFile(new URL("../wasm/docxtor_bg.wasm", here)));

test("open, edit and save a .docx losslessly", async () => {
  const bytes = new Uint8Array(await readFile(new URL("../../../fixtures/rich.docx", here)));
  const doc = Engine.open(bytes, "rich.docx");
  assert.equal(fidelityReport(bytes, doc.save()).score, 100);

  const [r] = doc.apply({ op: "replace_text", find: "Quarterly", replace: "Annual", all: true });
  assert.equal(r.count, 1);
  const saved = doc.save();
  const again = Engine.open(saved, "rich.docx");
  assert.match(again.raw.docscript(5), /Annual Product Report/);
  doc.dispose();
  again.dispose();
});

test("blank documents and AI tools", () => {
  const doc = Engine.blank("notes.docx");
  const tools = toolDefinitions();
  assert.ok(tools.some((t) => t.name === "insert_content"));
  doc.raw.callTool("insert_content", JSON.stringify({ markdown: "# Notes\n\nFirst line." }));
  assert.match(doc.raw.docscript(10), /\[p\d+ Heading1\] Notes/);
  doc.dispose();
});

test("bookmarks, horizontal lines and watermarks", () => {
  const doc = Engine.fromMarkdown("# Intro\n\nAlpha beta gamma");
  const [head, body] = doc.renderAll().order;
  doc.apply({ op: "insert_bookmark", from: { p: body, o: 6 }, to: { p: body, o: 10 }, name: "Beta" });
  assert.deepEqual(doc.bookmarks(), [{ name: "Beta", p: body, o: 6, endP: body, endO: 10, text: "beta" }]);
  doc.apply({ op: "rename_bookmark", from: "Beta", to: "Word" });
  assert.equal(doc.bookmarks()[0].name, "Word");
  assert.throws(() => doc.apply({ op: "insert_bookmark", from: { p: head, o: 0 }, to: { p: head, o: 5 }, name: "word" }), /already a bookmark/);

  const [line] = doc.apply({ op: "insert_horizontal_line", at: { p: body, o: 5 } });
  assert.equal(line.created?.length, 2);
  const rules = doc.renderAll().blocks.filter((b) => b.t === "p" && b.hr);
  assert.equal(rules.length, 1);

  assert.equal(doc.pageSetup().watermark, undefined);
  doc.apply({ op: "set_watermark", text: "DRAFT" });
  const w = doc.pageSetup().watermark;
  assert.equal(w?.text, "DRAFT");
  assert.equal(w?.layout, "diagonal");
  assert.equal(doc.pageSetup().sections[0].watermark?.text, "DRAFT");
  assert.match(Engine.open(doc.save(), "x.docx").raw.docscript(10), /Watermark: "DRAFT"/);
  doc.apply({ op: "set_watermark", text: null });
  assert.equal(doc.pageSetup().watermark, undefined);
  doc.dispose();
});

test("accessibility check, title and language", () => {
  const doc = Engine.fromMarkdown("# One\n\n### Three\n\nSee [click here](https://example.com).");
  const issues = doc.accessibility();
  assert.deepEqual(issues.find((i) => i.kind === "heading_skip")?.detail, "Heading 1 to Heading 3");
  assert.equal(issues.find((i) => i.kind === "link_text")?.detail, "click here");
  assert.equal(issues.find((i) => i.kind === "no_title")?.severity, "tip");
  assert.equal(issues.find((i) => i.kind === "no_title")?.p, undefined);
  doc.apply({ op: "set_doc_props", title: "Notes" });
  assert.equal(doc.docProps().title, "Notes");
  assert.ok(!doc.accessibility().some((i) => i.kind === "no_title"));
  assert.equal(Engine.open(doc.save(), "x.docx").docProps().title, "Notes");
  doc.apply({ op: "set_language", lang: "es-es" });
  assert.equal(doc.pageSetup().lang, "es-ES");
  assert.ok(doc.undo() && doc.pageSetup().lang === "en-US");
  assert.match(doc.raw.callTool("check_accessibility", "{}"), /heading_skip/);
  doc.dispose();
});

test("endnotes, captions, cross-references and the index", () => {
  const doc = Engine.fromMarkdown("# Findings\n\nApples grow on trees.\n\nA chart goes here.");
  const [head, text, chart] = doc.renderAll().order;
  // Endnotes are numbered i, ii and listed apart from footnotes.
  doc.apply({ op: "insert_endnote", at: { p: text, o: 6 }, text: "Source." });
  doc.apply({ op: "insert_footnote", at: { p: text, o: 3 }, text: "Aside." });
  assert.deepEqual(doc.pageSetup().endnotes.map((n) => n.label), ["i"]);
  assert.deepEqual(doc.pageSetup().notes.map((n) => n.label), ["1"]);
  const marks = doc.renderAll().blocks.flatMap((b) => (b.t === "p" && b.id === text ? b.segs.filter((s) => s.k === "note") : []));
  assert.deepEqual(marks.map((s) => [s.label, s.note]), [["1", undefined], ["i", "endnote"]]);
  doc.apply({ op: "set_note_format", kind: "endnote", format: "upperLetter" });
  assert.equal(doc.pageSetup().endnotes[0].label, "A");

  // A caption, a cross-reference to it, and the lists the dialog offers.
  const [cap] = doc.apply({ op: "insert_caption", after: chart, label: "Figure", text: "Apples per tree" });
  const capId = cap.created![0];
  const targets = doc.xrefTargets();
  assert.deepEqual(targets.captions.map((c) => [c.label, c.number, c.text]), [["Figure", "1", "Figure 1: Apples per tree"]]);
  assert.equal(targets.headings[0].text, "Findings");
  assert.equal(targets.endnotes[0].label, "A");
  doc.apply({ op: "insert_cross_reference", at: { p: head, o: 8 }, target: { type: "caption", p: capId }, show: "label_number" });
  assert.match(doc.text(), /FindingsFigure 1/);
  const linked = doc.renderAll().blocks.flatMap((b) => (b.t === "p" && b.id === head ? b.segs.filter((s) => s.link?.startsWith("#_Ref")) : []));
  assert.equal(linked.length, 1);

  // Index entries are hidden; the index lists them, in a section of its own.
  doc.apply({ op: "mark_index_entry", at: { p: text, o: 6 }, entry: "Apple" });
  doc.apply({ op: "insert_index", after: chart, pages: { [text]: 4 } });
  assert.match(doc.text(), /Apple\t4/);
  assert.equal(doc.pageSetup().sections.length, 3);
  assert.equal(doc.pageSetup().sections[1].cols.num, 2);
  // A caption before the first renumbers the rest after update_fields.
  doc.apply({ op: "insert_caption", before: chart, label: "Figure", text: "Cover" });
  doc.apply({ op: "update_fields" });
  assert.match(doc.text(), /Figure 1: Cover[\s\S]*Figure 2: Apples per tree/);
  Engine.open(doc.save(), "x.docx").dispose();
  assert.ok(toolDefinitions().some((t) => t.name === "insert_cross_reference"));
  doc.dispose();
});

test("compare two versions and save a redline", () => {
  const older = Engine.fromMarkdown("Keep this.\n\nThe quick brown fox.\n\nGone.");
  const newer = Engine.fromMarkdown("Keep this.\n\nThe slow brown fox.\n\nNew paragraph.");
  const old = older.save();
  const view = newer.compareView(old);
  assert.deepEqual(
    view.map((b) => b.kind),
    ["same", "changed", "removed", "added"],
  );
  assert.deepEqual(view[1].parts, [
    { t: "same", text: "The " },
    { t: "del", text: "quick" },
    { t: "ins", text: "slow" },
    { t: "same", text: " brown fox." },
  ]);
  const red = Engine.open(newer.redline(old, "Ada", "2026-10-12T09:30:00Z"), "redline.docx");
  const paras = red.renderAll().blocks.flatMap((b) => (b.t === "p" ? [b] : []));
  assert.ok(paras.some((p) => p.segs.some((s) => s.k === "ghost" && s.x === "quick")));
  assert.ok(paras.some((p) => p.segs.some((s) => s.ins && s.x === "slow")));
  assert.equal(red.text().replace(/\n+/g, "\n").trim(), "Keep this.\nThe slow brown fox.\nNew paragraph.");
  // A default date is filled in.
  assert.ok(newer.redline(old, "Ada").length > 0);
  for (const e of [older, newer, red]) e.dispose();
});

test("sources, citations and a bibliography", () => {
  const doc = Engine.fromMarkdown("Citing is hard here.\n\nSecond paragraph.");
  const [first] = doc.renderAll().order;
  assert.deepEqual(doc.sources(), []);
  assert.equal(doc.citationStyle(), "apa");

  // BibTeX and CSL-JSON become sources; `add_source` makes the tags.
  const fromBib = parseBibtex(`@article{k, author = {Smith, John and Lee, Karen}, title = {A study of things}, journal = {Journal of Stuff}, volume = {12}, number = {3}, pages = {45--67}, year = {2020}, doi = {10.1000/xyz}}`);
  assert.equal(fromBib[0].type, "article");
  assert.equal(fromBib[0].containerTitle, "Journal of Stuff");
  assert.equal(fromBib[0].tag, "");
  const fromCsl = Engine.parseCslJson(`{"type":"book","title":"The Art of Citing","author":[{"family":"Roe","given":"Jane"}],"issued":{"date-parts":[[2019]]},"publisher":"Penguin"}`);
  assert.equal(fromCsl[0].year, "2019");
  assert.throws(() => parseBibtex("nothing here"), /No BibTeX entries found/);
  assert.throws(() => parseCslJson("{oops"), /not valid JSON/);
  doc.apply([...fromBib, ...fromCsl].map((source) => ({ op: "add_source", source }) as Op));
  assert.deepEqual(doc.sources().map((s) => s.tag), ["Smi20", "Roe19"]);
  assert.deepEqual(doc.sources()[1].authors, [{ family: "Roe", given: "Jane" }]);
  assert.match(doc.sourcesBibtex(), /@article\{Smi20,/);

  // A citation is a marked unit in the view model.
  const [res] = doc.apply({ op: "insert_citation", at: { p: first, o: 14 }, tags: ["Smi20"], page: "12" });
  assert.equal(res.caret?.o, 14 + "(Smith & Lee, 2020, p. 12)".length);
  assert.deepEqual(doc.citations().map((c) => [c.o, c.len, c.tags, c.page]), [[14, 26, ["Smi20"], "12"]]);
  const para = doc.renderAll().blocks.find((b) => b.t === "p" && b.id === first);
  assert.ok(para && para.t === "p");
  const seg = para.segs.find((s) => s.cite);
  assert.equal(seg?.x, "(Smith & Lee, 2020, p. 12)");
  assert.deepEqual(seg?.cite, ["Smi20"]);

  // A bibliography is a container labelled Bibliography; switching style rewrites it.
  doc.apply({ op: "insert_bibliography" });
  const bib = () => doc.renderAll().blocks.find((b) => b.t === "c" && b.label === "Bibliography");
  assert.ok(bib());
  doc.apply({ op: "set_citation_style", style: "mla" });
  assert.equal(doc.citationStyle(), "mla");
  assert.match(doc.text(), /Works Cited/);
  assert.match(doc.text(), /Smith, John, and Karen Lee\. "A study of things\."/);
  assert.throws(() => doc.apply({ op: "delete_source", tag: "Smi20" }), /cited 1 time in the document/);

  // It saves as Word's own structures and opens again.
  const saved = doc.save();
  const again = Engine.open(saved, "x.docx");
  assert.equal(again.sources().length, 2);
  assert.equal(again.citationStyle(), "mla");
  assert.equal(again.citations()[0].tags[0], "Smi20");
  assert.ok(doc.undo());
  assert.equal(doc.citationStyle(), "apa");
  doc.dispose();
  again.dispose();
});

test("shapes and text boxes", () => {
  const doc = Engine.fromMarkdown("Intro paragraph");
  const [p] = doc.renderAll().order;
  const [made] = doc.apply({ op: "insert_text_box", at: { p, o: 5 }, w: 160, h: 50, text: "Boxed\nlines" });
  assert.equal(made.created?.length, 2);
  assert.deepEqual(made.caret, { p: made.created![0], o: 0 });
  const box = doc.shapeAt({ p, o: 5 });
  assert.equal(box?.textBox, true);
  assert.equal(box?.geometry, "rect");
  assert.equal(box?.wrap, "square");
  assert.equal(box?.fill?.color, "FFFFFF");
  assert.deepEqual(box?.text?.map((b) => b.id), made.created);
  // The shape is a segment of its paragraph.
  const para = doc.renderAll().blocks.find((b) => b.t === "p" && b.id === p);
  assert.ok(para?.t === "p" && para.segs.some((s) => s.k === "shape" && s.o === 5 && s.shape?.textBox));
  // Its text is edited with ordinary text ops.
  doc.apply({ op: "insert_text", at: { p: made.created![1], o: 5 }, text: "!" });
  assert.match(doc.raw.docscript(10), /\[shape: textbox "Boxed lines!"\]/);
  doc.apply({ op: "set_shape", at: { p, o: 5 }, w: 220, fill: "FFF2CC", line: { color: "FF0000", width_pt: 2 } });
  const moved = doc.shapeAt({ p, o: 5 });
  assert.equal(moved?.w, 220);
  assert.equal(moved?.fill?.color, "FFF2CC");
  assert.equal(moved?.line?.widthPt, 2);
  assert.equal(doc.shapeAt({ p, o: 0 }), null);
  // It survives a save.
  const again = Engine.open(doc.save(), "x.docx");
  assert.equal(again.shapeAt({ p: again.renderAll().order[0], o: 5 })?.text?.length, 2);
  assert.ok(toolDefinitions().some((t) => t.name === "insert_shape"));
  doc.dispose();
  again.dispose();
});

test("equations: LaTeX preview, insert, read back, edit, delete", () => {
  // A live preview is MathML Core; mistakes come back as plain words.
  assert.match(Engine.latexToMathml("\\frac{a}{b}"), /<mfrac>/);
  assert.match(Engine.latexToMathml("x^2", true), /display="block"/);
  assert.throws(() => Engine.latexToMathml("\\frac{a}"), /second group/);

  const doc = Engine.fromMarkdown("The area is  here.");
  const [p] = doc.renderAll().order;
  const [r] = doc.apply({ op: "insert_equation", at: { p, o: 12 }, latex: "A = \\pi r^2" });
  assert.deepEqual(r.caret, { p, o: 13 });
  // One position in the paragraph, like an image; the segment carries what a renderer needs.
  const para = doc.renderAll().blocks.find((b) => b.t === "p" && b.id === p);
  assert.equal(para?.t === "p" && para.len, 19);
  const seg = para?.t === "p" ? para.segs.find((s) => s.k === "math") : undefined;
  assert.equal(seg?.o, 12);
  assert.equal(seg?.math?.latex, "A=\\pi r^2");
  assert.match(seg?.math?.mathml ?? "", /<msup>/);
  assert.equal(seg?.math?.display, undefined);
  assert.deepEqual(doc.equationAt({ p, o: 12 }), { latex: "A=\\pi r^2", display: false });
  assert.equal(doc.equationAt({ p, o: 3 }), null);
  assert.match(doc.raw.docscript(10), /The area is \$A=\\pi r\^2\$ here\./);
  assert.equal(doc.textRange({ p, o: 0 }, { p, o: 19 }), "The area is $A=\\pi r^2$ here.");

  // A saved file opens again with the same equation, and a bad op changes nothing.
  const again = Engine.open(doc.save(), "eq.docx");
  assert.deepEqual(again.equationAt({ p: again.renderAll().order[0], o: 12 }), { latex: "A=\\pi r^2", display: false });
  assert.throws(() => doc.apply({ op: "set_equation", at: { p, o: 12 }, latex: "\\bogus" }), /Unknown command/);
  doc.apply({ op: "set_equation", at: { p, o: 12 }, latex: "E = mc^2", display: true });
  const ids = doc.renderAll().order;
  assert.equal(ids.length, 3);
  assert.deepEqual(doc.equationAt({ p: ids[1], o: 0 }), { latex: "E=mc^2", display: true });
  doc.apply({ op: "delete", from: { p: ids[1], o: 0 }, to: { p: ids[1], o: 1 } });
  assert.equal(doc.equationAt({ p: ids[1], o: 0 }), null);
  doc.dispose();
  again.dispose();
});

test("charts: insert, read, change, save, reopen and undo", () => {
  const doc = Engine.fromMarkdown("# Report\n\nSales rose.");
  const [, body] = doc.renderAll().order;
  const at = { p: body, o: 0 };
  doc.apply({
    op: "insert_chart",
    at,
    kind: "column",
    title: "Sales",
    categories: ["Q1", "Q2", "Q3"],
    series: [
      { name: "2024", values: [4, 5, 7] },
      { name: "2025", values: [5, null, 9] },
    ],
  } satisfies InsertChartOp);
  const chart = doc.chartAt(at);
  assert.equal(chart?.kind, "column");
  assert.equal(chart?.title, "Sales");
  assert.deepEqual(chart?.categories, ["Q1", "Q2", "Q3"]);
  assert.deepEqual(chart?.series[1].values, [5, null, 9]);
  // The view model carries the same chart in the paragraph's segment, with its size.
  const para = doc.renderAll().blocks.find((b) => b.t === "p" && b.id === body);
  assert.ok(para && para.t === "p");
  const seg = para.segs.find((s) => s.k === "chart");
  assert.equal(seg?.img?.w, 432);
  assert.equal(seg?.chart?.series.length, 2);
  assert.match(doc.raw.docscript(10), /\[chart p\d+@0: Sales \(column, legend bottom\)/);

  doc.apply({ op: "set_chart", at, kind: "line", title: "Sales by quarter", legend: "none" } satisfies SetChartOp);
  assert.equal(doc.chartAt(at)?.kind, "line");
  assert.equal(doc.chartAt(at)?.legend, undefined);
  assert.deepEqual(doc.chartAt(at)?.series[0].values, [4, 5, 7]);

  const again = Engine.open(doc.save(), "chart.docx");
  const [, reopened] = again.renderAll().order; // ids are made again when a file is opened
  assert.equal(again.chartAt({ p: reopened, o: 0 })?.title, "Sales by quarter");
  assert.equal(again.chartAt({ p: reopened, o: 1 }), null);
  assert.throws(() => doc.apply({ op: "set_chart", at: { p: body, o: 5 }, title: "x" }), /no chart/);
  assert.ok(doc.undo());
  assert.equal(doc.chartAt(at)?.kind, "column");
  assert.ok(doc.undo());
  assert.equal(doc.chartAt(at), null);
  doc.dispose();
  again.dispose();
});

test("export as a web page, EPUB, OpenDocument and Rich Text", async () => {
  const bytes = new Uint8Array(await readFile(new URL("../../../fixtures/rich.docx", here)));
  const doc = Engine.open(bytes, "rich.docx");
  const before = doc.save();
  const html = doc.exportHtml();
  assert.ok(html.startsWith("<!doctype html>"));
  assert.match(html, /<title>Docxtor fixture<\/title>/);
  assert.match(html, /Quarterly Product Report/);
  const rtf = doc.exportRtf();
  assert.ok(rtf.startsWith("{\\rtf1"));
  assert.match(rtf, /Quarterly Product Report/);
  const zipStart = (b: Uint8Array, mime: string) =>
    b[0] === 0x50 && b[1] === 0x4b && new TextDecoder().decode(b.subarray(30, 38)) === "mimetype" && new TextDecoder().decode(b.subarray(38, 38 + mime.length)) === mime;
  assert.ok(zipStart(doc.exportOdt(), "application/vnd.oasis.opendocument.text"));
  assert.ok(zipStart(doc.exportEpub(), "application/epub+zip"));
  assert.deepEqual(doc.save(), before, "exporting leaves the document as it was");
});

test("CSL citation styles: added to a document, chosen, and Chicago notes as footnotes", async () => {
  const csl = (name: string) => readFile(new URL(`../../../web/src/csl/${name}`, here), "utf8");
  const doc = Engine.fromMarkdown("Citing is hard here.\n\nSecond paragraph.");
  const [first] = doc.renderAll().order;
  doc.apply({ op: "add_source", source: { type: "book", title: "The Art of Citing", authors: [{ family: "Roe", given: "Jane" }], year: "2019", publisher: "Penguin", place: "New York", tag: "" } });
  assert.deepEqual(doc.citationStyles(), []);
  assert.throws(() => doc.apply({ op: "set_citation_style", style: "nature" }), /not a citation style Docxtor knows/);
  assert.throws(() => doc.registerCitationStyle("apa", "<x/>"), /one of Docxtor's own/);
  assert.throws(() => doc.registerCitationStyle("bad", "<html/>"), /not a CSL style/);

  const info = doc.registerCitationStyle("chicago-nb", await csl("chicago-notes-bibliography-16th-edition.csl"));
  assert.deepEqual(info, { id: "chicago-nb", title: "Chicago Manual of Style 16th edition (notes and bibliography)", note: true, numeric: false });
  doc.registerCitationStyle("nature", await csl("nature.csl"));
  assert.deepEqual(doc.citationStyles().map((s) => s.id), ["chicago-nb", "nature"]);

  doc.apply({ op: "set_citation_style", style: "chicago-nb" });
  assert.equal(doc.citationStyle(), "chicago-nb");
  assert.equal(doc.citationsAreNotes(), true);
  doc.apply({ op: "insert_citation", at: { p: first, o: 19 }, tags: ["Roe19"], page: "5" });
  doc.apply({ op: "insert_citation", at: { p: first, o: 20 }, tags: ["Roe19"], page: "5" });
  const notes = doc.pageSetup().notes;
  assert.equal(notes.length, 2);
  const text = (n: unknown): string => (Array.isArray(n) ? n.map(text).join("") : n && typeof n === "object" ? ((n as { k?: string }).k === "t" ? String((n as { x?: string }).x) : Object.values(n).map(text).join("")) : "");
  assert.equal(text(notes[0]).trim(), "Jane Roe, The Art of Citing (New York: Penguin, 2019), 5.");
  assert.equal(text(notes[1]).trim(), "Ibid.");
  doc.apply({ op: "set_citation_style", style: "nature" });
  assert.equal(doc.citationsAreNotes(), false);
  assert.equal(doc.pageSetup().notes.length, 0);
  assert.equal(doc.citations().length, 2);
  doc.dispose();
});

test("lists a person defines, set numbering value and sorting, through the engine", () => {
  const doc = Engine.fromMarkdown("Pear\n\nApple\n\nFig");
  const ids = doc.renderAll().order;
  assert.equal(doc.formatNumber(3, "upperRoman"), "III");
  assert.equal(doc.formatNumber(2, "cardinalText", "es-ES"), "Dos");
  assert.equal(doc.formatNumber(1, "japaneseCounting"), "一");

  const [made] = doc.apply({ op: "define_list", ids, def: { name: "Steps", levels: [{ format: "decimal", text: "Step %1:", start: 2 }] } });
  const numId = made.count!;
  doc.renderChanged();
  assert.equal(doc.formatAt({ p: ids[1], o: 0 })?.listNum, numId);
  assert.equal(doc.formatAt({ p: ids[1], o: 0 })?.listValue, 3);
  const def = doc.listDefinition(numId)!;
  assert.equal(def.name, "Steps");
  assert.equal(def.levels.length, 9);
  assert.equal(def.levels[0].text, "Step %1:");
  const [shown] = doc.listDefinitions();
  assert.deepEqual([shown.numId, shown.bullet, shown.levels[0].text], [numId, false, "Step 2:"]);

  doc.apply({ op: "sort_paragraphs", ids, keys: [{ descending: true }] });
  assert.match(doc.raw.docscript(10), /Pear[\s\S]*Fig[\s\S]*Apple/);
  const labels = doc.renderAll().blocks.map((b) => (b.t === "p" ? b.list?.text : ""));
  assert.deepEqual(labels, ["Step 2:", "Step 3:", "Step 4:"]);

  doc.apply({ op: "set_numbering_value", id: ids[1], value: 9 });
  doc.renderChanged();
  assert.equal(doc.formatAt({ p: ids[1], o: 0 })?.listValue, 9);
  assert.throws(() => doc.apply({ op: "define_list", ids, def: { levels: [{ format: "decimal", text: "no count" }] } }), /no count in it/);
  assert.ok(doc.save().length > 0);
  doc.dispose();
});

test("shape geometry: a freeform's commands, connection sites, connectors and the route between two sites", () => {
  const doc = Engine.fromMarkdown("A paragraph to hang shapes on");
  const p = doc.renderAll().order[0];
  doc.apply({ op: "insert_freeform", at: { p, o: 0 }, points: [[0, 0], [60, 0], [30, 50]], closed: true, x: 0, y: 0 });
  const free = doc.shapeAt({ p, o: 0 })!;
  assert.equal(free.geometry, "custom");
  assert.deepEqual(free.custom!.paths[0].cmds!.map((c) => c.t), ["M", "L", "L", "Z"]);
  // The corners are the connection sites.
  assert.equal(doc.shapeSites({ p, o: 0 }).length, 3);
  // Give a rectangle a new outline: a preset becomes a custom shape with the commands as sent.
  doc.apply({ op: "insert_shape", at: { p, o: 1 }, geometry: "rect", w: 100, h: 60, floating: true, x: 200, y: 0 });
  doc.apply({ op: "set_shape_path", at: { p, o: 1 }, paths: [{ w: 100, h: 60, cmds: [{ t: "M", pts: [[0, 0]] }, { t: "L", pts: [[100, 0]] }, { t: "L", pts: [[50, 60]] }, { t: "Z" }] }] });
  const tri = doc.shapeAt({ p, o: 1 })!;
  assert.equal(tri.geometry, "custom");
  assert.equal(tri.w, 100);
  // A rectangle's four sites: top, left, bottom, right, and the way a connector leaves each.
  doc.apply({ op: "insert_shape", at: { p, o: 2 }, geometry: "rect", w: 100, h: 60, floating: true, x: 400, y: 100 });
  assert.deepEqual(doc.shapeSites({ p, o: 2 }).map((s) => [s.x, s.y, s.ang]), [[50, 0, 270], [0, 30, 180], [50, 60, 90], [100, 30, 0]]);
  // A connector between the freeform's corner and the rectangle's left side is glued to both.
  doc.apply({ op: "insert_connector", at: { p, o: 3 }, kind: "elbow", start: { shape: { p, o: 1 }, idx: 0 }, end: { shape: { p, o: 2 }, idx: 1 } });
  const link = doc.shapeAt({ p, o: 3 })!;
  assert.match(link.geometry, /^bentConnector[2-5]$/);
  assert.equal(link.connect?.end?.idx, 1);
  // The route between two points, as the preview draws it: right of a box to the left of another, down and to the right.
  const route = doc.connectorRoute("elbow", [100, 30], [1, 0], [200, 130], [-1, 0]);
  assert.deepEqual([route.prst, route.x, route.y, route.w, route.h], ["bentConnector3", 100, 30, 100, 100]);
  const straight = doc.connectorRoute("straight", [100, 100], [0, 0], [40, 50], [0, 0]);
  assert.deepEqual([straight.prst, straight.w, straight.h, straight.flipH, straight.flipV], ["straightConnector1", 60, 50, true, true]);
  assert.throws(() => doc.connectorRoute("zigzag" as "elbow", [0, 0], [0, 0], [1, 1], [0, 0]), /unknown connector kind/);
  doc.dispose();
});

test("styles, themes, style sets and spacing through the engine", () => {
  const doc = Engine.fromMarkdown("# Heading\n\nBody text");
  const [head, body] = doc.renderAll().order;

  // The gallery draws each style from its resolved look, and offers Word's styles the file lacks.
  const gallery = doc.styleGallery();
  const h1 = gallery.find((s) => s.id === "Heading1")!;
  assert.deepEqual([h1.name, h1.latent, h1.quick, h1.uses], ["Heading 1", false, true, 1]);
  assert.equal(h1.run.font, "Calibri Light");
  assert.ok(gallery.some((s) => s.id === "IntenseQuote" && s.latent));

  // Modify, rename, create from the selection, delete: ops with the same shapes the dialogs send.
  doc.apply({ op: "set_style_definition", style: "Heading 1", def: { rpr: { size: 30, bold: true }, ppr: { spaceAfter: 240 } } });
  assert.equal(doc.formatAt({ p: head, o: 0 })?.run.size, 30);
  doc.apply({ op: "set_style_definition", def: { name: "Callout", kind: "paragraph", quick: true, rpr: { italic: true } } });
  assert.equal(doc.styleDetails("Callout")?.custom, true);
  doc.apply({ op: "set_style", ids: [body], style: "Callout" });
  assert.deepEqual(doc.styleUses("Callout"), [{ p: body, from: 0, to: 9 }]);
  doc.apply({ op: "delete_style", style: "Callout" });
  assert.equal(doc.styleDetails("Callout"), null);
  assert.throws(() => doc.apply({ op: "delete_style", style: "Normal" }), /can't be deleted/);

  // Reveal formatting says where a setting comes from.
  const reveal = doc.revealFormatting({ p: head, o: 1 })!;
  assert.equal(reveal.paragraphStyle.name, "Heading 1");
  assert.equal(reveal.rows.find((r) => r.key === "size")?.source, "paragraphStyle");

  // Themes: the library, the document's theme, and changing it.
  const library = Engine.themeLibrary();
  assert.ok(library.themes.length >= 24);
  assert.ok(library.themes.some((t) => t.id === "ion-boardroom" && t.fonts.major === "Century Gothic"));
  assert.ok(library.colorSchemes.some((c) => c.name === "Blue Warm"));
  assert.deepEqual([doc.themeInfo().name, doc.themeInfo().fonts.major], ["Office", "Calibri Light"]);
  const color = doc.formatAt({ p: head, o: 0 })!.run.color;
  doc.apply({ op: "set_theme", theme: "facet" });
  assert.deepEqual([doc.themeInfo().name, doc.formatAt({ p: head, o: 0 })!.run.font], ["Facet", "Trebuchet MS"]);
  assert.notEqual(doc.formatAt({ p: head, o: 0 })!.run.color, color);
  doc.apply({ op: "set_theme", colors: { name: "Mine", colors: { accent1: "112233" } } });
  assert.equal(doc.themeInfo().colors.colors.accent1, "112233");
  assert.throws(() => doc.apply({ op: "set_theme", colors: { name: "Bad", colors: { accent1: "nope" } } }), /not a colour/);

  // Style sets and spacing, as the gallery previews them, then applied.
  const previews = doc.styleSetPreviews();
  assert.equal(Engine.styleSetInfos().length, previews.length);
  const old = previews.find((p) => p.id === "word-2003")!;
  assert.equal(old.samples.find((s) => s.id === "Heading1")?.run.font, "Arial");
  assert.throws(() => doc.styleSetPreview({ name: "Empty", styles: "" }), /no styles/);
  doc.apply({ op: "set_style_set", style_set: "word-2003" });
  assert.equal(doc.formatAt({ p: body, o: 0 })?.run.font, "Times New Roman");
  assert.deepEqual(Engine.spacingPresets().map((p) => p.id), ["default", "none", "compact", "tight", "open", "relaxed", "double"]);
  doc.apply({ op: "set_paragraph_spacing", preset: "double" });
  assert.equal(doc.formatAt({ p: body, o: 0 })?.para.line, 480);

  // What Set as default keeps starts a new document.
  Engine.defaultDesign = doc.designSnapshot();
  const fresh = Engine.blank();
  Engine.defaultDesign = null;
  assert.equal(fresh.themeInfo().name, "Facet");
  assert.equal(fresh.raw.canUndo(), false);
  assert.equal(fresh.raw.isModified(), false);
  fresh.dispose();
  assert.ok(doc.save().length > 0);
  doc.dispose();
});

test("password-protected documents open with the password and save protected", async () => {
  const locked = new Uint8Array(await readFile(new URL("../../../fixtures/encrypted/encrypted-agile.docx", here)));
  assert.equal(isEncrypted(locked), true);
  assert.equal(isEncrypted(new Uint8Array(await readFile(new URL("../../../fixtures/rich.docx", here)))), false);
  // The two failures are told apart.
  assert.throws(() => Engine.open(locked, "locked.docx"), (e: unknown) => e instanceof EncryptedError && e.code === "encrypted");
  assert.throws(() => Engine.open(locked, "locked.docx", "nope"), (e: unknown) => e instanceof WrongPasswordError && e.code === "wrong_password");

  const doc = Engine.open(locked, "locked.docx", "docxtor-test-pw");
  assert.equal(doc.hasPassword, true);
  assert.match(doc.text(), /marmalade/);
  doc.apply({ op: "replace_text", find: "marmalade", replace: "quince", all: true });
  const saved = doc.save();
  assert.equal(isEncrypted(saved), true);
  assert.equal(isEncrypted(doc.savePlain()), false);
  assert.throws(() => Engine.open(saved, "locked.docx", "docxtor-test-pw-2"), WrongPasswordError);
  const again = Engine.open(saved, "locked.docx", "docxtor-test-pw");
  assert.match(again.text(), /quince/);

  // Change the password, then remove it.
  again.setPassword("another one");
  const changed = again.save();
  assert.throws(() => Engine.open(changed, "x.docx", "docxtor-test-pw"), WrongPasswordError);
  assert.match(Engine.open(changed, "x.docx", "another one").text(), /quince/);
  again.setPassword(null);
  assert.equal(again.hasPassword, false);
  assert.equal(isEncrypted(again.save()), false);
  // An older protected version can still be compared while the password is the same.
  doc.setPassword("docxtor-test-pw");
  assert.equal(doc.compareView(saved).length > 0, true);
  doc.dispose();
  again.dispose();
});

const fixture = async (name: string) => new Uint8Array(await readFile(new URL(`../../../fixtures/${name}`, here)));

test("other formats open as new documents, with a report of what was done", async () => {
  for (const [file, format, text] of [
    ["formats/sample.odt", "odt", "Quarterly report"],
    ["formats/sample.rtf", "rtf", "Quarterly report"],
    ["formats/sample.html", "html", "Quarterly report"],
    ["formats/sample.doc", "doc", "Plain closing paragraph."],
    ["formats/sample-word2003.xml", "word2003", "Plain closing paragraph."],
  ] as const) {
    const bytes = await fixture(file);
    const doc = Engine.open(bytes, file.split("/")[1]);
    assert.equal(doc.openReport?.format, format, file);
    assert.equal(doc.openReport?.converted, true, file);
    assert.match(doc.text(), new RegExp(text), file);
    // The new document is a .docx the engine reads back.
    const again = Engine.open(doc.save(), "x.docx");
    assert.equal(again.openReport?.converted, false);
    assert.equal(again.openReport?.notes.length, 0);
    doc.dispose();
    again.dispose();
  }
});

test("what a file is comes from its content, and files that are not documents give clear errors", async () => {
  assert.equal(Engine.sniff(await fixture("formats/sample.rtf")), "rtf");
  assert.equal(Engine.sniff(await fixture("formats/sample.doc")), "doc");
  assert.equal(Engine.sniff(await fixture("rich.docx")), "word");
  assert.equal(Engine.sniff(new TextEncoder().encode("%PDF-1.4")), "foreign:pdf");
  assert.throws(() => Engine.open(new Uint8Array(), "empty.docx"), (e: Error) => e instanceof OpenError && e.code === "empty");
  assert.throws(
    () => Engine.open(new TextEncoder().encode("%PDF-1.4 hello"), "x.docx"),
    (e: Error) => e instanceof OpenError && e.code === "not-word" && e.detail === "pdf",
  );
});

test("a cut-off file is repaired and the report says what happened", async () => {
  const bytes = await fixture("long.docx");
  const doc = Engine.open(bytes.slice(0, Math.floor(bytes.length * 0.6)), "cut.docx");
  const codes = doc.openReport?.notes.map((n) => n.code) ?? [];
  assert.ok(codes.includes("zip-directory"), codes.join());
  assert.equal(doc.openReport?.converted, false);
  assert.ok(doc.text().length > 0);
  doc.dispose();
});

test("a plain document gets a password and a fresh Engine reads it back", () => {
  const doc = Engine.fromMarkdown("# Private\n\nOnly for you.");
  assert.equal(doc.hasPassword, false);
  doc.setPassword("pässwörd");
  const bytes = doc.save();
  assert.deepEqual(Array.from(bytes.slice(0, 4)), [0xd0, 0xcf, 0x11, 0xe0]);
  // Saving twice keeps the file readable with the same password.
  const second = doc.save();
  for (const b of [bytes, second]) {
    const back = Engine.open(b, "private.docx", "pässwörd");
    assert.match(back.text(), /Only for you/);
    back.dispose();
  }
  doc.dispose();
});

test("document properties, custom properties and the statistics saved with a document", () => {
  const doc = Engine.fromMarkdown("Hello big world\n\nSecond paragraph here");
  doc.apply([
    { op: "set_doc_props", title: "Plan", author: "Ana", subject: "Budget", keywords: "a, b", category: "Finance", status: "Draft", company: "Analytical", manager: "Ada" },
    { op: "set_custom_props", set: [{ name: "Project", type: "text", value: "Babbage" }, { name: "Revision", type: "number", value: "7" }, { name: "Due", type: "date", value: "2026-10-14" }, { name: "Reviewed", type: "bool", value: "yes" }] },
  ]);
  const props = doc.docProps();
  assert.deepEqual([props.title, props.author, props.subject, props.keywords, props.category, props.status, props.company, props.manager], ["Plan", "Ana", "Budget", "a, b", "Finance", "Draft", "Analytical", "Ada"]);
  assert.deepEqual(props.custom?.map((c) => [c.name, c.type, c.value]), [["Project", "text", "Babbage"], ["Revision", "number", "7"], ["Due", "date", "2026-10-14"], ["Reviewed", "bool", "true"]]);
  // Undo takes both back in one step.
  assert.equal(doc.undo(), true);
  assert.equal(doc.docProps().custom, undefined);
  doc.redo();
  // A value that does not fit its type is refused.
  assert.throws(() => doc.apply({ op: "set_custom_props", set: [{ name: "N", type: "number", value: "twelve" }] }), /not a number/);

  // The next save records who saved, and the counts Word keeps in docProps/app.xml.
  doc.saveInfo = () => ({ user: "Ben", pages: 3 });
  const again = Engine.open(doc.save(), "plan.docx");
  const saved = again.docProps();
  assert.equal(saved.lastModifiedBy, "Ben");
  assert.equal(saved.company, "Analytical");
  assert.equal(saved.custom?.length, 4);
  doc.dispose();
  again.dispose();
});

test("the undo list names each step and can go back to any point", () => {
  const doc = Engine.fromMarkdown("One");
  const [p] = doc.renderAll().order;
  doc.apply({ op: "insert_text", at: { p, o: 3 }, text: "!" });
  doc.apply({ op: "set_doc_props", title: "T" });
  doc.apply({ op: "insert_text", at: { p, o: 4 }, text: "?" });
  assert.deepEqual(doc.undoLabels().slice(-3), ["insert_text", "set_doc_props", "insert_text"]);
  assert.equal(doc.undoSteps(2), true);
  assert.deepEqual(doc.redoLabels(), ["set_doc_props", "insert_text"]);
  assert.equal(doc.text().trim(), "One!");
  assert.equal(doc.docProps().title, undefined);
  assert.equal(doc.redoSteps(5), true);
  assert.equal(doc.text().trim(), "One!?");
  assert.deepEqual(doc.redoLabels(), []);
  doc.dispose();
});

test("a view-only document is refused by the engine whoever asks; showing a tab is no op, so it needs no exception", () => {
  const doc = Engine.fromMarkdown("One");
  const [p] = doc.renderAll().order;
  assert.equal(doc.access(), "edit");
  doc.apply({ op: "insert_text", at: { p, o: 3 }, text: "!" });
  doc.apply({ op: "tab", action: "add", name: "Two", first_name: "One" } as Op);
  const first = doc.tabsInfo().tabs[0].id;
  doc.setAccess("view");
  assert.equal(doc.access(), "view");
  assert.throws(() => doc.apply({ op: "insert_text", at: { p, o: 0 }, text: "X" }), /read-only/);
  assert.throws(() => doc.raw.callTool("insert_content", JSON.stringify({ markdown: "More" })), /read-only/);
  assert.throws(() => doc.addMedia("png", new Uint8Array([1, 2, 3])), /read-only/);
  assert.equal(doc.undo(), false);
  assert.equal(doc.redo(), false);
  assert.throws(() => doc.apply({ op: "tab", action: "open", id: first } as Op), /open/);
  doc.setOpenTab(first);
  assert.equal(doc.tabsInfo().active, first);
  assert.equal(doc.text().includes("X"), false);
  assert.throws(() => doc.setAccess("nobody" as "edit"));
  doc.setAccess("edit");
  assert.equal(doc.undo(), true);
  doc.dispose();
});

test("a selection copy holds only the selected text, and a document becomes a template and back", () => {
  const doc = Engine.fromMarkdown("# Title\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird paragraph.");
  const order = doc.renderAll().order;
  const copy = Engine.open(doc.selectionCopy({ p: order[1], o: 6 }, { p: order[2], o: 6 }), "part.docx");
  assert.deepEqual(copy.text().split("\n").filter(Boolean), ["paragraph.", "Second"]);
  // The document itself is not touched.
  assert.match(doc.text(), /Third paragraph/);
  copy.dispose();

  const bytes = doc.save();
  assert.deepEqual(packageKind(bytes), { template: false, macros: false });
  const template = packageAsTemplate(bytes);
  assert.deepEqual(packageKind(template), { template: true, macros: false });
  assert.match(Engine.open(template, "t.dotx").text(), /Second paragraph/);
  const made = packageAsDocument(template, new Date("2026-10-14T09:00:00Z"));
  assert.deepEqual(packageKind(made), { template: false, macros: false });
  const props = Engine.open(made, "made.docx").docProps();
  assert.equal(props.created, "2026-10-14T09:00:00Z");
  assert.equal(props.modified, undefined);
  doc.dispose();
});

test("a picture the engine cannot convert is tried once, not on every render", () => {
  const doc = Engine.blank("x.docx");
  let tries = 0;
  const raw = doc.raw as unknown as { media: (p: string) => Uint8Array; mediaDisplay: (p: string) => Uint8Array };
  raw.media = () => new Uint8Array([1, 2, 3]);
  raw.mediaDisplay = () => {
    tries++;
    return new Uint8Array([0]);
  };
  assert.equal(doc.mediaUrl("word/media/image1.tif"), "");
  assert.equal(doc.mediaUrl("word/media/image1.tif"), "");
  assert.equal(tries, 1);
  doc.dispose();
});

const FRAG_NS = [
  ["w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"],
  ["r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"],
  ["wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"],
  ["a", "http://schemas.openxmlformats.org/drawingml/2006/main"],
  ["pic", "http://schemas.openxmlformats.org/drawingml/2006/picture"],
];

test("a picture's object URL is not reused for another picture that takes the part name after an undo", async () => {
  const { resolveObjectURL } = await import("node:buffer");
  const doc = Engine.blank("x.docx");
  const first = doc.renderAll().order[0];
  // Paste a picture (its part is added inside the op, so undo takes it out again).
  const body = `<w:p><w:r><w:drawing><wp:inline><wp:extent cx="100" cy="100"/><wp:docPr id="1" name="Picture"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="m1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>`;
  const fragment = { version: 1, ns: FRAG_NS, body, links: {}, media: { m1: { ext: "png", data: "AAAA" } }, origin: "" };
  doc.apply({ op: "insert_fragment", at: { p: first, o: 0 }, fragment } as unknown as Op);
  const name = "word/media/docxtor1.png";
  const pasted = doc.mediaUrl(name);
  assert.ok(pasted);
  assert.equal((await resolveObjectURL(pasted)?.arrayBuffer())?.byteLength, 3);
  // Undo takes the part out; a different picture is added and takes the free name.
  assert.ok(doc.undo());
  assert.equal(doc.addMedia("png", new Uint8Array([1, 2, 3, 4, 5, 6, 7])), name);
  const chosen = doc.mediaUrl(name);
  assert.notEqual(chosen, pasted);
  assert.equal((await resolveObjectURL(chosen)?.arrayBuffer())?.byteLength, 7, "the new picture shows, not the pasted one");
  assert.equal(resolveObjectURL(pasted), undefined, "the stale URL was let go");
  doc.dispose();
});

test("with __DOCXTOR_CHECKS__ the engine reports a rule an edit breaks, as a console error", () => {
  const g = globalThis as { __DOCXTOR_CHECKS__?: boolean };
  const errors: string[] = [];
  const log = console.error;
  console.error = (...a: unknown[]) => void errors.push(a.join(" "));
  try {
    // Off by default: nothing is checked, nothing is reported.
    const plain = Engine.blank();
    plain.apply({ op: "insert_markdown", markdown: "[Up](#nowhere)" } as Op);
    assert.deepEqual(errors, []);
    plain.dispose();
    g.__DOCXTOR_CHECKS__ = true;
    // On, and an edit that breaks nothing: a Markdown link to a bookmark that is not there is its words (it once broke rule E14).
    const eng = Engine.blank();
    eng.apply({ op: "insert_markdown", markdown: "[Up](#nowhere)" } as Op);
    eng.save();
    assert.deepEqual(errors, []);
    // What the engine finds after an edit is reported: a finding planted where the check reads it (no edit through the API breaks a
    // rule any more, which is what this test should not depend on).
    let planted = true;
    (eng.raw as unknown as { takeInvariantViolations(): string }).takeInvariantViolations = () => {
      const out = planted ? JSON.stringify([{ rule: "E14", message: "a link points at bookmark 'x', which does not exist" }]) : "[]";
      planted = false;
      return out;
    };
    eng.apply({ op: "insert_markdown", markdown: "More" } as Op);
    eng.dispose();
  } finally {
    delete g.__DOCXTOR_CHECKS__;
    console.error = log;
  }
  assert.ok(errors.length === 1 && errors[0].startsWith("Docxtor invariant broken: {"), errors.join("\n"));
  assert.match(errors[0], /"rule":"E14"/);
});

test("revision: up by one for each change, and not for a refused or empty one; password changes count", () => {
  const doc = Engine.blank("rev.docx");
  const r0 = doc.revision;
  assert.equal(typeof r0, "number");
  doc.raw.callTool("insert_content", JSON.stringify({ markdown: "Hello" }));
  assert.ok(doc.revision > r0, "an edit raises it");
  const r1 = doc.revision;
  doc.save();
  assert.equal(doc.revision, r1, "saving is not a change");
  assert.throws(() => doc.apply({ op: "insert_text", at: { p: 999999, o: 0 }, text: "x" }));
  assert.equal(doc.revision, r1, "a refused batch is not a change");
  assert.ok(doc.undo());
  assert.equal(doc.revision, r1 + 1, "undo is a change");
  assert.ok(doc.redo());
  assert.equal(doc.revision, r1 + 2);
  doc.setPassword("secret");
  assert.equal(doc.revision, r1 + 3, "a new password is written by the next save");
  doc.dispose();
});

test("a closed document says so on every call", () => {
  const doc = Engine.blank("closed.docx");
  doc.dispose();
  assert.equal(doc.isDisposed(), true);
  assert.throws(() => doc.save(), /This document was closed\./);
  assert.throws(() => doc.undo(), /This document was closed\./);
  assert.throws(() => doc.revision, /This document was closed\./);
  assert.throws(() => doc.apply({ op: "insert_text", at: { p: 1, o: 0 }, text: "x" }), /This document was closed\./);
  doc.dispose(); // closing twice is fine
  doc.releaseUnusedMedia(); // so is the idle sweep arriving late
});

test("fidelity() gives the report of the original against a plain save, before and after an edit", async () => {
  const bytes = new Uint8Array(await readFile(new URL("../../../fixtures/rich.docx", here)));
  const doc = Engine.open(bytes, "rich.docx");
  assert.deepEqual(await doc.fidelity(bytes), fidelityReport(bytes, doc.savePlain()));
  doc.apply({ op: "replace_text", find: "Quarterly", replace: "Annual", all: true });
  const after = await doc.fidelity(bytes);
  assert.deepEqual(after, fidelityReport(bytes, doc.savePlain()));
  doc.dispose();
});

test("saveInSteps writes the bytes save() writes, begins at the revision it was asked at, and lets the pause run between the steps", async () => {
  const bytes = new Uint8Array(await readFile(new URL("../../../fixtures/rich.docx", here)));
  const a = Engine.open(bytes, "rich.docx");
  const b = Engine.open(bytes, "rich.docx");
  for (const d of [a, b]) d.apply({ op: "replace_text", find: "Quarterly", replace: "Annual", all: true });
  const now = new Date("2026-11-21T09:30:00Z");
  let paused = 0;
  const steps = await b.saveInSteps(async () => void paused++, now);
  assert.equal(paused, 1);
  assert.equal(steps.revision, b.revision);
  assert.deepEqual(steps.bytes, a.save(now));
  // A key typed during the pause is in the bytes or not, but the revision says the document is not saved.
  const before = b.revision;
  const later = await b.saveInSteps(async () => void b.apply({ op: "replace_text", find: "Annual", replace: "Yearly", all: true }), now);
  assert.equal(later.revision, before);
  assert.ok(b.revision > later.revision);
  a.dispose();
  b.dispose();
});

test("toDeflate counts only the parts a save compresses: a stored picture is a copy and does not send the save to the worker", async () => {
  const { toDeflate } = await import("../src/zip-client.ts");
  // The packed form: per part a stored flag, the name's length (u32 LE), the name, the data's length (u32 LE), the data.
  const part = (stored: boolean, name: string, size: number) => {
    const n = new TextEncoder().encode(name);
    const out = new Uint8Array(9 + n.length + size);
    const dv = new DataView(out.buffer);
    out[0] = stored ? 1 : 0;
    dv.setUint32(1, n.length, true);
    out.set(n, 5);
    dv.setUint32(5 + n.length, size, true);
    return out;
  };
  const join = (...ps: Uint8Array[]) => {
    const out = new Uint8Array(ps.reduce((t, p) => t + p.length, 0));
    let at = 0;
    for (const p of ps) out.set(p, (at += p.length) - p.length);
    return out;
  };
  assert.equal(toDeflate(join(part(false, "word/document.xml", 1000), part(true, "word/media/image1.png", 5_000_000), part(false, "word/styles.xml", 300))), 1300);
  assert.equal(toDeflate(new Uint8Array(0)), 0);
  // Cut short: everything counts, so a broken buffer never stays on the page for being small.
  const cut = join(part(false, "word/document.xml", 1000)).subarray(0, 500);
  assert.equal(toDeflate(cut), 500);
  // A real document: what the engine packs, read the same way.
  const doc = Engine.open(new Uint8Array(await readFile(new URL("../../../fixtures/rich.docx", here))), "rich.docx");
  const parts: Uint8Array[] = [];
  doc.raw.savePackedPieces(parts);
  const packed = join(...parts);
  const n = toDeflate(packed);
  assert.ok(n > 0 && n <= packed.length, `${n} of ${packed.length}`);
  doc.dispose();
});
