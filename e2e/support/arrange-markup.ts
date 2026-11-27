// Floating shapes written the way Word writes them, with what the position counts from and the stacking value
// spelled out, for the tests of arranging objects. Sizes and offsets are points.

export interface FloatSpec {
  id: number;
  name?: string;
  w?: number;
  h?: number;
  /** What the horizontal position counts from. */
  relH?: string;
  /** An offset in points, or an alignment (`left`, `center`, `right`). */
  x?: number | string;
  relV?: string;
  y?: number | string;
  /** `relativeHeight`. */
  z?: number;
  behind?: boolean;
  fill?: string;
  /** Text inside the shape. */
  text?: string;
  /** `wrapNone` (the default, in front of or behind the text) or `wrapSquare`. */
  wrap?: "none" | "square";
}

const emu = (pt: number) => Math.round(pt * 12700);
const NS_WPS = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";

/** The runs of one floating rectangle (an `mc:AlternateContent` with the DrawingML choice and the VML fallback). */
export function floatShape(o: FloatSpec): string {
  const w = o.w ?? 100;
  const h = o.h ?? 60;
  const pos = (v: number | string | undefined) => (typeof v === "string" ? `<wp:align>${v}</wp:align>` : `<wp:posOffset>${emu(v ?? 0)}</wp:posOffset>`);
  const z = o.z ?? 251658240;
  const fill = o.fill ?? "4472C4";
  const name = o.name ?? `Rectangle ${o.id}`;
  const text = o.text ? `<wps:txbx><w:txbxContent><w:p><w:r><w:t>${o.text}</w:t></w:r></w:p></w:txbxContent></wps:txbx>` : "";
  const vmlText = o.text ? `<v:textbox><w:txbxContent><w:p><w:r><w:t>${o.text}</w:t></w:r></w:p></w:txbxContent></v:textbox>` : "";
  const wrap = o.wrap === "square" ? '<wp:wrapSquare wrapText="bothSides"/>' : "<wp:wrapNone/>";
  return (
    `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="${z}" behindDoc="${o.behind ? 1 : 0}" locked="0" layoutInCell="1" allowOverlap="1">` +
    `<wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="${o.relH ?? "column"}">${pos(o.x)}</wp:positionH><wp:positionV relativeFrom="${o.relV ?? "paragraph"}">${pos(o.y)}</wp:positionV>` +
    `<wp:extent cx="${emu(w)}" cy="${emu(h)}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>${wrap}<wp:docPr id="${o.id}" name="${name}"/><wp:cNvGraphicFramePr/>` +
    `<a:graphic><a:graphicData uri="${NS_WPS}"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="${emu(w)}" cy="${emu(h)}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>` +
    `<a:solidFill><a:srgbClr val="${fill}"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln></wps:spPr>${text}<wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice>` +
    `<mc:Fallback><w:pict><v:rect id="${name}" o:spid="_x0000_s${2000 + o.id}" style="position:absolute;margin-left:0;margin-top:0;width:${w}pt;height:${h}pt;z-index:${o.behind ? -z : z};mso-position-horizontal-relative:${o.relH === "page" ? "page" : "text"};mso-position-vertical-relative:${o.relV === "page" ? "page" : "text"}" fillcolor="#${fill}" strokecolor="black">${vmlText}</v:rect></w:pict></mc:Fallback></mc:AlternateContent></w:r>`
  );
}
