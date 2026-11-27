// Format, Picture: the outline, shadow, glow, reflection and soft edges, brightness, contrast and transparency,
// and recolouring, with a preview drawn the way the page will draw the picture. Reset picture takes it all off.
import type { MPos, PicFmt, PicLine, PicRecolor, RImage } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { pictureNode, placeholder } from "../editor/picture";
import { h, modal } from "./dom";
import type { ImageTool } from "./image-tool";
import { swatchGrid } from "./swatches";
import { TEXT_COLORS } from "./toolbar";
import { hexOr } from "../editor/color.ts";

const PT_PX = 96 / 72;

const WIDTHS = [0.25, 0.5, 0.75, 1, 1.5, 2.25, 3, 4.5, 6];
const DASHES: [string, string][] = [
  ["solid", msg("Solid")],
  ["sysDot", msg("Round dot")],
  ["sysDash", msg("Square dot")],
  ["dash", msg("Dash")],
  ["lgDash", msg("Long dash")],
  ["dashDot", msg("Dash dot")],
  ["lgDashDot", msg("Long dash dot")],
];

interface ShadowPreset {
  id: string;
  label: string;
  angle?: number;
}
const SHADOWS: ShadowPreset[] = [
  { id: "none", label: msg("No shadow") },
  { id: "br", label: msg("Offset bottom right"), angle: 45 },
  { id: "b", label: msg("Offset bottom"), angle: 90 },
  { id: "bl", label: msg("Offset bottom left"), angle: 135 },
  { id: "r", label: msg("Offset right"), angle: 0 },
  { id: "l", label: msg("Offset left"), angle: 180 },
  { id: "tr", label: msg("Offset top right"), angle: 315 },
  { id: "t", label: msg("Offset top"), angle: 270 },
  { id: "tl", label: msg("Offset top left"), angle: 225 },
];
const SHADOW = { blurPt: 4, distPt: 3, color: "000000", alpha: 0.4 };

const GLOWS: [string, string][] = [
  ["0", msg("No glow")],
  ["5", msg("5 pt glow")],
  ["8", msg("8 pt glow")],
  ["11", msg("11 pt glow")],
  ["18", msg("18 pt glow")],
];

const REFLECTIONS: [string, string, { size: number; distPt: number } | null][] = [
  ["none", msg("No reflection"), null],
  ["tight", msg("Tight, touching"), { size: 0.35, distPt: 0 }],
  ["half", msg("Half, 4 pt offset"), { size: 0.5, distPt: 4 }],
  ["full", msg("Full, 8 pt offset"), { size: 0.9, distPt: 8 }],
];

const SOFT_EDGES = [0, 1, 2.5, 5, 10, 25, 50];

type Recolor = { kind: "none" } | { kind: "keep" } | { kind: "grayscale" } | { kind: "sepia" } | { kind: "blackWhite"; threshold: number } | { kind: "duotone"; color1: string; color2: string };

/** A darker shade of a colour (hex), as Word's "dark" recolouring uses. */
function shade(hex: string, k: number): string {
  const n = parseInt(hex, 16);
  const c = (v: number) => Math.round(v * k).toString(16).padStart(2, "0");
  return `${c((n >> 16) & 255)}${c((n >> 8) & 255)}${c(n & 255)}`.toUpperCase();
}

const near = (a: number | undefined, b: number, eps = 0.6) => a !== undefined && Math.abs(a - b) < eps;

export function openPictureFormat(ed: Editor, tool: ImageTool, at: MPos) {
  const im = tool.info();
  if (!im) return;
  const f0: PicFmt = im.pic ?? {};
  let theme: [string, string][] = [];
  try {
    theme = ed.engine.themeColors();
  } catch {
    theme = [];
  }
  const accents = theme.filter(([n]) => /^accent\d$/.test(n)).map(([, hex]) => hex.toUpperCase());
  const dark2 = theme.find(([n]) => n === "dk2")?.[1].toUpperCase();
  const palette = [...new Set([...accents, ...(dark2 ? [dark2] : []), ...TEXT_COLORS])];

  // -- the state, started from what the picture has ----------------------------------------------
  let line: PicLine | null = f0.line ? { ...f0.line } : null;
  let shadowId = "none";
  if (f0.shadow) {
    const m = SHADOWS.find((s) => s.angle !== undefined && near(f0.shadow!.angle, s.angle, 1) && near(f0.shadow!.distPt, SHADOW.distPt, 0.6) && near(f0.shadow!.blurPt, SHADOW.blurPt, 0.6));
    shadowId = m?.id ?? "keep";
  }
  let glowSize = f0.glow ? (GLOWS.find(([v]) => near(f0.glow!.radiusPt, Number(v), 0.5))?.[0] ?? "keep") : "0";
  let glowColor = f0.glow?.color ?? accents[0] ?? "5B9BD5";
  let reflectId = "none";
  if (f0.reflection) reflectId = REFLECTIONS.find(([, , v]) => v && near(f0.reflection!.size, v.size, 0.05) && near(f0.reflection!.distPt, v.distPt, 0.6))?.[0] ?? "keep";
  let soft = f0.softEdge ?? 0;
  let bright = Math.round((f0.bright ?? 0) * 100);
  let contrast = Math.round((f0.contrast ?? 0) * 100);
  let transparency = Math.round((1 - (f0.opacity ?? 1)) * 100);
  let recolor: Recolor = !f0.recolor ? { kind: "none" } : f0.recolor.kind === "grayscale" ? { kind: "grayscale" } : f0.recolor.kind === "blackWhite" ? { kind: "blackWhite", threshold: f0.recolor.threshold ?? 0.5 } : { kind: "keep" };

  // -- the preview -------------------------------------------------------------------------------
  const preview = h("div", { class: "pf-preview", "aria-hidden": "true" });
  const url = ed.engine.mediaUrl(f0.svg ?? im.src);

  const current = (): PicFmt => {
    const f: PicFmt = { ...f0 };
    delete f.line;
    delete f.shadow;
    delete f.glow;
    delete f.softEdge;
    delete f.reflection;
    delete f.bright;
    delete f.contrast;
    delete f.opacity;
    if (line) f.line = line;
    const sp = SHADOWS.find((s) => s.id === shadowId);
    if (shadowId === "keep") f.shadow = f0.shadow;
    else if (sp?.angle !== undefined) f.shadow = { ...SHADOW, angle: sp.angle };
    if (glowSize === "keep") f.glow = f0.glow;
    else if (Number(glowSize) > 0) f.glow = { radiusPt: Number(glowSize), color: glowColor, alpha: 0.4 };
    const rp = REFLECTIONS.find(([id]) => id === reflectId);
    if (reflectId === "keep") f.reflection = f0.reflection;
    else if (rp?.[2]) f.reflection = { size: rp[2].size, alpha: 0.5, distPt: rp[2].distPt, blurPt: 0 };
    if (soft > 0) f.softEdge = soft;
    if (bright) f.bright = bright / 100;
    if (contrast) f.contrast = contrast / 100;
    if (transparency) f.opacity = 1 - transparency / 100;
    delete f.recolor;
    const rc = recolorSpec();
    if (recolor.kind === "keep") f.recolor = f0.recolor;
    else if (rc) f.recolor = rc.kind === "sepia" ? { kind: "duotone", color1: "000000", color2: "C9A77C" } : (rc as PicRecolor);
    return f;
  };
  function recolorSpec(): { kind: string; threshold?: number; color1?: string; color2?: string } | null {
    if (recolor.kind === "none" || recolor.kind === "keep") return null;
    if (recolor.kind === "blackWhite") return { kind: "blackWhite", threshold: recolor.threshold };
    if (recolor.kind === "duotone") return { kind: "duotone", color1: recolor.color1, color2: recolor.color2 };
    return { kind: recolor.kind };
  }
  const redraw = () => {
    const s = Math.min(1, 220 / Math.max(1, im.w * PT_PX), 150 / Math.max(1, im.h * PT_PX));
    const f = current();
    const scaled: PicFmt = {
      ...f,
      line: f.line && { ...f.line, widthPt: f.line.widthPt * s },
      shadow: f.shadow && { ...f.shadow, blurPt: f.shadow.blurPt * s, distPt: f.shadow.distPt * s },
      glow: f.glow && { ...f.glow, radiusPt: f.glow.radiusPt * s },
      softEdge: f.softEdge && f.softEdge * s,
      reflection: f.reflection && { ...f.reflection, distPt: f.reflection.distPt * s },
    };
    // The preview has no turn or flip of its own to explain: it shows the same picture, upright.
    const pv: RImage = { src: im.src, w: im.w * s, h: im.h * s, wrap: "inline", alt: "", pic: { ...scaled, rot: undefined } };
    preview.replaceChildren(url ? pictureNode(pv, url) : placeholder(pv, f0.link ?? im.src));
  };

  // -- the controls ------------------------------------------------------------------------------
  const sync: Array<() => void> = [];
  const changed = () => {
    sync.forEach((fn) => fn());
    redraw();
  };
  const field = (label: string, ...body: (HTMLElement | null)[]) => h("div", { class: "field" }, h("label", {}, label), ...body);
  const section = (title: string, ...body: HTMLElement[]) => h("section", { class: "pf-section" }, h("h3", {}, title), ...body);
  const select = (label: string, options: [string, string][], get: () => string, set: (v: string) => void, extra?: () => [string, string] | null) => {
    const el = h("select", { "aria-label": label });
    const fill = () => {
      const ex = extra?.();
      el.replaceChildren(...(ex ? [h("option", { value: ex[0] }, ex[1])] : []), ...options.map(([v, text]) => h("option", { value: v }, text)));
      el.value = get();
    };
    fill();
    el.addEventListener("change", () => {
      set(el.value);
      changed();
    });
    sync.push(() => (el.value = get()));
    return el;
  };

  // Border
  const lineOn = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Outline") });
  const lineNone = h("button", { type: "button", role: "radio" }, t("None"));
  const lineSolid = h("button", { type: "button", role: "radio" }, t("Line"));
  lineOn.append(lineNone, lineSolid);
  const widthSel = h("select", { "aria-label": t("Outline width") }, ...WIDTHS.map((w) => h("option", { value: String(w) }, t("{n} pt", { n: String(w) }))));
  const dashSel = h("select", { "aria-label": t("Outline style") }, ...DASHES.map(([v, l]) => h("option", { value: v }, t(l))));
  const customColor = h("input", { type: "color", value: "#000000", "aria-label": t("Custom color") });
  const colors = swatchGrid(palette, line?.color ?? null, (c) => {
    line = { ...(line ?? { color: c, widthPt: 1, dash: "solid" }), color: c };
    changed();
  });
  customColor.addEventListener("change", () => {
    line = { ...(line ?? { color: "000000", widthPt: 1, dash: "solid" }), color: customColor.value.slice(1).toUpperCase() };
    changed();
  });
  lineNone.addEventListener("click", () => {
    line = null;
    changed();
  });
  lineSolid.addEventListener("click", () => {
    line ??= { color: palette[0] ?? "000000", widthPt: 1, dash: "solid" };
    changed();
  });
  widthSel.addEventListener("change", () => {
    line = { ...(line ?? { color: palette[0] ?? "000000", widthPt: 1, dash: "solid" }), widthPt: Number(widthSel.value) };
    changed();
  });
  dashSel.addEventListener("change", () => {
    line = { ...(line ?? { color: palette[0] ?? "000000", widthPt: 1, dash: "solid" }), dash: dashSel.value };
    changed();
  });
  sync.push(() => {
    lineNone.classList.toggle("on", !line);
    lineSolid.classList.toggle("on", !!line);
    lineNone.setAttribute("aria-checked", String(!line));
    lineSolid.setAttribute("aria-checked", String(!!line));
    colors.select(line?.color ?? null);
    if (line) {
      widthSel.value = String(WIDTHS.reduce((a, b) => (Math.abs(b - line!.widthPt) < Math.abs(a - line!.widthPt) ? b : a)));
      dashSel.value = DASHES.some(([v]) => v === line!.dash) ? line!.dash : "solid";
    }
    widthSel.disabled = dashSel.disabled = !line;
  });

  // Effects
  const shadowSel = select(t("Shadow"), SHADOWS.map((s) => [s.id, t(s.label)]), () => shadowId, (v) => (shadowId = v), () => (shadowId === "keep" ? ["keep", t("Custom")] : null));
  const glowSel = select(t("Glow"), GLOWS.map(([v, l]) => [v, t(l)]), () => glowSize, (v) => (glowSize = v), () => (glowSize === "keep" ? ["keep", t("Custom")] : null));
  const glowColors = swatchGrid(accents.length ? accents : palette.slice(0, 6), glowColor, (c) => {
    glowColor = c;
    if (glowSize === "0") glowSize = "8";
    changed();
  });
  sync.push(() => glowColors.select(glowColor));
  const reflectSel = select(t("Reflection"), REFLECTIONS.map(([id, l]) => [id, t(l)]), () => reflectId, (v) => (reflectId = v), () => (reflectId === "keep" ? ["keep", t("Custom")] : null));
  const softOptions: [string, string][] = SOFT_EDGES.map((v) => [String(v), v === 0 ? t("No soft edges") : t("{n} pt", { n: String(v) })]);
  if (soft && !SOFT_EDGES.includes(soft)) softOptions.push([String(soft), t("{n} pt", { n: String(soft) })]);
  const softSel = select(t("Soft edges"), softOptions, () => String(soft), (v) => (soft = Number(v)));

  // Adjustments
  const slider = (label: string, get: () => number, set: (v: number) => void, min: number, max: number) => {
    const range = h("input", { type: "range", min: String(min), max: String(max), step: "1", value: String(get()), "aria-label": label });
    const num = h("input", { type: "number", min: String(min), max: String(max), step: "1", value: String(get()), "aria-label": t("{name} (percent)", { name: label }), class: "pf-num" });
    const put = (v: number) => {
      set(Math.max(min, Math.min(max, Math.round(v) || 0)));
      changed();
    };
    range.addEventListener("input", () => put(Number(range.value)));
    num.addEventListener("change", () => put(Number(num.value)));
    sync.push(() => {
      range.value = String(get());
      num.value = String(get());
    });
    return h("div", { class: "field" }, h("label", {}, label), h("div", { class: "pf-slider" }, range, num, h("span", { class: "pf-unit" }, "%")));
  };
  const brightness = slider(t("Brightness"), () => bright, (v) => (bright = v), -100, 100);
  const contrastCtl = slider(t("Contrast"), () => contrast, (v) => (contrast = v), -100, 100);
  const transp = slider(t("Transparency"), () => transparency, (v) => (transparency = v), 0, 100);

  // Recolor
  const recolors = h("div", { class: "pf-recolors", role: "group", "aria-label": t("Recolor") });
  const recolorBtn = (label: string, on: () => boolean, run: () => void, swatch?: string) => {
    const b = h("button", { class: "pf-chip", type: "button", "aria-pressed": "false", title: label }, swatch ? h("span", { class: "pf-chip-sw", style: swatch }) : null, h("span", {}, label));
    b.addEventListener("click", () => {
      run();
      changed();
    });
    sync.push(() => {
      b.classList.toggle("on", on());
      b.setAttribute("aria-pressed", String(on()));
    });
    recolors.append(b);
  };
  recolorBtn(t("No recolor"), () => recolor.kind === "none", () => (recolor = { kind: "none" }));
  recolorBtn(t("Grayscale"), () => recolor.kind === "grayscale", () => (recolor = { kind: "grayscale" }), "background:linear-gradient(90deg,#000,#fff)");
  recolorBtn(t("Sepia"), () => recolor.kind === "sepia", () => (recolor = { kind: "sepia" }), "background:linear-gradient(90deg,#2b1d0e,#e6cfa6)");
  recolorBtn(t("Black and white"), () => recolor.kind === "blackWhite", () => (recolor = { kind: "blackWhite", threshold: 0.5 }), "background:linear-gradient(90deg,#000 50%,#fff 50%)");
  recolorBtn(
    t("Washout"),
    () => bright === 70 && contrast === -70,
    () => {
      bright = 70;
      contrast = -70;
    },
    "background:linear-gradient(90deg,#b8b8b8,#fff)",
  );
  const ramps = h("div", { class: "pf-ramps" });
  const rampRow = (label: string, make: (accent: string) => [string, string]) => {
    const row = h("div", { class: "pf-ramp-row", role: "group", "aria-label": label }, h("span", { class: "pf-ramp-label" }, label));
    accents.forEach((accent, i) => {
      const [c1, c2] = make(accent);
      const b = h("button", { class: "pf-ramp", type: "button", "aria-pressed": "false", title: `${label} ${i + 1}`, "aria-label": `${label} ${i + 1}`, style: `background:linear-gradient(90deg,${hexOr(c1)},${hexOr(c2)})` });
      b.addEventListener("click", () => {
        recolor = { kind: "duotone", color1: c1, color2: c2 };
        changed();
      });
      sync.push(() => {
        const on = recolor.kind === "duotone" && recolor.color1 === c1 && recolor.color2 === c2;
        b.classList.toggle("on", on);
        b.setAttribute("aria-pressed", String(on));
      });
      row.append(b);
    });
    ramps.append(row);
  };
  if (accents.length) {
    rampRow(t("Dark theme color"), (a) => [shade(a, 0.45), "FFFFFF"]);
    rampRow(t("Light theme color"), (a) => [a, "FFFFFF"]);
  }

  const error = h("p", { class: "pf-error", role: "alert" });
  const resetBtn = h("button", { class: "btn ghost pf-reset", type: "button" }, t("Reset picture"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog pf-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Format picture") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Format picture"))),
    h(
      "div",
      { class: "dialog-body pf-body" },
      h(
        "div",
        { class: "pf-controls" },
        section(t("Outline"), h("div", { class: "pf-row" }, lineOn, widthSel, dashSel), field(t("Outline color"), colors.el, h("label", { class: "pf-custom" }, h("span", {}, t("Custom color")), customColor))),
        section(
          t("Effects"),
          h("div", { class: "pf-grid" }, field(t("Shadow"), shadowSel), field(t("Reflection"), reflectSel), field(t("Glow"), glowSel), field(t("Soft edges"), softSel)),
          field(t("Glow color"), glowColors.el),
        ),
        section(t("Adjustments"), brightness, contrastCtl, transp),
        section(t("Recolor"), recolors, ramps),
      ),
      h("div", { class: "pf-side" }, h("div", { class: "field" }, h("label", {}, t("Preview")), preview), error),
    ),
    h("div", { class: "dialog-actions" }, resetBtn, h("span", { class: "pf-spacer" }), cancel, apply),
  );
  sync.forEach((fn) => fn());
  redraw();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  resetBtn.addEventListener("click", () => {
    close();
    tool.reset();
  });
  apply.addEventListener("click", () => {
    const patch: Record<string, unknown> = {};
    const f = current();
    const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
    if (!same(f.line, f0.line)) patch.border = f.line ? { color: f.line.color, width_pt: f.line.widthPt, dash: f.line.dash } : "none";
    if (!same(f.shadow, f0.shadow)) patch.shadow = f.shadow ? { blur_pt: f.shadow.blurPt, dist_pt: f.shadow.distPt, angle: f.shadow.angle, color: f.shadow.color, alpha: f.shadow.alpha } : "none";
    if (!same(f.glow, f0.glow)) patch.glow = f.glow ? { radius_pt: f.glow.radiusPt, color: f.glow.color, alpha: f.glow.alpha } : "none";
    if (!same(f.reflection, f0.reflection)) patch.reflection = f.reflection ? { size: f.reflection.size, alpha: f.reflection.alpha, dist_pt: f.reflection.distPt, blur_pt: f.reflection.blurPt } : "none";
    if (!same(f.softEdge, f0.softEdge)) patch.soft_edge = f.softEdge ?? 0;
    if (!same(f.bright, f0.bright)) patch.brightness = f.bright ?? 0;
    if (!same(f.contrast, f0.contrast)) patch.contrast = f.contrast ?? 0;
    if (!same(f.opacity, f0.opacity)) patch.opacity = f.opacity ?? 1;
    if (recolor.kind !== "keep" && !same(recolorSpec(), f0.recolor ? { kind: f0.recolor.kind, threshold: f0.recolor.threshold, color1: f0.recolor.color1, color2: f0.recolor.color2 } : null)) {
      const rc = recolorSpec();
      patch.recolor = rc ?? "none";
    }
    if (!Object.keys(patch).length) return close();
    error.textContent = "";
    ed.select({ anchor: at, focus: { p: at.p, o: at.o + 1 } });
    if (tool.apply(patch, { onError: (m) => (error.textContent = m) })) close();
  });
}
