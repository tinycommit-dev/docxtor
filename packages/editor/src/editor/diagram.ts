// SmartArt on the page: the shapes of the graphic's drawing as one SVG over the object's box, each with its text.
//
// The graphic is one object in the paragraph, placed and sized like a picture (see shapes.ts and place.ts); its
// shapes are not selectable or editable. The SVG's viewBox is the box in points and it is stretched over the
// object, so resizing the object scales the whole drawing at once (the text with it, until the page draws the
// graphic again at the new size).
import type { Engine, RDPara, RDRun, RShape } from "@docxtor/core";
import { fontStack, hex } from "./render";
import { shapeSvg } from "./shapes";

const NS = "http://www.w3.org/2000/svg";
const XHTML = "http://www.w3.org/1999/xhtml";

function el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, ...kids: Node[]): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  e.append(...kids);
  return e;
}

const num = (v: number) => String(Math.round(v * 100) / 100);

/** The SVG for a SmartArt graphic (`kind: "diagram"`): its shapes, back to front. */
export function diagramSvg(s: RShape, env: { engine: Engine }): SVGSVGElement {
  const svg = el("svg", { class: "shape-svg diagram-svg", viewBox: `0 0 ${num(s.w || 1)} ${num(s.h || 1)}`, preserveAspectRatio: "none", "aria-hidden": "true", focusable: "false" });
  for (const c of s.children ?? []) svg.append(member(c, env));
  return svg;
}

/** One shape of the drawing: its outline and fill, then its text, turned about its middle. */
function member(c: RShape, env: { engine: Engine }): SVGGElement {
  const w = Math.max(c.w, 0.01);
  const h = Math.max(c.h, 0.01);
  const turn = c.rot ? ` rotate(${num(c.rot)})` : "";
  const g = el("g", { transform: `translate(${num((c.childX ?? 0) + w / 2)} ${num((c.childY ?? 0) + h / 2)})${turn} translate(${num(-w / 2)} ${num(-h / 2)})`, class: "dg-shape" });
  const flip = c.flipH || c.flipV ? `translate(${c.flipH ? num(w) : 0} ${c.flipV ? num(h) : 0}) scale(${c.flipH ? -1 : 1} ${c.flipV ? -1 : 1})` : "";
  const art = el("g", flip ? { transform: flip } : {});
  if (c.kind === "picture" && c.src) {
    const img = el("image", { x: 0, y: 0, width: num(w), height: num(h), preserveAspectRatio: "none" });
    img.setAttribute("href", env.engine.mediaUrl(c.src));
    art.append(img);
  } else {
    // The shape's own SVG, put at the origin of its group; flips are done by the group, not by CSS.
    const inner = shapeSvg({ ...c, flipH: false, flipV: false, text: undefined, rot: 0 }, env);
    inner.removeAttribute("class");
    inner.setAttribute("x", "0");
    inner.setAttribute("y", "0");
    inner.setAttribute("width", num(w));
    inner.setAttribute("height", num(h));
    inner.style.overflow = "visible";
    art.append(inner);
  }
  g.append(art);
  if (c.dtext?.paras.length) g.append(text(c));
  return g;
}

/** The shape's text in its text rectangle: HTML in a foreignObject, so the browser wraps the lines. */
function text(c: RShape): SVGElement {
  const t = c.dtext!;
  const fo = el("foreignObject", { x: num(t.x), y: num(t.y), width: num(Math.max(t.w, 1)), height: num(Math.max(t.h, 1)), class: "dg-text" });
  fo.style.overflow = "visible";
  const box = document.createElementNS(XHTML, "div") as HTMLDivElement;
  box.className = "dg-text-box";
  const i = t.insets;
  box.setAttribute(
    "style",
    [
      "box-sizing:border-box",
      "width:100%",
      "height:100%",
      "display:flex",
      "flex-direction:column",
      `justify-content:${t.vAnchor === "middle" ? "center" : t.vAnchor === "bottom" ? "flex-end" : "flex-start"}`,
      `padding:${num(i.t)}px ${num(i.r)}px ${num(i.b)}px ${num(i.l)}px`,
      `white-space:${t.wrap ? "pre-wrap" : "pre"}`,
      "overflow-wrap:break-word",
      "color:#000",
      "line-height:1.2",
    ].join(";"),
  );
  for (const p of t.paras) box.append(paragraph(p));
  fo.append(box);
  if (!t.rot) return fo;
  // The text turns about the middle of its rectangle.
  return el("g", { transform: `rotate(${num(t.rot)} ${num(t.x + t.w / 2)} ${num(t.y + t.h / 2)})` }, fo);
}

function paragraph(p: RDPara): HTMLElement {
  const d = document.createElementNS(XHTML, "div") as HTMLDivElement;
  const st = [`text-align:${p.align}`, "margin:0"];
  const lead = p.margin ?? 0;
  const hang = p.indent ?? 0;
  st.push(`padding-left:${num(lead)}px`, `text-indent:${num(hang)}px`);
  if (p.linePct) st.push(`line-height:${num((p.linePct / 100) * 1.2)}`);
  if (p.before) st.push(`margin-top:${num(p.before)}px`);
  if (p.after) st.push(`margin-bottom:${num(p.after)}px`);
  d.setAttribute("style", st.join(";"));
  if (p.bullet) {
    const b = document.createElementNS(XHTML, "span") as HTMLSpanElement;
    b.textContent = p.bullet;
    // A hanging bullet takes the space the first line is moved left by.
    b.setAttribute("style", `display:inline-block;width:${num(Math.max(-hang, 6))}px;text-indent:0`);
    d.append(b);
  }
  for (const r of p.runs) d.append(run(r));
  // An empty paragraph still takes its line.
  if (!p.runs.some((r) => r.text)) d.append(document.createTextNode("​"));
  return d;
}

function run(r: RDRun): HTMLElement {
  const span = document.createElementNS(XHTML, "span") as HTMLSpanElement;
  const st = [`font-size:${num(r.size)}px`];
  if (r.bold) st.push("font-weight:700");
  if (r.italic) st.push("font-style:italic");
  if (r.underline) st.push("text-decoration:underline");
  if (hex(r.color)) st.push(`color:${hex(r.color)}`);
  const font = fontStack(r.font);
  if (font) st.push(`font-family:${font}`);
  span.setAttribute("style", st.join(";"));
  span.textContent = r.text;
  return span;
}
