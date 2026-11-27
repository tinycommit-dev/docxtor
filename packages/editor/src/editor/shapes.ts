// Shapes and text boxes on the page: one SVG for the outline and fill, the text inside as ordinary paragraphs
// over it. A shape is an atom of one character in its paragraph, placed like an image (see place.ts).
//
// The SVG is stretched over the box (`preserveAspectRatio="none"`, strokes that don't scale), so resizing a
// shape, or a text box that grows with its text, is only a change of the box's size.
import { t } from "@docxtor/extensions";
import type { Engine, RBlock, RLine, RShape } from "@docxtor/core";
import { isLine, presetPaths, type PathSpec } from "./presets";
import { floatData, placeCss } from "./place";
import { nextId, paintFor } from "./shape-fill";
import { applyEffects } from "./shape-effects";
import { diagramSvg } from "./diagram";
import { warpSvg } from "./warp-draw";
import { hexOr } from "./color.ts";

const NS = "http://www.w3.org/2000/svg";
const PT_PX = 96 / 72;

/**
 * Ids of the paragraphs inside text that is being edited. The shape tool adds them while the caret is in a text
 * box; the page redraws the paragraph that holds the shape after every keystroke, and the text it draws
 * has to come back editable.
 */
export const editing = new Set<number>();

/** DrawingML's dash patterns, in multiples of the line's width (dot and dash, then gaps). */
const DASHES: Record<string, number[]> = {
  dot: [1, 3],
  sysDot: [1, 1],
  dash: [4, 3],
  sysDash: [3, 1],
  lgDash: [8, 3],
  dashDot: [4, 3, 1, 3],
  sysDashDot: [3, 1, 1, 1],
  lgDashDot: [8, 3, 1, 3],
  lgDashDotDot: [8, 3, 1, 3, 1, 3],
  sysDashDotDot: [3, 1, 1, 1, 1, 1],
};

function el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, ...kids: Node[]): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  e.append(...kids);
  return e;
}

/** A colour of the shape as CSS; a value that is not six hex digits is black (the engine hands out nothing else, this is the second check). */
const hex = (c: string) => hexOr(c);

/** The shape's first text paragraph's id (what tells a shape's text apart from every other). */
export function textParas(s: RShape): number[] {
  return (s.text ?? []).flatMap((b) => (b.t === "p" ? [b.id] : []));
}

/** How large an arrowhead is, as a share of the medium one (Word: small 2, medium 3, large 5 times the line's width). */
const HEAD_SIZE = { sm: 2 / 3, med: 1, lg: 5 / 3 };

/** The line drawn at the end of an open path: `triangle`, `stealth`, `diamond`, `oval` or an open `arrow`. */
function marker(kind: string, color: string, opacity: number, widthPx: number, w: keyof typeof HEAD_SIZE = "med", len: keyof typeof HEAD_SIZE = "med"): SVGMarkerElement {
  const id = nextId();
  // Arrow heads are about four times the line's width, and never smaller than a few pixels.
  const size = Math.max(4, 11 / Math.max(widthPx, 0.5));
  const open = kind === "arrow";
  const shape =
    kind === "diamond"
      ? el("path", { d: "M0,5 L5,0 L10,5 L5,10 Z" })
      : kind === "oval"
        ? el("circle", { cx: 5, cy: 5, r: 4.5 })
        : kind === "stealth"
          ? el("path", { d: "M0,0 L10,5 L0,10 L3,5 Z" })
          : open
            ? el("path", { d: "M1,1 L9,5 L1,9", fill: "none", stroke: color, "stroke-width": 1.4, "stroke-linejoin": "round", "stroke-linecap": "round", "stroke-opacity": opacity })
            : el("path", { d: "M0,0 L10,5 L0,10 Z" });
  if (!open) {
    shape.setAttribute("fill", color);
    shape.setAttribute("fill-opacity", String(opacity));
  }
  return el(
    "marker",
    {
      id,
      viewBox: "0 0 10 10",
      preserveAspectRatio: "none",
      refX: kind === "diamond" || kind === "oval" ? 5 : 9,
      refY: 5,
      markerWidth: size * HEAD_SIZE[len],
      markerHeight: size * HEAD_SIZE[w],
      orient: "auto-start-reverse",
      markerUnits: "strokeWidth",
    },
    shape,
  );
}

/** The outline and fill of a shape, as an SVG that fills its box. `env` gives pictures their URLs. */
export function shapeSvg(s: RShape, env?: { engine?: Engine }): SVGSVGElement {
  const defs = el("defs");
  const svg = el("svg", { class: "shape-svg", viewBox: `0 0 ${s.w || 1} ${s.h || 1}`, preserveAspectRatio: "none", "aria-hidden": "true", focusable: "false" }, defs);
  const open = isLine(s.geometry);
  const engine = env?.engine;
  let paint = "none";
  let paintOpacity = 1;
  if (s.fill && !open) {
    const p = paintFor(s.fill, defs, s.w || 1, s.h || 1, { url: engine ? (src) => engine.mediaUrl(src) : undefined, rot: s.rot });
    paint = p.paint;
    paintOpacity = p.opacity;
  }
  const stroke = strokeAttrs(s.line, defs);
  const paths: (PathSpec & { scale?: [number, number] })[] =
    s.geometry === "custom" && s.custom
      ? s.custom.paths.map((p) => ({ d: p.d, fill: p.fill, stroke: p.stroke, scale: [(s.w || 1) / (p.w || 1), (s.h || 1) / (p.h || 1)] as [number, number] }))
      : presetPaths(s.geometry, s.w, s.h, s.adj);
  // With effects the paths sit in a group of their own, which the effects refer to.
  const fx = s.effects;
  const body = fx ? el("g", { id: nextId(), class: "shape-body" }) : null;
  const into = body ?? svg;
  const compound = s.line?.compound;
  for (const p of paths) {
    const path = el("path", { d: p.d, "vector-effect": "non-scaling-stroke", ...(p.stroke && !compound ? stroke : { stroke: "none" }) });
    path.setAttribute("fill", p.fill ? paint : "none");
    if (p.fill && paintOpacity < 1) path.setAttribute("fill-opacity", String(paintOpacity));
    if (p.scale) path.setAttribute("transform", `scale(${p.scale[0]} ${p.scale[1]})`);
    path.setAttribute("fill-rule", "evenodd");
    into.append(path);
    if (compound && p.stroke && s.line) {
      // A compound line: the stroke is drawn once, seen through a mask that cuts the gaps out.
      const line = el("path", { d: p.d, "vector-effect": "non-scaling-stroke", fill: "none", ...stroke });
      if (p.scale) line.setAttribute("transform", `scale(${p.scale[0]} ${p.scale[1]})`);
      line.setAttribute("mask", `url(#${compoundMask(defs, p, s)})`);
      into.append(line);
    }
  }
  if (body) {
    svg.append(body);
    applyEffects(svg, defs, body, fx!, { w: s.w || 1, h: s.h || 1, flipH: !!s.flipH, flipV: !!s.flipV, rot: s.rot ?? 0, fillColor: s.fill?.color });
  }
  if (s.flipH || s.flipV) svg.style.transform = `scale(${s.flipH ? -1 : 1},${s.flipV ? -1 : 1})`;
  return svg;
}

/** The parts of a compound line, as stripes across it from one side to the other (widths in sixths of the line). */
const COMPOUND: Record<string, [number, boolean][]> = {
  // Each entry draws a stroke of that many sixths of the line's width: white shows the line, black cuts it.
  dbl: [[6, true], [2, false]],
  tri: [[6, true], [4, false], [2, true]],
  thickThin: [[6, true], [1.4, false]],
  thinThick: [[6, true], [1.4, false]],
};

function compoundMask(defs: SVGElement, p: PathSpec & { scale?: [number, number] }, s: RShape): string {
  const id = nextId();
  const widthPx = Math.max(0.25, s.line!.widthPt) * PT_PX;
  const pad = s.line!.widthPt + 2;
  const mask = el("mask", { id, maskUnits: "userSpaceOnUse", x: n(-pad), y: n(-pad), width: n(s.w + pad * 2), height: n(s.h + pad * 2) });
  for (const [sixths, on] of COMPOUND[s.line!.compound!] ?? COMPOUND.dbl) {
    const path = el("path", { d: p.d, "vector-effect": "non-scaling-stroke", fill: "none", stroke: on ? "#fff" : "#000", "stroke-width": n((widthPx * sixths) / 6), "stroke-linejoin": s.line!.join ?? "miter" });
    if (p.scale) path.setAttribute("transform", `scale(${p.scale[0]} ${p.scale[1]})`);
    mask.append(path);
  }
  defs.append(mask);
  return id;
}

function strokeAttrs(line: RLine | undefined, defs: SVGElement): Record<string, string | number> {
  if (!line) return { stroke: "none" };
  const width = Math.max(0.25, line.widthPt) * PT_PX;
  const out: Record<string, string | number> = {
    stroke: hex(line.color),
    "stroke-width": width,
    "stroke-linejoin": line.join ?? "miter",
    "stroke-miterlimit": line.miter ? line.miter / 100 : 8,
  };
  if (line.cap) out["stroke-linecap"] = line.cap === "square" ? "square" : "round";
  if (line.alpha !== undefined && line.alpha < 1) out["stroke-opacity"] = line.alpha;
  const dash = DASHES[line.dash];
  if (dash) {
    const unit = Math.max(width, 1);
    out["stroke-dasharray"] = dash.map((d) => n(d * unit)).join(" ");
    if (line.dash === "dot" || line.dash === "sysDot") out["stroke-linecap"] = "butt";
  }
  for (const [side, kind, w, len] of [["start", line.headArrow, line.headW, line.headLen], ["end", line.tailArrow, line.tailW, line.tailLen]] as const) {
    if (!kind || kind === "none") continue;
    const m = marker(kind, hex(line.color), line.alpha ?? 1, width, w, len);
    defs.append(m);
    out[`marker-${side}`] = `url(#${m.id})`;
  }
  return out;
}

const n = (v: number) => String(Math.round(v * 100) / 100);

type Blocks = (b: RBlock[]) => HTMLElement[];

/** Whether the browser lays text out upwards in lines that stack to the right (`sideways-lr`). */
const SIDEWAYS_LR = typeof CSS !== "undefined" && typeof CSS.supports === "function" && CSS.supports("writing-mode", "sideways-lr");

/** The text inside a shape: its paragraphs, in a box inset from the shape's edges. */
function textBox(s: RShape, blocks: Blocks): HTMLElement {
  const box = document.createElement("div");
  box.className = "shape-text";
  const i = s.insets;
  const st: string[] = [];
  const transforms: string[] = [];
  // Text that shrinks to fit (`normAutofit`) is laid out at the size it would have, then scaled.
  const scale = s.autoFitMode === "text" && s.fontScale && s.fontScale < 100 ? s.fontScale / 100 : 1;
  const inset = { l: i.l * PT_PX, t: i.t * PT_PX, r: i.r * PT_PX, b: i.b * PT_PX };
  if (s.autoFit) st.push(`position:relative;padding:${inset.t}px ${inset.r}px ${inset.b}px ${inset.l}px;min-height:${s.h * PT_PX}px;box-sizing:border-box`);
  else if (scale < 1) {
    st.push(`position:absolute;left:${inset.l}px;top:${inset.t}px;width:calc((100% - ${inset.l + inset.r}px) / ${scale});height:calc((100% - ${inset.t + inset.b}px) / ${scale});transform-origin:0 0`);
    transforms.push(`scale(${scale})`);
  } else st.push(`position:absolute;left:${inset.l}px;top:${inset.t}px;right:${inset.r}px;bottom:${inset.b}px`);
  if (s.columns && s.columns > 1) {
    // Columns are laid out by the browser; they fill one after the other (the box has a height).
    st.push(`display:block;column-count:${s.columns};column-gap:${(s.columnGapPt ?? 0) * PT_PX}px;column-fill:auto`);
  } else {
    st.push(`justify-content:${s.vAnchor === "middle" ? "center" : s.vAnchor === "bottom" ? "flex-end" : "flex-start"}`);
    // The text block as a whole sits in the middle of the box's width.
    if (s.anchorCenter) st.push("align-items:center");
  }
  switch (s.vert) {
    case "vert":
      st.push("writing-mode:vertical-rl;text-orientation:sideways");
      break;
    case "vert270":
      // Lines run upwards and stack to the right. Browsers without `sideways-lr` turn a downward box over.
      if (SIDEWAYS_LR) st.push("writing-mode:sideways-lr");
      else {
        st.push("writing-mode:vertical-rl;text-orientation:sideways");
        transforms.push("rotate(180deg)");
      }
      break;
    case "eaVert":
      st.push("writing-mode:vertical-rl;text-orientation:mixed");
      break;
    case "wordArtVert":
      st.push("writing-mode:vertical-rl;text-orientation:upright");
      break;
    case "wordArtVertRtl":
      st.push("writing-mode:vertical-lr;text-orientation:upright");
      break;
    case "mongolianVert":
      st.push("writing-mode:vertical-lr;text-orientation:sideways");
      break;
  }
  // Flipped upside down, Word turns the text over with the shape; `upright` keeps it as it was (unturned by the shape's rotation too).
  if (s.upright) {
    if (s.rot) transforms.unshift(`rotate(${-s.rot}deg)`);
  } else if (s.flipV) transforms.unshift("rotate(180deg)");
  if (transforms.length) st.push(`transform:${transforms.join(" ")}`);
  box.setAttribute("style", st.join(";"));
  if (!s.wrapText) box.classList.add("nowrap");
  for (const b of blocks(s.text ?? [])) {
    // Text that shrinks to fit also gives up some of its line spacing (`lnSpcReduction`).
    if (s.autoFitMode === "text" && s.lineSpacingReduction) {
      for (const p of [b, ...Array.from(b.querySelectorAll<HTMLElement>(".fp"))]) {
        const lh = Number(p.style?.lineHeight);
        if (lh > 0) p.style.lineHeight = String(Math.round(lh * (1 - s.lineSpacingReduction / 100) * 1000) / 1000);
      }
    }
    box.append(b);
  }
  const ids = textParas(s);
  if (ids.length && ids.some((id) => editing.has(id))) {
    box.contentEditable = "true";
    box.dataset.editing = "1";
    box.spellcheck = true;
  }
  box.dataset.st = String(ids[0] ?? "");
  return box;
}

/** Fill a box (a shape's atom, or a member of a group) with its drawing, its text and its members. */
function fillBox(box: HTMLElement, s: RShape, env: { engine: Engine }, blocks: Blocks, path: number[] = []) {
  // SmartArt: its shapes and their text in one SVG; the members are not selectable.
  if (s.kind === "diagram") {
    box.append(diagramSvg(s, env));
    return;
  }
  if (s.kind === "picture" && s.src) {
    const img = document.createElement("img");
    img.src = env.engine.mediaUrl(s.src);
    img.alt = s.alt ?? "";
    img.draggable = false;
    img.className = "shape-pic";
    if (s.flipH || s.flipV) img.style.transform = `scale(${s.flipH ? -1 : 1},${s.flipV ? -1 : 1})`;
    box.append(img);
  } else if (s.kind === "shape" || s.fill || s.line) box.append(shapeSvg(s, env));
  // A flipped group mirrors where its members are, and flips them too (their text stays as it is).
  const gh = s.kind !== "shape" && !!s.flipH;
  const gv = s.kind !== "shape" && !!s.flipV;
  (s.children ?? []).forEach((c0, i) => {
    const c: RShape = gh || gv ? { ...c0, flipH: !!c0.flipH !== gh, flipV: !!c0.flipV !== gv, rot: gh !== gv && c0.rot ? 360 - c0.rot : c0.rot, childX: gh ? s.w - (c0.childX ?? 0) - c0.w : c0.childX, childY: gv ? s.h - (c0.childY ?? 0) - c0.h : c0.childY } : c0;
    const m = document.createElement("span");
    m.className = `shape-child ${c.kind}`;
    // Which member it is (its index in the group, then in the group inside that): what an op addresses it by.
    m.dataset.m = [...path, i].join(".");
    m.dataset.w = String(c.w);
    m.dataset.h = String(c.h);
    if (c.rot) m.dataset.rot = String(c.rot);
    m.setAttribute(
      "style",
      `position:absolute;left:${(c.childX ?? 0) * PT_PX}px;top:${(c.childY ?? 0) * PT_PX}px;width:${c.w * PT_PX}px;height:${c.h * PT_PX}px${c.rot ? `;transform:rotate(${c.rot}deg)` : ""}`,
    );
    fillBox(m, c, env, blocks, [...path, i]);
    box.append(m);
  });
  // WordArt: the text bent to its preset is drawn over the shape; the plain text stays underneath, hidden. While the
  // text is being edited the bend is switched off (`warp-off`, see the shape tool) and the plain text shows.
  const warped = s.warp ? warpSvg(s, env) : null;
  if (warped) box.append(warped);
  if (s.text) {
    const tb = textBox(s, blocks);
    if (warped) {
      tb.classList.add("warped");
      if (textParas(s).some((id) => editing.has(id))) box.classList.add("warp-off");
    }
    box.append(tb);
  }
}

/** The atom for a shape segment: sized in points like an image, placed by its wrap mode. */
export function shapeAtom(s: RShape, env: { engine: Engine }, blocks: Blocks): HTMLElement {
  const atom = document.createElement("span");
  const wrap = s.wrap || "inline";
  atom.className = `atom shape wrap-${wrap}${s.textBox ? " textbox" : ""}${isLine(s.geometry) ? " line" : ""}${s.text ? " has-text" : ""}${s.kind === "diagram" ? " diagram" : ""}${s.warp ? " wordart" : ""}`;
  atom.dataset.w = String(s.w);
  atom.dataset.h = String(s.h);
  atom.dataset.wrap = wrap;
  if (s.align) atom.dataset.align = s.align;
  if (s.rot) atom.dataset.rot = String(s.rot);
  atom.dataset.kind = s.kind;
  if (s.alt) atom.setAttribute("aria-label", s.alt);
  else if (s.kind === "diagram") atom.setAttribute("aria-label", t("SmartArt graphic"));
  if (s.name) atom.dataset.name = s.name;
  fillBox(atom, s, env, blocks);
  const size = `width:${s.w * PT_PX}px;${s.autoFit && s.text ? "min-height" : "height"}:${Math.max(s.h, 0.5) * PT_PX}px`;
  const spec = { wrap, align: s.align, alignY: s.alignY, x: s.x, y: s.y, relX: s.relX, relY: s.relY, w: s.w, z: s.z };
  const place = placeCss(spec);
  floatData(atom, { ...spec, h: s.h });
  const rot = s.rot ? `transform:rotate(${s.rot}deg)` : "";
  // `data-place` is what the segment's final style is built from (see render.ts).
  atom.dataset.place = [size, place, rot].filter(Boolean).join(";");
  return atom;
}

/**
 * Give the gradients and arrow heads in a copy of the pages ids of their own: the copy sits in the same document
 * as the pages it was made from (printing), where the first element with an id is the one every reference finds.
 */
export function renameIds(root: Element, suffix: string) {
  for (const e of Array.from(root.querySelectorAll("[id^='fa']"))) e.id += suffix;
  for (const e of Array.from(root.querySelectorAll(".shape-svg *"))) {
    for (const attr of ["fill", "stroke", "marker-start", "marker-end", "filter", "mask", "clip-path"]) {
      const v = e.getAttribute(attr);
      if (v?.startsWith("url(#fa")) e.setAttribute(attr, v.replace(/\)$/, `${suffix})`));
    }
    const href = e.getAttribute("href");
    if (href?.startsWith("#fa")) e.setAttribute("href", href + suffix);
  }
}
