// The Line section of the Format shape panel: no line or a solid one, with colour, transparency, width, compound type, dash,
// cap, join, and the arrowheads at either end (type, width and length). Changes are `set_shape` ops with a `line` patch; the
// parts left out of a patch stay as the file has them.
import type { RLine, RShape } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { isOpenShape, toAlpha, toTransparency, type SectionCtx, type ShapeSection } from "./format-shape-ctx";
import { h } from "./dom";
import { colorField, numberField, radioField, section, selectField, sliderField, subhead } from "./panel-controls";

const DEFAULT_LINE = "2F528F";

const sizes = (): [string, string][] => [
  ["sm", t("Small")],
  ["med", t("Medium")],
  ["lg", t("Large")],
];

export function lineSection(ctx: SectionCtx): ShapeSection {
  const sec = section("line", t("Line"));
  let shape: RShape | null = null;
  const line = (): RLine | undefined => shape?.line;
  const set = (patch: Record<string, unknown>, opts?: { merge?: boolean }) => ctx.patch({ line: { ...(line() ? {} : { color: DEFAULT_LINE, width_pt: 1 }), ...patch } }, opts);

  const kinds = radioField<"none" | "solid">({
    label: t("Line type"),
    columns: 2,
    options: [
      ["none", t("No line")],
      ["solid", t("Solid line")],
    ],
    commit(k) {
      if (k === "none") ctx.patch({ line: "none" });
      else if (!line()) set({});
    },
  });
  const color = colorField({ label: t("Color"), commit: (c) => c && set({ color: c }) });
  const alpha = sliderField({
    label: t("Transparency"),
    unit: "%",
    min: 0,
    max: 100,
    live: (v, first) => set({ alpha: toAlpha(v) ?? 1 }, { merge: !first }),
    commit: (v) => set({ alpha: toAlpha(v) ?? 1 }),
  });
  const width = numberField({ label: t("Width"), unit: "pt", min: 0, max: 1584, step: 0.25, digits: 2, commit: (v) => set({ width_pt: v }) });
  const compound = selectField<string>({
    label: t("Compound type"),
    options: [
      ["single", t("Single")],
      ["dbl", t("Double")],
      ["thickThin", t("Thick and thin")],
      ["thinThick", t("Thin and thick")],
      ["tri", t("Triple")],
    ],
    commit: (v) => set({ compound: v }),
  });
  const dash = selectField<string>({
    label: t("Dash type"),
    options: [
      ["solid", t("Solid")],
      ["sysDot", t("Round dot")],
      ["sysDash", t("Square dot")],
      ["dash", t("Dash")],
      ["dashDot", t("Dash dot")],
      ["lgDash", t("Long dash")],
      ["lgDashDot", t("Long dash dot")],
      ["lgDashDotDot", t("Long dash dot dot")],
    ],
    commit: (v) => set({ dash: v }),
  });
  const cap = selectField<string>({
    label: t("Cap type"),
    options: [
      ["flat", t("Flat")],
      ["round", t("Round")],
      ["square", t("Square")],
    ],
    commit: (v) => set({ cap: v }),
  });
  const join = selectField<string>({
    label: t("Join type"),
    options: [
      ["round", t("Round")],
      ["bevel", t("Bevel")],
      ["miter", t("Miter")],
    ],
    commit: (v) => set({ join: v }),
  });

  // Arrowheads: the start of the line is the head, the end the tail.
  const arrowTypes = (): [string, string][] => [
    ["none", t("None")],
    ["triangle", t("Arrow")],
    ["stealth", t("Stealth arrow")],
    ["arrow", t("Open arrow")],
    ["diamond", t("Diamond arrow")],
    ["oval", t("Oval arrow")],
  ];
  const end = (which: "head" | "tail", title: string) => {
    const type = selectField<string>({ label: t("{end} type", { end: title }), options: arrowTypes(), commit: (v) => set({ [`${which}_arrow`]: v }) });
    const w = selectField<string>({ label: t("{end} width", { end: title }), options: sizes(), commit: (v) => set({ [`${which}_w`]: v }) });
    const len = selectField<string>({ label: t("{end} length", { end: title }), options: sizes(), commit: (v) => set({ [`${which}_len`]: v }) });
    return { type, w, len, els: [type.el, w.el, len.el] };
  };
  const begin = end("head", t("Begin arrow"));
  const finish = end("tail", t("End arrow"));
  const arrows = h("div", {}, subhead(t("Arrows")), ...begin.els, ...finish.els);

  const box = h("div", { class: "fs-box" }, color.el, alpha.el, width.el, compound.el, dash.el, cap.el, join.el, arrows);
  sec.body.append(kinds.el, box);

  function update(s: RShape) {
    shape = s;
    const l = s.line;
    kinds.set(l ? "solid" : "none");
    box.hidden = !l;
    arrows.hidden = !isOpenShape(s);
    if (!l) return;
    color.set(l.color);
    alpha.set(toTransparency(l.alpha));
    width.set(l.widthPt);
    compound.set(l.compound ?? "single");
    dash.set(l.dash || "solid");
    cap.set(l.cap ?? "flat");
    join.set(l.join ?? "miter");
    begin.type.set(l.headArrow ?? "none");
    begin.w.set(l.headW ?? "med");
    begin.len.set(l.headLen ?? "med");
    finish.type.set(l.tailArrow ?? "none");
    finish.w.set(l.tailW ?? "med");
    finish.len.set(l.tailLen ?? "med");
  }

  return { sec, applies: (s) => s.kind === "shape", update };
}
