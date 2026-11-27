// Shapes with looks (fills, outlines, effects, text frame) written the way Word writes them, for the display tests.

export interface LooksSpec {
  /** Points. */
  w?: number;
  h?: number;
  geometry?: string;
  /** What goes in `wps:spPr` after the geometry: a fill, `a:ln`, effects, 3-D. */
  sppr?: string;
  /** `wps:style`. */
  style?: string;
  /** The whole `wps:bodyPr`. */
  body?: string;
  /** Text paragraphs inside, one per entry. */
  text?: string[];
  /** The VML fallback shape. */
  fallback?: string;
  id?: number;
}

const EMU = 12700;

/** One shape the way Word writes it (`mc:AlternateContent` with the DrawingML choice and a VML fallback), as a run. */
export function looksShape(o: LooksSpec = {}): string {
  const w = o.w ?? 100;
  const h = o.h ?? 50;
  const id = o.id ?? 30;
  const cx = Math.round(w * EMU);
  const cy = Math.round(h * EMU);
  const body = o.body ?? '<wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr>';
  const text = o.text ? `<wps:txbx><w:txbxContent>${o.text.map((t) => `<w:p><w:r><w:t>${t}</w:t></w:r></w:p>`).join("")}</w:txbxContent></wps:txbx>` : "";
  const fallback = o.fallback ?? `<w:pict><v:rect id="Shape ${id}" o:spid="_x0000_s${1000 + id}" style="width:${w}pt;height:${h}pt" fillcolor="#4472c4" strokecolor="#223962"/></w:pict>`;
  return (
    `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="${cx}" cy="${cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="${id}" name="Shape ${id}"/><wp:cNvGraphicFramePr/>` +
    `<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${cx}" cy="${cy}"/></a:xfrm><a:prstGeom prst="${o.geometry ?? "rect"}"><a:avLst/></a:prstGeom>${o.sppr ?? ""}</wps:spPr>${o.style ?? ""}${text}${body}</wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice>` +
    `<mc:Fallback>${fallback}</mc:Fallback></mc:AlternateContent></w:r>`
  );
}

/** A gradient fill: stops are `[position 0 to 100, hex, alpha 0 to 1?]`; `lin` is the angle in degrees, `path` a path gradient. */
export function gradFill(stops: [number, string, number?][], o: { lin?: number; scaled?: boolean; path?: "circle" | "rect" | "shape"; rotWithShape?: boolean } = {}): string {
  const gs = stops.map(([p, c, a]) => `<a:gs pos="${p * 1000}"><a:srgbClr val="${c}">${a !== undefined && a < 1 ? `<a:alpha val="${a * 100000}"/>` : ""}</a:srgbClr></a:gs>`).join("");
  const dir = o.path
    ? `<a:path path="${o.path}"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path>`
    : `<a:lin ang="${Math.round((o.lin ?? 90) * 60000)}" scaled="${o.scaled ? 1 : 0}"/>`;
  return `<a:gradFill rotWithShape="${o.rotWithShape === false ? 0 : 1}"><a:gsLst>${gs}</a:gsLst>${dir}</a:gradFill>`;
}

export const solidFill = (hex: string, alpha?: number) => `<a:solidFill><a:srgbClr val="${hex}">${alpha !== undefined ? `<a:alpha val="${alpha * 100000}"/>` : ""}</a:srgbClr></a:solidFill>`;

export const pattFill = (prst: string, fg: string, bg: string) => `<a:pattFill prst="${prst}"><a:fgClr><a:srgbClr val="${fg}"/></a:fgClr><a:bgClr><a:srgbClr val="${bg}"/></a:bgClr></a:pattFill>`;

export const lineXml = (hex: string, wPt: number, attrs = "", children = "") => `<a:ln w="${Math.round(wPt * EMU)}"${attrs}><a:solidFill><a:srgbClr val="${hex}"/></a:solidFill>${children}</a:ln>`;

/** Every DrawingML preset pattern. */
export const PATTERN_PRESETS = [
  "pct5", "pct10", "pct20", "pct25", "pct30", "pct40", "pct50", "pct60", "pct70", "pct75", "pct80", "pct90", "horz", "vert", "ltHorz", "ltVert", "dkHorz",
  "dkVert", "narHorz", "narVert", "dashHorz", "dashVert", "cross", "dnDiag", "upDiag", "ltDnDiag", "ltUpDiag", "dkDnDiag", "dkUpDiag", "wdDnDiag",
  "wdUpDiag", "dashDnDiag", "dashUpDiag", "diagCross", "smCheck", "lgCheck", "smGrid", "lgGrid", "dotGrid", "smConfetti", "lgConfetti", "horzBrick",
  "diagBrick", "solidDmnd", "openDmnd", "dotDmnd", "plaid", "sphere", "weave", "divot", "shingle", "wave", "trellis", "zigZag",
];

/** Shadow, glow and the rest as Word writes them in `a:effectLst`. */
export const FX = {
  shadow: '<a:outerShdw blurRad="63500" dist="50800" dir="2700000" algn="tl" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="45000"/></a:srgbClr></a:outerShdw>',
  inner: '<a:innerShdw blurRad="76200" dist="50800" dir="2700000"><a:srgbClr val="000000"><a:alpha val="60000"/></a:srgbClr></a:innerShdw>',
  glow: '<a:glow rad="152400"><a:srgbClr val="FFC000"><a:alpha val="45000"/></a:srgbClr></a:glow>',
  soft: '<a:softEdge rad="127000"/>',
  reflection: '<a:reflection blurRad="6350" stA="50000" stPos="0" endA="300" endPos="55000" dist="0" dir="5400000" sy="-100000" algn="bl" rotWithShape="0"/>',
};

export const effectLst = (...parts: string[]) => `<a:effectLst>${parts.join("")}</a:effectLst>`;

/** A bevel and an extrusion, with the scene they need. */
export const threeD = (o: { camera?: string; bevel?: string; bevelPt?: number; depthPt?: number; color?: string }) =>
  `<a:scene3d><a:camera prst="${o.camera ?? "orthographicFront"}"><a:rot lat="0" lon="0" rev="0"/></a:camera><a:lightRig rig="threePt" dir="t"/></a:scene3d>` +
  `<a:sp3d${o.depthPt ? ` extrusionH="${Math.round(o.depthPt * EMU)}"` : ""} prstMaterial="matte">${o.bevel ? `<a:bevelT w="${Math.round((o.bevelPt ?? 6) * EMU)}" h="${Math.round((o.bevelPt ?? 6) * EMU)}" prst="${o.bevel}"/>` : ""}${o.color ? `<a:extrusionClr><a:srgbClr val="${o.color}"/></a:extrusionClr>` : ""}</a:sp3d>`;

/** A `wps:bodyPr` with the settings that matter for the text frame. */
export function bodyPr(o: { vert?: string; wrap?: string; anchor?: string; anchorCtr?: boolean; cols?: number; gapPt?: number; ins?: [number, number, number, number]; fit?: string; upright?: boolean } = {}): string {
  const [l, t, r, b] = (o.ins ?? [7.2, 3.6, 7.2, 3.6]).map((v) => Math.round(v * EMU));
  return (
    `<wps:bodyPr rot="0" vert="${o.vert ?? "horz"}" wrap="${o.wrap ?? "square"}" lIns="${l}" tIns="${t}" rIns="${r}" bIns="${b}" numCol="${o.cols ?? 1}" spcCol="${Math.round((o.gapPt ?? 0) * EMU)}" anchor="${o.anchor ?? "t"}" anchorCtr="${o.anchorCtr ? 1 : 0}"${o.upright ? ' upright="1"' : ""}>` +
    `${o.fit ?? "<a:noAutofit/>"}</wps:bodyPr>`
  );
}

/** A paragraph's worth of shapes in a row, as `{raw}` for `openBuilt`. */
export const row = (...shapes: string[]) => ({ raw: shapes.join('<w:r><w:t xml:space="preserve"> </w:t></w:r>') });
