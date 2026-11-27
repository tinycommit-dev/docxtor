// What the Font dialog writes: the state it shows, and the patch for what differs between two states. Only what changed
// is written, so text keeps what it had otherwise.
import type { RunPatch } from "@docxtor/core";
import { colorPatch, underlineColorPatch, type Hue } from "./swatches.ts";

/** What the dialog shows, and the values to compare with to find what changed. */
export interface FontState {
  font: string;
  /** The East Asian font (`w:eastAsia`); empty is none. */
  fontEa: string;
  style: "regular" | "italic" | "bold" | "boldItalic";
  size: number;
  color: Hue;
  underline: string;
  underlineColor: Hue;
  shading: Hue;
  em: string;
  strike: boolean;
  dstrike: boolean;
  vert: "baseline" | "superscript" | "subscript";
  smallCaps: boolean;
  caps: boolean;
  vanish: boolean;
  outline: boolean;
  shadow: boolean;
  emboss: boolean;
  imprint: boolean;
  scale: number;
  spacingKind: "normal" | "expanded" | "condensed";
  spacingBy: number;
  positionKind: "normal" | "raised" | "lowered";
  positionBy: number;
  kern: boolean;
  kernFrom: number;
  ligatures: string;
  numSpacing: string;
  numForm: string;
  styleSet: number;
  cntxtAlts: boolean;
}

/** The patch for what differs between `now` and `was`. */
export function fontPatch(was: FontState, now: FontState): RunPatch {
  const p: RunPatch = {};
  if (now.font && now.font !== was.font) p.font = now.font;
  if (now.fontEa !== was.fontEa) p.fontEa = now.fontEa;
  if (now.style !== was.style) {
    p.bold = now.style === "bold" || now.style === "boldItalic";
    p.italic = now.style === "italic" || now.style === "boldItalic";
  }
  if (now.size !== was.size && now.size > 0) p.size = now.size;
  if (now.color.hex !== was.color.hex || now.color.theme?.name !== was.color.theme?.name) {
    if (now.color.hex) Object.assign(p, colorPatch({ hex: now.color.hex, theme: now.color.theme }));
    else p.color = "auto";
  }
  if (now.underline !== was.underline) p.underline = now.underline;
  if (now.underlineColor.hex !== was.underlineColor.hex || now.underlineColor.theme?.name !== was.underlineColor.theme?.name) {
    if (now.underlineColor.hex) Object.assign(p, underlineColorPatch({ hex: now.underlineColor.hex, theme: now.underlineColor.theme }));
    else p.underlineColor = "auto";
  }
  if (now.shading.hex !== was.shading.hex) p.shading = now.shading.hex ?? "none";
  if (now.em !== was.em) p.em = now.em;
  for (const k of ["strike", "dstrike", "smallCaps", "caps", "vanish", "outline", "shadow", "emboss", "imprint"] as const) {
    if (now[k] !== was[k]) p[k] = now[k];
  }
  if (now.vert !== was.vert) p.vertAlign = now.vert;
  if (now.scale !== was.scale && now.scale > 0) p.scale = Math.round(now.scale);
  const spacing = (s: FontState) => (s.spacingKind === "normal" ? 0 : Math.round((s.spacingKind === "expanded" ? 1 : -1) * s.spacingBy * 20));
  if (spacing(now) !== spacing(was)) p.spacing = spacing(now);
  const position = (s: FontState) => (s.positionKind === "normal" ? 0 : Math.round((s.positionKind === "raised" ? 1 : -1) * s.positionBy * 2));
  if (position(now) !== position(was)) p.position = position(now);
  const kern = (s: FontState) => (s.kern ? Math.max(1, Math.round(s.kernFrom * 2)) : 0);
  if (kern(now) !== kern(was)) p.kern = kern(now);
  if (now.ligatures !== was.ligatures) p.ligatures = now.ligatures;
  if (now.numSpacing !== was.numSpacing) p.numSpacing = now.numSpacing;
  if (now.numForm !== was.numForm) p.numForm = now.numForm;
  if (now.styleSet !== was.styleSet) p.styleSets = now.styleSet ? [now.styleSet] : [];
  if (now.cntxtAlts !== was.cntxtAlts) p.cntxtAlts = now.cntxtAlts;
  return p;
}
