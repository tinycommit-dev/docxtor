// The colour choice of the Edit chart dialog: the document theme's colours (with their lighter and darker shades),
// Word's standard colours and any hex. A chosen theme colour is kept as a theme colour, so the chart follows the theme.
import { t } from "@docxtor/extensions";
import { STANDARD, resolveColor, themeSwatches, type Swatch } from "../chart/colors";
import { h } from "./dom";
import { hexOr } from "../editor/color.ts";

export interface ColorPicker {
  el: HTMLElement;
  /** Show the colour that is current (a spec, or nothing for the automatic colour). */
  show(spec: string | undefined): void;
}

let counter = 0;

const SHADES: Record<string, () => string> = {
  "lighter 80%": () => t("lighter 80%"),
  "lighter 60%": () => t("lighter 60%"),
  "lighter 40%": () => t("lighter 40%"),
  "darker 25%": () => t("darker 25%"),
  "darker 50%": () => t("darker 50%"),
};

/**
 * `automatic` is what the colour is when none is set (shown on the Automatic button). `set` is called with the
 * colour as the chart stores it: `RRGGBB`, a `scheme:` setting, or undefined for automatic.
 */
export function colorPicker(theme: Record<string, string> | undefined, label: string, set: (spec: string | undefined) => void, allowAutomatic = true): ColorPicker {
  const id = `chart-color-${++counter}`;
  const rows = themeSwatches(theme, (name, shade) => {
    const names: Record<string, string> = { bg1: t("Background 1"), tx1: t("Text 1"), bg2: t("Background 2"), tx2: t("Text 2") };
    const base = names[name] ?? t("Accent {n}", { n: name.replace("accent", "") });
    return shade ? t("{name}, {shade}", { name: base, shade: SHADES[shade]!() }) : base;
  });
  const standard: Swatch[] = STANDARD.map((hex) => ({ spec: hex, hex, label: `#${hex}` }));
  const buttons: [string, HTMLButtonElement][] = [];
  const grid = (list: Swatch[], cls: string) => {
    const g = h("div", { class: `swatches chart-swatches ${cls}` });
    for (const sw of list) {
      const b = h("button", { type: "button", class: "swatch", title: sw.label, "aria-label": sw.label, "aria-pressed": "false", "data-spec": sw.spec, style: `background:${hexOr(sw.hex)}` });
      b.addEventListener("click", () => {
        set(sw.spec);
        show(sw.spec);
      });
      g.append(b);
      buttons.push([sw.spec, b]);
    }
    return g;
  };
  const themeGrid = h("div", { class: "chart-theme-colors", role: "group", "aria-label": t("Theme colors") }, ...rows.map((r, i) => grid(r, i === 0 ? "base" : "shade")));
  const stdGrid = h("div", { role: "group", "aria-label": t("Standard colors") }, grid(standard, "std"));
  const hex = h("input", { type: "text", id, class: "chart-hex", maxlength: "7", placeholder: "RRGGBB", autocomplete: "off", spellcheck: "false", "aria-label": t("{label}, hex color", { label }) });
  const chip = h("span", { class: "chart-chip", "aria-hidden": "true" });
  hex.addEventListener("input", () => {
    const v = hex.value.trim().replace(/^#/, "");
    const ok = /^[0-9a-f]{6}$/i.test(v);
    hex.toggleAttribute("aria-invalid", hex.value.trim() !== "" && !ok);
    if (ok) {
      set(v.toUpperCase());
      mark(v.toUpperCase());
      chip.style.background = `#${v}`;
    }
  });
  const auto = h("button", { type: "button", class: "btn", "aria-pressed": "false" }, t("Automatic"));
  auto.addEventListener("click", () => {
    set(undefined);
    show(undefined);
  });
  auto.hidden = !allowAutomatic;

  const mark = (spec: string | undefined) => {
    for (const [s, b] of buttons) {
      const on = !!spec && s.toLowerCase() === spec.toLowerCase();
      b.classList.toggle("on", on);
      b.setAttribute("aria-pressed", String(on));
    }
    auto.classList.toggle("on", !spec);
    auto.setAttribute("aria-pressed", String(!spec));
  };
  const show = (spec: string | undefined) => {
    mark(spec);
    const rgb = resolveColor(spec, theme);
    chip.style.background = rgb ? `#${rgb}` : "transparent";
    if (document.activeElement !== hex) hex.value = spec && !spec.startsWith("scheme:") ? spec : (rgb ?? "");
  };
  const el = h(
    "div",
    { class: "chart-colors" },
    h("p", { class: "chart-sub" }, t("Theme colors")),
    themeGrid,
    h("p", { class: "chart-sub" }, t("Standard colors")),
    stdGrid,
    h("div", { class: "chart-hexrow" }, chip, h("label", { for: id }, t("Hex color")), hex, auto),
  );
  return { el, show };
}
