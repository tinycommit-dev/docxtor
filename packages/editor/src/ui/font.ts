// The Font dialog (Format, Font…): font, style and size; colour; underline style and colour; strike-through,
// superscript, small caps, hidden, outline, shadow, emboss and engrave; and on a second tab the width, spacing and
// position of the letters, kerning and the OpenType features (ligatures, number forms, stylistic sets). A preview
// draws the same CSS the page will. Only what the person changed is written, so text keeps what it had otherwise;
// Set as default writes the same changes into the document's defaults.
import type { RSeg, RunFmt, ThemeRef } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { prepareSegs, UNDERLINE_CHOICES } from "../editor/charfmt";
import type { Editor } from "../editor/editor";
import { fontStack, runStyle } from "../editor/render";
import { MENU_FONTS } from "../fonts";
import { confirmDialog, h, modal, toast } from "./dom";
import { fontPatch, type FontState } from "./font-patch";
import { selectionRunTarget, type RunTarget } from "./format-target";
import { colorButton, type Hue } from "./swatches";
import { EAST_ASIAN_FONTS } from "./asian-typography";

const EMPHASIS: [string, string][] = [
  ["none", msg("(none)")],
  ["dot", msg("Dot")],
  ["comma", msg("Comma")],
  ["circle", msg("Circle")],
  ["underDot", msg("Dot below")],
];

const STYLES: [string, string][] = [
  ["regular", msg("Regular")],
  ["italic", msg("Italic")],
  ["bold", msg("Bold")],
  ["boldItalic", msg("Bold italic")],
];

const LIGATURES: [string, string][] = [
  ["default", msg("Default")],
  ["none", msg("None")],
  ["standard", msg("Standard only")],
  ["standardContextual", msg("Standard and contextual")],
  ["historicalDiscretional", msg("Historical and discretionary")],
  ["all", msg("All")],
];

const NUMBER_SPACING: [string, string][] = [
  ["default", msg("Default")],
  ["proportional", msg("Proportional")],
  ["tabular", msg("Tabular")],
];

const NUMBER_FORMS: [string, string][] = [
  ["default", msg("Default")],
  ["lining", msg("Lining")],
  ["oldStyle", msg("Old-style")],
];

const SIZES = [8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];

const hue = (hex?: string, theme?: ThemeRef): Hue => (hex && hex !== "auto" ? { hex: hex.toUpperCase(), theme } : {});

function read(r: RunFmt): FontState {
  const fx = r.fx ?? {};
  const spacing = r.spacing ?? 0;
  const position = r.position ?? 0;
  return {
    font: r.font ?? "",
    fontEa: r.fontEa ?? "",
    style: r.bold ? (r.italic ? "boldItalic" : "bold") : r.italic ? "italic" : "regular",
    size: r.size ?? 11,
    color: hue(r.color, r.colorTheme),
    underline: r.underline ?? "none",
    underlineColor: hue(r.underlineColor, r.underlineTheme),
    shading: hue(r.shading),
    em: r.em ?? "none",
    strike: !!r.strike,
    dstrike: !!r.dstrike,
    vert: r.vertAlign === "superscript" || r.vertAlign === "subscript" ? r.vertAlign : "baseline",
    smallCaps: !!r.smallCaps,
    caps: !!r.caps,
    vanish: !!r.vanish,
    outline: !!r.outline,
    shadow: !!r.shadow,
    emboss: !!r.emboss,
    imprint: !!r.imprint,
    scale: r.scale ?? 100,
    spacingKind: spacing > 0 ? "expanded" : spacing < 0 ? "condensed" : "normal",
    spacingBy: Math.abs(spacing) / 20,
    positionKind: position > 0 ? "raised" : position < 0 ? "lowered" : "normal",
    positionBy: Math.abs(position) / 2,
    kern: (r.kern ?? 0) > 0,
    kernFrom: (r.kern ?? 0) > 0 ? r.kern! / 2 : 12,
    ligatures: fx.ligatures ?? "default",
    numSpacing: fx.numSpacing ?? "default",
    numForm: fx.numForm ?? "default",
    styleSet: fx.styleSets?.[0] ?? 0,
    cntxtAlts: !!fx.cntxtAlts,
  };
}

/** The run formatting the dialog's state describes, for the preview. */
function runOf(s: FontState): RunFmt {
  const num = (v: number, k: "raised" | "lowered" | "expanded" | "condensed", unit: number) => (k === "raised" || k === "expanded" ? v * unit : -v * unit);
  return {
    font: s.font || undefined,
    fontEa: s.fontEa || undefined,
    size: s.size,
    bold: s.style === "bold" || s.style === "boldItalic" || undefined,
    italic: s.style === "italic" || s.style === "boldItalic" || undefined,
    color: s.color.hex,
    underline: s.underline === "none" ? undefined : s.underline,
    underlineColor: s.underlineColor.hex,
    shading: s.shading.hex,
    em: s.em === "none" ? undefined : s.em,
    strike: s.strike || undefined,
    dstrike: s.dstrike || undefined,
    vertAlign: s.vert === "baseline" ? undefined : s.vert,
    smallCaps: s.smallCaps || undefined,
    caps: s.caps || undefined,
    outline: s.outline || undefined,
    shadow: s.shadow || undefined,
    emboss: s.emboss || undefined,
    imprint: s.imprint || undefined,
    scale: s.scale,
    spacing: s.spacingKind === "normal" ? undefined : Math.round(num(s.spacingBy, s.spacingKind, 20)),
    position: s.positionKind === "normal" ? undefined : Math.round(num(s.positionBy, s.positionKind, 2)),
    kern: s.kern ? Math.round(s.kernFrom * 2) : undefined,
    fx: {
      ligatures: s.ligatures === "default" ? undefined : s.ligatures,
      numSpacing: s.numSpacing === "default" ? undefined : s.numSpacing,
      numForm: s.numForm === "default" ? undefined : s.numForm,
      styleSets: s.styleSet ? [s.styleSet] : undefined,
      cntxtAlts: s.cntxtAlts || undefined,
    },
  };
}

/** The Font dialog for the selection, or for whatever `target` stands for (a style being edited). */
export function openFont(ed: Editor, target: RunTarget = selectionRunTarget(ed)) {
  const was = read(target.state());
  const now: FontState = { ...was, color: { ...was.color }, underlineColor: { ...was.underlineColor }, shading: { ...was.shading } };
  const theme = ed.engine?.themeColors() ?? null;

  const select = (id: string, items: [string, string][], value: string) => h("select", { id }, ...items.map(([v, label]) => h("option", { value: v, selected: v === value }, t(label))));
  const check = (id: string, label: string, on: boolean) => {
    const input = h("input", { id, type: "checkbox", checked: on });
    return { input, row: h("label", { class: "check", for: id }, input, h("span", {}, label)) };
  };
  const number = (id: string, value: number, min: number, max: number, step: number) => h("input", { id, type: "number", min: String(min), max: String(max), step: String(step), value: String(Math.round(value * 100) / 100) });
  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);
  const group = (title: string, ...body: (HTMLElement | null)[]) => h("div", { class: "pd-group" }, h("h3", {}, title), ...body);
  const grid = (cols: 2 | 3, ...cells: HTMLElement[]) => h("div", { class: `pd-grid fd-grid-${cols}` }, ...cells);

  // The fonts: the usual ones, those the document uses, and the one the text has.
  const used = (ed.engine?.fontsUsed() ?? []).map((u) => u.family);
  const families = [...new Set([...(was.font ? [was.font] : []), ...MENU_FONTS, ...used])];
  const font = h("select", { id: "fd-font" }, ...(was.font ? [] : [h("option", { value: "" }, t("Default"))]), ...families.map((f) => h("option", { value: f, selected: f === was.font, style: `font-family:${fontStack(f)}` }, f)));
  // The East Asian font: the usual ones, those the document uses, and the one the text has.
  const eaFamilies = [...new Set([...(was.fontEa ? [was.fontEa] : []), ...EAST_ASIAN_FONTS])];
  const fontEa = h("select", { id: "fd-font-ea" }, h("option", { value: "", selected: !was.fontEa }, t("Default")), ...eaFamilies.map((f) => h("option", { value: f, selected: f === was.fontEa, style: `font-family:${fontStack(f)}` }, f)));
  const style = select("fd-style", STYLES, was.style);
  const size = h("input", { id: "fd-size", type: "text", inputmode: "decimal", autocomplete: "off", list: "fd-sizes", value: String(was.size) });
  const sizes = h("datalist", { id: "fd-sizes" }, ...SIZES.map((v) => h("option", { value: String(v) })));
  // An underline style the list doesn't have stays as it is until another is chosen.
  const underlines: [string, string][] = UNDERLINE_CHOICES.some(([v]) => v === was.underline) ? UNDERLINE_CHOICES : [...UNDERLINE_CHOICES, [was.underline, was.underline]];
  const underline = select("fd-underline", underlines, was.underline);
  const em = select("fd-em", EMPHASIS, was.em);

  // A colour is a button that shows it and opens the palette.
  const colorBtn = (id: string, get: () => Hue, set: (c: Hue) => void, empty: string, label: string) => colorButton({ id, label, empty, theme, get, set, changed: refresh });
  const color = colorBtn("fd-color", () => now.color, (c) => (now.color = c), t("Automatic"), t("Font color"));
  const underlineColor = colorBtn("fd-underline-color", () => now.underlineColor, (c) => (now.underlineColor = c), t("Automatic"), t("Underline color"));
  const shading = colorBtn("fd-shading", () => now.shading, (c) => (now.shading = c), t("No shading"), t("Shading"));

  // Effects
  const strike = check("fd-strike", t("Strikethrough"), was.strike);
  const dstrike = check("fd-dstrike", t("Double strikethrough"), was.dstrike);
  const sup = check("fd-sup", t("Superscript"), was.vert === "superscript");
  const sub = check("fd-sub", t("Subscript"), was.vert === "subscript");
  const smallCaps = check("fd-small-caps", t("Small caps"), was.smallCaps);
  const caps = check("fd-caps", t("All caps"), was.caps);
  const vanish = check("fd-hidden", t("Hidden"), was.vanish);
  const outline = check("fd-outline", t("Outline"), was.outline);
  const shadow = check("fd-shadow", t("Shadow"), was.shadow);
  const emboss = check("fd-emboss", t("Emboss"), was.emboss);
  const imprint = check("fd-imprint", t("Engrave"), was.imprint);
  // Some pairs exclude each other, as in Word.
  const exclusive = (a: HTMLInputElement, b: HTMLInputElement) => {
    a.addEventListener("change", () => {
      if (a.checked) b.checked = false;
    });
    b.addEventListener("change", () => {
      if (b.checked) a.checked = false;
    });
  };
  exclusive(strike.input, dstrike.input);
  exclusive(sup.input, sub.input);
  exclusive(emboss.input, imprint.input);

  // Advanced
  const scale = number("fd-scale", was.scale, 1, 600, 1);
  const spacingKind = select("fd-spacing", [["normal", msg("Normal")], ["expanded", msg("Expanded")], ["condensed", msg("Condensed")]], was.spacingKind);
  const spacingBy = number("fd-spacing-by", was.spacingBy, 0, 1584, 0.05);
  const positionKind = select("fd-position", [["normal", msg("Normal")], ["raised", msg("Raised")], ["lowered", msg("Lowered")]], was.positionKind);
  const positionBy = number("fd-position-by", was.positionBy, 0, 1584, 0.5);
  const kern = check("fd-kern", t("Kerning for fonts"), was.kern);
  const kernFrom = number("fd-kern-from", was.kernFrom, 1, 1638, 0.5);
  const ligOptions: [string, string][] = LIGATURES.some(([v]) => v === was.ligatures) ? LIGATURES : [...LIGATURES, [was.ligatures, msg("Other")]];
  const ligatures = select("fd-ligatures", ligOptions, was.ligatures);
  const numSpacing = select("fd-num-spacing", NUMBER_SPACING, was.numSpacing);
  const numForm = select("fd-num-form", NUMBER_FORMS, was.numForm);
  const styleSet = h(
    "select",
    { id: "fd-style-set" },
    h("option", { value: "0", selected: was.styleSet === 0 }, t("Default")),
    ...Array.from({ length: 20 }, (_, i) => h("option", { value: String(i + 1), selected: was.styleSet === i + 1 }, String(i + 1))),
  );
  const cntxt = check("fd-cntxt", t("Use contextual alternates"), was.cntxtAlts);

  const syncDisabled = () => {
    spacingBy.disabled = spacingKind.value === "normal";
    positionBy.disabled = positionKind.value === "normal";
    kernFrom.disabled = !kern.input.checked;
  };

  // The preview: a piece of text drawn with the page's own CSS.
  const sample = h("div", { class: "fd-sample" });
  const preview = h("div", { class: "fd-preview", "aria-hidden": "true" }, sample);

  const collect = (): FontState => {
    const v = (el: HTMLInputElement, fallback: number) => (Number.isFinite(parseFloat(el.value.replace(",", "."))) ? parseFloat(el.value.replace(",", ".")) : fallback);
    now.font = font.value;
    now.fontEa = fontEa.value;
    now.style = style.value as FontState["style"];
    now.size = v(size, was.size);
    now.underline = underline.value;
    now.em = em.value;
    now.strike = strike.input.checked;
    now.dstrike = dstrike.input.checked;
    now.vert = sup.input.checked ? "superscript" : sub.input.checked ? "subscript" : "baseline";
    now.smallCaps = smallCaps.input.checked;
    now.caps = caps.input.checked;
    now.vanish = vanish.input.checked;
    now.outline = outline.input.checked;
    now.shadow = shadow.input.checked;
    now.emboss = emboss.input.checked;
    now.imprint = imprint.input.checked;
    now.scale = v(scale, was.scale);
    now.spacingKind = spacingKind.value as FontState["spacingKind"];
    now.spacingBy = Math.max(0, v(spacingBy, was.spacingBy));
    now.positionKind = positionKind.value as FontState["positionKind"];
    now.positionBy = Math.max(0, v(positionBy, was.positionBy));
    now.kern = kern.input.checked;
    now.kernFrom = Math.max(0.5, v(kernFrom, was.kernFrom));
    now.ligatures = ligatures.value;
    now.numSpacing = numSpacing.value;
    now.numForm = numForm.value;
    now.styleSet = Number(styleSet.value);
    now.cntxtAlts = cntxt.input.checked;
    return now;
  };

  function refresh() {
    syncDisabled();
    const f = runOf(collect());
    // The preview is drawn at a fixed size so a large font doesn't overflow it; the sizes are shown by the field.
    const seg: RSeg = { o: 0, k: "t", x: t("Sample text"), f: { ...f, size: Math.min(f.size ?? 11, 28), vanish: undefined } };
    sample.replaceChildren(
      ...prepareSegs([seg], {}, fontStack).map((piece) => {
        const span = h("span", { class: piece.cls.join(" ") });
        span.setAttribute("style", [runStyle(piece.seg.f), ...piece.css].filter(Boolean).join(";"));
        span.textContent = piece.seg.x ?? "";
        return span;
      }),
    );
  }

  // Layout
  const fontPane = h(
    "div",
    { class: "pd-pane", role: "tabpanel", id: "fd-pane-1", "aria-labelledby": "fd-tab-1" },
    grid(3, field(t("Font"), font, "fd-font"), field(t("Font style"), style, "fd-style"), field(t("Size"), size, "fd-size"), sizes),
    grid(3, field(t("Font color"), color.b, "fd-color"), field(t("Underline style"), underline, "fd-underline"), field(t("Underline color"), underlineColor.b, "fd-underline-color")),
    group(
      t("Effects"),
      h("div", { class: "fd-effects" }, strike.row, dstrike.row, sup.row, sub.row, smallCaps.row, caps.row, vanish.row, outline.row, shadow.row, emboss.row, imprint.row),
    ),
    grid(3, field(t("Shading"), shading.b, "fd-shading"), field(t("Emphasis mark"), em, "fd-em"), field(t("Asian text font"), fontEa, "fd-font-ea")),
  );
  const advancedPane = h(
    "div",
    { class: "pd-pane", role: "tabpanel", id: "fd-pane-2", "aria-labelledby": "fd-tab-2", hidden: true },
    group(
      t("Character spacing"),
      grid(
        2,
        field(t("Scale (%)"), scale, "fd-scale"),
        h("div", { class: "field" }),
        field(t("Spacing"), spacingKind, "fd-spacing"),
        field(t("By (pt)"), spacingBy, "fd-spacing-by"),
        field(t("Position"), positionKind, "fd-position"),
        field(t("By (pt)"), positionBy, "fd-position-by"),
        h("div", { class: "field" }, kern.row),
        field(t("Points and above"), kernFrom, "fd-kern-from"),
      ),
    ),
    group(
      t("OpenType features"),
      grid(2, field(t("Ligatures"), ligatures, "fd-ligatures"), field(t("Number spacing"), numSpacing, "fd-num-spacing"), field(t("Number forms"), numForm, "fd-num-form"), field(t("Stylistic sets"), styleSet, "fd-style-set")),
      cntxt.row,
    ),
  );
  const tab1 = h("button", { type: "button", role: "tab", id: "fd-tab-1", "aria-controls": "fd-pane-1", "aria-selected": "true" }, t("Font"));
  const tab2 = h("button", { type: "button", role: "tab", id: "fd-tab-2", "aria-controls": "fd-pane-2", "aria-selected": "false" }, t("Advanced"));
  const show = (n: 1 | 2) => {
    tab1.classList.toggle("on", n === 1);
    tab2.classList.toggle("on", n === 2);
    tab1.setAttribute("aria-selected", String(n === 1));
    tab2.setAttribute("aria-selected", String(n === 2));
    fontPane.hidden = n !== 1;
    advancedPane.hidden = n !== 2;
  };
  tab1.addEventListener("click", () => show(1));
  tab2.addEventListener("click", () => show(2));
  show(1);

  const asDefault = h("button", { class: "btn ghost pd-tabs", type: "button" }, t("Set as default"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog para-dialog font-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Font") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Font"))),
    h("div", { class: "pd-tablist seg", role: "tablist" }, tab1, tab2),
    h("div", { class: "dialog-body" }, fontPane, advancedPane, h("div", { class: "field" }, h("label", {}, t("Preview")), preview)),
    h("div", { class: "dialog-actions" }, target.defaults ? asDefault : null, cancel, ok),
  );
  dialog.addEventListener("input", refresh);
  dialog.addEventListener("change", refresh);
  refresh();
  const close = modal(dialog);

  ok.addEventListener("click", () => {
    const patch = fontPatch(was, collect());
    close();
    if (Object.keys(patch).length) target.setRun(patch);
    target.done();
  });
  cancel.addEventListener("click", close);
  asDefault.addEventListener("click", async () => {
    const patch = fontPatch(was, collect());
    if (!Object.keys(patch).length) return;
    const yes = await confirmDialog({ title: t("Set as default"), body: t("Use these settings for all text in this document that doesn't say otherwise?"), ok: t("Set as default") });
    if (!yes) return;
    close();
    if (ed.exec({ op: "set_default_format", format: patch })) toast(t("Default font settings changed"));
    ed.focus();
  });
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && (e.target as HTMLInputElement).type !== "checkbox") {
      e.preventDefault();
      ok.click();
    }
  });
}
