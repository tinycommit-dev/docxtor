// Charts of the new kinds (chartex): the markup and a whole document around it.
import { crc32, esc, zip } from "./docx-zip";

// -- charts of the new kinds (chartex) ------------------------------------------------------------
// Word 2016 writes waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst and region map charts as
// a drawing in `mc:AlternateContent` (the chart in `mc:Choice`, a picture in `mc:Fallback`) whose chart part is
// `word/charts/chartEx1.xml`. These builders write that markup, and a whole document around it.

export type ChartExName = "waterfall" | "histogram" | "pareto" | "box" | "funnel" | "treemap" | "sunburst" | "region";

export interface ChartExOptions {
  title?: string;
  /** Histogram: the inside of the `cx:binning` element, e.g. `<cx:binSize val="5"/>`; `binAttrs` go on the element itself. */
  binning?: string;
  binAttrs?: string;
  /** Box and whisker. */
  quartile?: "exclusive" | "inclusive";
  meanLine?: boolean;
  outliers?: boolean;
  points?: boolean;
  /** Treemap and sunburst: the file lists the leaf level first (the order classic charts use). */
  leafFirst?: boolean;
  parentLabels?: "banner" | "overlapping" | "none";
  /** Data labels on (waterfall, histogram, funnel); for trees, show the values as well as the names. */
  labels?: boolean;
  legend?: "t" | "b" | "l" | "r";
  connectors?: boolean;
}

const cxStr = (f: string, count: number, levels: [number, string][][]) =>
  `<cx:strDim type="cat"><cx:f>${f}</cx:f>${levels.map((pts) => `<cx:lvl ptCount="${count}">${pts.map(([i, s]) => `<cx:pt idx="${i}">${esc(s)}</cx:pt>`).join("")}</cx:lvl>`).join("")}</cx:strDim>`;
const cxNum = (type: string, f: string, vals: number[]) =>
  `<cx:numDim type="${type}"><cx:f>${f}</cx:f><cx:lvl ptCount="${vals.length}" formatCode="General">${vals.map((v, i) => `<cx:pt idx="${i}">${v}</cx:pt>`).join("")}</cx:lvl></cx:numDim>`;
const cxTx = (f: string, name: string) => `<cx:tx><cx:txData><cx:f>${f}</cx:f><cx:v>${esc(name)}</cx:v></cx:txData></cx:tx>`;
const cxTitle = (t?: string) => (t ? `<cx:title pos="t" align="ctr" overlay="0"><cx:tx><cx:txData><cx:f>_xlchart.v1.0</cx:f><cx:v>${esc(t)}</cx:v></cx:txData></cx:tx></cx:title>` : "");
const cxLabels = (on: boolean | undefined, names = false, values = true) => (on === false ? "" : `<cx:dataLabels pos="outEnd"><cx:visibility seriesName="0" categoryName="${names ? 1 : 0}" value="${values ? 1 : 0}"/></cx:dataLabels>`);

/** The values the histogram sample holds (twenty numbers). */
export const CHARTEX_SAMPLE = [1, 2, 2, 3, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 9, 10, 11, 12, 14, 20];

/** A chartex part as Word writes one, with sample data for each kind. */
export function chartexXml(kind: ChartExName, o: ChartExOptions = {}): string {
  const head = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cx:chartSpace xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex">`;
  const wrapIt = (data: string, series: string, axes: string, title: string | undefined, legend?: string) =>
    `${head}<cx:chartData><cx:externalData r:id="rId1" cx:autoUpdate="0"/>${data}</cx:chartData><cx:chart>${cxTitle(title)}<cx:plotArea><cx:plotAreaRegion>${series}</cx:plotAreaRegion>${axes}</cx:plotArea>${legend ? `<cx:legend pos="${legend}" align="ctr" overlay="0"/>` : ""}</cx:chart></cx:chartSpace>`;
  const catAxis = (gap: string) => `<cx:axis id="0"><cx:catScaling gapWidth="${gap}"/><cx:tickLabels/></cx:axis>`;
  const valAxis = `<cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis>`;
  switch (kind) {
    case "waterfall": {
      const data = `<cx:data id="0">${cxStr("Sheet1!$A$2:$A$7", 6, [[[0, "Start"], [1, "Sales"], [2, "Returns"], [3, "Costs"], [4, "Tax"], [5, "Profit"]]])}${cxNum("val", "Sheet1!$B$2:$B$7", [100, 80, -20, -35, -10, 115])}</cx:data>`;
      const vis = o.connectors === false ? `<cx:visibility connectorLines="0"/>` : "";
      const ser = `<cx:series layoutId="waterfall" uniqueId="{00000000-0000-0000-0000-000000000001}">${cxTx("Sheet1!$B$1", "Amount")}<cx:dataPt idx="2"><cx:spPr><a:solidFill><a:srgbClr val="C00000"/></a:solidFill></cx:spPr></cx:dataPt>${cxLabels(o.labels)}<cx:dataId val="0"/><cx:layoutPr>${vis}<cx:subtotals><cx:idx val="0"/><cx:idx val="5"/></cx:subtotals></cx:layoutPr></cx:series>`;
      return wrapIt(data, ser, catAxis("0.5") + valAxis, o.title ?? "Cash flow", o.legend ?? "t");
    }
    case "histogram": {
      const data = `<cx:data id="0">${cxNum("val", "Sheet1!$A$2:$A$21", CHARTEX_SAMPLE)}</cx:data>`;
      const bin = o.binning ?? `<cx:binSize val="5"/>`;
      const ser = `<cx:series layoutId="clusteredColumn" uniqueId="{00000000-0000-0000-0000-000000000002}">${o.labels ? cxLabels(true) : ""}<cx:dataId val="0"/><cx:layoutPr><cx:binning intervalClosed="r"${o.binAttrs ? " " + o.binAttrs : ""}>${bin}</cx:binning></cx:layoutPr></cx:series>`;
      return wrapIt(data, ser, catAxis("0") + valAxis, o.title ?? "Scores", o.legend);
    }
    case "pareto": {
      const data = `<cx:data id="0">${cxStr("Sheet1!$A$2:$A$6", 5, [[[0, "Scratch"], [1, "Dent"], [2, "Crack"], [3, "Stain"], [4, "Other"]]])}${cxNum("val", "Sheet1!$B$2:$B$6", [12, 40, 8, 25, 15])}</cx:data>`;
      const ser = `<cx:series layoutId="clusteredColumn" uniqueId="{1}">${cxTx("Sheet1!$B$1", "Defects")}<cx:dataId val="0"/><cx:layoutPr><cx:aggregation/></cx:layoutPr><cx:axisId val="1"/></cx:series><cx:series layoutId="paretoLine" uniqueId="{2}"><cx:spPr><a:ln w="28575"><a:solidFill><a:srgbClr val="ED7D31"/></a:solidFill></a:ln></cx:spPr><cx:axisId val="2"/><cx:dataId val="0"/></cx:series>`;
      return wrapIt(data, ser, catAxis("0") + valAxis + `<cx:axis id="2"><cx:valScaling max="1" min="0"/><cx:units unit="percentage"/><cx:tickLabels/></cx:axis>`, o.title ?? "Defects", o.legend ?? "b");
    }
    case "box": {
      const a = [4, 5, 6, 7, 8, 9, 10, 11, 30];
      const b = [1, 2, 3, 4, 5, 6, 7, 8];
      const all = [...a, ...b];
      const labels: [number, string][] = all.map((_, i) => [i, i < a.length ? "Ward A" : "Ward B"]);
      const data = `<cx:data id="0">${cxStr("Sheet1!$A$2:$A$18", all.length, [labels])}${cxNum("val", "Sheet1!$B$2:$B$18", all)}</cx:data>`;
      const vis = `<cx:visibility meanLine="${o.meanLine ? 1 : 0}" meanMarker="1" nonoutliers="${o.points ? 1 : 0}" outliers="${o.outliers === false ? 0 : 1}"/>`;
      const ser = `<cx:series layoutId="boxWhisker" uniqueId="{3}">${cxTx("Sheet1!$B$1", "Wait")}<cx:dataId val="0"/><cx:layoutPr>${vis}<cx:statistics quartileMethod="${o.quartile ?? "exclusive"}"/></cx:layoutPr></cx:series>`;
      return wrapIt(data, ser, catAxis("1") + valAxis, o.title ?? "Waiting time", o.legend);
    }
    case "funnel": {
      const data = `<cx:data id="0">${cxStr("Sheet1!$A$2:$A$5", 4, [[[0, "Visits"], [1, "Sign-ups"], [2, "Trials"], [3, "Paid"]]])}${cxNum("val", "Sheet1!$B$2:$B$5", [1000, 400, 150, 60])}</cx:data>`;
      const ser = `<cx:series layoutId="funnel" uniqueId="{4}">${cxTx("Sheet1!$B$1", "Count")}<cx:dataLabels pos="inEnd"><cx:visibility seriesName="0" categoryName="0" value="1"/></cx:dataLabels><cx:dataId val="0"/></cx:series>`;
      return wrapIt(data, ser, catAxis("0.06"), o.title ?? "Sales funnel", o.legend);
    }
    case "treemap":
    case "sunburst": {
      const parents: [number, string][] = [[0, "Fruit"], [3, "Veg"]];
      const leaves: [number, string][] = [[0, "Apple"], [1, "Pear"], [2, "Plum"], [3, "Kale"], [4, "Leek"]];
      const data = `<cx:data id="0">${cxStr("Sheet1!$A$2:$B$6", 5, o.leafFirst ? [leaves, parents] : [parents, leaves])}${cxNum("size", "Sheet1!$C$2:$C$6", [4, 3, 1, 2, 5])}</cx:data>`;
      const layout = kind === "treemap" && o.parentLabels ? `<cx:layoutPr><cx:parentLabelLayout val="${o.parentLabels}"/></cx:layoutPr>` : "";
      const ser = `<cx:series layoutId="${kind}" uniqueId="{5}">${cxTx("Sheet1!$C$1", "Size")}<cx:dataLabels pos="inEnd"><cx:visibility seriesName="0" categoryName="1" value="${o.labels ? 1 : 0}"/></cx:dataLabels><cx:dataId val="0"/>${layout}</cx:series>`;
      return wrapIt(data, ser, "", o.title ?? (kind === "treemap" ? "Produce" : "Produce by kind"), o.legend);
    }
    case "region": {
      const data = `<cx:data id="0">${cxStr("Sheet1!$A$2:$A$4", 3, [[[0, "France"], [1, "Spain"], [2, "Italy"]]])}${cxNum("colorVal", "Sheet1!$B$2:$B$4", [5, 3, 8])}</cx:data>`;
      const ser = `<cx:series layoutId="regionMap" uniqueId="{6}">${cxTx("Sheet1!$B$1", "Sales")}<cx:dataId val="0"/><cx:layoutPr><cx:geography cultureLanguage="en-US" cultureRegion="US" attribution="Powered by Bing"><cx:geoCache provider="{E9337A44-BEBE-4D9F-B70C-5C5E7DAFC167}"/></cx:geography></cx:layoutPr></cx:series>`;
      return wrapIt(data, ser, "", o.title ?? "Sales by country", o.legend ?? "r");
    }
  }
}

/** A solid-colour PNG (the picture Word keeps beside a chart for programs that can't draw it). */
export function pngBytes(w: number, h: number, rgb: [number, number, number]): Uint8Array {
  const row = new Uint8Array(1 + w * 3);
  for (let x = 0; x < w; x++) row.set(rgb, 1 + x * 3);
  const raw = new Uint8Array(row.length * h);
  for (let y = 0; y < h; y++) raw.set(row, y * row.length);
  // zlib with stored blocks.
  const blocks: number[] = [0x78, 0x01];
  for (let at = 0; at < raw.length; at += 65535) {
    const n = Math.min(65535, raw.length - at);
    blocks.push(at + n >= raw.length ? 1 : 0, n & 255, n >>> 8, ~n & 255, (~n >>> 8) & 255, ...raw.subarray(at, at + n));
  }
  let a = 1;
  let b = 0;
  for (const v of raw) {
    a = (a + v) % 65521;
    b = (b + a) % 65521;
  }
  blocks.push(b >>> 8, b & 255, a >>> 8, a & 255);
  const u32 = (v: number) => [(v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255];
  const chunk = (type: string, data: number[] | Uint8Array) => {
    const body = new Uint8Array([...new TextEncoder().encode(type), ...data]);
    return [...u32(data.length), ...body, ...u32(crc32(body))];
  };
  return new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, ...chunk("IHDR", [...u32(w), ...u32(h), 8, 2, 0, 0, 0]), ...chunk("IDAT", blocks), ...chunk("IEND", [])]);
}

export interface ChartExDoc {
  /** `picture`: a DrawingML picture in the fallback, `vml`: a VML picture, `none`: no fallback. */
  fallback?: "picture" | "vml" | "none";
  /** Size of the drawing in EMU; default 5486400 x 3200400 (432 x 252 pt). */
  extent?: [number, number];
  /** Leave the chartex part out of the package, as a file that lost it. */
  noPart?: boolean;
  before?: string;
  after?: string;
  /** The drawing's alt text (`descr`). */
  alt?: string;
}

/** Colour of the fallback picture's pixels, so a test can tell it from a native drawing. */
export const CHARTEX_FALLBACK_RGB: [number, number, number] = [192, 95, 160];

/** A .docx the way Word saves one with a chartex chart between two paragraphs; `parts` are the chart's files as written. */
export function buildChartExDocx(chartXml: string, o: ChartExDoc = {}): { bytes: Uint8Array; parts: Record<string, string | Uint8Array> } {
  const W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
  const R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const [cx, cy] = o.extent ?? [5486400, 3200400];
  const fb = o.fallback ?? "picture";
  const alt = o.alt ? ` descr="${esc(o.alt)}"` : "";
  const choice = `<mc:Choice Requires="cx1" xmlns:cx1="http://schemas.microsoft.com/office/drawing/2015/9/8/chartex"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="${cx}" cy="${cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="7" name="Chart 7"${alt}/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chart xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex" xmlns:r="${R}" r:id="rId5"/></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice>`;
  const fallback =
    fb === "picture"
      ? `<mc:Fallback><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="${cx}" cy="${cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="7" name="Chart 7"${alt}/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="0" name="Chart 7"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId6" xmlns:r="${R}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${cx}" cy="${cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Fallback>`
      : fb === "vml"
        ? `<mc:Fallback><w:pict><v:shape id="Chart 7" o:spid="_x0000_i1025" type="#_x0000_t75" style="width:${cx / 12700}pt;height:${cy / 12700}pt" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><v:imagedata r:id="rId6" o:title=""/></v:shape></w:pict></mc:Fallback>`
        : "";
  const drawing = `<w:r><mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">${choice}${fallback}</mc:AlternateContent></w:r>`;
  const png = pngBytes(64, 36, CHARTEX_FALLBACK_RGB);
  const parts: Record<string, string | Uint8Array> = {};
  if (!o.noPart) parts["word/charts/chartEx1.xml"] = chartXml;
  if (fb !== "none") parts["word/media/image1.png"] = png;
  const files: [string, string | Uint8Array][] = [
    ["[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>${o.noPart ? "" : '<Override PartName="/word/charts/chartEx1.xml" ContentType="application/vnd.ms-office.chartex+xml"/>'}</Types>`],
    ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${R}/officeDocument" Target="word/document.xml"/></Relationships>`],
    ["word/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${o.noPart ? "" : '<Relationship Id="rId5" Type="http://schemas.microsoft.com/office/2014/relationships/chartEx" Target="charts/chartEx1.xml"/>'}${fb !== "none" ? `<Relationship Id="rId6" Type="${R}/image" Target="media/image1.png"/>` : ""}</Relationships>`],
    ["word/document.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="${W}" xmlns:r="${R}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body><w:p><w:r><w:t>${o.before ?? "Before the chart"}</w:t></w:r></w:p><w:p>${drawing}</w:p><w:p><w:r><w:t>${o.after ?? "After the chart"}</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>`],
  ];
  if (!o.noPart) {
    files.push(["word/charts/chartEx1.xml", chartXml]);
    files.push(["word/charts/_rels/chartEx1.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${R}/package" Target="../embeddings/Microsoft_Excel_Sheet1.xlsx"/></Relationships>`]);
    files.push(["word/embeddings/Microsoft_Excel_Sheet1.xlsx", "not really a workbook"]);
    parts["word/embeddings/Microsoft_Excel_Sheet1.xlsx"] = "not really a workbook";
  }
  if (fb !== "none") files.push(["word/media/image1.png", png]);
  return { bytes: zip(files), parts };
}

