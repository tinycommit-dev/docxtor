// Page borders (Format, Page borders…): which sides, the line style or an art border, its colour and width, where it
// counts from, and which pages show it. Art borders are drawn on screen as a plain line in the border's colour.
import type { Op, PageBorder, PageBorders } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { borderCss } from "../editor/render";
import { h, modal } from "./dom";
import { applyScoped, check, field, grid, group, selectOf } from "./form";
import { swatchGrid } from "./swatches";
import { TEXT_COLORS } from "./toolbar";

type Side = "top" | "left" | "bottom" | "right";
const SIDES: [Side, string][] = [
  ["top", msg("Top")],
  ["left", msg("Left")],
  ["bottom", msg("Bottom")],
  ["right", msg("Right")],
];

/** The line styles the dialog offers (3-D settings have their own). */
const STYLES: [string, string][] = [
  ["single", msg("Solid")],
  ["dotted", msg("Dotted")],
  ["dashed", msg("Dashed")],
  ["dotDash", msg("Dot dash")],
  ["dotDotDash", msg("Dot dot dash")],
  ["double", msg("Double")],
  ["triple", msg("Triple")],
  ["thinThickSmallGap", msg("Thin thick")],
  ["thickThinSmallGap", msg("Thick thin")],
  ["thinThickThinSmallGap", msg("Thin thick thin")],
  ["wave", msg("Wave")],
  ["doubleWave", msg("Double wave")],
];
const STYLES_3D: [string, string][] = [
  ["threeDEmboss", msg("Embossed")],
  ["threeDEngrave", msg("Engraved")],
  ["outset", msg("Outset")],
  ["inset", msg("Inset")],
];

/** A few art borders, drawn as plain lines here and by Word as pictures. */
const ART: [string, string][] = [
  ["", msg("None")],
  ["hearts", msg("Hearts")],
  ["stars", msg("Stars")],
  ["apples", msg("Apples")],
  ["archedScallops", msg("Scallops")],
  ["triangles", msg("Triangles")],
  ["zigZag", msg("Zigzag")],
  ["checkered", msg("Checkered")],
  ["sawtooth", msg("Sawtooth")],
  ["waveline", msg("Wave line")],
  ["balloonsHotAir", msg("Balloons")],
];

/** Eighths of a point, with the name Word gives each width. */
const WIDTHS: [number, string][] = [
  [2, msg("¼ pt")],
  [4, msg("½ pt")],
  [6, msg("¾ pt")],
  [8, msg("1 pt")],
  [12, msg("1½ pt")],
  [18, msg("2¼ pt")],
  [24, msg("3 pt")],
  [36, msg("4½ pt")],
  [48, msg("6 pt")],
];

type Setting = "none" | "box" | "shadow" | "3d" | "custom";
type Apply = "document" | "section" | "first" | "rest";

export function openPageBorders(ed: Editor) {
  const setup = ed.engine.pageSetup();
  const info = ed.caretSectionInfo();
  const old: PageBorders | undefined = (info as { pageBorders?: PageBorders }).pageBorders;
  const several = (setup.sections?.length ?? 1) > 1;
  const firstSide = old && SIDES.map(([s]) => old[s]).find(Boolean);
  const sides = new Set<Side>(old ? SIDES.filter(([s]) => old[s]).map(([s]) => s) : []);

  const sameLook = (a?: PageBorder, b?: PageBorder) => !!a && !!b && a.val === b.val && a.sz === b.sz && a.color === b.color;
  const all = old && sides.size === 4 && SIDES.every(([s]) => sameLook(old[s], firstSide));
  let setting: Setting = !old ? "none" : all && firstSide?.shadow ? "shadow" : all && STYLES_3D.some(([v]) => v === firstSide?.val) ? "3d" : all ? "box" : "custom";
  const isArt = (v?: string) => !!v && !STYLES.some(([s]) => s === v) && !STYLES_3D.some(([s]) => s === v) && !["thick", "nil", "none"].includes(v);
  let art = isArt(firstSide?.val) ? firstSide!.val : "";
  let style = firstSide && !isArt(firstSide.val) ? firstSide.val : "single";
  let sz = firstSide && !isArt(firstSide.val) ? firstSide.sz : 8;
  let color = firstSide?.color && firstSide.color !== "auto" ? firstSide.color.toUpperCase() : "auto";
  let offsetFrom: "page" | "text" = old?.offsetFrom ?? "page";
  const space: Record<Side, number> = { top: old?.top?.space ?? 24, left: old?.left?.space ?? 24, bottom: old?.bottom?.space ?? 24, right: old?.right?.space ?? 24 };
  const display = old?.display ?? "allPages";

  const settingGroup = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Setting") });
  const settingButtons = (
    [
      ["none", t("None")],
      ["box", t("Box")],
      ["shadow", t("Shadow")],
      ["3d", t("3-D")],
      ["custom", t("Custom")],
    ] as [Setting, string][]
  ).map(([v, label]) => {
    const b = h("button", { type: "button", role: "radio" }, label);
    b.addEventListener("click", () => {
      setting = v;
      if (v === "none") sides.clear();
      else if (v !== "custom") for (const [s] of SIDES) sides.add(s);
      if (v === "3d" && !STYLES_3D.some(([s]) => s === style)) style = "threeDEmboss";
      if (v !== "3d" && STYLES_3D.some(([s]) => s === style)) style = "single";
      refresh();
    });
    settingGroup.append(b);
    return [v, b] as const;
  });

  const position = h("div", { class: "seg", role: "group", "aria-label": t("Borders") });
  const sideButtons = SIDES.map(([side, label]) => {
    const b = h("button", { type: "button", "aria-pressed": "false" }, t(label));
    b.addEventListener("click", () => {
      if (sides.has(side)) sides.delete(side);
      else sides.add(side);
      setting = sides.size ? (setting === "none" ? "custom" : setting) : "none";
      refresh();
    });
    position.append(b);
    return [side, b] as const;
  });

  const styleSel = h("select", { id: "pb-style" });
  const artSel = selectOf("pb-art", ART.map(([v, l]) => [v, t(l)] as const), art);
  const widthSel = selectOf("pb-width", WIDTHS.map(([v, l]) => [String(v), t(l)] as const), String(WIDTHS.some(([v]) => v === sz) ? sz : 8));
  const fillStyles = () => {
    const list = setting === "3d" ? STYLES_3D : STYLES;
    styleSel.replaceChildren(...list.map(([v, l]) => h("option", { value: v, selected: v === style }, t(l))));
    styleSel.value = list.some(([v]) => v === style) ? style : list[0][0];
    style = styleSel.value;
  };
  styleSel.addEventListener("change", () => (style = styleSel.value));
  artSel.addEventListener("change", () => (art = artSel.value));
  widthSel.addEventListener("change", () => (sz = Number(widthSel.value)));

  const swatches = swatchGrid(TEXT_COLORS, null, (c) => {
    color = c;
    refresh();
  });
  const auto = h("button", { type: "button", "aria-pressed": "false" }, t("Automatic"));
  auto.addEventListener("click", () => {
    color = "auto";
    refresh();
  });
  const hex = h("input", { id: "pb-hex", type: "text", placeholder: "#RRGGBB", "aria-label": t("Custom color"), autocomplete: "off", spellcheck: "false", maxlength: "7" });
  hex.addEventListener("input", () => {
    const v = hex.value.replace(/^#/, "").toUpperCase();
    if (/^[0-9A-F]{6}$/.test(v)) {
      color = v;
      refresh(false);
    }
  });

  // Options: where the border counts from and how far it is from there, and its stacking.
  const from = selectOf("pb-from", [["page", t("Edge of page")], ["text", t("Text")]], offsetFrom);
  const spaces = Object.fromEntries(SIDES.map(([s, label]) => [s, h("input", { id: `pb-space-${s}`, type: "number", min: "0", max: "31", step: "1", value: String(space[s]), "aria-label": t("{side} distance (pt)", { side: t(label) }), style: "width:100%;min-width:0" })])) as Record<Side, HTMLInputElement>;
  const front = check("pb-front", t("Always display in front"), (old?.zOrder ?? "front") === "front");
  from.addEventListener("change", () => {
    offsetFrom = from.value as "page" | "text";
    // Word's own distances: 24 points from the page edge, 4 from the text.
    if (offsetFrom === "text") for (const [s] of SIDES) if (spaces[s].value === "24") spaces[s].value = "4";
    if (offsetFrom === "page") for (const [s] of SIDES) if (spaces[s].value === "4") spaces[s].value = "24";
  });
  const applyTo = selectOf(
    "pb-apply",
    [
      ["document", t("Whole document")],
      ...(several ? [["section", t("This section")]] : []),
      ["first", t("This section, first page only")],
      ["rest", t("This section, all except first page")],
    ] as [Apply, string][],
    display === "firstPage" ? "first" : display === "notFirstPage" ? "rest" : several && old ? "section" : "document",
  );

  const sample = h("div", { class: "bs-sample pb-sample" }, h("span", {}, t("Sample page")));
  const preview = h("div", { class: "bs-preview pb-preview", "aria-hidden": "true" }, sample);

  function refresh(updateHex = true) {
    for (const [v, b] of settingButtons) {
      b.classList.toggle("on", v === setting);
      b.setAttribute("aria-checked", String(v === setting));
    }
    for (const [side, b] of sideButtons) {
      b.classList.toggle("on", sides.has(side));
      b.setAttribute("aria-pressed", String(sides.has(side)));
    }
    fillStyles();
    swatches.select(color === "auto" ? null : color);
    auto.classList.toggle("on", color === "auto");
    auto.setAttribute("aria-pressed", String(color === "auto"));
    if (updateHex) hex.value = color === "auto" ? "" : `#${color}`;
    styleSel.disabled = !!art;
    widthSel.disabled = false;
    const line = art ? { val: "single", sz: 12, color, space: 0 } : { val: style, sz, color, space: 0 };
    const css: string[] = [];
    for (const [s] of SIDES) if (sides.has(s)) css.push(`border-${s}:${borderCss(line)}`);
    if (setting === "shadow") css.push("box-shadow:3px 3px 0 0 rgb(0 0 0 / 0.55)");
    sample.setAttribute("style", css.join(";"));
  }

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog bs-dialog pb-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Page borders") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Page borders"))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", {}, t("Setting")), settingGroup),
      h("div", { class: "bs-cols" }, field(t("Style"), styleSel, "pb-style"), field(t("Art"), artSel, "pb-art")),
      h("div", { class: "bs-cols" }, field(t("Width"), widthSel, "pb-width"), h("div", { class: "field" }, h("label", { for: "pb-hex" }, t("Color")), swatches.el, h("div", { class: "pb-hex-row" }, auto, hex))),
      h("div", { class: "field" }, h("label", {}, t("Preview")), h("div", { class: "bs-row" }, position, preview)),
      group(t("Options"), grid(field(t("Measure from"), from, "pb-from"), field(t("Apply to"), applyTo, "pb-apply")), grid(...SIDES.map(([s, label]) => h("div", { class: "field" }, h("label", { for: `pb-space-${s}` }, t("{side} (pt)", { side: t(label) })), spaces[s]))), front.row),
    ),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  refresh();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    const target = applyTo.value as Apply;
    close();
    const line = (side: Side): PageBorder => ({
      val: art || style,
      sz: art ? 16 : sz,
      space: Math.max(0, Math.min(31, Math.round(Number(spaces[side].value) || 0))),
      color,
      ...(setting === "shadow" ? { shadow: true } : {}),
    });
    let borders: PageBorders | null = null;
    if (sides.size) {
      borders = {
        offsetFrom,
        display: target === "first" ? "firstPage" : target === "rest" ? "notFirstPage" : "allPages",
        zOrder: front.input.checked ? "front" : "back",
        ...Object.fromEntries(SIDES.filter(([s]) => sides.has(s)).map(([s]) => [s, line(s)])),
      };
    }
    applyScoped(ed, target === "document" ? "document" : "section", (section) => [{ op: "set_page_borders", ...(section !== undefined ? { section } : {}), borders } as Op]);
    ed.focus();
  });
}
