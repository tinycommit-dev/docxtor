// Design: themes, colour schemes, font schemes, style sets and paragraph spacing. Each gallery shows what the choice
// would look like (a theme's colours and heading font, a style set's title and headings drawn in the document's own
// theme), and picking one is one op. What the galleries offer comes from the `documentThemes` and `styleSets`
// contribution points, so Docxtor's own and an extension's sit side by side.
import { Engine, THEME_COLOR_SLOTS, type Op, type RunFmt, type SpacingPreset, type StyleSetPreview, type ThemeColorSlot } from "@docxtor/core";
import { msg, t, tn, type DocumentThemeContribution, type ExtensionHost, type StyleSetContribution } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { fontStack } from "../editor/render";
import { h, modal, popover } from "./dom";
import { fontOptions } from "./style-parts";
import { tileCss } from "./style-preview";
import { hex, hexOr } from "../editor/color.ts";

export interface DesignContext {
  ed: Editor;
  host: ExtensionHost;
}

type Anchor = HTMLElement | { x: number; y: number };

/** Where a gallery opens when a menu or the palette asked for it: just inside the top left of the page area. */
export function designAnchor(ed: Editor): { x: number; y: number } {
  const r = ed.canvas.getBoundingClientRect();
  return { x: r.left + 24, y: r.top + 8 };
}

/** The names Word's colour slots go by in its colours dialog. */
export const SLOT_NAMES: Record<ThemeColorSlot, string> = {
  dk1: msg("Text/Background, Dark 1"),
  lt1: msg("Text/Background, Light 1"),
  dk2: msg("Text/Background, Dark 2"),
  lt2: msg("Text/Background, Light 2"),
  accent1: msg("Accent 1"),
  accent2: msg("Accent 2"),
  accent3: msg("Accent 3"),
  accent4: msg("Accent 4"),
  accent5: msg("Accent 5"),
  accent6: msg("Accent 6"),
  hlink: msg("Hyperlink"),
  folHlink: msg("Followed hyperlink"),
};

// Word's paragraph spacing presets, listed so the extractor finds their names (the engine sends them in English).
const PRESET_NAMES = [msg("Default"), msg("No paragraph space"), msg("Compact"), msg("Tight"), msg("Open"), msg("Relaxed"), msg("Double")];
export const presetName = (name: string) => (PRESET_NAMES.includes(name) ? t(name) : name);

/** The last contribution of each id, in the order they were added. */
function lastOfEach<T extends { id: string }>(items: T[]): T[] {
  const byId = new Map<string, T>();
  for (const i of items) byId.set(i.id, i);
  return [...byId.values()];
}

export const wholeThemes = (host: ExtensionHost) => lastOfEach(host.documentThemes.list()).filter((d) => d.colors && d.fonts);
export const colorSchemes = (host: ExtensionHost) => lastOfEach(host.documentThemes.list()).filter((d) => d.colors && !d.fonts);
export const fontSchemes = (host: ExtensionHost) => lastOfEach(host.documentThemes.list()).filter((d) => d.fonts && !d.colors);

// -- applying a choice -------------------------------------------------------------------------

function themeOp(d: DocumentThemeContribution): Op {
  if (d.colors && d.fonts) return { op: "set_theme", definition: { name: d.label, colors: { name: d.label, colors: d.colors }, fonts: { name: d.label, ...d.fonts } } };
  if (d.colors) return { op: "set_theme", colors: { name: d.label, colors: d.colors } };
  return { op: "set_theme", fonts: { name: d.label, ...d.fonts! } };
}

export function applyDocumentTheme(ed: Editor, d: DocumentThemeContribution) {
  ed.exec(themeOp(d), { select: ed.sel, keepExtra: true });
  ed.reloadPageSetup();
  ed.focus();
}

export function applyStyleSet(ed: Editor, s: StyleSetContribution) {
  const op: Op = s.styles ? { op: "set_style_set", definition: { name: s.label, styles: s.styles, ...(s.spacing ? { spacing: s.spacing } : {}) } } : { op: "set_style_set", style_set: s.id };
  ed.exec(op, { select: ed.sel, keepExtra: true });
  ed.reloadPageSetup();
  ed.focus();
}

export function applySpacing(ed: Editor, preset: SpacingPreset | string) {
  ed.exec({ op: "set_paragraph_spacing", preset: typeof preset === "string" ? preset : preset.id }, { select: ed.sel, keepExtra: true });
  ed.reloadPageSetup();
  ed.focus();
}

// -- galleries -----------------------------------------------------------------------------------

const CHIP_SLOTS: ThemeColorSlot[] = ["dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6"];

function chips(colors: Partial<Record<ThemeColorSlot, string>>, slots: ThemeColorSlot[] = CHIP_SLOTS): HTMLElement {
  return h("span", { class: "theme-chips", "aria-hidden": "true" }, ...slots.map((s) => h("i", { style: hex(colors[s]) ? `background:${hex(colors[s])}` : "" })));
}

/** Pick and close; the gallery keeps arrow-key movement among its buttons. */
function keyboard(root: HTMLElement, selector: string, columns: number) {
  root.addEventListener("keydown", (ev) => {
    const all = Array.from(root.querySelectorAll<HTMLElement>(selector));
    const i = all.indexOf(document.activeElement as HTMLElement);
    if (i < 0) return;
    const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: columns, ArrowUp: -columns };
    let to = -1;
    if (ev.key in step) to = i + step[ev.key];
    else if (ev.key === "Home") to = 0;
    else if (ev.key === "End") to = all.length - 1;
    else return;
    ev.preventDefault();
    if (to < 0 || to >= all.length) return;
    all.forEach((x) => (x.tabIndex = -1));
    all[to].tabIndex = 0;
    all[to].focus();
  });
}

function groups<T extends { group?: string }>(items: T[], first: string): [string, T[]][] {
  const out = new Map<string, T[]>();
  for (const i of items) out.set(i.group ?? first, [...(out.get(i.group ?? first) ?? []), i]);
  return [...out];
}

/** Design, Themes: every theme as its colours and heading font. */
export function openThemes({ ed, host }: DesignContext, anchor: Anchor = designAnchor(ed)) {
  const engine = ed.engine;
  if (!engine) return;
  const now = engine.themeInfo();
  const root = h("div", { class: "design-gallery theme-gallery", role: "group", "aria-label": t("Themes") });
  let close = () => {};
  for (const [title, list] of groups(wholeThemes(host), t("Built-in themes"))) {
    const grid = h("div", { class: "theme-grid", role: "listbox", "aria-label": title });
    for (const d of list) {
      const on = now.name === d.label;
      const tile = h(
        "button",
        { class: "theme-tile", type: "button", role: "option", "aria-selected": String(on), "data-theme-id": d.id, title: d.label, tabindex: on ? 0 : -1 },
        h("span", { class: "theme-aa", style: `font-family:${fontStack(d.fonts!.major)}` }, "Aa"),
        chips(d.colors!, ["accent1", "accent2", "accent3", "accent4", "accent5", "accent6"]),
        h("span", { class: "theme-name" }, d.label),
      );
      tile.addEventListener("click", () => {
        close();
        applyDocumentTheme(ed, d);
      });
      grid.append(tile);
    }
    root.append(h("div", { class: "menu-title" }, title), grid);
  }
  if (!root.querySelector('.theme-tile[tabindex="0"]')) root.querySelector<HTMLElement>(".theme-tile")?.setAttribute("tabindex", "0");
  keyboard(root, ".theme-tile", 4);
  close = popover(anchor, root, { onClose: () => ed.focus() });
  root.querySelector<HTMLElement>('.theme-tile[tabindex="0"]')?.focus();
}

/** Design, Colors: the colour schemes, and Customize colors at the foot. */
export function openColors(ctx: DesignContext, anchor: Anchor = designAnchor(ctx.ed)) {
  const { ed, host } = ctx;
  const engine = ed.engine;
  if (!engine) return;
  const now = engine.themeInfo();
  const root = h("div", { class: "design-gallery scheme-gallery", role: "group", "aria-label": t("Colors") });
  let close = () => {};
  const list = h("div", { class: "scheme-list", role: "listbox", "aria-label": t("Colors") });
  for (const d of colorSchemes(host)) {
    const on = now.colors.name === d.label;
    const row = h("button", { class: "scheme-row", type: "button", role: "option", "aria-selected": String(on), "data-theme-id": d.id, tabindex: on ? 0 : -1 }, chips(d.colors!), h("span", { class: "theme-name" }, d.label));
    row.addEventListener("click", () => {
      close();
      applyDocumentTheme(ed, d);
    });
    list.append(row);
  }
  if (!list.querySelector('[tabindex="0"]') && list.firstElementChild) (list.firstElementChild as HTMLElement).tabIndex = 0;
  const custom = h("button", { class: "menu-item", type: "button" }, h("span", {}, t("Customize colors…")));
  custom.addEventListener("click", () => {
    close();
    openCustomizeColors(ctx);
  });
  root.append(list, h("div", { class: "menu-sep" }), custom);
  keyboard(list, ".scheme-row", 1);
  close = popover(anchor, root, { onClose: () => ed.focus() });
  (list.querySelector('[tabindex="0"]') as HTMLElement | null)?.focus();
}

/** Design, Fonts: the font schemes, and Customize fonts at the foot. */
export function openFonts(ctx: DesignContext, anchor: Anchor = designAnchor(ctx.ed)) {
  const { ed, host } = ctx;
  const engine = ed.engine;
  if (!engine) return;
  const now = engine.themeInfo();
  const root = h("div", { class: "design-gallery scheme-gallery", role: "group", "aria-label": t("Fonts") });
  let close = () => {};
  const list = h("div", { class: "scheme-list", role: "listbox", "aria-label": t("Fonts") });
  for (const d of fontSchemes(host)) {
    const on = now.fonts.name === d.label;
    const row = h(
      "button",
      { class: "scheme-row font-row", type: "button", role: "option", "aria-selected": String(on), "data-theme-id": d.id, tabindex: on ? 0 : -1 },
      h("span", { class: "font-aa", style: `font-family:${fontStack(d.fonts!.major)}` }, "Aa"),
      h("span", { class: "font-aa body", style: `font-family:${fontStack(d.fonts!.minor)}` }, "Aa"),
      h("span", { class: "theme-name" }, d.label),
      h("span", { class: "font-names" }, d.fonts!.major === d.fonts!.minor ? d.fonts!.major : `${d.fonts!.major} / ${d.fonts!.minor}`),
    );
    row.addEventListener("click", () => {
      close();
      applyDocumentTheme(ed, d);
    });
    list.append(row);
  }
  if (!list.querySelector('[tabindex="0"]') && list.firstElementChild) (list.firstElementChild as HTMLElement).tabIndex = 0;
  const custom = h("button", { class: "menu-item", type: "button" }, h("span", {}, t("Customize fonts…")));
  custom.addEventListener("click", () => {
    close();
    openCustomizeFonts(ctx);
  });
  root.append(list, h("div", { class: "menu-sep" }), custom);
  keyboard(list, ".scheme-row", 1);
  close = popover(anchor, root, { onClose: () => ed.focus() });
  (list.querySelector('[tabindex="0"]') as HTMLElement | null)?.focus();
}

/** A style set drawn as a small page: its title, a heading and a few lines in the document's theme. */
function setTile(p: StyleSetPreview): HTMLElement {
  const sample = (id: string) => p.samples.find((s) => s.id === id);
  const line = (id: string, text: string, scale: number) => {
    const s = sample(id);
    if (!s) return null;
    const run: RunFmt = { ...s.run, size: (s.run.size ?? 11) * scale };
    return h("div", { class: `set-line set-${id}`, style: `${tileCss(run, s.para, { min: 5, max: 16 })};${s.para.borders?.bottom ? "border-bottom:1px solid currentColor;" : ""}${hex(s.para.shading) ? `background:${hex(s.para.shading)};padding:1px 2px;` : ""}` }, text);
  };
  return h(
    "span",
    { class: "set-page", "aria-hidden": "true" },
    line("Title", t("Title"), 0.5),
    line("Heading1", t("Heading 1"), 0.5),
    line("Normal", t("Body text. Body text."), 0.5),
    line("Heading2", t("Heading 2"), 0.5),
    line("Normal", t("Body text. Body text."), 0.5),
  );
}

/** Design, Style set. */
export function openStyleSets({ ed, host }: DesignContext, anchor: Anchor = designAnchor(ed)) {
  const engine = ed.engine;
  if (!engine) return;
  const root = h("div", { class: "design-gallery set-gallery", role: "group", "aria-label": t("Style set") });
  let close = () => {};
  const builtin = new Map(engine.styleSetPreviews().map((p) => [p.id, p]));
  const grid = h("div", { class: "set-grid", role: "listbox", "aria-label": t("Style set") });
  const sets = lastOfEach(host.styleSets.list());
  for (const s of sets) {
    let preview: StyleSetPreview | undefined = s.styles ? undefined : builtin.get(s.id);
    if (s.styles) {
      try {
        preview = engine.styleSetPreview({ name: s.label, styles: s.styles, ...(s.spacing ? { spacing: s.spacing } : {}) });
      } catch (e) {
        console.warn(`Docxtor: style set ${s.id} was left out: ${(e as Error).message ?? e}`);
        continue;
      }
    }
    if (!preview) continue;
    const tile = h("button", { class: "set-tile", type: "button", role: "option", "aria-selected": "false", "data-set-id": s.id, title: s.description ?? s.label, tabindex: grid.childElementCount ? -1 : 0 }, setTile(preview), h("span", { class: "theme-name" }, s.label));
    tile.addEventListener("click", () => {
      close();
      applyStyleSet(ed, s);
    });
    grid.append(tile);
  }
  root.append(grid);
  const note = h("p", { class: "menu-hint" }, t("A style set rewrites Normal, Title, Subtitle, the headings and Quote. Paragraphs keep their styles. Docxtor's sets are close to Word's, not copies."));
  root.append(note);
  keyboard(root, ".set-tile", 4);
  close = popover(anchor, root, { onClose: () => ed.focus() });
  root.querySelector<HTMLElement>('.set-tile[tabindex="0"]')?.focus();
}

/** Design, Paragraph spacing. */
export function openSpacing({ ed, host }: DesignContext, anchor: Anchor = designAnchor(ed)) {
  const engine = ed.engine;
  if (!engine) return;
  const presets = Engine.spacingPresets();
  const normal = engine.styleDetails("Normal");
  const after = normal?.para.spaceAfter ?? 0;
  const line = normal?.para.line ?? 240;
  const list = h("div", {});
  let close = () => {};
  for (const p of presets) {
    const on = p.after === after && p.line === line;
    const lines = Math.round((p.line / 240) * 100) / 100;
    const item = h("button", { class: `menu-item${on ? " active" : ""}`, type: "button", role: "menuitemradio", "aria-checked": String(on), "data-preset": p.id }, h("span", {}, presetName(p.name)), h("span", { class: "hint" }, tn(lines,"{pt} pt after, {n} line", "{pt} pt after, {n} lines", { pt: String(p.after / 20) })));
    item.addEventListener("click", () => {
      close();
      applySpacing(ed, p);
    });
    list.append(item);
  }
  const lines = host.command("design.setAsDefault");
  if (lines) {
    list.append(h("div", { class: "menu-sep", role: "separator" }));
    const def = h("button", { class: "menu-item", type: "button", role: "menuitem" }, h("span", {}, t("Set as default")));
    def.addEventListener("click", () => {
      close();
      void host.execute("design.setAsDefault").catch(() => undefined);
    });
    list.append(def);
  }
  close = popover(anchor, list, { onClose: () => ed.focus() });
}

// -- dialogs -------------------------------------------------------------------------------------

/** Design, Colors, Customize colors: the twelve colours of the theme, with a name for the scheme. */
export function openCustomizeColors({ ed }: DesignContext) {
  const engine = ed.engine;
  if (!engine) return;
  const now = engine.themeInfo();
  const values: Record<string, string> = {};
  for (const slot of THEME_COLOR_SLOTS) values[slot] = (now.colors.colors[slot] ?? "000000").toUpperCase();
  const name = h("input", { id: "cc-name", type: "text", value: now.colors.name && !["Office", "Office 2007-2010"].includes(now.colors.name) ? now.colors.name : t("Custom"), autocomplete: "off", spellcheck: "false" });
  const sample = h("div", { class: "cc-sample", "aria-hidden": "true" });
  const paint = () => sample.replaceChildren(...THEME_COLOR_SLOTS.map((s) => h("i", { style: `background:${hexOr(values[s])}`, title: t(SLOT_NAMES[s]) })));
  const inputs: Record<string, { picker: HTMLInputElement; hex: HTMLInputElement }> = {};
  const rows = THEME_COLOR_SLOTS.map((slot) => {
    const picker = h("input", { type: "color", id: `cc-${slot}`, value: `#${values[slot].toLowerCase()}`, "aria-label": t(SLOT_NAMES[slot]) });
    const hex = h("input", { type: "text", value: values[slot], maxlength: "7", "aria-label": t("{slot} (hex)", { slot: t(SLOT_NAMES[slot]) }), autocomplete: "off", spellcheck: "false", class: "cc-hex" });
    picker.addEventListener("input", () => {
      values[slot] = picker.value.slice(1).toUpperCase();
      hex.value = values[slot];
      paint();
    });
    hex.addEventListener("input", () => {
      const v = hex.value.replace(/^#/, "");
      if (/^[0-9a-fA-F]{6}$/.test(v)) {
        values[slot] = v.toUpperCase();
        picker.value = `#${v.toLowerCase()}`;
        paint();
      }
    });
    inputs[slot] = { picker, hex };
    return h("div", { class: "cc-row" }, h("label", { for: `cc-${slot}` }, t(SLOT_NAMES[slot])), picker, hex);
  });
  paint();
  const reset = h("button", { class: "btn ghost", type: "button" }, t("Reset"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const save = h("button", { class: "btn primary", type: "button" }, t("Save"));
  const err = h("p", { class: "ms-error help", role: "alert" });
  const dialog = h(
    "div",
    { class: "dialog theme-colors-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Customize colors") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Customize colors")), h("p", { class: "sub" }, t("Text, headings, tables, shapes and charts that use theme colors change with these."))),
    h("div", { class: "dialog-body" }, h("div", { class: "cc-grid" }, ...rows), sample, h("div", { class: "field" }, h("label", { for: "cc-name" }, t("Name")), name), err),
    h("div", { class: "dialog-actions" }, reset, cancel, save),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  reset.addEventListener("click", () => {
    for (const slot of THEME_COLOR_SLOTS) {
      values[slot] = (now.colors.colors[slot] ?? "000000").toUpperCase();
      inputs[slot].hex.value = values[slot];
      inputs[slot].picker.value = `#${values[slot].toLowerCase()}`;
    }
    paint();
  });
  save.addEventListener("click", () => {
    const label = name.value.trim() || t("Custom");
    const res = ed.exec({ op: "set_theme", colors: { name: label, colors: values } }, { select: ed.sel, keepExtra: true, onError: (m) => (err.textContent = m) });
    if (!res) return;
    close();
    ed.reloadPageSetup();
  });
}

/** Design, Fonts, Customize fonts: the heading font and the body font. */
export function openCustomizeFonts({ ed }: DesignContext) {
  const engine = ed.engine;
  if (!engine) return;
  const now = engine.themeInfo();
  const used = engine.fontsUsed().map((f) => f.family);
  const select = (id: string, current: string) => h("select", { id }, ...fontOptions([...used, now.fonts.major, now.fonts.minor], current).map((f) => h("option", { value: f, selected: f === current }, f)));
  const major = select("cf-major", now.fonts.major);
  const minor = select("cf-minor", now.fonts.minor);
  const name = h("input", { id: "cf-name", type: "text", value: t("Custom"), autocomplete: "off", spellcheck: "false" });
  const headingSample = h("div", { class: "cf-sample-head" }, t("Heading"));
  const bodySample = h("div", { class: "cf-sample-body" }, t("Body text sits here, in the body font."));
  const paint = () => {
    headingSample.style.fontFamily = fontStack(major.value) ?? "";
    bodySample.style.fontFamily = fontStack(minor.value) ?? "";
  };
  major.addEventListener("change", paint);
  minor.addEventListener("change", paint);
  paint();
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const save = h("button", { class: "btn primary", type: "button" }, t("Save"));
  const err = h("p", { class: "ms-error help", role: "alert" });
  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Customize fonts") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Customize fonts")), h("p", { class: "sub" }, t("Text that uses the theme fonts changes with these."))),
    h("div", { class: "dialog-body" }, field(t("Heading font"), major, "cf-major"), field(t("Body font"), minor, "cf-minor"), h("div", { class: "cf-sample" }, headingSample, bodySample), field(t("Name"), name, "cf-name"), err),
    h("div", { class: "dialog-actions" }, cancel, save),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  save.addEventListener("click", () => {
    const label = name.value.trim() || t("Custom");
    const res = ed.exec({ op: "set_theme", fonts: { name: label, major: major.value, minor: minor.value } }, { select: ed.sel, keepExtra: true, onError: (m) => (err.textContent = m) });
    if (!res) return;
    close();
    ed.reloadPageSetup();
  });
}
