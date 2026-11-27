// Table markup for the browser tests: tables written out as Word's XML, and the definitions of some of Word's built-in
// table styles as Word writes them (so the tests do not depend on how Docxtor writes them).

/** `w:tblLook` as Word writes it. */
export const LOOK = (v: string, f: { firstRow?: boolean; lastRow?: boolean; firstColumn?: boolean; lastColumn?: boolean; noHBand?: boolean; noVBand?: boolean } = {}) => {
  const b = (x?: boolean) => (x ? "1" : "0");
  return `<w:tblLook w:val="${v}" w:firstRow="${b(f.firstRow)}" w:lastRow="${b(f.lastRow)}" w:firstColumn="${b(f.firstColumn)}" w:lastColumn="${b(f.lastColumn)}" w:noHBand="${b(f.noHBand)}" w:noVBand="${b(f.noVBand)}"/>`;
};

/** Word's default look for a new table: header row, first column, banded rows. */
export const LOOK_DEFAULT = LOOK("04A0", { firstRow: true, firstColumn: true, noVBand: true });

export interface TableSpec {
  rows: number;
  cols: number;
  /** Content of `w:tblPr`. */
  tblPr?: string;
  /** Content of every row's `w:trPr`. */
  trPr?: string;
  /** Column width in twips. */
  colW?: number;
  /** The text of a cell; `r1c2` by default. */
  text?: (r: number, c: number) => string;
  /** Extra `w:tcPr` content of a cell. */
  tcPr?: (r: number, c: number) => string;
}

/** A table with the text `r0c0`, `r0c1`... in its cells, as a block for `DocSpec.paragraphs` (`{ body: … }`). */
export function table(s: TableSpec): { body: string } {
  const w = s.colW ?? 2000;
  const text = s.text ?? ((r, c) => `r${r}c${c}`);
  let x = `<w:tbl><w:tblPr>${s.tblPr ?? ""}</w:tblPr><w:tblGrid>${`<w:gridCol w:w="${w}"/>`.repeat(s.cols)}</w:tblGrid>`;
  for (let r = 0; r < s.rows; r++) {
    x += `<w:tr>${s.trPr ? `<w:trPr>${s.trPr}</w:trPr>` : ""}`;
    for (let c = 0; c < s.cols; c++) x += `<w:tc><w:tcPr><w:tcW w:w="${w}" w:type="dxa"/>${s.tcPr?.(r, c) ?? ""}</w:tcPr><w:p><w:r><w:t>${text(r, c)}</w:t></w:r></w:p></w:tc>`;
    x += "</w:tr>";
  }
  return { body: `${x}</w:tbl>` };
}

export const TABLE_NORMAL =
  '<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>';

const line = (name: string, sz: number, color: string, theme: string, tint = "", val = "single") =>
  `<w:${name} w:val="${val}" w:sz="${sz}" w:space="0" w:color="${color}" w:themeColor="${theme}"${tint}/>`;

/** Grid Table 4 Accent 1, written the way Word writes it. */
export const GRID_TABLE_4_ACCENT_1 = `<w:style w:type="table" w:styleId="GridTable4-Accent1"><w:name w:val="Grid Table 4 Accent 1"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="49"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblStyleRowBandSize w:val="1"/><w:tblStyleColBandSize w:val="1"/><w:tblBorders>${["top", "left", "bottom", "right", "insideH", "insideV"].map((n) => line(n, 4, "8EAADB", "accent1", ' w:themeTint="99"')).join("")}</w:tblBorders></w:tblPr><w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:bCs/><w:color w:val="FFFFFF" w:themeColor="background1"/></w:rPr><w:tblPr/><w:tcPr><w:tcBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="4472C4" w:themeColor="accent1"/><w:left w:val="single" w:sz="4" w:space="0" w:color="4472C4" w:themeColor="accent1"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="4472C4" w:themeColor="accent1"/><w:right w:val="single" w:sz="4" w:space="0" w:color="4472C4" w:themeColor="accent1"/></w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="4472C4" w:themeFill="accent1"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="lastRow"><w:rPr><w:b/><w:bCs/></w:rPr><w:tblPr/><w:tcPr><w:tcBorders><w:top w:val="double" w:sz="4" w:space="0" w:color="4472C4" w:themeColor="accent1"/></w:tcBorders></w:tcPr></w:tblStylePr><w:tblStylePr w:type="firstCol"><w:rPr><w:b/><w:bCs/></w:rPr></w:tblStylePr><w:tblStylePr w:type="lastCol"><w:rPr><w:b/><w:bCs/></w:rPr></w:tblStylePr><w:tblStylePr w:type="band1Vert"><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="D9E2F3" w:themeFill="accent1" w:themeFillTint="33"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Horz"><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="D9E2F3" w:themeFill="accent1" w:themeFillTint="33"/></w:tcPr></w:tblStylePr></w:style>`;

/** Plain Table 1, written the way Word writes it. */
export const PLAIN_TABLE_1 = `<w:style w:type="table" w:styleId="PlainTable1"><w:name w:val="Plain Table 1"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="41"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblStyleRowBandSize w:val="1"/><w:tblStyleColBandSize w:val="1"/><w:tblBorders>${line("top", 4, "BFBFBF", "background1", ' w:themeShade="BF"')}${line("bottom", 4, "BFBFBF", "background1", ' w:themeShade="BF"')}</w:tblBorders></w:tblPr><w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:bCs/></w:rPr><w:tblPr/><w:tcPr><w:tcBorders>${line("bottom", 4, "BFBFBF", "background1", ' w:themeShade="BF"')}</w:tcBorders></w:tcPr></w:tblStylePr><w:tblStylePr w:type="lastRow"><w:rPr><w:b/><w:bCs/></w:rPr><w:tblPr/><w:tcPr><w:tcBorders>${line("top", 4, "BFBFBF", "background1", ' w:themeShade="BF"')}</w:tcBorders></w:tcPr></w:tblStylePr><w:tblStylePr w:type="firstCol"><w:rPr><w:b/><w:bCs/></w:rPr></w:tblStylePr><w:tblStylePr w:type="lastCol"><w:rPr><w:b/><w:bCs/></w:rPr></w:tblStylePr><w:tblStylePr w:type="band1Vert"><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2" w:themeFill="background1" w:themeFillShade="F2"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Horz"><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2" w:themeFill="background1" w:themeFillShade="F2"/></w:tcPr></w:tblStylePr></w:style>`;

/** List Table 3, written the way Word writes it. */
export const LIST_TABLE_3 = `<w:style w:type="table" w:styleId="ListTable3"><w:name w:val="List Table 3"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="48"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblStyleRowBandSize w:val="1"/><w:tblStyleColBandSize w:val="1"/><w:tblBorders>${["top", "left", "bottom", "right"].map((n) => line(n, 4, "000000", "text1")).join("")}</w:tblBorders></w:tblPr><w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:bCs/><w:color w:val="FFFFFF" w:themeColor="background1"/></w:rPr><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="000000" w:themeFill="text1"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="lastRow"><w:rPr><w:b/><w:bCs/></w:rPr><w:tblPr/><w:tcPr><w:tcBorders>${line("top", 6, "000000", "text1", "", "double")}</w:tcBorders></w:tcPr></w:tblStylePr><w:tblStylePr w:type="firstCol"><w:rPr><w:b/><w:bCs/></w:rPr></w:tblStylePr><w:tblStylePr w:type="lastCol"><w:rPr><w:b/><w:bCs/></w:rPr></w:tblStylePr><w:tblStylePr w:type="band1Vert"><w:tblPr/><w:tcPr><w:tcBorders>${line("left", 4, "000000", "text1")}${line("right", 4, "000000", "text1")}</w:tcBorders></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Horz"><w:tblPr/><w:tcPr><w:tcBorders>${line("top", 4, "000000", "text1")}${line("bottom", 4, "000000", "text1")}</w:tcBorders></w:tcPr></w:tblStylePr></w:style>`;

/** Grid Table 5 Dark, written the way Word writes it. */
export const GRID_TABLE_5_DARK = `<w:style w:type="table" w:styleId="GridTable5Dark"><w:name w:val="Grid Table 5 Dark"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="50"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblStyleRowBandSize w:val="1"/><w:tblStyleColBandSize w:val="1"/><w:tblBorders>${["top", "left", "bottom", "right", "insideH", "insideV"].map((n) => line(n, 4, "FFFFFF", "background1")).join("")}</w:tblBorders></w:tblPr><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="CBCBCB" w:themeFill="text1" w:themeFillTint="33"/></w:tcPr><w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:bCs/><w:color w:val="FFFFFF" w:themeColor="background1"/></w:rPr><w:tblPr/><w:tcPr><w:tcBorders>${line("bottom", 12, "FFFFFF", "background1")}</w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="000000" w:themeFill="text1"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="lastRow"><w:rPr><w:b/><w:bCs/><w:color w:val="FFFFFF" w:themeColor="background1"/></w:rPr><w:tblPr/><w:tcPr><w:tcBorders>${line("top", 12, "FFFFFF", "background1")}</w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="000000" w:themeFill="text1"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="firstCol"><w:rPr><w:b/><w:bCs/><w:color w:val="FFFFFF" w:themeColor="background1"/></w:rPr><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="000000" w:themeFill="text1"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="lastCol"><w:rPr><w:b/><w:bCs/><w:color w:val="FFFFFF" w:themeColor="background1"/></w:rPr><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="000000" w:themeFill="text1"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Vert"><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="999999" w:themeFill="text1" w:themeFillTint="66"/></w:tcPr></w:tblStylePr><w:tblStylePr w:type="band1Horz"><w:tblPr/><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="999999" w:themeFill="text1" w:themeFillTint="66"/></w:tcPr></w:tblStylePr></w:style>`;

/** Table Grid, the style Word gives a new table. */
export const TABLE_GRID =
  '<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="39"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr></w:style>';
