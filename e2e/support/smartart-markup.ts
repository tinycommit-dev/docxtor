// Word's own markup for SmartArt and WordArt, for the browser tests: the drawing part Word saves with a graphic
// (shapes with their fill, line, geometry and text), the data model, and WordArt in DrawingML and in VML.

export const A = "http://schemas.openxmlformats.org/drawingml/2006/main";
export const DGM = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
export const DSP = "http://schemas.microsoft.com/office/drawing/2008/diagram";

const EMU = 12700;
const e = (pt: number) => Math.round(pt * EMU);

/** A shape of Word's diagram drawing (`dsp:sp`), in points inside the frame. */
export interface DspShape {
  x: number;
  y: number;
  w: number;
  h: number;
  prst?: string;
  adj?: Record<string, number>;
  /** Scheme colour name, or a hex for a plain RGB fill. */
  fill?: string;
  /** A shape-level tint, as a percentage kept of the colour (60 is a 60% tint). */
  tint?: number;
  rot?: number;
  flipV?: boolean;
  text?: string;
  size?: number;
  bold?: boolean;
  /** Text colour as a scheme name (the style's font reference), white by default. */
  textColor?: string;
  line?: string;
}

const color = (c: string, tint?: number) => (/^[0-9A-Fa-f]{6}$/.test(c) ? `<a:srgbClr val="${c}"/>` : `<a:schemeClr val="${c}">${tint ? `<a:tint val="${tint * 1000}"/>` : ""}<a:hueOff val="0"/><a:satOff val="0"/><a:lumOff val="0"/><a:alphaOff val="0"/></a:schemeClr>`);

export function dspShape(i: number, s: DspShape): string {
  const rot = s.rot ? ` rot="${Math.round(s.rot * 60000)}"` : "";
  const flip = s.flipV ? ' flipV="1"' : "";
  const av = Object.entries(s.adj ?? {}).map(([k, v]) => `<a:gd name="${k}" fmla="val ${v}"/>`).join("");
  const line = s.line ?? "lt1";
  const body = s.text
    ? `<dsp:txBody><a:bodyPr spcFirstLastPara="0" vert="horz" wrap="square" lIns="53340" tIns="53340" rIns="53340" bIns="53340" numCol="1" spcCol="1270" anchor="ctr" anchorCtr="0"><a:noAutofit/></a:bodyPr><a:lstStyle/><a:p><a:pPr marL="0" lvl="0" indent="0" algn="ctr" defTabSz="${Math.round((s.size ?? 14) * 44450)}"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPct val="0"/></a:spcBef><a:spcAft><a:spcPct val="35000"/></a:spcAft><a:buNone/></a:pPr><a:r><a:rPr lang="en-US" sz="${Math.round((s.size ?? 14) * 100)}"${s.bold ? ' b="1"' : ""} kern="1200" dirty="0"/><a:t>${s.text}</a:t></a:r></a:p></dsp:txBody>`
    : `<dsp:txBody><a:bodyPr spcFirstLastPara="0" vert="horz" wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" numCol="1" spcCol="1270" anchor="ctr" anchorCtr="0"><a:noAutofit/></a:bodyPr><a:lstStyle/><a:p><a:endParaRPr lang="en-US" sz="1000" kern="1200"/></a:p></dsp:txBody>`;
  return (
    `<dsp:sp modelId="{S${i}}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm${rot}${flip}><a:off x="${e(s.x)}" y="${e(s.y)}"/><a:ext cx="${e(s.w)}" cy="${e(s.h)}"/></a:xfrm><a:prstGeom prst="${s.prst ?? "rect"}"><a:avLst>${av}</a:avLst></a:prstGeom>` +
    `<a:solidFill>${color(s.fill ?? "accent1", s.tint)}</a:solidFill><a:ln w="12700" cap="flat" cmpd="sng" algn="ctr"><a:solidFill>${color(line)}</a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:effectLst/></dsp:spPr>` +
    `<dsp:style><a:lnRef idx="2"><a:scrgbClr r="0" g="0" b="0"/></a:lnRef><a:fillRef idx="1"><a:scrgbClr r="0" g="0" b="0"/></a:fillRef><a:effectRef idx="0"><a:scrgbClr r="0" g="0" b="0"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="${s.textColor ?? "lt1"}"/></a:fontRef></dsp:style>${body}` +
    `<dsp:txXfrm><a:off x="${e(s.x)}" y="${e(s.y)}"/><a:ext cx="${e(s.w)}" cy="${e(s.h)}"/></dsp:txXfrm></dsp:sp>`
  );
}

/** A `dsp:drawing` of shapes, as Word saves in `word/diagrams/drawingN.xml`. */
export function dspDrawing(shapes: DspShape[]): string {
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dsp:drawing xmlns:dgm="${DGM}" xmlns:dsp="${DSP}" xmlns:a="${A}"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/>${shapes.map((s, i) => dspShape(i, s)).join("")}</dsp:spTree></dsp:drawing>`;
}

/** Word's basic block list for `n` nodes in a frame of `w` by `h` points (the snake layout: boxes 0.6 as tall as wide). */
export function blockListShapes(texts: string[], w = 432, h = 252): DspShape[] {
  const n = texts.length;
  let best = { cols: 1, bw: 0 };
  for (let cols = 1; cols <= n; cols++) {
    const rows = Math.ceil(n / cols);
    const bw = Math.min(w / (cols + 0.1 * (cols - 1)), h / (0.6 * rows + 0.1 * (rows - 1)));
    if (bw > best.bw + 1e-9) best = { cols, bw };
  }
  const { cols, bw } = best;
  const bh = bw * 0.6;
  const gap = bw * 0.1;
  const rows = Math.ceil(n / cols);
  const top = (h - (rows * bh + (rows - 1) * gap)) / 2;
  return texts.map((text, i) => {
    const r = Math.floor(i / cols);
    const inRow = r === rows - 1 ? n - r * cols : cols;
    const left = (w - (inRow * bw + (inRow - 1) * gap)) / 2;
    return { x: left + (i % cols) * (bw + gap), y: top + r * (bh + gap), w: bw, h: bh, text, size: 20, bold: false };
  });
}

/** Word's basic process: rounded boxes with a small arrow in the gap between each pair. */
export function processShapes(texts: string[], w = 432, h = 252): DspShape[] {
  const n = texts.length;
  const a = Math.min(w / (n + (n - 1) * 0.4), h / 0.6);
  const bh = a * 0.6;
  const gap = a * 0.4;
  const left = (w - (n * a + (n - 1) * gap)) / 2;
  const top = (h - bh) / 2;
  const out: DspShape[] = [];
  texts.forEach((text, i) => {
    const x = left + i * (a + gap);
    out.push({ x, y: top, w: a, h: bh, prst: "roundRect", adj: { adj: 10000 }, text, size: 18, fill: "accent1" });
    if (i < n - 1) out.push({ x: x + a + gap * 0.235, y: top + (bh - gap * 0.6) / 2, w: gap * 0.53, h: gap * 0.6, prst: "rightArrow", adj: { adj1: 60000, adj2: 50000 }, fill: "accent1", tint: 60 });
  });
  return out;
}

/** Word's basic cycle: ellipses on a circle with an arrow between neighbours, turned along the circle. */
export function cycleShapes(texts: string[], w = 300, h = 300): DspShape[] {
  const n = texts.length;
  const s = Math.min(w, h);
  const sin = Math.sin(Math.PI / n);
  const r = s / 2 / (1 + 0.6 * sin);
  const d = Math.min(0.6 * 2 * r * sin, s * 0.45);
  const out: DspShape[] = [];
  const at = (i: number) => -Math.PI / 2 + (2 * Math.PI * i) / n;
  texts.forEach((text, i) => {
    out.push({ x: w / 2 + r * Math.cos(at(i)) - d / 2, y: h / 2 + r * Math.sin(at(i)) - d / 2, w: d, h: d, prst: "ellipse", text, size: 14, fill: "accent1" });
  });
  const chord = 2 * r * sin;
  const len = Math.max((chord - d) * 0.6, d * 0.25);
  texts.forEach((_, i) => {
    const mid = at(i) + Math.PI / n;
    out.push({ x: w / 2 + r * Math.cos(mid) - len / 2, y: h / 2 + r * Math.sin(mid) - (len * 1.1) / 2, w: len, h: len * 1.1, prst: "rightArrow", adj: { adj1: 60000, adj2: 50000 }, rot: ((mid * 180) / Math.PI + 90 + 360) % 360, fill: "accent1", tint: 60 });
  });
  return out;
}

/** A node of a diagram's data model: its text, or its text and the nodes below it. */
export type Tree = string | { text: string; children?: Tree[] };

/** The data model of a diagram with the given nodes, and the drawing's relationship id if it has one. */
export function dataModel(layout: string, nodes: Tree[], drawingRid?: string): string {
  let pts = `<dgm:pt modelId="{DOC}" type="doc"><dgm:prSet loTypeId="urn:microsoft.com/office/officeart/2005/8/layout/${layout}" loCatId="list" qsTypeId="urn:microsoft.com/office/officeart/2005/8/quickstyle/simple1" qsCatId="simple" csTypeId="urn:microsoft.com/office/officeart/2005/8/colors/accent1_2" csCatId="accent1" phldr="0"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></dgm:t></dgm:pt>`;
  let cx = "";
  let n = 0;
  const walk = (list: Tree[], parent: string) => {
    list.forEach((node, i) => {
      const id = `{N${n++}}`;
      const text = typeof node === "string" ? node : node.text;
      pts += `<dgm:pt modelId="${id}"><dgm:prSet phldrT="[Text]"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" dirty="0"/><a:t>${text}</a:t></a:r></a:p></dgm:t></dgm:pt>`;
      cx += `<dgm:cxn modelId="{C${n}}" srcId="${parent}" destId="${id}" srcOrd="${i}" destOrd="0"/>`;
      if (typeof node !== "string" && node.children) walk(node.children, id);
    });
  };
  walk(nodes, "{DOC}");
  const ext = drawingRid ? `<dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="${DSP}" relId="${drawingRid}" minVer="http://schemas.openxmlformats.org/drawingml/2006/diagram"/></a:ext></dgm:extLst>` : "";
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:dataModel xmlns:dgm="${DGM}" xmlns:a="${A}"><dgm:ptLst>${pts}</dgm:ptLst><dgm:cxnLst>${cx}</dgm:cxnLst><dgm:bg/><dgm:whole/>${ext}</dgm:dataModel>`;
}

export const layoutPart = (name: string) =>
  `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:layoutDef xmlns:dgm="${DGM}" xmlns:a="${A}" uniqueId="urn:microsoft.com/office/officeart/2005/8/layout/${name}"><dgm:title val=""/><dgm:desc val=""/><dgm:layoutNode name="diagram"><dgm:alg type="snake"/></dgm:layoutNode></dgm:layoutDef>`;

export const stylePart = () => `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:styleDef xmlns:dgm="${DGM}" xmlns:a="${A}" uniqueId="urn:microsoft.com/office/officeart/2005/8/quickstyle/simple1"><dgm:title val=""/><dgm:desc val=""/></dgm:styleDef>`;

export const colorsPart = () =>
  `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:colorsDef xmlns:dgm="${DGM}" xmlns:a="${A}" uniqueId="urn:microsoft.com/office/officeart/2005/8/colors/accent1_2"><dgm:title val=""/><dgm:desc val=""/><dgm:styleLbl name="node1"><dgm:fillClrLst meth="repeat"><a:schemeClr val="accent1"/></dgm:fillClrLst><dgm:linClrLst meth="repeat"><a:schemeClr val="lt1"/></dgm:linClrLst><dgm:effectClrLst/><dgm:txLinClrLst/><dgm:txFillClrLst/><dgm:txEffectClrLst/></dgm:styleLbl></dgm:colorsDef>`;

export const THEME = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="${A}" name="Office"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/></a:minorFont></a:fontScheme><a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"/><a:ln w="12700"/><a:ln w="19050"/></a:lnStyleLst></a:fmtScheme></a:themeElements></a:theme>`;

/** The paragraph that holds a SmartArt graphic: an inline drawing (or an anchored one) with `dgm:relIds`. */
export function diagramRun(ids: { dm: string; lo: string; qs: string; cs: string }, o: { w: number; h: number; id?: number; alt?: string; anchor?: { x: number; y: number } }): string {
  const rel = `<dgm:relIds xmlns:dgm="${DGM}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:dm="${ids.dm}" r:lo="${ids.lo}" r:qs="${ids.qs}" r:cs="${ids.cs}"/>`;
  const pre = `<wp:extent cx="${e(o.w)}" cy="${e(o.h)}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>`;
  const post = `<wp:docPr id="${o.id ?? 1}" name="Diagram ${o.id ?? 1}"${o.alt ? ` descr="${o.alt}"` : ""}/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="${A}"><a:graphicData uri="${DGM}">${rel}</a:graphicData></a:graphic>`;
  const drawing = o.anchor
    ? `<wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>${e(o.anchor.x)}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>${e(o.anchor.y)}</wp:posOffset></wp:positionV>${pre}<wp:wrapSquare wrapText="bothSides"/>${post}</wp:anchor>`
    : `<wp:inline distT="0" distB="0" distL="0" distR="0">${pre}${post}</wp:inline>`;
  return `<w:r><w:rPr><w:noProof/></w:rPr><w:drawing>${drawing}</w:drawing></w:r>`;
}

// ---- WordArt ---------------------------------------------------------------------------------------------

export interface WordArtOpts {
  /** The DrawingML preset (`textArchUp`, `textWave1`, ...). */
  prst: string;
  av?: string;
  text?: string;
  /** Extra run properties (`w14:textFill`, `w14:textOutline`, ...). */
  rpr?: string;
  /** Size in points. */
  w?: number;
  h?: number;
  alt?: string;
  id?: number;
  size?: number;
  /** The shape's own fill and outline (`wps:spPr` children after the geometry). */
  shapePr?: string;
}

/** WordArt as Word 2010 and later saves it: a text-box shape with `a:prstTxWarp`, text properties in the run. */
export function wordArtRun(o: WordArtOpts): string {
  const w = o.w ?? 300;
  const h = o.h ?? 100;
  const text = o.text ?? "Hello WordArt";
  return (
    `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="${e(w)}" cy="${e(h)}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="${o.id ?? 2}" name="WordArt ${o.id ?? 2}"${o.alt ? ` descr="${o.alt}"` : ""}/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${e(w)}" cy="${e(h)}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>${o.shapePr ?? "<a:noFill/><a:ln><a:noFill/></a:ln>"}</wps:spPr>` +
    `<wps:txbx><w:txbxContent><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val="${(o.size ?? 48) * 2}"/>${o.rpr ?? ""}</w:rPr><w:t>${text}</w:t></w:r></w:p></w:txbxContent></wps:txbx>` +
    `<wps:bodyPr rot="0" spcFirstLastPara="0" vertOverflow="overflow" horzOverflow="overflow" vert="horz" wrap="none" lIns="91440" tIns="45720" rIns="91440" bIns="45720" numCol="1" spcCol="0" rtlCol="0" fromWordArt="1" anchor="ctr" anchorCtr="0" forceAA="0" compatLnSpc="1"><a:prstTxWarp prst="${o.prst}">${o.av ?? "<a:avLst/>"}</a:prstTxWarp><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice>` +
    `<mc:Fallback><w:pict><v:shape id="WordArt ${o.id ?? 2}" o:spid="_x0000_s${1026 + (o.id ?? 2)}" type="#_x0000_t136" style="width:${w}pt;height:${h}pt" fillcolor="#4472c4" stroked="f"><v:textpath style="font-family:&quot;Calibri&quot;;font-size:${o.size ?? 48}pt;font-weight:bold" string="${text}"/></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r>`
  );
}

/** The `v:shapetype` WordArt (`o:spt` 136 to 175) uses. */
export const wordArtType = (spt: number) =>
  `<v:shapetype id="_x0000_t${spt}" coordsize="21600,21600" o:spt="${spt}" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/><o:lock v:ext="edit" text="t" shapetype="t"/></v:shapetype>`;

/** WordArt from Word 97 to 2007: a VML shape whose text is in `v:textpath`. */
export function vmlWordArt(o: { spt: number; text: string; w?: number; h?: number; fill?: string; stroke?: string; font?: string; id?: number; extra?: string }): string {
  return (
    `<w:r><w:pict>${wordArtType(o.spt)}<v:shape id="WordArt${o.id ?? 1}" o:spid="_x0000_s${1026 + (o.id ?? 1)}" type="#_x0000_t${o.spt}" style="position:static;width:${o.w ?? 300}pt;height:${o.h ?? 90}pt;mso-position-horizontal-relative:text;mso-position-vertical-relative:text" ` +
    `fillcolor="${o.fill ?? "#c00000"}" strokecolor="${o.stroke ?? "#600000"}" strokeweight="1pt">${o.extra ?? ""}<v:textpath style="font-family:&quot;${o.font ?? "Impact"}&quot;;font-size:36pt;v-text-kern:t" string="${o.text}"/></v:shape></w:pict></w:r>`
  );
}
