// The document spec and buildDocx: sections, headers and footers, and opening the result in the editor.
import { O, R, REL, V, W, W10, esc, zip } from "./docx-zip";
import type { Page } from "../helpers";
import { documentReady, expect } from "../helpers";

export interface HfSpec {
  default?: string;
  first?: string;
  even?: string;
  /** A text watermark made the way Word makes one (a WordArt shape in the header), put in the header named by `default`, `first` and `even` that exist. */
  watermark?: { text: string; color?: string; opacity?: number; angle?: number; font?: string };
  /** Runs written out as Word's markup (a shape, say), put at the start of the first paragraph of each header or footer named here. */
  raw?: string;
}

export interface SectionSpec {
  /** Paper in twips, portrait or landscape as written (width first). Letter portrait by default. */
  size?: [number, number];
  /** top, right, bottom, left (twips). One inch each by default. */
  margins?: [number, number, number, number];
  /** Distance of the header and footer from the paper edge (twips). */
  header?: number;
  footer?: number;
  /** How this section starts. Ignored for the first. */
  start?: "nextPage" | "continuous" | "evenPage" | "oddPage";
  titlePage?: boolean;
  /** Extra margin for binding (twips), on the left, or on the right with `rtlGutter`. */
  gutter?: number;
  rtlGutter?: boolean;
  /** Header and footer parts; leave a section's out to inherit the previous section's, as Word does. */
  headers?: HfSpec;
  footers?: HfSpec;
  /** Page numbering of the section: the format (`lowerRoman`, `upperRoman`, `lowerLetter`, `upperLetter`) and the number its first page restarts at. */
  pageNumbers?: { fmt?: string; start?: number };
  /** Columns: how many, the gap between them (twips; half an inch by default) and whether a line is drawn between. */
  cols?: { num: number; space?: number; sep?: boolean };
  /**
   * More of the section's properties as Word's XML, in schema order: `notes` (footnotePr, endnotePr) goes before the
   * page size, `afterMar` (pgBorders, lnNumType) after the margins and `end` (vAlign, textDirection...) after the columns.
   */
  xml?: { notes?: string; afterMar?: string; end?: string };
  /** Columns written out as Word's XML (`<w:cols ...>` with its `<w:col>` children), in place of `cols`. */
  colsXml?: string;
  /** Paragraphs: texts ({CBR} is a column break, {PBR} a page break, {TAB} a tab, {LBR} a line break), or a text with its paragraph properties (`w:pPr` content). */
  paragraphs: (string | { text: string; ppr?: string; xml?: string } | { raw: string } | { body: string })[];
}

export interface DocSpec {
  sections: SectionSpec[];
  /** Different headers for even and odd pages (`w:evenAndOddHeaders`). */
  evenAndOdd?: boolean;
  /** The document's default language (`w:lang` in the style defaults), e.g. `de-DE`. */
  lang?: string;
  /** Pictures kept in the file (`word/media/<name>`), each reached from the text by `r:embed="<rid>"`. */
  media?: { name: string; rid: string; bytes: Uint8Array }[];
  /** More relationships of the main part: a picture that is only linked has an external one. */
  extraRels?: { id: string; type: string; target: string; external?: boolean }[];
  /** Parts of any kind under word/, with their content type (an imported chunk's web page, say). */
  files?: { name: string; contentType: string; data: string | Uint8Array }[];
  /** The children of `w:numbering` (abstract definitions, then nums), for tests of lists. */
  numbering?: string;
  /** More of `w:settings` (`<w:defaultTabStop w:val="1440"/>`, say), as XML. */
  settings?: string;
  /**
   * The children of `word/footnotes.xml` and `word/endnotes.xml`: the notes themselves, `<w:footnote w:id="1">...</w:footnote>`,
   * after the two separator notes Word always writes (leave `separators` as false to give your own).
   */
  footnotes?: string;
  endnotes?: string;
  separators?: boolean;
  /** The page background as Word's `<w:background>` element, put before the body. */
  background?: string;
  /** More styles for styles.xml, as XML (`<w:style ...>` elements), e.g. table styles. */
  styles?: string;
  /** More parts of the package (a custom XML part, say), with the relationship each needs from the main document and its content type override. */
  extra?: { name: string; xml: string; rel?: { type: string; target: string }; override?: string; partRel?: { owner: string; type: string; target: string } }[];
  /** The file's properties (`docProps/core.xml`), as Word writes them. */
  props?: { title?: string; created?: string; modified?: string; lastModifiedBy?: string; creator?: string; subject?: string; description?: string; keywords?: string };
  /** The extended properties (`docProps/app.xml`): the XML inside `Properties`, such as `<Company>Acme</Company>`. */
  app?: string;
  /** Custom document properties (`docProps/custom.xml`): the XML inside `Properties`. */
  custom?: string;
  /** The comments (`word/comments.xml`): the `<w:comment>` elements inside `w:comments`. */
  comments?: string;
  /** A custom XML item (`customXml/item1.xml`): the whole file. */
  customXml?: string;
  /** The building blocks (`word/glossary/document.xml`): the `w:docPart` elements inside `w:docParts`, and the glossary's own `w:style` elements. */
  glossary?: { parts: string; styles?: string };
}

/** The run Word puts in a header for a text watermark (Design, Watermark): a WordArt shape behind the text, centred in the margins. */
function watermarkRun(w: NonNullable<HfSpec["watermark"]>): string {
  const angle = w.angle ?? 315;
  const shape =
    '<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/><o:lock v:ext="edit" text="t" shapetype="t"/></v:shapetype>' +
    `<v:shape id="PowerPlusWaterMarkObject1" o:spid="_x0000_s2049" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:412.4pt;height:247.45pt;rotation:${angle};z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="${w.color ?? "silver"}" stroked="f">` +
    `<v:fill opacity="${w.opacity ?? 0.5}"/><v:textpath style="font-family:&quot;${w.font ?? "Calibri"}&quot;;font-size:1pt" string="${esc(w.text)}"/><w10:wrap anchorx="margin" anchory="margin"/></v:shape>`;
  return `<w:r><w:rPr><w:noProof/></w:rPr><w:pict>${shape}</w:pict></w:r>`;
}


function field(name: string): string {
  const r = (inner: string) => `<w:r>${inner}</w:r>`;
  return (
    r('<w:fldChar w:fldCharType="begin"/>') +
    r(`<w:instrText xml:space="preserve"> ${name} </w:instrText>`) +
    r('<w:fldChar w:fldCharType="separate"/>') +
    r("<w:t>1</w:t>") +
    r('<w:fldChar w:fldCharType="end"/>')
  );
}

/** Runs for a text in which {NAME} marks a field ({CBR} and {PBR} are breaks, {TAB} a tab, {LBR} a line break). */
export function runs(text: string): string {
  return text
    .split(/(\{[A-Z]+\})/)
    .filter(Boolean)
    .map((part) => {
      if (part === "{CBR}") return '<w:r><w:br w:type="column"/></w:r>';
      if (part === "{PBR}") return '<w:r><w:br w:type="page"/></w:r>';
      if (part === "{TAB}") return "<w:r><w:tab/></w:r>";
      if (part === "{LBR}") return "<w:r><w:br/></w:r>";
      return /^\{[A-Z]+\}$/.test(part) ? field(part.slice(1, -1)) : `<w:r><w:t xml:space="preserve">${esc(part)}</w:t></w:r>`;
    })
    .join("");
}

const para = (text: string, ppr = "", xml = "") => `<w:p>${ppr ? `<w:pPr>${ppr}</w:pPr>` : ""}${runs(text)}${xml}</w:p>`;

export function buildDocx(spec: DocSpec): Uint8Array {
  const parts: [string, string][] = [];
  const rels: string[] = [];
  const overrides: string[] = [];
  let n = 0;
  const hf = (kind: "header" | "footer", text: string, watermark?: HfSpec["watermark"], raw?: string) => {
    const id = `rId${10 + ++n}`;
    const name = `${kind}${n}.xml`;
    const tag = kind === "header" ? "hdr" : "ftr";
    // {BR} starts another paragraph.
    const paras = text.split("{BR}").map((t) => para(t));
    if (watermark) paras[0] = paras[0].replace(/^<w:p>/, `<w:p>${watermarkRun(watermark)}`);
    if (raw) paras[0] = paras[0].replace(/^<w:p>/, `<w:p>${raw}`);
    parts.push([`word/${name}`, `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:${tag} xmlns:w="${W}" xmlns:r="${R}" xmlns:v="${V}" xmlns:o="${O}" xmlns:w10="${W10}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">${paras.join("")}</w:${tag}>`]);
    rels.push(`<Relationship Id="${id}" Type="${REL}/${kind}" Target="${name}"/>`);
    overrides.push(`<Override PartName="/word/${name}" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.${kind}+xml"/>`);
    return id;
  };
  const sectPr = (s: SectionSpec, i: number) => {
    const refs: string[] = [];
    for (const [kind, set] of [["header", s.headers], ["footer", s.footers]] as const) {
      for (const v of ["default", "first", "even"] as const) {
        const text = set?.[v];
        if (text !== undefined) refs.push(`<w:${kind}Reference w:type="${v}" r:id="${hf(kind, text, set?.watermark, set?.raw)}"/>`);
      }
    }
    const [pw, ph] = s.size ?? [12240, 15840];
    const m = s.margins ?? [1440, 1440, 1440, 1440];
    const start = i > 0 && s.start && s.start !== "nextPage" ? `<w:type w:val="${s.start === "continuous" ? "continuous" : s.start === "evenPage" ? "evenPage" : "oddPage"}"/>` : "";
    return (
      `<w:sectPr>${refs.join("")}${s.xml?.notes ?? ""}${start}<w:pgSz w:w="${pw}" w:h="${ph}"${pw > ph ? ' w:orient="landscape"' : ""}/>` +
      `<w:pgMar w:top="${m[0]}" w:right="${m[1]}" w:bottom="${m[2]}" w:left="${m[3]}" w:header="${s.header ?? 720}" w:footer="${s.footer ?? 720}" w:gutter="${s.gutter ?? 0}"/>` +
      `${s.xml?.afterMar ?? ""}` +
      `${s.pageNumbers ? `<w:pgNumType${s.pageNumbers.fmt ? ` w:fmt="${s.pageNumbers.fmt}"` : ""}${s.pageNumbers.start !== undefined ? ` w:start="${s.pageNumbers.start}"` : ""}/>` : ""}` +
      `${s.colsXml ?? (s.cols ? `<w:cols w:num="${s.cols.num}" w:space="${s.cols.space ?? 720}"${s.cols.sep ? ' w:sep="1"' : ""}/>` : "")}` +
      `${s.xml?.end ?? ""}${s.titlePage ? "<w:titlePg/>" : ""}${s.rtlGutter ? "<w:rtlGutter/>" : ""}</w:sectPr>`
    );
  };
  const body: string[] = [];
  spec.sections.forEach((s, i) => {
    const last = i === spec.sections.length - 1;
    s.paragraphs.forEach((p, j) => {
      // The paragraph that ends a section carries its properties.
      const end = !last && j === s.paragraphs.length - 1;
      // A paragraph written out as Word's markup (drawings and the like); not for the last paragraph of a section.
      if (typeof p === "object" && "raw" in p) {
        body.push(`<w:p>${p.raw}</w:p>`);
        return;
      }
      // A block directly under the body, outside any paragraph (an equation on its own, as Word can write one); not for the last paragraph of a section.
      if (typeof p === "object" && "body" in p) {
        body.push(p.body);
        return;
      }
      const { text, ppr, xml } = typeof p === "string" ? { text: p, ppr: "", xml: "" } : { text: p.text, ppr: p.ppr ?? "", xml: p.xml ?? "" };
      body.push(para(text, (ppr ?? "") + (end ? sectPr(s, i) : ""), xml));
    });
    if (last) body.push(sectPr(s, i));
  });
  const document = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="${W}" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:r="${R}" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" xmlns:v="${V}" xmlns:o="${O}" xmlns:w10="${W10}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas">${spec.background ?? ""}<w:body>${body.join("")}</w:body></w:document>`;
  parts.unshift(["word/document.xml", document]);
  parts.push(["word/settings.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="${W}">${spec.settings ?? ""}${spec.evenAndOdd ? "<w:evenAndOddHeaders/>" : ""}</w:settings>`]);
  rels.push(`<Relationship Id="rId1" Type="${REL}/settings" Target="settings.xml"/>`);
  overrides.push('<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>');
  for (const [kind, tag, body] of [["footnotes", "footnote", spec.footnotes], ["endnotes", "endnote", spec.endnotes]] as const) {
    if (body === undefined) continue;
    const sep =
      spec.separators === false
        ? ""
        : `<w:${tag} w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:${tag}><w:${tag} w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:${tag}>`;
    parts.push([`word/${kind}.xml`, `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:${kind} xmlns:w="${W}" xmlns:r="${R}">${sep}${body}</w:${kind}>`]);
    rels.push(`<Relationship Id="rId${kind === "footnotes" ? 20 : 21}" Type="${REL}/${kind}" Target="${kind}.xml"/>`);
    overrides.push(`<Override PartName="/word/${kind}.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.${kind}+xml"/>`);
  }
  if (spec.numbering) {
    parts.push(["word/numbering.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="${W}">${spec.numbering}</w:numbering>`]);
    rels.push(`<Relationship Id="rId3" Type="${REL}/numbering" Target="numbering.xml"/>`);
    overrides.push('<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>');
  }
  if (spec.lang || spec.styles) {
    parts.push(["word/styles.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="${W}"><w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/>${spec.lang ? `<w:lang w:val="${spec.lang}"/>` : ""}</w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>${spec.styles ?? ""}</w:styles>`]);
    rels.push(`<Relationship Id="rId2" Type="${REL}/styles" Target="styles.xml"/>`);
    overrides.push('<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>');
  }
  if (spec.comments) {
    parts.push(["word/comments.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="${W}">${spec.comments}</w:comments>`]);
    rels.push(`<Relationship Id="rId30" Type="${REL}/comments" Target="comments.xml"/>`);
    overrides.push('<Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/>');
  }
  if (spec.customXml) {
    parts.push(["customXml/item1.xml", spec.customXml]);
    rels.push(`<Relationship Id="rId31" Type="${REL}/customXml" Target="../customXml/item1.xml"/>`);
  }
  if (spec.glossary) {
    parts.push(["word/glossary/document.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:glossaryDocument xmlns:w="${W}" xmlns:r="${R}"><w:docParts>${spec.glossary.parts}</w:docParts></w:glossaryDocument>`]);
    parts.push(["word/glossary/styles.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="${W}">${spec.glossary.styles ?? ""}</w:styles>`]);
    parts.push(["word/glossary/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/styles" Target="styles.xml"/></Relationships>`]);
    rels.push(`<Relationship Id="rId40" Type="${REL}/glossaryDocument" Target="glossary/document.xml"/>`);
    overrides.push('<Override PartName="/word/glossary/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.glossary+xml"/>');
    overrides.push('<Override PartName="/word/glossary/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>');
  }
  for (const r of spec.extraRels ?? []) rels.push(`<Relationship Id="${r.id}" Type="${REL}/${r.type}" Target="${esc(r.target)}"${r.external ? ' TargetMode="External"' : ""}/>`);
  for (const f of spec.files ?? []) {
    parts.push([`word/${f.name}`, f.data] as unknown as [string, string]);
    overrides.push(`<Override PartName="/word/${f.name}" ContentType="${f.contentType}"/>`);
  }
  for (const m of spec.media ?? []) {
    parts.push([`word/media/${m.name}`, m.bytes] as unknown as [string, string]);
    rels.push(`<Relationship Id="${m.rid}" Type="${REL}/image" Target="media/${m.name}"/>`);
    const ext = m.name.split(".").pop()!.toLowerCase();
    const ct = ({ png: "image/png", jpg: "image/jpeg", gif: "image/gif", svg: "image/svg+xml", tif: "image/tiff", tiff: "image/tiff", emf: "image/x-emf", wmf: "image/x-wmf" } as Record<string, string>)[ext] ?? "application/octet-stream";
    if (!overrides.some((o) => o.includes(`Extension="${ext}"`))) overrides.push(`<Default Extension="${ext}" ContentType="${ct}"/>`);
  }
  if (spec.props) {
    const p = spec.props;
    const date = (tag: string, v?: string) => (v ? `<dcterms:${tag} xsi:type="dcterms:W3CDTF">${v}</dcterms:${tag}>` : "");
    parts.push([
      "docProps/core.xml",
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">${p.title ? `<dc:title>${esc(p.title)}</dc:title>` : ""}${p.creator ? `<dc:creator>${esc(p.creator)}</dc:creator>` : ""}${p.subject ? `<dc:subject>${esc(p.subject)}</dc:subject>` : ""}${p.description ? `<dc:description>${esc(p.description)}</dc:description>` : ""}${p.keywords ? `<cp:keywords>${esc(p.keywords)}</cp:keywords>` : ""}${p.lastModifiedBy ? `<cp:lastModifiedBy>${esc(p.lastModifiedBy)}</cp:lastModifiedBy>` : ""}${date("created", p.created)}${date("modified", p.modified)}</cp:coreProperties>`,
    ]);
    overrides.push('<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>');
  }
  for (const [i, x] of (spec.extra ?? []).entries()) {
    parts.push([x.name, x.xml]);
    if (x.rel) rels.push(`<Relationship Id="rId${40 + i}" Type="${x.rel.type}" Target="${x.rel.target}"/>`);
    if (x.override) overrides.push(`<Override PartName="/${x.name}" ContentType="${x.override}"/>`);
    // A relationship owned by another part (the properties part of a custom XML part).
    if (x.partRel) {
      const dir = x.partRel.owner.slice(0, x.partRel.owner.lastIndexOf("/") + 1);
      const file = x.partRel.owner.slice(dir.length);
      parts.push([`${dir}_rels/${file}.rels`, `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${x.partRel.type}" Target="${x.partRel.target}"/></Relationships>`]);
    }
  }
  const extra = (name: string, xml: string | undefined, ns: string, ct: string, rel: string) => {
    if (!xml) return;
    parts.push([`docProps/${name}.xml`, `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="${ns}" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">${xml}</Properties>`]);
    overrides.push(`<Override PartName="/docProps/${name}.xml" ContentType="${ct}"/>`);
    extraRels.push(`<Relationship Id="rId${5 + extraRels.length}" Type="${rel}" Target="docProps/${name}.xml"/>`);
  };
  const extraRels: string[] = [];
  extra("app", spec.app, "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties", "application/vnd.openxmlformats-officedocument.extended-properties+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties");
  extra("custom", spec.custom, "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties", "application/vnd.openxmlformats-officedocument.custom-properties+xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties");
  const files: [string, string | Uint8Array][] = [
    [
      "[Content_Types].xml",
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>${overrides.join("")}</Types>`,
    ],
    ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/officeDocument" Target="word/document.xml"/>${spec.props ? '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>' : ""}${extraRels.join("")}</Relationships>`],
    ["word/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${rels.join("")}</Relationships>`],
    ...parts,
  ];
  return zip(files);
}

/** Open a built document in the editor (replacing what is on screen) and wait for it to be laid out. */
export async function openBuilt(page: Page, spec: DocSpec) {
  const bytes = Array.from(buildDocx(spec));
  await page.goto("/?open=simple.docx");
  await documentReady(page);
  await page.evaluate(async (b) => {
    const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
    await w.fe.open(new Uint8Array(b), "built.docx");
  }, bytes);
  const first = spec.sections[0].paragraphs[0];
  await expect(page.locator(".flow .fp").first()).toContainText((typeof first === "string" ? first : "text" in first ? first.text : "").replace(/\{(TAB|LBR)\}/g, " ").replace(/\{[A-Z]+\}/g, ""));
  await page.evaluate(() => document.fonts.ready);
}

export async function openBytes(page: Page, bytes: Uint8Array, first: string) {
  await page.goto("/?open=simple.docx");
  await documentReady(page);
  await page.evaluate(async (b) => {
    const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
    await w.fe.open(new Uint8Array(b), "built.docx");
  }, Array.from(bytes));
  await expect(page.locator(".flow .fp").first()).toContainText(first);
  await page.evaluate(() => document.fonts.ready);
}

