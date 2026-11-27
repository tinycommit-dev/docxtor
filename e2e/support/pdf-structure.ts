// Reads the structure of a tagged PDF Docxtor wrote, for the browser tests: the structure tree with each element's type,
// description and text (through the marked content on the pages), and the basic checks a PDF/UA validator starts with.
import { inflateSync } from "node:zlib";
import { pageContent, type PdfFile } from "./pdf-reader";

export interface Elem {
  obj: number;
  type: string;
  alt?: string;
  lang?: string;
  scope?: string;
  colSpan?: number;
  listNumbering?: string;
  /** Child elements, marked content and annotations, in reading order. */
  kids: (Elem | { mcid: number; page: number } | { annot: number; page: number })[];
}

const latin1 = (b: Buffer) => b.toString("latin1");
const refOf = (dict: string, key: string): number | undefined => {
  const m = new RegExp(`/${key} (\\d+) 0 R`).exec(dict);
  return m ? Number(m[1]) : undefined;
};
const streamText = (pdf: PdfFile, obj: number): string => {
  const o = pdf.objects.get(obj)!;
  return o.stream ? latin1(/\/FlateDecode/.test(o.dict) ? inflateSync(o.stream) : o.stream) : "";
};

/** A string object (ASCII or UTF-16 with a byte order mark) as text. */
function unstring(s: string): string {
  if (s.startsWith("<")) {
    const hex = s.slice(1, -1);
    const bytes = Buffer.from(hex, "hex");
    return bytes.subarray(2).swap16().toString("utf16le");
  }
  return s.slice(1, -1).replace(/\\(.)/g, "$1");
}

/** The structure tree root, if the file has one: the document element and everything below it. */
export function structTree(pdf: PdfFile): Elem | null {
  const trailer = pdf.text.slice(pdf.text.lastIndexOf("trailer"));
  const catalog = pdf.objects.get(refOf(trailer, "Root")!)!;
  const rootObj = refOf(catalog.dict, "StructTreeRoot");
  if (rootObj === undefined) return null;
  const pageIndex = new Map(pdf.pages.map((p, i) => [p, i] as const));
  const read = (obj: number, inherited: number | undefined): Elem => {
    const d = pdf.objects.get(obj)!.dict;
    const pg = refOf(d, "Pg") ?? inherited;
    const e: Elem = { obj, type: /\/S \/(\w+)/.exec(d)![1]!, kids: [] };
    const alt = /\/Alt (\((?:\\.|[^\\)])*\)|<[0-9A-Fa-f]*>)/.exec(d);
    if (alt) e.alt = unstring(alt[1]!);
    const lang = /\/Lang (\((?:\\.|[^\\)])*\))/.exec(d);
    if (lang) e.lang = unstring(lang[1]!);
    e.scope = /\/Scope \/(\w+)/.exec(d)?.[1];
    const span = /\/ColSpan (\d+)/.exec(d);
    if (span) e.colSpan = Number(span[1]);
    e.listNumbering = /\/ListNumbering \/(\w+)/.exec(d)?.[1];
    const k = /\/K \[([\s\S]*)\]/.exec(d)?.[1] ?? "";
    // Entries: an element (`n 0 R`), a marked-content number on /Pg, an MCR or an OBJR.
    const re = /<< \/Type \/MCR \/Pg (\d+) 0 R \/MCID (\d+) >>|<< \/Type \/OBJR \/Pg (\d+) 0 R \/Obj (\d+) 0 R >>|(\d+) 0 R|(\d+)/g;
    for (let m = re.exec(k); m; m = re.exec(k)) {
      if (m[1] !== undefined) e.kids.push({ mcid: Number(m[2]), page: pageIndex.get(Number(m[1]))! });
      else if (m[3] !== undefined) e.kids.push({ annot: Number(m[4]), page: pageIndex.get(Number(m[3]))! });
      else if (m[5] !== undefined) e.kids.push(read(Number(m[5]), pg));
      else e.kids.push({ mcid: Number(m[6]), page: pageIndex.get(pg!)! });
    }
    return e;
  };
  const rootDict = pdf.objects.get(rootObj)!.dict;
  return read(Number(/(\d+) 0 R/.exec(/\/K \[([^\]]*)\]/.exec(rootDict)![1]!)![1]), undefined);
}

/** Every element, depth first. */
export function walk(e: Elem, visit: (e: Elem, depth: number) => void, depth = 0) {
  visit(e, depth);
  for (const k of e.kids) if ("type" in k) walk(k, visit, depth + 1);
}

export const find = (root: Elem, type: string): Elem[] => {
  const out: Elem[] = [];
  walk(root, (e) => e.type === type && out.push(e));
  return out;
};

/** The element types in reading order, with the depth: `P`, `L`, `  LI`... */
export function outline(root: Elem): string[] {
  const out: string[] = [];
  walk(root, (e, d) => out.push(`${"  ".repeat(d)}${e.type}`));
  return out;
}

interface Sequence {
  tag: string;
  mcid?: number;
  text: string;
  /** Operators that draw (text, path painting, pictures) outside any marked content. */
  bare: number;
}

/** What a page's content stream marks: each sequence's tag, number and text, and drawing outside any sequence. */
export function marked(pdf: PdfFile, page: number): { sequences: Sequence[]; bare: string[]; unbalanced: boolean } {
  const content = pageContent(pdf, page);
  const resources = pdf.objects.get(refOf(pdf.objects.get(page)!.dict, "Resources")!)!.dict;
  const fonts = new Map<string, Map<string, string> | null>();
  for (const m of /\/Font << ([^>]*)>>/.exec(resources)?.[1].matchAll(/\/(\w+) (\d+) 0 R/g) ?? []) {
    const font = pdf.objects.get(Number(m[2]))!.dict;
    const tu = refOf(font, "ToUnicode");
    const map = new Map<string, string>();
    if (tu) for (const x of streamText(pdf, tu).matchAll(/<([0-9A-Fa-f]{4})>\s*<([0-9A-Fa-f]+)>/g)) map.set(x[1]!.toUpperCase(), String.fromCharCode(...(x[2]!.match(/.{4}/g) ?? []).map((h) => parseInt(h, 16))));
    fonts.set(m[1]!, tu ? map : null);
  }
  const sequences: Sequence[] = [];
  const stack: Sequence[] = [];
  const bare: string[] = [];
  let font: Map<string, string> | null = null;
  const draws = /\/(\w+) << \/MCID (\d+) >> BDC|\/(\w+) BMC|\bEMC\b|\/(\w+) [\d.]+ Tf|<([0-9A-Fa-f]+)> Tj|\(((?:\\.|[^\\)])*)\) Tj|(?:^|\s)(f\*?|S|s|B|b|sh)(?=\s|$)|\/(\w+) Do/gm;
  for (let m = draws.exec(content); m; m = draws.exec(content)) {
    if (m[1] !== undefined) {
      const s: Sequence = { tag: m[1], mcid: Number(m[2]), text: "", bare: 0 };
      sequences.push(s);
      stack.push(s);
    } else if (m[3] !== undefined) {
      const s: Sequence = { tag: m[3], text: "", bare: 0 };
      sequences.push(s);
      stack.push(s);
    } else if (m[0] === "EMC") stack.pop();
    else if (m[4] !== undefined) font = fonts.get(m[4]) ?? null;
    else {
      const top = stack[stack.length - 1];
      if (!top) bare.push(m[0].trim());
      else if (m[5] !== undefined && font) for (let i = 0; i < m[5].length; i += 4) top.text += font.get(m[5].slice(i, i + 4).toUpperCase()) ?? "";
      else if (m[6] !== undefined) top.text += m[6];
    }
  }
  return { sequences, bare, unbalanced: stack.length > 0 };
}

/** The text drawn in the marked content of an element and everything below it. */
export function textOf(pdf: PdfFile, e: Elem, cache = new Map<number, ReturnType<typeof marked>>()): string {
  let out = "";
  for (const k of e.kids) {
    if ("type" in k) out += textOf(pdf, k, cache);
    else if ("mcid" in k) {
      const page = pdf.pages[k.page]!;
      let m = cache.get(page);
      if (!m) cache.set(page, (m = marked(pdf, page)));
      out += m.sequences.find((s) => s.mcid === k.mcid)?.text ?? "";
    }
  }
  return out;
}

/**
 * The checks a PDF/UA validator starts with, as a list of problems (empty: none): the file says it is tagged, names its
 * language and title, every page is ordered and tied to the tree, everything drawn is marked content or an artifact, every
 * number is used once and refers back to its element, figures have descriptions, links are in the tree, fonts are held
 * in the file, and tables and lists have the elements they should.
 */
export function pdfUaIssues(pdf: PdfFile): string[] {
  const issues: string[] = [];
  const trailer = pdf.text.slice(pdf.text.lastIndexOf("trailer"));
  const catalog = pdf.objects.get(refOf(trailer, "Root")!)!.dict;
  if (!/\/MarkInfo << \/Marked true >>/.test(catalog)) issues.push("no MarkInfo with Marked true");
  if (!/\/Lang \(/.test(catalog)) issues.push("no document language");
  if (!/\/DisplayDocTitle true/.test(catalog)) issues.push("DisplayDocTitle is not true");
  const info = pdf.info;
  if (!/\/Title \((?:\\.|[^\\)])+\)|\/Title <[0-9A-F]+>/.test(info)) issues.push("no title");
  const tree = structTree(pdf);
  if (!tree) return [...issues, "no structure tree"];
  if (tree.type !== "Document") issues.push(`the root element is ${tree.type}, not Document`);
  const metadata = refOf(catalog, "Metadata");
  if (metadata === undefined || !/<pdfuaid:part>1<\/pdfuaid:part>/.test(streamText(pdf, metadata))) issues.push("the metadata does not identify PDF/UA");

  // Pages: ordered, tied to the tree; content marked.
  const used = new Map<string, number>();
  walk(tree, (e) => {
    for (const k of e.kids) if ("mcid" in k) used.set(`${k.page}:${k.mcid}`, (used.get(`${k.page}:${k.mcid}`) ?? 0) + 1);
  });
  pdf.pages.forEach((page, i) => {
    const dict = pdf.objects.get(page)!.dict;
    if (!/\/StructParents \d+/.test(dict)) issues.push(`page ${i + 1} has no StructParents`);
    if (!/\/Tabs \/S/.test(dict)) issues.push(`page ${i + 1} has no tab order`);
    const m = marked(pdf, page);
    if (m.unbalanced) issues.push(`page ${i + 1} has an unclosed marked-content sequence`);
    if (m.bare.length) issues.push(`page ${i + 1} draws outside marked content: ${[...new Set(m.bare)].join(" ")}`);
    const numbers = m.sequences.filter((s) => s.mcid !== undefined).map((s) => s.mcid!);
    if (new Set(numbers).size !== numbers.length) issues.push(`page ${i + 1} reuses a marked-content number`);
    numbers.forEach((n, at) => {
      if (n !== at) issues.push(`page ${i + 1} numbers its marked content out of order (${n} at ${at})`);
      if (used.get(`${i}:${n}`) !== 1) issues.push(`page ${i + 1}: marked content ${n} is in the tree ${used.get(`${i}:${n}`) ?? 0} times`);
    });
  });

  // The elements.
  walk(tree, (e) => {
    if (e.type === "Figure" && !e.alt) issues.push("a figure has no description");
    if (e.type === "Table") {
      for (const k of e.kids) if ("type" in k && k.type !== "TR") issues.push(`a table holds a ${k.type}`);
    }
    if (e.type === "TR") for (const k of e.kids) if ("type" in k && k.type !== "TH" && k.type !== "TD") issues.push(`a table row holds a ${k.type}`);
    if (e.type === "L") for (const k of e.kids) if ("type" in k && k.type !== "LI") issues.push(`a list holds a ${k.type}`);
    if (e.type === "LI") for (const k of e.kids) if ("type" in k && k.type !== "Lbl" && k.type !== "LBody") issues.push(`a list item holds a ${k.type}`);
    if (e.type === "Link" && !e.kids.some((k) => "annot" in k)) issues.push("a link has no annotation");
  });

  // Annotations are links with a description and a place in the tree; every font is held in the file.
  for (const [n, o] of pdf.objects) {
    if (/\/Subtype \/Link/.test(o.dict)) {
      if (!/\/Contents \(/.test(o.dict) && !/\/Contents </.test(o.dict)) issues.push(`link ${n} has no Contents`);
      if (!/\/StructParent \d+/.test(o.dict)) issues.push(`link ${n} has no StructParent`);
    }
    if (/\/Type \/Font\b/.test(o.dict) && /\/Subtype \/Type1\b/.test(o.dict)) issues.push(`font ${n} is not held in the file`);
    if (/\/Type \/FontDescriptor/.test(o.dict) && !/\/FontFile\d?/.test(o.dict)) issues.push(`font ${n} has no font file`);
  }
  return issues;
}
