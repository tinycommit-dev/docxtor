// Hostile .docx files for e2e/hostile.spec.ts: script in every place text can hide.
//
// Every payload is built for one slot (`text`, `links`, ...) and says which one it came from. A payload that runs does
// `__pwned.push("slot:where")`, so the test reports the place that ran it. A payload that reaches for the network asks for
// `http://evil.test/<slot>/<where>` (`.test` never resolves), so a request to it names the place too.
//
// One file per slot, and one with all of them (`all`). The files are built here, not committed.
import { zip, pngBytes } from "./docx-builder";
import { A, REL, R, W, XML, esc, evilUrl, run, SLOTS, type Slot } from "./hostile-payloads";
import { textSlot, linksSlot, fieldsSlot, altSlot, imagesSlot, svgSlot } from "./hostile-text";
import { externalSlot, altchunkSlot, notesSlot, stylesSlot, numberingSlot, customxmlSlot, themeSlot, propsSlot } from "./hostile-parts";
import { shapesSlot, coloursSlot } from "./hostile-shapes";
export * from "./hostile-payloads";

export interface Hostile {
  bytes: Uint8Array;
  /** The hyperlink addresses of each kind in the document (what the click test follows). */
  links: { text: string; href: string }[];
  /** A sentence of ordinary text that must show, so the test can tell the document opened. */
  marker: string;
  slots: Slot[];
}

export interface Parts {
  files: [string, string | Uint8Array][];
  rels: string[];
  types: string[];
  rootRels: string[];
}

/** What a picture in the document says about itself. */
export interface PicOptions {
  rid: string;
  name?: string;
  descr?: string;
  title?: string;
  svgRid?: string;
  id: number;
  embed?: boolean;
  link?: string;
}

/** What the slots share while they write the document: the parts so far, the helpers that make markup, and the strings the final parts take. */
export interface Ctx {
  on: Set<Slot>;
  body: string[];
  links: Hostile["links"];
  p: Parts;
  nextRid(): string;
  t(s: string): string;
  para(inner: string, ppr?: string): string;
  addPart(name: string, xml: string | Uint8Array, contentType?: string): void;
  /** The attribute value (escaped) of a hostile colour. */
  cc(id: string): string;
  pic(o: PicOptions): string;
  needPng(file: string): string;
  docPrId: number;
  hdrRef: string;
  settingsExtra: string;
  stylesXml: string;
  documentBackground: string;
  sectionExtra: string;
  coloursHeader: string;
}

/** The document `slots` describe. */
export function hostileDocx(slots: readonly Slot[]): Hostile {
  const on = new Set(slots);
  // A hostile colour, in three shapes by turns: one that adds a CSS declaration (and asks `evil.test` for something), one that
  // leaves its attribute and runs script, and six characters that are not hex digits and carry nothing a scan for addresses would find.
  let colourCount = 0;
  const badColour = (id: string) => {
    const form = colourCount++ % 3;
    if (form === 0) return `FF0000;background:url(${evilUrl("colours", id)})`;
    if (form === 1) return `FF0000" onload="${run(`colours:${id}`)}`;
    return "a;b:c)";
  };
  /** The attribute value (escaped) of a hostile colour, or the colour the document would have without the slot. */
  const cc = (id: string) => esc(badColour(id));
  const body: string[] = [];
  const links: Hostile["links"] = [];
  const p: Parts = { files: [], rels: [], types: [], rootRels: [] };
  let rid = 100;
  const nextRid = () => `rId${rid++}`;
  const marker = "Hostile document opened";
  const t = (s: string) => `<w:r><w:t xml:space="preserve">${esc(s)}</w:t></w:r>`;
  const para = (inner: string, ppr = "") => `<w:p>${ppr ? `<w:pPr>${ppr}</w:pPr>` : ""}${inner}</w:p>`;
  const addPart = (name: string, xml: string | Uint8Array, contentType?: string) => {
    p.files.push([name, xml]);
    if (contentType) p.types.push(`<Override PartName="/${name}" ContentType="${contentType}"/>`);
  };
  body.push(para(t(marker)));

  const pic = (o: PicOptions) =>
    `<w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="914400" cy="914400"/>` +
    `<wp:docPr id="${o.id}" name="${esc(o.name ?? `pic${o.id}`)}" descr="${esc(o.descr ?? "")}" title="${esc(o.title ?? "")}"/>` +
    `<a:graphic xmlns:a="${A}"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">` +
    `<pic:nvPicPr><pic:cNvPr id="0" name="${esc(o.name ?? `pic${o.id}`)}" descr="${esc(o.descr ?? "")}"/><pic:cNvPicPr/></pic:nvPicPr>` +
    `<pic:blipFill><a:blip ${o.link ? `r:link="${o.link}"` : `r:embed="${o.rid}"`}>${o.svgRid ? `<a:extLst><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="${o.svgRid}"/></a:ext></a:extLst>` : ""}</a:blip><a:stretch><a:fillRect/></a:stretch></pic:blipFill>` +
    `<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="914400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`;
  const png = pngBytes(8, 8, [200, 30, 30]);
  const needPng = (file: string) => {
    const id = nextRid();
    addPart(`word/media/${file}`, png);
    p.rels.push(`<Relationship Id="${id}" Type="${REL}/image" Target="media/${esc(file)}"/>`);
    return id;
  };
  const c: Ctx = { on, body, links, p, nextRid, t, para, addPart, cc, pic, needPng, docPrId: 1, hdrRef: "", settingsExtra: "", stylesXml: "", documentBackground: "", sectionExtra: "", coloursHeader: "" };
  textSlot(c);
  linksSlot(c);
  fieldsSlot(c);
  altSlot(c);
  imagesSlot(c);
  svgSlot(c);
  externalSlot(c);
  altchunkSlot(c);
  notesSlot(c);
  stylesSlot(c);
  numberingSlot(c);
  customxmlSlot(c);
  themeSlot(c);
  shapesSlot(c);
  coloursSlot(c);
  propsSlot(c);

  const sect = `<w:sectPr>${c.hdrRef}${c.coloursHeader}<w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>${c.sectionExtra}</w:sectPr>`;
  const document = `${XML}<w:document xmlns:w="${W}" xmlns:r="${R}" xmlns:a="${A}" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office">${c.documentBackground}<w:body>${body.join("")}${sect}</w:body></w:document>`;
  const baseStyles = `<w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>`;
  addPart("word/styles.xml", `${XML}<w:styles xmlns:w="${W}">${baseStyles}${c.stylesXml}</w:styles>`, "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml");
  p.rels.push(`<Relationship Id="rId98" Type="${REL}/styles" Target="styles.xml"/>`);
  addPart("word/settings.xml", `${XML}<w:settings xmlns:w="${W}" xmlns:r="${R}">${c.settingsExtra}</w:settings>`, "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml");
  p.rels.push(`<Relationship Id="rId99" Type="${REL}/settings" Target="settings.xml"/>`);

  const types =
    `${XML}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>` +
    `<Default Extension="png" ContentType="image/png"/><Default Extension="svg" ContentType="image/svg+xml"/>` +
    `<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>${p.types.join("")}</Types>`;
  const files: [string, string | Uint8Array][] = [
    ["[Content_Types].xml", types],
    ["_rels/.rels", `${XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/officeDocument" Target="word/document.xml"/>${p.rootRels.join("")}</Relationships>`],
    ["word/_rels/document.xml.rels", `${XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${p.rels.join("")}</Relationships>`],
    ["word/document.xml", document],
    ...p.files,
  ];
  return { bytes: zip(files), links, marker, slots: [...on] };
}

/** Every file the spec opens: one per slot, then all of them together. */
export function hostileSet(): { name: string; slots: Slot[] }[] {
  return [...SLOTS.map((s) => ({ name: s, slots: [s] as Slot[] })), { name: "all", slots: [...SLOTS] }];
}
