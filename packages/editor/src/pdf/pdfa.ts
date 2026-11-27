// What an archive PDF (PDF/A-2b) needs besides its pages: an output intent with an sRGB colour profile, which says what
// the file's RGB colours mean, and a metadata stream (XMP) that says which standard the file follows and repeats the
// document's details. Both are made here, with no files to bundle. No DOM.

const enc = new TextEncoder();

// -- the colour profile -------------------------------------------------------------------------------------------------

const u32 = (v: number) => [(v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255];
const u16 = (v: number) => [(v >>> 8) & 255, v & 255];
const s15 = (v: number) => u32(Math.round(v * 65536) >>> 0);
const tag = (s: string) => [...s].map((c) => c.charCodeAt(0));

/** sRGB's transfer curve: the light a channel value of 0 to 1 stands for. */
function srgbCurve(x: number): number {
  return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
}

/**
 * An ICC profile for sRGB (version 2, a matrix and curves profile): the colours of the web, which is what Docxtor's
 * pages are drawn in. Its colorants are sRGB's adapted to the profile connection space's white (D50).
 */
export function srgbProfile(): Uint8Array {
  const text = (value: string) => {
    // desc: an ASCII description with empty Unicode and Macintosh parts, as version 2 profiles have it.
    const ascii = [...enc.encode(`${value}\0`)];
    return [...tag("desc"), 0, 0, 0, 0, ...u32(ascii.length), ...ascii, ...new Array(78).fill(0)];
  };
  const copyright = (value: string) => [...tag("text"), 0, 0, 0, 0, ...enc.encode(`${value}\0`)];
  const xyz = (x: number, y: number, z: number) => [...tag("XYZ "), 0, 0, 0, 0, ...s15(x), ...s15(y), ...s15(z)];
  const samples = 1024;
  const curve = [...tag("curv"), 0, 0, 0, 0, ...u32(samples)];
  for (let i = 0; i < samples; i++) curve.push(...u16(Math.round(srgbCurve(i / (samples - 1)) * 65535)));

  const tags: [string, number[]][] = [
    ["cprt", copyright("Public domain")],
    ["desc", text("sRGB IEC61966-2.1")],
    ["wtpt", xyz(0.9642, 1, 0.8249)],
    ["rXYZ", xyz(0.43607, 0.22249, 0.01392)],
    ["gXYZ", xyz(0.38515, 0.71687, 0.09708)],
    ["bXYZ", xyz(0.14307, 0.06061, 0.7141)],
    ["rTRC", curve],
    ["gTRC", curve],
    ["bTRC", curve],
  ];
  // Every tag's data starts on a four-byte boundary; identical curves share one copy.
  const table: number[] = [];
  const data: number[] = [];
  const first = 128 + 4 + tags.length * 12;
  const placed = new Map<string, number>();
  for (const [name, bytes] of tags) {
    const key = bytes.join(",");
    let at = placed.get(key);
    if (at === undefined) {
      at = first + data.length;
      placed.set(key, at);
      data.push(...bytes);
      while (data.length % 4) data.push(0);
    }
    table.push(...tag(name), ...u32(at), ...u32(bytes.length));
  }
  const size = first + data.length;
  const header = [
    ...u32(size),
    ...tag("none"), // preferred CMM
    ...u32(0x02100000), // version 2.1
    ...tag("mntr"), // a display
    ...tag("RGB "),
    ...tag("XYZ "),
    ...u16(2009), ...u16(1), ...u16(1), ...u16(0), ...u16(0), ...u16(0), // a fixed date, so the same bytes come out every time
    ...tag("acsp"),
    ...tag("MSFT"),
    ...u32(0), // flags
    ...u32(0), // manufacturer
    ...u32(0), // model
    ...u32(0), ...u32(0), // attributes
    ...u32(0), // rendering intent: perceptual
    ...s15(0.9642), ...s15(1), ...s15(0.8249), // the illuminant of the profile connection space (D50)
    ...tag("none"), // creator
    ...new Array(16).fill(0), // profile id
    ...new Array(28).fill(0),
  ];
  return new Uint8Array([...header, ...u32(tags.length), ...table, ...data]);
}

// -- the metadata stream ------------------------------------------------------------------------------------------------

/** What a file has to be like before it may say it is PDF/UA-1. */
export interface PdfUaFacts {
  /** Some text is drawn in a standard font, which the file does not hold (PDF/UA wants every font held). */
  usesStandardFonts: boolean;
  /** The headings' levels in reading order. */
  headingLevels: number[];
  /** A picture is in the file as a figure with no description. */
  figureWithoutAlt: boolean;
  /** A table has no header cell at all. */
  headerlessTable: boolean;
}

/**
 * Whether the file may claim PDF/UA-1: a checker (PAC, veraPDF) trusts the claim, so it is made only when the basics hold:
 * every font held, no heading level skipped (the first is level 1), every figure described, every table headed.
 * A tagged file that fails is still tagged, it only makes no claim.
 */
export function meetsPdfUa(f: PdfUaFacts): boolean {
  if (f.usesStandardFonts || f.figureWithoutAlt || f.headerlessTable) return false;
  let level = 0;
  for (const l of f.headingLevels) {
    const now = Math.min(Math.max(l, 1), 6);
    if (now > level + 1) return false;
    level = now;
  }
  return true;
}

export interface XmpInfo {
  title: string;
  author?: string;
  subject?: string;
  keywords?: string;
  creatorTool: string;
  producer: string;
  created: Date;
  modified: Date;
  /** The document's language, for the title and description. */
  lang?: string;
  /** Says the file follows PDF/A-2b. */
  pdfa: boolean;
  /** Says the file is a tagged, accessible PDF (PDF/UA-1). */
  pdfua: boolean;
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** A date as XMP writes it: `2026-10-14T09:30:00Z`. */
export const xmpDate = (d: Date): string => d.toISOString().replace(/\.\d+Z$/, "Z");

/** The XMP packet for the file: its details, and the standards it says it follows. */
export function xmpPacket(i: XmpInfo): string {
  const alt = (text: string) => `<rdf:Alt><rdf:li xml:lang="x-default">${esc(text)}</rdf:li></rdf:Alt>`;
  const blocks: string[] = [];
  blocks.push(
    `<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">` +
      `<dc:format>application/pdf</dc:format><dc:title>${alt(i.title)}</dc:title>` +
      `${i.author ? `<dc:creator><rdf:Seq><rdf:li>${esc(i.author)}</rdf:li></rdf:Seq></dc:creator>` : ""}` +
      `${i.subject ? `<dc:description>${alt(i.subject)}</dc:description>` : ""}` +
      `</rdf:Description>`,
  );
  blocks.push(
    `<rdf:Description rdf:about="" xmlns:pdf="http://ns.adobe.com/pdf/1.3/"><pdf:Producer>${esc(i.producer)}</pdf:Producer>${i.keywords ? `<pdf:Keywords>${esc(i.keywords)}</pdf:Keywords>` : ""}</rdf:Description>`,
  );
  blocks.push(
    `<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/"><xmp:CreatorTool>${esc(i.creatorTool)}</xmp:CreatorTool>` +
      `<xmp:CreateDate>${xmpDate(i.created)}</xmp:CreateDate><xmp:ModifyDate>${xmpDate(i.modified)}</xmp:ModifyDate><xmp:MetadataDate>${xmpDate(i.modified)}</xmp:MetadataDate></rdf:Description>`,
  );
  if (i.pdfa) blocks.push(`<rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"><pdfaid:part>2</pdfaid:part><pdfaid:conformance>B</pdfaid:conformance></rdf:Description>`);
  if (i.pdfua) blocks.push(`<rdf:Description rdf:about="" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/"><pdfuaid:part>1</pdfuaid:part></rdf:Description>`);
  // PDF/A wants every property in a file's metadata to be one its standard knows or the file describes: the PDF/UA
  // identification is described here.
  if (i.pdfa && i.pdfua) {
    blocks.push(
      `<rdf:Description rdf:about="" xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/" xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#" xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#">` +
        `<pdfaExtension:schemas><rdf:Bag><rdf:li rdf:parseType="Resource">` +
        `<pdfaSchema:schema>PDF/UA identification schema</pdfaSchema:schema>` +
        `<pdfaSchema:namespaceURI>http://www.aiim.org/pdfua/ns/id/</pdfaSchema:namespaceURI>` +
        `<pdfaSchema:prefix>pdfuaid</pdfaSchema:prefix>` +
        `<pdfaSchema:property><rdf:Seq><rdf:li rdf:parseType="Resource">` +
        `<pdfaProperty:name>part</pdfaProperty:name><pdfaProperty:valueType>Integer</pdfaProperty:valueType><pdfaProperty:category>internal</pdfaProperty:category>` +
        `<pdfaProperty:description>Indicates, which part of ISO 14289 standard is followed</pdfaProperty:description>` +
        `</rdf:li></rdf:Seq></pdfaSchema:property>` +
        `</rdf:li></rdf:Bag></pdfaExtension:schemas></rdf:Description>`,
    );
  }
  return (
    `<?xpacket begin="\u{FEFF}" id="W5M0MpCehiHzreSzNTczkc9d"?>\n<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">` +
    `${blocks.join("")}</rdf:RDF></x:xmpmeta>\n<?xpacket end="w"?>`
  );
}
