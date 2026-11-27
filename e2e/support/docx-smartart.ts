// SmartArt and WordArt documents.
import { O, R, REL, V, W, W10, zip } from "./docx-zip";
import type { Tree } from "./smartart-markup";
import { THEME as SMARTART_THEME, colorsPart, dataModel, diagramRun, layoutPart, stylePart } from "./smartart-markup";

// -- SmartArt and WordArt -------------------------------------------------------------------------------
// Word saves a SmartArt graphic as five parts (data model, layout, quick style, colours and the drawing it laid
// out), related to the document; `smartArtDocx` writes those around the paragraphs of a test document. The markup
// itself is in smartart-markup.ts.

export interface DiagramSpec {
  /** The layout's name (`default`, `process1`, `cycle2`, `vList2`, `orgChart1`, ...). */
  layout: string;
  /** The nodes of the data model; a node with `children` has sub-nodes. */
  nodes: Tree[];
  /** The drawing part Word saved (`dspDrawing(...)`); leave it out to have the graphic laid out from the data model. */
  drawing?: string;
  /** Size in points. */
  w?: number;
  h?: number;
  alt?: string;
  /** Float the graphic at these offsets (points from the column and the paragraph) instead of putting it in the line. */
  anchor?: { x: number; y: number };
}

/** A document with a paragraph, then one paragraph for each SmartArt graphic, then a last paragraph. */
export function smartArtDocx(diagrams: DiagramSpec[], opts: { before?: string; after?: string } = {}): { bytes: Uint8Array; parts: Record<string, string> } {
  const files: [string, string][] = [];
  const rels: string[] = [`<Relationship Id="rId1" Type="${REL}/theme" Target="theme/theme1.xml"/>`];
  const types: string[] = ['<Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>'];
  const parts: Record<string, string> = {};
  const add = (name: string, xml: string, type: string) => {
    files.push([name, xml]);
    parts[name] = xml;
    types.push(`<Override PartName="/${name}" ContentType="${type}"/>`);
  };
  const ct = "application/vnd.openxmlformats-officedocument.drawingml";
  const body = diagrams.map((d, i) => {
    const n = i + 1;
    const base = 10 * n;
    let drawingRid: string | undefined;
    if (d.drawing) {
      drawingRid = `rId${base + 5}`;
      add(`word/diagrams/drawing${n}.xml`, d.drawing, "application/vnd.ms-office.drawingml.diagramDrawing+xml");
      rels.push(`<Relationship Id="${drawingRid}" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="diagrams/drawing${n}.xml"/>`);
    }
    add(`word/diagrams/data${n}.xml`, dataModel(d.layout, d.nodes, drawingRid), `${ct}.diagramData+xml`);
    add(`word/diagrams/layout${n}.xml`, layoutPart(d.layout), `${ct}.diagramLayout+xml`);
    add(`word/diagrams/quickStyle${n}.xml`, stylePart(), `${ct}.diagramStyle+xml`);
    add(`word/diagrams/colors${n}.xml`, colorsPart(), `${ct}.diagramColors+xml`);
    const ids = { dm: `rId${base + 1}`, lo: `rId${base + 2}`, qs: `rId${base + 3}`, cs: `rId${base + 4}` };
    rels.push(
      `<Relationship Id="${ids.dm}" Type="${REL}/diagramData" Target="diagrams/data${n}.xml"/>`,
      `<Relationship Id="${ids.lo}" Type="${REL}/diagramLayout" Target="diagrams/layout${n}.xml"/>`,
      `<Relationship Id="${ids.qs}" Type="${REL}/diagramQuickStyle" Target="diagrams/quickStyle${n}.xml"/>`,
      `<Relationship Id="${ids.cs}" Type="${REL}/diagramColors" Target="diagrams/colors${n}.xml"/>`,
    );
    return `<w:p>${diagramRun(ids, { w: d.w ?? 432, h: d.h ?? 252, id: n, alt: d.alt, anchor: d.anchor })}</w:p>`;
  });
  const doc = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="${W}" xmlns:r="${R}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram"><w:body><w:p><w:r><w:t>${opts.before ?? "Before the graphic"}</w:t></w:r></w:p>${body.join("")}<w:p><w:r><w:t>${opts.after ?? "After the graphic"}</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>`;
  files.unshift(["word/document.xml", doc]);
  files.push(["word/theme/theme1.xml", SMARTART_THEME]);
  files.unshift(
    ["[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>${types.join("")}</Types>`],
    ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/officeDocument" Target="word/document.xml"/></Relationships>`],
    ["word/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${rels.join("")}</Relationships>`],
  );
  return { bytes: zip(files), parts };
}

/** A document of paragraphs written out as runs of Word's markup (WordArt, say), after a first paragraph of plain text. */
export function wordArtDocx(runs: string[], opts: { before?: string } = {}): Uint8Array {
  const body = runs.map((r) => `<w:p>${r}</w:p>`).join("");
  const doc = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="${W}" xmlns:r="${R}" xmlns:v="${V}" xmlns:o="${O}" xmlns:w10="${W10}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" mc:Ignorable="w14"><w:body><w:p><w:r><w:t>${opts.before ?? "Before the WordArt"}</w:t></w:r></w:p>${body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>`;
  return zip([
    ["[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>`],
    ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/officeDocument" Target="word/document.xml"/></Relationships>`],
    ["word/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/theme" Target="theme/theme1.xml"/></Relationships>`],
    ["word/document.xml", doc],
    ["word/theme/theme1.xml", SMARTART_THEME],
  ]);
}

/** Open the bytes of a document in the editor (replacing what is on screen) and wait until its first paragraph shows `first`. */
