// Word's own markup for shapes (the same documents the engine's tests read), as the body of a paragraph each.

export const STYLED_RECT = `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1524000" cy="762000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="3" name="Rectangle 3" descr="A blue rectangle"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm rot="1800000"><a:off x="0" y="0"/><a:ext cx="1524000" cy="762000"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 25000"/></a:avLst></a:prstGeom></wps:spPr><wps:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></wps:style><wps:bodyPr rot="0" spcFirstLastPara="0" vertOverflow="overflow" horzOverflow="overflow" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" numCol="1" spcCol="0" rtlCol="0" fromWordArt="0" anchor="ctr" anchorCtr="0" forceAA="0" compatLnSpc="1"><a:prstTxWarp prst="textNoShape"><a:avLst/></a:prstTxWarp><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:roundrect id="Rectangle 3" o:spid="_x0000_s1027" style="width:120pt;height:60pt;rotation:30" arcsize="16384f" fillcolor="#4472c4" strokecolor="#223962" strokeweight="1pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r>`;

export const ELLIPSE_WITH_TEXT = `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251660288" behindDoc="1" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:align>center</wp:align></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>914400</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="4" name="Oval 4"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"><a:alpha val="50000"/></a:srgbClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:ln><a:noFill/></a:ln></wps:spPr><wps:txbx><w:txbxContent><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Hello oval</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:oval id="Oval 4" o:spid="_x0000_s1028" style="position:absolute;margin-left:0;margin-top:72pt;width:100pt;height:50pt;z-index:-251656192;mso-position-horizontal:center;mso-position-horizontal-relative:page;mso-position-vertical-relative:page" stroked="f"><v:textbox inset="0,0,0,0"><w:txbxContent><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Hello oval</w:t></w:r></w:p></w:txbxContent></v:textbox></v:oval></w:pict></mc:Fallback></mc:AlternateContent></w:r>`;

export const ARROW_LINE = `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="2540000" cy="0"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="8" name="Straight Arrow Connector 8"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvCnPr/><wps:spPr><a:xfrm flipH="1"><a:off x="0" y="0"/><a:ext cx="2540000" cy="0"/></a:xfrm><a:prstGeom prst="straightConnector1"><a:avLst/></a:prstGeom><a:ln w="19050"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:prstDash val="dash"/><a:tailEnd type="triangle"/></a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:line id="Straight Arrow Connector 8" o:spid="_x0000_s1029" style="position:absolute;flip:x;z-index:251661312" from="0,0" to="200pt,0" strokecolor="black" strokeweight="1.5pt"><v:stroke endarrow="block" dashstyle="dash"/></v:line></w:pict></mc:Fallback></mc:AlternateContent></w:r>`;

export const GROUP = `<w:r><mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3048000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="5" name="Group 5"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3048000" cy="1270000"/><a:chOff x="0" y="0"/><a:chExt cx="6096000" cy="2540000"/></a:xfrm></wpg:grpSpPr><wps:wsp><wps:cNvPr id="6" name="Left box"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="7" name="Right oval"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="3556000" y="1270000"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>In the group</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp></wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:group id="Group 5" o:spid="_x0000_s1030" style="width:240pt;height:100pt" coordsize="480,200"><v:rect id="Left box" style="position:absolute;width:200;height:100" fillcolor="red"/><v:oval id="Right oval" style="position:absolute;left:280;top:100;width:200;height:100" fillcolor="lime"><v:textbox><w:txbxContent><w:p><w:r><w:t>In the group</w:t></w:r></w:p></w:txbxContent></v:textbox></v:oval></v:group></w:pict></mc:Fallback></mc:AlternateContent></w:r>`;

export const CANVAS = `<w:r><mc:AlternateContent><mc:Choice Requires="wpc"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5486400" cy="3200400"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="9" name="Canvas 9"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"><wpc:wpc><wpc:bg><a:noFill/></wpc:bg><wpc:whole><a:ln><a:noFill/></a:ln></wpc:whole><wps:wsp><wps:cNvPr id="10" name="Canvas box"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="254000" y="254000"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FFC000"/></a:solidFill></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>Canvas text</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="11" name="Canvas star"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="2540000" y="1270000"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="star5"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="70AD47"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp></wpc:wpc></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect id="Canvas" style="width:432pt;height:252pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r>`;

export const VML_BOX = `<w:r><w:t xml:space="preserve">Old: </w:t></w:r><w:r><w:pict><v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype><v:shape id="_x0000_s1026" type="#_x0000_t202" style="position:absolute;margin-left:72pt;margin-top:36pt;width:144pt;height:72pt;z-index:251658240;mso-position-horizontal-relative:text;mso-position-vertical-relative:text;v-text-anchor:middle" fillcolor="#ffffcc" strokecolor="red" strokeweight="2pt"><v:textbox inset="3.6pt,1.8pt,3.6pt,1.8pt"><w:txbxContent><w:p><w:r><w:t>Old text box</w:t></w:r></w:p></w:txbxContent></v:textbox><w10:wrap type="square"/></v:shape></w:pict></w:r>`;

// -- shape geometry: connectors and freeforms as Word writes them (sizes and offsets are points) --

const GEO_WPS = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";
const geoEmu = (pt: number) => Math.round(pt * 12700);

export interface WordConnector {
  id: number;
  name?: string;
  /** `straightConnector1`, `bentConnector2` to `5` or `curvedConnector2` to `5`. */
  prst: string;
  /** The unturned box, counted from the page like the shapes it joins. */
  x: number;
  y: number;
  w: number;
  h: number;
  rot?: number;
  flipH?: boolean;
  flipV?: boolean;
  /** What the ends are glued to: the drawing id of the shape and the index of its site. */
  start?: { id: number; idx: number };
  end?: { id: number; idx: number };
  adj?: Record<string, number>;
  /** An arrow at the end. */
  arrow?: boolean;
}

/** A connector as Word writes it: `wps:cNvCnPr` with `a:stCxn` and `a:endCxn`, a preset, and the VML copy. */
export function wordConnector(o: WordConnector): string {
  const name = o.name ?? `Connector ${o.id}`;
  const [cx, cy] = [geoEmu(o.w), geoEmu(o.h)];
  const cxn = (o.start ? `<a:stCxn id="${o.start.id}" idx="${o.start.idx}"/>` : "") + (o.end ? `<a:endCxn id="${o.end.id}" idx="${o.end.idx}"/>` : "");
  const av = Object.entries(o.adj ?? {})
    .map(([k, v]) => `<a:gd name="${k}" fmla="val ${v}"/>`)
    .join("");
  const xf = `${o.rot ? ` rot="${Math.round(o.rot * 60000)}"` : ""}${o.flipH ? ' flipH="1"' : ""}${o.flipV ? ' flipV="1"' : ""}`;
  return (
    `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251665408" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">` +
    `<wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>${geoEmu(o.x)}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>${geoEmu(o.y)}</wp:posOffset></wp:positionV>` +
    `<wp:extent cx="${cx}" cy="${cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="${o.id}" name="${name}"/><wp:cNvGraphicFramePr/>` +
    `<a:graphic><a:graphicData uri="${GEO_WPS}"><wps:wsp><wps:cNvCnPr>${cxn}</wps:cNvCnPr><wps:spPr><a:xfrm${xf}><a:off x="0" y="0"/><a:ext cx="${cx}" cy="${cy}"/></a:xfrm>` +
    `<a:prstGeom prst="${o.prst}">${av ? `<a:avLst>${av}</a:avLst>` : "<a:avLst/>"}</a:prstGeom><a:ln w="19050"><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill><a:miter lim="800000"/>${o.arrow ? '<a:tailEnd type="triangle"/>' : ""}</a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice>` +
    `<mc:Fallback><w:pict><v:line id="${name}" o:spid="_x0000_s${3000 + o.id}" style="position:absolute;z-index:251665408;mso-position-horizontal-relative:page;mso-position-vertical-relative:page" from="${o.x}pt,${o.y}pt" to="${o.x + o.w}pt,${o.y + o.h}pt" strokecolor="#4472c4" strokeweight="1.5pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r>`
  );
}

export interface WordFreeform {
  id: number;
  name?: string;
  x: number;
  y: number;
  /** The corners, in points from the box's top-left. They are joined by lines; `curve` makes the last segment a curve. */
  corners: [number, number][];
  /** A curve from the last corner back to the first. */
  curve?: boolean;
  closed?: boolean;
  fill?: string;
}

/** A freeform as Word writes it: `a:custGeom` with a connection site at each corner, and the VML copy. */
export function wordFreeform(o: WordFreeform): string {
  const xs = o.corners.map((c) => c[0]);
  const ys = o.corners.map((c) => c[1]);
  const [w, h] = [Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)];
  const [cx, cy] = [geoEmu(w), geoEmu(h)];
  const pt = (c: [number, number]) => `<a:pt x="${geoEmu(c[0])}" y="${geoEmu(c[1])}"/>`;
  const [first, ...rest] = o.corners;
  let path = `<a:moveTo>${pt(first)}</a:moveTo>` + rest.map((c) => `<a:lnTo>${pt(c)}</a:lnTo>`).join("");
  if (o.curve) path += `<a:cubicBezTo>${pt([w, 0])}${pt([0, 0])}${pt(first)}</a:cubicBezTo>`;
  if (o.closed) path += "<a:close/>";
  const sites = o.corners.map((c) => `<a:cxn ang="0"><a:pos x="${geoEmu(c[0])}" y="${geoEmu(c[1])}"/></a:cxn>`).join("");
  const name = o.name ?? `Freeform: Shape ${o.id}`;
  const fill = o.fill ? `<a:solidFill><a:srgbClr val="${o.fill}"/></a:solidFill>` : "<a:noFill/>";
  const vml = "m" + o.corners.map((c, i) => `${i ? "l" : ""}${geoEmu(c[0])},${geoEmu(c[1])}`).join("") + (o.closed ? "x" : "") + "e";
  return (
    `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251666432" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">` +
    `<wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>${geoEmu(o.x)}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>${geoEmu(o.y)}</wp:posOffset></wp:positionV>` +
    `<wp:extent cx="${cx}" cy="${cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="${o.id}" name="${name}"/><wp:cNvGraphicFramePr/>` +
    `<a:graphic><a:graphicData uri="${GEO_WPS}"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${cx}" cy="${cy}"/></a:xfrm>` +
    `<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst>${sites}</a:cxnLst><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="${cx}" h="${cy}">${path}</a:path></a:pathLst></a:custGeom>` +
    `${fill}<a:ln w="19050"><a:solidFill><a:srgbClr val="2F528F"/></a:solidFill><a:miter lim="800000"/></a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice>` +
    `<mc:Fallback><w:pict><v:shape id="${name}" o:spid="_x0000_s${3100 + o.id}" style="position:absolute;margin-left:${o.x}pt;margin-top:${o.y}pt;width:${w}pt;height:${h}pt;z-index:251666432;mso-position-horizontal-relative:page;mso-position-vertical-relative:page" coordsize="${cx},${cy}" o:spt="100" path="${vml}" filled="${o.fill ? "t" : "f"}" strokecolor="#2f528f" strokeweight="1.5pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r>`
  );
}
