// The Effects section of the Format shape panel: shadow (outer or inner, with Word's presets), glow, soft edges, reflection and
// 3-D format (a bevel and a depth). Each change is a `set_shape` op with an `effects` patch that names only what changed.
import type { EffectsIn, REffects, RShadow, RShape } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { toAlpha, toTransparency, type SectionCtx, type ShapeSection } from "./format-shape-ctx";
import { h } from "./dom";
import { colorField, numberField, section, selectField, sliderField, subhead } from "./panel-controls";

/** Where the shadow falls, as Word names the directions; the angle is clockwise from the right. */
const DIRECTIONS: [number, string][] = [
  [45, msg("Bottom right")], [90, msg("Bottom")], [135, msg("Bottom left")],
  [0, msg("Right")], [180, msg("Left")],
  [315, msg("Top right")], [270, msg("Top")], [225, msg("Top left")],
];

const GLOW_SIZES = [5, 8, 11, 18];
const SOFT_SIZES = [1, 2.5, 5, 10, 25, 50];
const GLOW_COLOR = "4472C4";

/** Word's three reflection presets, all touching the shape. */
const REFLECTIONS: Record<string, { start: number; end: number; endPos: number }> = {
  tight: { start: 0.52, end: 0.003, endPos: 35 },
  half: { start: 0.5, end: 0.003, endPos: 55.5 },
  full: { start: 0.5, end: 0.003, endPos: 90 },
};

const BEVELS: [string, string][] = [
  ["circle", msg("Circle")], ["relaxedInset", msg("Relaxed inset")], ["cross", msg("Cross")], ["coolSlant", msg("Cool slant")], ["angle", msg("Angle")], ["softRound", msg("Soft round")],
  ["convex", msg("Convex")], ["slope", msg("Slope")], ["divot", msg("Divot")], ["riblet", msg("Riblet")], ["hardEdge", msg("Hard edge")], ["artDeco", msg("Art deco")],
];

export function effectsSection(ctx: SectionCtx): ShapeSection {
  const sec = section("effects", t("Effects"));
  let shape: RShape | null = null;
  const fx = (): REffects => shape?.effects ?? {};
  const change = (e: EffectsIn, opts?: { merge?: boolean }) => ctx.patch({ effects: e }, opts);

  // -- shadow ------------------------------------------------------------------------------
  /** Which shadow the controls below edit: the outer one, else the inner one. */
  const key = (): "shadow" | "inner_shadow" => (fx().shadow || !fx().innerShadow ? "shadow" : "inner_shadow");
  const current = (): RShadow | undefined => fx().shadow ?? fx().innerShadow;
  const shadowPreset = selectField<string>({
    label: t("Shadow presets"),
    options: [["none", t("No shadow")], ["custom", t("Custom")]],
    groups: [
      { label: t("Outer"), options: [...DIRECTIONS.map(([a, n]): [string, string] => [`outer:${a}`, t(n)]), ["outer:center", t("Center")]] },
      { label: t("Inner"), options: [...DIRECTIONS.map(([a, n]): [string, string] => [`inner:${a}`, t(n)]), ["inner:center", t("Center")]] },
    ],
    commit(v) {
      if (v === "none") return change({ shadow: "none", inner_shadow: "none" });
      if (v === "custom") return;
      const [kind, where] = v.split(":");
      const angle = where === "center" ? 45 : Number(where);
      const look = kind === "outer" ? { blur_pt: 4, dist_pt: where === "center" ? 0 : 3, alpha: 0.4 } : { blur_pt: 5, dist_pt: where === "center" ? 0 : 3, alpha: 0.5 };
      const spec = { color: current()?.color ?? "000000", angle, ...look };
      change(kind === "outer" ? { shadow: { ...spec, sx: 100, sy: 100 }, inner_shadow: "none" } : { inner_shadow: spec, shadow: "none" });
    },
  });
  const edit = (patch: Record<string, unknown>, opts?: { merge?: boolean }) => change({ [key()]: patch }, opts);
  const shColor = colorField({ label: t("Color"), commit: (c) => c && edit({ color: c }) });
  const shAlpha = sliderField({ label: t("Transparency"), unit: "%", min: 0, max: 100, live: (v, first) => edit({ alpha: toAlpha(v) ?? 1 }, { merge: !first }), commit: (v) => edit({ alpha: toAlpha(v) ?? 1 }) });
  const shSize = sliderField({ label: t("Size"), unit: "%", min: 0, max: 200, live: (v, first) => edit({ sx: v, sy: v }, { merge: !first }), commit: (v) => edit({ sx: v, sy: v }) });
  const shBlur = numberField({ label: t("Blur"), unit: "pt", min: 0, max: 100, step: 0.5, commit: (v) => edit({ blur_pt: v }) });
  const shDist = numberField({ label: t("Distance"), unit: "pt", min: 0, max: 200, step: 0.5, commit: (v) => edit({ dist_pt: v }) });
  const shAngle = numberField({ label: t("Angle"), unit: "°", min: 0, max: 359, commit: (v) => edit({ angle: v }) });
  const shadowBox = h("div", { class: "fs-box" }, shColor.el, shAlpha.el, shSize.el, shBlur.el, shDist.el, shAngle.el);

  // -- glow --------------------------------------------------------------------------------
  const glowPreset = selectField<string>({
    label: t("Glow presets"),
    options: [["none", t("No glow")], ...GLOW_SIZES.map((n): [string, string] => [String(n), t("{n} pt glow", { n })]), ["custom", t("Custom")]],
    commit(v) {
      if (v === "none") return change({ glow: "none" });
      if (v === "custom") return;
      const g = fx().glow;
      change({ glow: { color: g?.color ?? GLOW_COLOR, alpha: g?.alpha ?? 0.4, radius_pt: Number(v) } });
    },
  });
  const glColor = colorField({ label: t("Color"), commit: (c) => c && change({ glow: { color: c } }) });
  const glAlpha = sliderField({ label: t("Transparency"), unit: "%", min: 0, max: 100, live: (v, first) => change({ glow: { alpha: toAlpha(v) ?? 1 } }, { merge: !first }), commit: (v) => change({ glow: { alpha: toAlpha(v) ?? 1 } }) });
  const glSize = numberField({ label: t("Size"), unit: "pt", min: 0, max: 150, step: 1, commit: (v) => change({ glow: { radius_pt: v } }) });
  const glowBox = h("div", { class: "fs-box" }, glColor.el, glAlpha.el, glSize.el);

  // -- soft edges --------------------------------------------------------------------------
  const soft = selectField<string>({
    label: t("Soft edge presets"),
    options: [["none", t("No soft edges")], ...SOFT_SIZES.map((n): [string, string] => [String(n), t("{n} pt", { n })]), ["custom", t("Custom")]],
    commit(v) {
      if (v === "custom") return;
      change({ soft_edge_pt: v === "none" ? "none" : Number(v) });
    },
  });

  // -- reflection --------------------------------------------------------------------------
  const refl = () => fx().reflection;
  const reflPreset = selectField<string>({
    label: t("Reflection presets"),
    options: [["none", t("No reflection")], ["tight", t("Tight reflection")], ["half", t("Half reflection")], ["full", t("Full reflection")], ["custom", t("Custom")]],
    commit(v) {
      if (v === "none") return change({ reflection: "none" });
      const p = REFLECTIONS[v];
      if (!p) return;
      change({ reflection: { blur_pt: 0.5, start_alpha: p.start, end_alpha: p.end, dist_pt: 0, direction: 90, fade_direction: 90, sx: 100, sy: -100, start_pos: 0, end_pos: p.endPos } });
    },
  });
  const rfAlpha = sliderField({
    label: t("Transparency"),
    unit: "%",
    min: 0,
    max: 100,
    live: (v, first) => change({ reflection: { start_alpha: 1 - v / 100 } }, { merge: !first }),
    commit: (v) => change({ reflection: { start_alpha: 1 - v / 100 } }),
  });
  const rfSize = sliderField({ label: t("Size"), unit: "%", min: 0, max: 100, live: (v, first) => change({ reflection: { end_pos: v } }, { merge: !first }), commit: (v) => change({ reflection: { end_pos: v } }) });
  const rfBlur = numberField({ label: t("Blur"), unit: "pt", min: 0, max: 100, step: 0.5, commit: (v) => change({ reflection: { blur_pt: v } }) });
  const rfDist = numberField({ label: t("Distance"), unit: "pt", min: 0, max: 200, step: 0.5, commit: (v) => change({ reflection: { dist_pt: v } }) });
  const reflBox = h("div", { class: "fs-box" }, rfAlpha.el, rfSize.el, rfBlur.el, rfDist.el);

  // -- 3-D format --------------------------------------------------------------------------
  const bevel = selectField<string>({
    label: t("Bevel"),
    options: [["none", t("No bevel")], ...BEVELS.map(([id, n]): [string, string] => [id, t(n)])],
    commit(v) {
      if (v === "none") return change({ bevel: "none" });
      const b = fx().bevel;
      change({ bevel: { preset: v, w_pt: b?.wPt ?? 6, h_pt: b?.hPt ?? 6 } });
    },
  });
  const bvW = numberField({ label: t("Bevel width"), unit: "pt", min: 0, max: 100, step: 0.5, commit: (v) => change({ bevel: { w_pt: v } }) });
  const bvH = numberField({ label: t("Bevel height"), unit: "pt", min: 0, max: 100, step: 0.5, commit: (v) => change({ bevel: { h_pt: v } }) });
  const depth = numberField({ label: t("Depth"), unit: "pt", min: 0, max: 600, step: 1, commit: (v) => change({ extrusion: v <= 0 ? "none" : { height_pt: v } }) });
  const depthColor = colorField({ label: t("Depth color"), commit: (c) => c && change({ extrusion: { color: c } }) });
  const bevelBox = h("div", { class: "fs-box" }, bvW.el, bvH.el);

  sec.body.append(
    subhead(t("Shadow")), shadowPreset.el, shadowBox,
    subhead(t("Glow")), glowPreset.el, glowBox,
    subhead(t("Soft edges")), soft.el,
    subhead(t("Reflection")), reflPreset.el, reflBox,
    subhead(t("3-D format")), bevel.el, bevelBox, depth.el, depthColor.el,
  );

  function update(s: RShape) {
    shape = s;
    const e = s.effects ?? {};
    const sh = e.shadow ?? e.innerShadow;
    // A shadow the presets don't describe shows as Custom.
    const want = !sh ? "none" : `${e.shadow ? "outer" : "inner"}:${sh.distPt === 0 ? "center" : sh.angle}`;
    shadowPreset.set(Array.from(shadowPreset.select.options).some((o) => o.value === want) ? want : "custom");
    shadowBox.hidden = !sh;
    if (sh) {
      shColor.set(sh.color);
      shAlpha.set(toTransparency(sh.alpha));
      shSize.set(sh.sx ?? 100);
      shSize.disable(!e.shadow);
      shBlur.set(sh.blurPt);
      shDist.set(sh.distPt);
      shAngle.set(sh.angle);
    }
    const g = e.glow;
    glowPreset.set(!g ? "none" : GLOW_SIZES.includes(g.radiusPt) ? String(g.radiusPt) : "custom");
    glowBox.hidden = !g;
    if (g) {
      glColor.set(g.color);
      glAlpha.set(toTransparency(g.alpha));
      glSize.set(g.radiusPt);
    }
    soft.set(e.softEdgePt === undefined ? "none" : SOFT_SIZES.includes(e.softEdgePt) ? String(e.softEdgePt) : "custom");
    const r = refl();
    const match = r && Object.entries(REFLECTIONS).find(([, p]) => Math.abs((r.endPos ?? 100) - p.endPos) < 0.6)?.[0];
    reflPreset.set(!r ? "none" : (match ?? "custom"));
    reflBox.hidden = !r;
    if (r) {
      rfAlpha.set(Math.round((1 - r.startAlpha) * 100));
      rfSize.set(r.endPos ?? 100);
      rfBlur.set(r.blurPt);
      rfDist.set(r.distPt);
    }
    bevel.set(e.bevel?.preset ?? "none");
    bevelBox.hidden = !e.bevel;
    if (e.bevel) {
      bvW.set(e.bevel.wPt);
      bvH.set(e.bevel.hPt);
    }
    depth.set(e.extrusion?.heightPt ?? 0);
    depthColor.el.hidden = !e.extrusion;
    if (e.extrusion) depthColor.set(e.extrusion.color ?? "000000");
  }

  return { sec, applies: (s) => s.kind === "shape", update };
}
