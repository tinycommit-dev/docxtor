// Charts: a document with a Word-made chart and a workbook made the way Excel makes one.
import { zip } from "./docx-zip";

// ---------------------------------------------------------------------------------------------------------------
// Charts (item 54): a document with a Word-made chart and a workbook made the way Excel makes one
// ---------------------------------------------------------------------------------------------------------------

const C_NS = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const A_NS = "http://schemas.openxmlformats.org/drawingml/2006/main";

/** A cell of a test workbook: text, a number, a formula (`=B2*2`, with its cached number after a `|`), or nothing. */
export type XCell = string | number | null;

/** A workbook the way Excel writes one: shared strings, a number format on the values, a formula column, a second sheet and a defined name. */
export function xlsxBytes(rows: XCell[][], opts: { sheet?: string; second?: boolean } = {}): Uint8Array {
  const sheet = opts.sheet ?? "Sheet1";
  const strings: string[] = [];
  const col = (n: number) => String.fromCharCode(65 + n);
  const body = rows
    .map((r, ri) => {
      const cells = r
        .map((v, ci) => {
          const ref = `${col(ci)}${ri + 1}`;
          if (v === null) return "";
          if (typeof v === "number") return `<c r="${ref}" s="1"><v>${v}</v></c>`;
          if (v.startsWith("=")) {
            const [f, cached] = v.slice(1).split("|");
            return `<c r="${ref}" s="1"><f>${f}</f><v>${cached ?? 0}</v></c>`;
          }
          let i = strings.indexOf(v);
          if (i < 0) i = strings.push(v) - 1;
          return `<c r="${ref}" t="s"><v>${i}</v></c>`;
        })
        .join("");
      return cells ? `<row r="${ri + 1}">${cells}</row>` : "";
    })
    .join("");
  const width = Math.max(...rows.map((r) => r.length));
  const NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
  const R2 = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const decl = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\r\n';
  return zip([
    ["[Content_Types].xml", `${decl}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>`],
    ["_rels/.rels", `${decl}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${R2}/officeDocument" Target="xl/workbook.xml"/></Relationships>`],
    ["xl/workbook.xml", `${decl}<workbook xmlns="${NS}" xmlns:r="${R2}"><sheets><sheet name="${sheet}" sheetId="1" r:id="rId1"/><sheet name="Notes" sheetId="2" r:id="rId4"/></sheets><definedNames><definedName name="Region">${sheet}!$A$2:$A$5</definedName></definedNames></workbook>`],
    ["xl/_rels/workbook.xml.rels", `${decl}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${R2}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId4" Type="${R2}/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId2" Type="${R2}/styles" Target="styles.xml"/><Relationship Id="rId3" Type="${R2}/sharedStrings" Target="sharedStrings.xml"/></Relationships>`],
    ["xl/worksheets/sheet1.xml", `${decl}<worksheet xmlns="${NS}"><dimension ref="A1:${col(width - 1)}${rows.length}"/><sheetData>${body}</sheetData></worksheet>`],
    ["xl/worksheets/sheet2.xml", `${decl}<worksheet xmlns="${NS}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Do not touch</t></is></c></row></sheetData></worksheet>`],
    ["xl/styles.xml", `${decl}<styleSheet xmlns="${NS}"><numFmts count="1"><numFmt numFmtId="164" formatCode="0.0"/></numFmts><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs></styleSheet>`],
    ["xl/sharedStrings.xml", `${decl}<sst xmlns="${NS}" count="${strings.length}" uniqueCount="${strings.length}">${strings.map((s) => `<si><t>${s}</t></si>`).join("")}</sst>`],
  ]);
}

/** A chart part as Word writes it: `groups` are the plot groups (with their axes) written out, and `legend` the legend. */
export function wordChartPart(plot: string, opts: { title?: string; legend?: string; extra?: string; style?: boolean } = {}): string {
  const title = opts.title ? `<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>${opts.title}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/>` : `<c:autoTitleDeleted val="1"/>`;
  const style = opts.style ? `<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="c14" xmlns:c14="http://schemas.microsoft.com/office/drawing/2007/8/2/chart"><c14:style val="102"/></mc:Choice><mc:Fallback><c:style val="2"/></mc:Fallback></mc:AlternateContent>` : "";
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><c:chartSpace xmlns:c="${C_NS}" xmlns:a="${A_NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><c:date1904 val="0"/><c:lang val="en-US"/><c:roundedCorners val="0"/>${style}<c:chart>${title}<c:plotArea><c:layout/>${plot}</c:plotArea>${opts.legend ?? ""}<c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/>${opts.extra ?? ""}</c:chart><c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData></c:chartSpace>`;
}

/** One series as Word writes it, reading its cells from the workbook (`col` is the column letter; rows 2 to 5). */
export function wordSeries(i: number, name: string, values: number[], cats: string[], opts: { col?: string; spPr?: string; extra?: string; tag?: "ser" } = {}): string {
  const col = opts.col ?? "BCDEFG"[i]!;
  const last = values.length + 1;
  return `<c:ser><c:idx val="${i}"/><c:order val="${i}"/><c:tx><c:strRef><c:f>Sheet1!$${col}$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>${name}</c:v></c:pt></c:strCache></c:strRef></c:tx>${opts.spPr ?? `<c:spPr><a:solidFill><a:schemeClr val="accent${i + 1}"/></a:solidFill></c:spPr>`}${opts.extra ?? ""}<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$${last}</c:f><c:strCache><c:ptCount val="${cats.length}"/>${cats.map((c, k) => `<c:pt idx="${k}"><c:v>${c}</c:v></c:pt>`).join("")}</c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!$${col}$2:$${col}$${last}</c:f><c:numCache><c:formatCode>0.0</c:formatCode><c:ptCount val="${values.length}"/>${values.map((v, k) => `<c:pt idx="${k}"><c:v>${v}</c:v></c:pt>`).join("")}</c:numCache></c:numRef></c:val></c:ser>`;
}

const CAT_AX = (id: number, cross: number, extra = "") => `<c:catAx><c:axId val="${id}"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/>${extra}<c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="${cross}"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:noMultiLvlLbl val="0"/></c:catAx>`;
const VAL_AX = (id: number, cross: number, extra = "") => `<c:valAx><c:axId val="${id}"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/>${extra}<c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="${cross}"/><c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>`;

/** A clustered column chart of two series over four regions, as Word writes it (with `<c:extLst>` and a style that Docxtor does not model). */
export function wordColumnChart(opts: { title?: string; extra?: string } = {}): string {
  const cats = ["North", "South", "East", "West"];
  const plot = `<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>${wordSeries(0, "Revenue", [4.3, 2.5, 3.5, 4.5], cats)}${wordSeries(1, "Costs", [2.4, 4.4, 1.8, 2.8], cats)}<c:gapWidth val="219"/><c:overlap val="-27"/><c:axId val="111"/><c:axId val="222"/></c:barChart>${CAT_AX(111, 222)}${VAL_AX(222, 111)}`;
  return wordChartPart(plot, { title: opts.title ?? "Revenue by region", legend: '<c:legend><c:legendPos val="b"/><c:overlay val="0"/></c:legend>', style: true, extra: opts.extra });
}

/** A document with one chart in its second paragraph: the chart part, its embedded workbook, and any other parts. */
export function chartDocx(chartXml: string, opts: { workbook?: Uint8Array; parts?: [string, string | Uint8Array][]; chartRels?: string } = {}): Uint8Array {
  const WNS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
  const RNS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const drawing = `<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="4572000" cy="2743200"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="1" name="Chart 1"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="${A_NS}"><a:graphicData uri="${C_NS}"><c:chart xmlns:c="${C_NS}" xmlns:r="${RNS}" r:id="rId5"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`;
  const files: [string, string | Uint8Array][] = [
    ["[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="xlsx" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/></Types>`],
    ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${RNS}/officeDocument" Target="word/document.xml"/></Relationships>`],
    ["word/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId5" Type="${RNS}/chart" Target="charts/chart1.xml"/></Relationships>`],
    ["word/document.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="${WNS}" xmlns:r="${RNS}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><w:body><w:p><w:r><w:t>Before the chart</w:t></w:r></w:p><w:p>${drawing}</w:p><w:p><w:r><w:t>After the chart</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>`],
    ["word/charts/chart1.xml", chartXml],
  ];
  if (opts.workbook) {
    files.push(["word/embeddings/Microsoft_Excel_Worksheet.xlsx", opts.workbook]);
    files.push(["word/charts/_rels/chart1.xml.rels", opts.chartRels ?? `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${RNS}/package" Target="../embeddings/Microsoft_Excel_Worksheet.xlsx"/></Relationships>`]);
  }
  files.push(...(opts.parts ?? []));
  return zip(files);
}
