// A small reader for the PDFs Docxtor writes, for the browser tests: objects, page order, the text of each page
// (through the fonts' ToUnicode maps, the way a viewer's copy and search find it) and what the pages hold.
import { inflateSync } from "node:zlib";

export interface PdfObject {
  dict: string;
  stream?: Buffer;
}

export interface PdfFile {
  text: string;
  objects: Map<number, PdfObject>;
  /** Object numbers of the pages, in order. */
  pages: number[];
  /** The info dictionary. */
  info: string;
  /** The text of each page, as a viewer would copy it. */
  pageText: string[];
}

const latin1 = (b: Buffer) => b.toString("latin1");

/** The data of a stream object, decompressed when it is Flate-compressed. */
function streamData(o: PdfObject): Buffer {
  if (!o.stream) return Buffer.alloc(0);
  return /\/FlateDecode/.test(o.dict) ? inflateSync(o.stream) : o.stream;
}

const refOf = (dict: string, key: string): number | undefined => {
  const m = new RegExp(`/${key} (\\d+) 0 R`).exec(dict);
  return m ? Number(m[1]) : undefined;
};

export function readPdf(buf: Buffer): PdfFile {
  const text = latin1(buf);
  const objects = new Map<number, PdfObject>();
  const re = /(?:^|\n)(\d+) 0 obj\n/g;
  for (let m = re.exec(text); m; m = re.exec(text)) {
    const start = m.index + m[0].length;
    const n = Number(m[1]);
    const streamAt = text.indexOf("\nstream\n", start);
    const endAt = text.indexOf("\nendobj\n", start);
    if (streamAt >= 0 && streamAt < endAt) {
      const dict = text.slice(start, streamAt);
      const length = Number(/\/Length (\d+)/.exec(dict)![1]);
      objects.set(n, { dict, stream: buf.subarray(streamAt + 8, streamAt + 8 + length) });
    } else objects.set(n, { dict: text.slice(start, endAt) });
  }
  const trailer = text.slice(text.lastIndexOf("trailer"));
  const root = objects.get(refOf(trailer, "Root")!)!;
  const kids: number[] = [];
  const walk = (n: number) => {
    const o = objects.get(n)!;
    if (/\/Type \/Pages/.test(o.dict)) for (const k of /\/Kids \[([^\]]*)\]/.exec(o.dict)![1].matchAll(/(\d+) 0 R/g)) walk(Number(k[1]));
    else kids.push(n);
  };
  walk(refOf(root.dict, "Pages")!);
  const info = objects.get(refOf(trailer, "Info")!)?.dict ?? "";
  const pdf: PdfFile = { text, objects, pages: kids, info, pageText: [] };
  pdf.pageText = kids.map((p) => pageText(pdf, p));
  return pdf;
}

/** Bytes of a standard font's string (WinAnsi) as text. */
const latin1ToText = (s: string) => new TextDecoder("windows-1252").decode(Buffer.from(s, "latin1"));

/** Glyph number to text, from a ToUnicode map. */
function toUnicode(data: string): Map<string, string> {
  const map = new Map<string, string>();
  for (const m of data.matchAll(/<([0-9A-Fa-f]{4})>\s*<([0-9A-Fa-f]+)>/g)) {
    let s = "";
    for (let i = 0; i < m[2].length; i += 4) s += String.fromCharCode(parseInt(m[2].slice(i, i + 4), 16));
    map.set(m[1].toUpperCase(), s);
  }
  return map;
}

/** The operators of a page's content stream. */
export function pageContent(pdf: PdfFile, page: number): string {
  return latin1(streamData(pdf.objects.get(refOf(pdf.objects.get(page)!.dict, "Contents")!)!));
}

/** One string a page shows, with the font it is shown in and how it is spaced. */
export interface TextRun {
  text: string;
  /** The font's BaseFont name, e.g. `Times-Bold` or `ABCDEF+Caladea-Bold`. */
  font: string;
  /** Character spacing (Tc, points) and horizontal scale (Tz, percent) when it was drawn. */
  tc: number;
  tz: number;
}

/**
 * The text of a page in drawing order, each string with its font. Text drawn as an artifact (a line-end hyphen; in a tagged
 * file the header, footer and page numbers too) is left out, as a screen reader leaves it out, unless `artifacts` asks for it.
 */
export function textRuns(pdf: PdfFile, page: number, opts: { artifacts?: boolean } = {}): TextRun[] {
  const p = pdf.objects.get(page)!;
  const content = pageContent(pdf, page);
  const resources = pdf.objects.get(refOf(p.dict, "Resources")!)!.dict;
  const fonts = new Map<string, { map: Map<string, string> | null; name: string }>();
  for (const m of /\/Font << ([^>]*)>>/.exec(resources)?.[1].matchAll(/\/(\w+) (\d+) 0 R/g) ?? []) {
    const font = pdf.objects.get(Number(m[2]))!.dict;
    const tu = refOf(font, "ToUnicode");
    fonts.set(m[1], { map: tu ? toUnicode(latin1(streamData(pdf.objects.get(tu)!))) : null, name: /\/BaseFont \/([^\s/]+)/.exec(font)?.[1] ?? "" });
  }
  const runs: TextRun[] = [];
  let current = { map: null as Map<string, string> | null, name: "" };
  let tc = 0;
  let tz = 100;
  let artifact = false;
  for (const m of content.matchAll(/\/Artifact BMC|EMC|([\d.-]+) Tc|([\d.-]+) Tz|\/(\w+) [\d.]+ Tf|<([0-9a-fA-F]+)> Tj|\(((?:\\.|[^\\)])*)\) Tj/g)) {
    if (m[0] === "/Artifact BMC") artifact = true;
    else if (m[0] === "EMC") artifact = false;
    else if (m[1] !== undefined) tc = Number(m[1]);
    else if (m[2] !== undefined) tz = Number(m[2]);
    else if (m[3]) current = fonts.get(m[3]) ?? { map: null, name: "" };
    else if (artifact && !opts.artifacts) continue;
    else if (m[4] && current.map) {
      let text = "";
      for (let i = 0; i < m[4].length; i += 4) text += current.map.get(m[4].slice(i, i + 4).toUpperCase()) ?? "";
      runs.push({ text, font: current.name, tc, tz });
    } else if (m[5] !== undefined) runs.push({ text: latin1ToText(m[5].replace(/\\(\d{3}|.)/g, (_, c: string) => (c.length === 3 ? String.fromCharCode(parseInt(c, 8)) : c))), font: current.name, tc, tz });
  }
  return runs;
}

function pageText(pdf: PdfFile, page: number): string {
  return textRuns(pdf, page)
    .map((r) => r.text)
    .join("");
}

/** The size of a page in points. */
export function mediaBox(pdf: PdfFile, page: number): [number, number] {
  const m = /\/MediaBox \[0 0 ([\d.]+) ([\d.]+)\]/.exec(pdf.objects.get(page)!.dict)!;
  return [Number(m[1]), Number(m[2])];
}
