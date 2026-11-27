// A tagged PDF's structure tree and marked content, the archive pieces (colour profile, metadata) and how the page stream
// is grouped: what a screen reader and a PDF/A checker read. No DOM; the tree is built by hand the way the page walker builds it.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { meetsPdfUa, srgbProfile, xmpPacket } from "../src/pdf/pdfa.ts";
import { pageStream, StructTree, type Item } from "../src/pdf/structure.ts";
import { numbering } from "../src/pdf/tagmap.ts";
import { PdfWriter } from "../src/pdf/writer.ts";

const text = (b: Uint8Array) => new TextDecoder("latin1").decode(b);

/** Objects of a written file by number, as text. */
function objects(bytes: Uint8Array): Map<number, string> {
  const out = new Map<number, string>();
  const src = text(bytes);
  for (const m of src.matchAll(/(?<![^\n])(\d+) 0 obj\n([\s\S]*?)\nendobj\n/g)) out.set(Number(m[1]), m[2]);
  return out;
}

const refs = (s: string) => [...s.matchAll(/(\d+) 0 R/g)].map((m) => Number(m[1]));

test("a tree of a heading, a paragraph, a list, a table and a figure is written as StructElem objects", () => {
  const w = new PdfWriter();
  const pages = [w.alloc(), w.alloc()];
  const tree = new StructTree(2);
  const h1 = tree.node(tree.root, "H1");
  tree.mark(h1, 0);
  const p = tree.node(tree.root, "P", { lang: "fr-FR" });
  tree.mark(p, 0);
  // A paragraph that runs over to the next page has marked content on both pages.
  tree.mark(p, 1);
  const list = tree.node(tree.root, "L", { listNumbering: "Decimal" });
  const item = tree.node(list, "LI");
  tree.mark(tree.node(item, "Lbl"), 1);
  tree.mark(tree.node(item, "LBody"), 1);
  const table = tree.node(tree.root, "Table");
  const head = tree.node(table, "TR");
  tree.mark(tree.node(head, "TH", { scope: "Column" }), 1);
  tree.mark(tree.node(head, "TH", { scope: "Column", colSpan: 2 }), 1);
  const row = tree.node(table, "TR");
  tree.node(row, "TD");
  tree.mark(tree.node(row, "TD"), 1);
  tree.mark(tree.node(tree.root, "Figure", { alt: "A chart of sales" }), 1);
  const root = tree.write(w, pages);
  for (const n of pages) w.put(n, "<< /Type /Page >>");
  const o = objects(w.build(root));

  const elems = [...o].filter(([, b]) => b.includes("/Type /StructElem"));
  const byType = (type: string) => elems.filter(([, b]) => new RegExp(`/S /${type} `).test(b));
  assert.equal(byType("Document").length, 1);
  assert.equal(byType("H1").length, 1);
  assert.equal(byType("P").length, 1);
  assert.equal(byType("TH").length, 2);
  assert.equal(byType("TD").length, 2);
  // The figure carries its description; the paragraph its language; the header cells their scope and span.
  assert.match(byType("Figure")[0]![1], /\/Alt \(A chart of sales\)/);
  assert.match(byType("P")[0]![1], /\/Lang \(fr-FR\)/);
  assert.match(byType("TH")[0]![1], /\/A << \/O \/Table \/Scope \/Column >>/);
  assert.match(byType("TH")[1]![1], /\/ColSpan 2/);
  assert.match(byType("L")[0]![1], /\/A << \/O \/List \/ListNumbering \/Decimal >>/);
  // The root names the document element, and the parent tree numbers the pages from 0.
  const rootBody = o.get(root)!;
  assert.match(rootBody, /\/Type \/StructTreeRoot \/K \[\d+ 0 R\] \/ParentTree \d+ 0 R \/ParentTreeNextKey 2/);
  // Reading order: a heading, a paragraph, a list, a table, a figure.
  const doc = byType("Document")[0]![1];
  assert.deepEqual(refs(doc.slice(doc.indexOf("/K"))).map((n) => /\/S \/(\w+) /.exec(o.get(n)!)![1]), ["H1", "P", "L", "Table", "Figure"]);
  // A paragraph with marked content on two pages names its first page; the other is an MCR.
  const pBody = byType("P")[0]![1];
  assert.match(pBody, /\/Pg 1 0 R/);
  assert.match(pBody, /\/K \[1 << \/Type \/MCR \/Pg 2 0 R \/MCID 0 >>\]/);
  // The table's empty first cell is still there, so the grid is whole.
  assert.equal(byType("TR").length, 2);
});

test("the parent tree goes from each page's marked content back to its element, and an element without content is left out", () => {
  const w = new PdfWriter();
  const page = w.alloc();
  const tree = new StructTree(1);
  const a = tree.node(tree.root, "P");
  const empty = tree.node(tree.root, "P");
  const b = tree.node(tree.root, "H2");
  void empty;
  assert.equal(tree.mark(a, 0), 0);
  assert.equal(tree.mark(b, 0), 1);
  assert.equal(tree.mark(a, 0), 2);
  const root = tree.write(w, [page]);
  w.put(page, "<< /Type /Page >>");
  const o = objects(w.build(root));
  const types = [...o.values()].filter((b) => b.includes("/StructElem")).map((b) => /\/S \/(\w+)/.exec(b)![1]);
  assert.deepEqual(types.sort(), ["Document", "H2", "P"]);
  // Parent tree: page 0's marked content 0, 1, 2 belong to the first P, the H2 and the first P.
  const pt = [...o.values()].find((b) => b.startsWith("<< /Nums"))!;
  const entry = /^<< \/Nums \[0 \[(\d+) 0 R (\d+) 0 R (\d+) 0 R\]\] >>$/.exec(pt)!;
  assert.equal(entry[1], entry[3]);
  assert.notEqual(entry[1], entry[2]);
  assert.match(o.get(Number(entry[2]))!, /\/S \/H2/);
});

test("a link annotation is the content of its Link element and the parent tree finds the element from the annotation", () => {
  const w = new PdfWriter();
  const page = w.alloc();
  const annot = w.alloc();
  const tree = new StructTree(1);
  const para = tree.node(tree.root, "P");
  const link = tree.node(para, "Link", { alt: "Example" });
  tree.mark(link, 0);
  const key = tree.annotate(link, 0, annot);
  assert.equal(key, 1);
  const root = tree.write(w, [page]);
  w.put(page, "<< /Type /Page >>");
  w.put(annot, "<< /Type /Annot /Subtype /Link >>");
  const o = objects(w.build(root));
  const linkBody = [...o.values()].find((b) => /\/S \/Link /.test(b))!;
  assert.match(linkBody, /\/Alt \(Example\)/);
  assert.match(linkBody, new RegExp(`<< /Type /OBJR /Pg ${page} 0 R /Obj ${annot} 0 R >>`));
  const pt = [...o.values()].find((b) => b.startsWith("<< /Nums"))!;
  assert.match(pt, new RegExp(`^<< /Nums \\[0 \\[\\d+ 0 R\\] 1 \\d+ 0 R\\] >>$`));
  assert.match(o.get(root)!, /\/ParentTreeNextKey 2/);
});

test("footnotes come after the text, whatever order they were met in", () => {
  const w = new PdfWriter();
  const page = w.alloc();
  const tree = new StructTree(1);
  tree.mark(tree.node(tree.notes(), "P"), 0);
  tree.mark(tree.node(tree.root, "P"), 0);
  const root = tree.write(w, [page]);
  w.put(page, "<< /Type /Page >>");
  const o = objects(w.build(root));
  const doc = [...o.values()].find((b) => /\/S \/Document /.test(b))!;
  assert.deepEqual(refs(doc.slice(doc.indexOf("/K"))).map((n) => /\/S \/(\w+) /.exec(o.get(n)!)![1]), ["P", "Sect"]);
});

test("a page's stream wraps runs for one element in a marked-content sequence, artifacts as artifacts, and leaves state operators out", () => {
  const w = new PdfWriter();
  const tree = new StructTree(1);
  const p = tree.node(tree.root, "P");
  const q = tree.node(tree.root, "H1");
  const items: Item[] = [
    { s: "0 0 1 1 re f" }, // a background
    { s: "BT (a) Tj ET", tag: p },
    { s: "BT (b) Tj ET", tag: p },
    { s: "q 1 0 0 1 5 5 cm", raw: true },
    { s: "BT (c) Tj ET", tag: q },
    { s: "Q", raw: true },
    { s: "BT (d) Tj ET", tag: p },
  ];
  const stream = pageStream(items, 0, tree);
  assert.equal(
    stream,
    ["/Artifact BMC", "0 0 1 1 re f", "EMC", "/P << /MCID 0 >> BDC", "BT (a) Tj ET", "BT (b) Tj ET", "EMC", "q 1 0 0 1 5 5 cm", "/H1 << /MCID 1 >> BDC", "BT (c) Tj ET", "EMC", "Q", "/P << /MCID 2 >> BDC", "BT (d) Tj ET", "EMC"].join("\n"),
  );
  // Every sequence is closed, and the numbers run from 0 without gaps.
  assert.equal([...stream.matchAll(/ BDC|BMC/g)].length, [...stream.matchAll(/^EMC$/gm)].length);
  // Without a tree it is the operators as they were, except text that is always an artifact.
  assert.equal(pageStream([{ s: "a" }, { s: "b", artifact: true }, { s: "c", tag: p }], 0, null), "a\n/Artifact BMC b EMC\nc");
});

test("a list's label tells how it counts", () => {
  assert.equal(numbering("1."), "Decimal");
  assert.equal(numbering("(12)"), "Decimal");
  assert.equal(numbering("iv."), "LowerRoman");
  assert.equal(numbering("IV)"), "UpperRoman");
  assert.equal(numbering("b."), "LowerAlpha");
  assert.equal(numbering("B."), "UpperAlpha");
  assert.equal(numbering("•"), "Disc");
  assert.equal(numbering("☐", true), "None");
  assert.equal(numbering(""), undefined);
  assert.equal(numbering("Step"), undefined);
});

test("the sRGB profile is a valid ICC profile with a matrix and curves for RGB", () => {
  const p = srgbProfile();
  const dv = new DataView(p.buffer, p.byteOffset, p.byteLength);
  const sig = (at: number) => String.fromCharCode(...p.slice(at, at + 4));
  assert.equal(dv.getUint32(0), p.length, "the size in the header is the size");
  assert.equal(p.length % 4, 0);
  assert.equal(sig(12), "mntr");
  assert.equal(sig(16), "RGB ");
  assert.equal(sig(20), "XYZ ");
  assert.equal(sig(36), "acsp");
  assert.equal(dv.getUint32(8) >>> 24, 2, "version 2");
  const count = dv.getUint32(128);
  const tags = new Map<string, { at: number; size: number }>();
  for (let i = 0; i < count; i++) {
    const at = 132 + i * 12;
    tags.set(sig(at), { at: dv.getUint32(at + 4), size: dv.getUint32(at + 8) });
  }
  for (const t of ["desc", "cprt", "wtpt", "rXYZ", "gXYZ", "bXYZ", "rTRC", "gTRC", "bTRC"]) assert.ok(tags.has(t), t);
  for (const { at, size } of tags.values()) {
    assert.equal(at % 4, 0);
    assert.ok(at + size <= p.length);
  }
  // The white point is D50; the red colorant's Y is sRGB's 0.2225.
  const xyz = (name: string) => [8, 12, 16].map((o) => dv.getInt32(tags.get(name)!.at + o) / 65536);
  assert.ok(Math.abs(xyz("wtpt")[0]! - 0.9642) < 0.0002 && xyz("wtpt")[1] === 1);
  assert.ok(Math.abs(xyz("rXYZ")[1]! - 0.22249) < 0.0002);
  // The curve rises from 0 to 1 and follows sRGB: 0.5 gives about 0.214.
  const curve = tags.get("rTRC")!;
  const n = dv.getUint32(curve.at + 8);
  assert.equal(n, 1024);
  const y = (i: number) => dv.getUint16(curve.at + 12 + i * 2) / 65535;
  assert.equal(y(0), 0);
  assert.equal(y(n - 1), 1);
  assert.ok(Math.abs(y(Math.round((n - 1) / 2)) - 0.214) < 0.002);
  // The same bytes come out every time, and the three curves are one copy.
  assert.deepEqual(srgbProfile(), p);
  assert.equal(tags.get("rTRC")!.at, tags.get("gTRC")!.at);
});

test("a file claims PDF/UA only when the basics hold", () => {
  const ok = { usesStandardFonts: false, headingLevels: [1, 2, 2, 3, 1], figureWithoutAlt: false, headerlessTable: false };
  assert.equal(meetsPdfUa(ok), true);
  assert.equal(meetsPdfUa({ ...ok, headingLevels: [] }), true);
  // A font the file does not hold, a picture with no description, a table with no header cell.
  assert.equal(meetsPdfUa({ ...ok, usesStandardFonts: true }), false);
  assert.equal(meetsPdfUa({ ...ok, figureWithoutAlt: true }), false);
  assert.equal(meetsPdfUa({ ...ok, headerlessTable: true }), false);
  // A heading level skipped: the first heading is level 2, or level 1 goes to level 3. Going back up is fine.
  assert.equal(meetsPdfUa({ ...ok, headingLevels: [2] }), false);
  assert.equal(meetsPdfUa({ ...ok, headingLevels: [1, 3] }), false);
  assert.equal(meetsPdfUa({ ...ok, headingLevels: [1, 2, 3, 1, 2] }), true);
  // Levels past 6 are written as 6.
  assert.equal(meetsPdfUa({ ...ok, headingLevels: [1, 2, 3, 4, 5, 6, 7, 8] }), true);
});

test("the XMP packet repeats the document's details and names the standards", () => {
  const info = { title: "Plan & <Budget>", author: "Ana", subject: "Money", keywords: "a, b", creatorTool: "Docxtor", producer: "Docxtor", created: new Date("2026-10-14T09:30:00Z"), modified: new Date("2026-10-14T09:31:02.5Z"), pdfa: true, pdfua: true };
  const xmp = xmpPacket(info);
  assert.ok(xmp.startsWith('<?xpacket begin="\u{FEFF}" id="W5M0MpCehiHzreSzNTczkc9d"?>'));
  assert.ok(xmp.trimEnd().endsWith('<?xpacket end="w"?>'));
  assert.match(xmp, /<dc:title><rdf:Alt><rdf:li xml:lang="x-default">Plan &amp; &lt;Budget&gt;<\/rdf:li><\/rdf:Alt><\/dc:title>/);
  assert.match(xmp, /<dc:creator><rdf:Seq><rdf:li>Ana<\/rdf:li><\/rdf:Seq><\/dc:creator>/);
  assert.match(xmp, /<pdf:Keywords>a, b<\/pdf:Keywords>/);
  assert.match(xmp, /<xmp:CreateDate>2026-10-14T09:30:00Z<\/xmp:CreateDate>/);
  assert.match(xmp, /<xmp:ModifyDate>2026-10-14T09:31:02Z<\/xmp:ModifyDate>/);
  assert.match(xmp, /<pdfaid:part>2<\/pdfaid:part><pdfaid:conformance>B<\/pdfaid:conformance>/);
  assert.match(xmp, /<pdfuaid:part>1<\/pdfuaid:part>/);
  // PDF/A wants the PDF/UA property described.
  assert.match(xmp, /<pdfaSchema:prefix>pdfuaid<\/pdfaSchema:prefix>/);
  // A file that is tagged but not an archive names only the first; one that is neither has neither.
  assert.doesNotMatch(xmpPacket({ ...info, pdfa: false }), /pdfaid|pdfaExtension/);
  assert.match(xmpPacket({ ...info, pdfa: false }), /pdfuaid:part/);
  assert.doesNotMatch(xmpPacket({ ...info, pdfa: false, pdfua: false }), /pdfuaid/);
});
