// avoid-ai-design-ignore-file: C1 (the hue track of a colour picker has to be the whole rainbow)
// Colour choices shared by the toolbar's menus and the dialogs: a grid of swatches (`swatchGrid`), the palette Word
// shows (`colorPalette`: the document's theme colours with their lighter and darker rows, standard colours, recent
// colours, and More colors…), and the dialog behind More colors… (`openColors`: hex, RGB and HSL, and an
// eyedropper where the browser has one).
import { msg, t } from "@docxtor/extensions";
import { h, modal, popover } from "./dom.ts";
import { hexOr } from "../editor/color.ts";

export interface SwatchGrid {
  el: HTMLElement;
  /** Mark the swatch of this colour as the current one (none when the colour isn't in the grid). */
  select(color: string | null): void;
}

/**
 * `colors` are hex values without the `#`. `name` labels each swatch for assistive technology. With `more`, a
 * More colors… button below the swatches opens the colour dialog for any other colour.
 */
export function swatchGrid(colors: string[], current: string | null, pick: (color: string) => void, name: (color: string) => string = (c) => `#${c}`, opts: { more?: boolean } = {}): SwatchGrid {
  const grid = h("div", { class: "swatches" });
  const el = opts.more ? h("div", { class: "swatch-wrap" }, grid) : grid;
  if (opts.more) {
    const more = h("button", { class: "btn ghost swatch-more", type: "button" }, t("More colors…"));
    more.addEventListener("click", () => openColors(current, (hex) => pick(hex)));
    el.append(more);
  }
  const buttons = colors.map((c) => {
    const b = h("button", { class: "swatch", type: "button", title: name(c), "aria-label": name(c), "aria-pressed": "false", style: `background:${hexOr(c)}` });
    b.addEventListener("click", () => pick(c));
    grid.append(b);
    return [c, b] as const;
  });
  const select = (color: string | null) => {
    for (const [c, b] of buttons) {
      const on = !!color && c.toUpperCase() === color.toUpperCase();
      b.classList.toggle("on", on);
      b.setAttribute("aria-pressed", String(on));
    }
  };
  select(current);
  return { el, select };
}

// ---------------------------------------------------------------------------
// Colour maths
// ---------------------------------------------------------------------------

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const byte = (n: number) => clamp(Math.round(n), 0, 255).toString(16).padStart(2, "0").toUpperCase();

export function parseHex(hex: string): [number, number, number] {
  const c = hex.replace("#", "");
  return [0, 2, 4].map((i) => parseInt(c.slice(i, i + 2), 16) || 0) as [number, number, number];
}

export const toHex = (r: number, g: number, b: number) => `${byte(r)}${byte(g)}${byte(b)}`;

/** Word's tint (towards white) and shade (towards black), as the file writes them: hex 00 to FF. */
export function tinted(hex: string, tint?: string, shade?: string): string {
  let [r, g, b] = parseHex(hex);
  if (tint) {
    const t0 = parseInt(tint, 16) / 255;
    [r, g, b] = [r, g, b].map((c) => c * t0 + 255 * (1 - t0)) as [number, number, number];
  }
  if (shade) {
    const s = parseInt(shade, 16) / 255;
    [r, g, b] = [r, g, b].map((c) => c * s) as [number, number, number];
  }
  return toHex(r, g, b);
}

export function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B);
  const min = Math.min(R, G, B);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l * 100];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const hue = max === R ? (G - B) / d + (G < B ? 6 : 0) : max === G ? (B - R) / d + 2 : (R - G) / d + 4;
  return [hue * 60, s * 100, l * 100];
}

export function hslToRgb(h0: number, s0: number, l0: number): [number, number, number] {
  const [hh, s, l] = [(((h0 % 360) + 360) % 360) / 360, clamp(s0, 0, 100) / 100, clamp(l0, 0, 100) / 100];
  if (s === 0) return [l * 255, l * 255, l * 255];
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const channel = (t0: number) => {
    const x = ((t0 % 1) + 1) % 1;
    const v = x < 1 / 6 ? p + (q - p) * 6 * x : x < 0.5 ? q : x < 2 / 3 ? p + (q - p) * (2 / 3 - x) * 6 : p;
    return v * 255;
  };
  return [channel(hh + 1 / 3), channel(hh), channel(hh - 1 / 3)];
}

// ---------------------------------------------------------------------------
// The palette
// ---------------------------------------------------------------------------

/** A colour picked from the palette: its hex, and the theme colour it stands for (written as themeColor, themeTint, themeShade). */
export interface ColorPick {
  hex: string;
  theme?: { name: string; tint?: string; shade?: string };
}

interface Variant {
  tint?: string;
  shade?: string;
  /** Lighter (+) or darker (-) by this many percent, for the tooltip. */
  by: number;
}

const lighter = (tint: string, by: number): Variant => ({ tint, by });
const darker = (shade: string, by: number): Variant => ({ shade, by: -by });

/** Word's ten theme columns and the five variants each offers, with the scheme colour each one is. */
const THEME_COLUMNS: { name: string; scheme: string; label: string; rows: Variant[] }[] = [
  { name: "background1", scheme: "lt1", label: msg("Background 1"), rows: [darker("F2", 5), darker("D9", 15), darker("BF", 25), darker("A6", 35), darker("80", 50)] },
  { name: "text1", scheme: "dk1", label: msg("Text 1"), rows: [lighter("80", 50), lighter("A6", 35), lighter("BF", 25), lighter("D9", 15), lighter("F2", 5)] },
  { name: "background2", scheme: "lt2", label: msg("Background 2"), rows: [darker("E6", 10), darker("BF", 25), darker("80", 50), darker("40", 75), darker("1A", 90)] },
  { name: "text2", scheme: "dk2", label: msg("Text 2"), rows: [lighter("1A", 90), lighter("40", 75), lighter("66", 60), darker("BF", 25), darker("80", 50)] },
  ...[1, 2, 3, 4, 5, 6].map((n) => ({
    name: `accent${n}`,
    scheme: `accent${n}`,
    label: [msg("Accent 1"), msg("Accent 2"), msg("Accent 3"), msg("Accent 4"), msg("Accent 5"), msg("Accent 6")][n - 1],
    rows: [lighter("33", 80), lighter("66", 60), lighter("99", 40), darker("BF", 25), darker("80", 50)],
  })),
];

/** The ten standard colours of Word's menu. */
export const STANDARD_COLORS: [string, string][] = [
  ["C00000", msg("Dark red")],
  ["FF0000", msg("Red")],
  ["FFC000", msg("Orange")],
  ["FFFF00", msg("Yellow")],
  ["92D050", msg("Light green")],
  ["00B050", msg("Green")],
  ["00B0F0", msg("Light blue")],
  ["0070C0", msg("Blue")],
  ["002060", msg("Dark blue")],
  ["7030A0", msg("Purple")],
];

const RECENT_MAX = 10;
let recent: string[] | null = null;

/** Where the recent colours last between sessions. The host gives the place (the editor keeps nothing in the browser itself). */
export interface RecentColorStore {
  get(): unknown;
  set(colors: string[]): void;
}
let store: RecentColorStore | null = null;

/** Keep the recent colours in `s` from now on (the editing built-in passes the extension's own state). */
export function keepRecentColorsIn(s: RecentColorStore | null) {
  store = s;
  recent = null;
}

/** The colours picked lately (hex, newest first). Without a store they last for this page only. */
export function recentColors(): string[] {
  if (!recent) {
    const v = store?.get();
    recent = Array.isArray(v) ? v.filter((c): c is string => typeof c === "string" && /^[0-9a-f]{6}$/i.test(c)).slice(0, RECENT_MAX) : [];
  }
  return recent;
}

/** Remember a colour as the newest recent one. */
export function rememberColor(hex: string) {
  const c = hex.replace("#", "").toUpperCase();
  recent = [c, ...recentColors().filter((x) => x.toUpperCase() !== c)].slice(0, RECENT_MAX);
  store?.set(recent);
}

/** What a colour menu needs. */
export interface PaletteOptions {
  /** The document's theme colours (`Engine.themeColors()`: `[name, hex]` pairs, or a map by name); without them the theme rows are left out. */
  theme?: [string, string][] | Record<string, string> | null;
  /** The colour that is on now (hex), or null. */
  current?: string | null;
  pick: (c: ColorPick) => void;
  /** A first line for the automatic colour (text) ... */
  automatic?: { label: string; on?: boolean; run: () => void };
  /** ... and a last one for no colour (fills). */
  none?: { label: string; on?: boolean; run: () => void };
  /** Called just before the More colors dialog opens (a menu closes itself here). */
  beforeMore?: () => void;
  /** The title of the palette, as the first line. */
  title?: string;
}

export interface ColorPalette {
  el: HTMLElement;
  /** Mark the swatch of this colour as the current one; `special` marks the Automatic or None line instead. */
  select(hex: string | null, special?: "automatic" | "none"): void;
}

/**
 * The colours Word offers: theme colours in columns with lighter and darker rows below, the standard colours, the
 * ones picked lately, and More colors…. A theme colour comes back with its name, tint and shade so it can be
 * written as the file writes it.
 */
export function colorPalette(o: PaletteOptions): ColorPalette {
  const el = h("div", { class: "palette" });
  const swatches: [string, HTMLButtonElement][] = [];
  const swatch = (hex: string, label: string, pick: () => void, parent: HTMLElement) => {
    const b = h("button", { class: "swatch", type: "button", title: label, "aria-label": label, "aria-pressed": "false", style: `background:${hexOr(hex)}` });
    b.addEventListener("click", pick);
    parent.append(b);
    swatches.push([hex, b]);
    return b;
  };
  const choose = (c: ColorPick) => {
    rememberColor(c.hex);
    o.pick(c);
  };
  const line = (item: NonNullable<PaletteOptions["automatic"]>, cls = "") => {
    const b = h("button", { class: `menu-item pal-line ${cls}`.trim(), type: "button", "aria-pressed": String(!!item.on) }, item.label);
    b.classList.toggle("on", !!item.on);
    b.addEventListener("click", item.run);
    return b;
  };
  if (o.title) el.append(h("div", { class: "menu-title" }, o.title));
  const autoLine = o.automatic ? line(o.automatic) : null;
  if (autoLine) el.append(autoLine);
  if (o.theme) {
    const theme: Record<string, string> = Array.isArray(o.theme) ? Object.fromEntries(o.theme) : o.theme;
    el.append(h("div", { class: "pal-title" }, t("Theme colors")));
    const grid = h("div", { class: "pal-theme" });
    for (const col of THEME_COLUMNS) {
      const base = theme[col.scheme];
      if (!base) continue;
      const cell = h("div", { class: "pal-col" });
      swatch(base.toUpperCase(), t(col.label), () => choose({ hex: base.toUpperCase(), theme: { name: col.name } }), cell);
      for (const v of col.rows) {
        const hex = tinted(base, v.tint, v.shade);
        const name = v.by > 0 ? t("{color}, lighter {n}%", { color: t(col.label), n: v.by }) : t("{color}, darker {n}%", { color: t(col.label), n: -v.by });
        swatch(hex, name, () => choose({ hex, theme: { name: col.name, tint: v.tint, shade: v.shade } }), cell);
      }
      grid.append(cell);
    }
    el.append(grid);
  }
  el.append(h("div", { class: "pal-title" }, t("Standard colors")));
  const std = h("div", { class: "pal-row" });
  for (const [hex, name] of STANDARD_COLORS) swatch(hex, `${t(name)} #${hex}`, () => choose({ hex }), std);
  el.append(std);
  const lately = recentColors();
  if (lately.length) {
    el.append(h("div", { class: "pal-title" }, t("Recent colors")));
    const row = h("div", { class: "pal-row" });
    for (const hex of lately) swatch(hex.toUpperCase(), `#${hex.toUpperCase()}`, () => choose({ hex: hex.toUpperCase() }), row);
    el.append(row);
  }
  const noneLine = o.none ? line(o.none) : null;
  if (noneLine) el.append(noneLine);
  const more = h("button", { class: "menu-item pal-line", type: "button" }, t("More colors…"));
  more.addEventListener("click", () => {
    o.beforeMore?.();
    openColors(o.current ?? null, (hex) => choose({ hex }));
  });
  el.append(more);
  const select = (color: string | null, special?: "automatic" | "none") => {
    for (const [line, key] of [[autoLine, "automatic"], [noneLine, "none"]] as const) {
      line?.classList.toggle("on", special === key);
      line?.setAttribute("aria-pressed", String(special === key));
    }
    let found = false;
    for (const [hex, b] of swatches) {
      // A theme colour and a plain one can look the same; the first match is the one that is on.
      const on: boolean = !!color && !found && hex.toUpperCase() === color.toUpperCase();
      found ||= on;
      b.classList.toggle("on", on);
      b.setAttribute("aria-pressed", String(on));
    }
  };
  select(o.current ?? null, o.automatic?.on ? "automatic" : o.none?.on ? "none" : undefined);
  return { el, select };
}

/** A colour a button holds: none (automatic, or no fill), or a hex with the theme colour it came from. */
export interface Hue {
  hex?: string;
  theme?: { name: string; tint?: string; shade?: string };
}

/**
 * A button that shows a colour and opens the palette in a menu under it, for dialogs. `empty` names the choice with
 * no colour ("Automatic", "No shading"); `set` hears the choice and `changed` follows it.
 */
export function colorButton(o: { id: string; label: string; empty: string; theme: PaletteOptions["theme"]; get: () => Hue; set: (c: Hue) => void; changed?: () => void }): { b: HTMLButtonElement; paint: () => void } {
  const dot = h("span", { class: "fd-dot", "aria-hidden": "true" });
  const text = h("span", {}, "");
  const b = h("button", { id: o.id, type: "button", class: "fd-color", "aria-haspopup": "menu", "aria-label": o.label }, dot, text);
  const paint = () => {
    const c = o.get();
    dot.style.background = c.hex ? `#${c.hex}` : "transparent";
    dot.classList.toggle("none", !c.hex);
    text.textContent = c.hex ? (c.theme ? t("Theme color") : `#${c.hex}`) : o.empty;
    // The name holds the words shown on the key ("Font color: Automatic"), so voice control finds it (WCAG 2.5.3).
    b.setAttribute("aria-label", `${o.label}: ${text.textContent}`);
  };
  b.addEventListener("click", () => {
    const box = h("div", {});
    const close = popover(b, box);
    const done = (c: Hue) => {
      close();
      o.set(c);
      paint();
      o.changed?.();
    };
    box.append(
      colorPalette({
        theme: o.theme,
        current: o.get().hex ?? null,
        automatic: { label: o.empty, on: !o.get().hex, run: () => done({}) },
        pick: (c) => done({ hex: c.hex, theme: c.theme }),
        beforeMore: close,
      }).el,
    );
  });
  paint();
  return { b, paint };
}

/** The run formatting for a colour pick, with the theme colour beside the hex as Word writes them. */
export function colorPatch(c: ColorPick): { color: string; themeColor?: string; themeTint?: string; themeShade?: string } {
  return { color: c.hex, ...(c.theme ? { themeColor: c.theme.name, ...(c.theme.tint ? { themeTint: c.theme.tint } : {}), ...(c.theme.shade ? { themeShade: c.theme.shade } : {}) } : {}) };
}

/** The same for an underline colour. */
export function underlineColorPatch(c: ColorPick): { underlineColor: string; underlineTheme?: string; underlineTint?: string; underlineShade?: string } {
  return { underlineColor: c.hex, ...(c.theme ? { underlineTheme: c.theme.name, ...(c.theme.tint ? { underlineTint: c.theme.tint } : {}), ...(c.theme.shade ? { underlineShade: c.theme.shade } : {}) } : {}) };
}

// ---------------------------------------------------------------------------
// More colors
// ---------------------------------------------------------------------------

interface EyeDropperLike {
  open(): Promise<{ sRGBHex: string }>;
}

/** The dialog behind More colors…: sliders and fields for hex, red, green and blue, hue, saturation and lightness. */
export function openColors(initial: string | null, done: (hex: string) => void) {
  let [r, g, b] = parseHex(initial && /^[0-9a-f]{6}$/i.test(initial) ? initial : "000000");
  let [hue, sat, lig] = rgbToHsl(r, g, b);
  const was = toHex(r, g, b);

  const num = (id: string, min: number, max: number) => h("input", { id, type: "number", min: String(min), max: String(max), step: "1", inputmode: "numeric" });
  const range = (id: string, max: number, label: string) => h("input", { id, type: "range", min: "0", max: String(max), step: "1", "aria-label": label, class: "cd-range" });
  const hex = h("input", { id: "cd-hex", type: "text", maxlength: "7", autocomplete: "off", spellcheck: "false" });
  const R = num("cd-r", 0, 255);
  const G = num("cd-g", 0, 255);
  const B = num("cd-b", 0, 255);
  const H = num("cd-h", 0, 360);
  const S = num("cd-s", 0, 100);
  const L = num("cd-l", 0, 100);
  const hueRange = range("cd-hue", 360, t("Hue"));
  const satRange = range("cd-sat", 100, t("Saturation"));
  const ligRange = range("cd-lig", 100, t("Lightness"));
  const now = h("span", { class: "cd-now", "aria-hidden": "true" });
  const old = h("span", { class: "cd-old", style: `background:${hexOr(was)}`, "aria-hidden": "true" });

  const paint = () => {
    const color = toHex(r, g, b);
    now.style.background = `#${color}`;
    hueRange.style.background = "linear-gradient(90deg,#f00,#ff0,#0f0,#0ff,#00f,#f0f,#f00)";
    satRange.style.background = `linear-gradient(90deg,hsl(${hue} 0% ${lig}%),hsl(${hue} 100% ${lig}%))`;
    ligRange.style.background = `linear-gradient(90deg,#000,hsl(${hue} ${sat}% 50%),#fff)`;
    if (document.activeElement !== hex) hex.value = `#${color}`;
    const set = (input: HTMLInputElement, v: number) => {
      if (document.activeElement !== input) input.value = String(Math.round(v));
    };
    set(R, r);
    set(G, g);
    set(B, b);
    set(H, hue);
    set(S, sat);
    set(L, lig);
    hueRange.value = String(Math.round(hue));
    satRange.value = String(Math.round(sat));
    ligRange.value = String(Math.round(lig));
  };
  const fromRgb = () => {
    [hue, sat, lig] = rgbToHsl(r, g, b);
    paint();
  };
  const fromHsl = () => {
    [r, g, b] = hslToRgb(hue, sat, lig).map((v) => clamp(Math.round(v), 0, 255)) as [number, number, number];
    paint();
  };
  const val = (input: HTMLInputElement, max: number) => clamp(Number(input.value) || 0, 0, max);
  for (const [input, apply] of [
    [R, () => (r = val(R, 255))],
    [G, () => (g = val(G, 255))],
    [B, () => (b = val(B, 255))],
  ] as const)
    input.addEventListener("input", () => {
      apply();
      fromRgb();
    });
  for (const [input, apply] of [
    [H, () => (hue = val(H, 360))],
    [S, () => (sat = val(S, 100))],
    [L, () => (lig = val(L, 100))],
    [hueRange, () => (hue = val(hueRange, 360))],
    [satRange, () => (sat = val(satRange, 100))],
    [ligRange, () => (lig = val(ligRange, 100))],
  ] as const)
    input.addEventListener("input", () => {
      apply();
      fromHsl();
    });
  hex.addEventListener("input", () => {
    const v = hex.value.replace("#", "");
    if (/^[0-9a-f]{6}$/i.test(v)) {
      [r, g, b] = parseHex(v);
      fromRgb();
    } else if (/^[0-9a-f]{3}$/i.test(v)) {
      [r, g, b] = parseHex([...v].map((c) => c + c).join(""));
      fromRgb();
    }
  });
  hex.addEventListener("blur", paint);

  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);
  const eyedropper = (window as unknown as { EyeDropper?: new () => EyeDropperLike }).EyeDropper;
  const pickFromScreen = eyedropper ? h("button", { class: "btn ghost cd-eye", type: "button" }, t("Pick from screen")) : null;
  pickFromScreen?.addEventListener("click", async () => {
    try {
      const res = await new eyedropper!().open();
      [r, g, b] = parseHex(res.sRGBHex);
      fromRgb();
    } catch {
      /* cancelled with Escape */
    }
  });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog colors-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Colors") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Colors"))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "cd-top" }, h("div", { class: "cd-compare", role: "img", "aria-label": t("Old and new color") }, old, now), field(t("Hex"), hex, "cd-hex"), pickFromScreen),
      h("div", { class: "cd-sliders" }, hueRange, satRange, ligRange),
      h(
        "div",
        { class: "cd-grid" },
        field(t("Red"), R, "cd-r"),
        field(t("Green"), G, "cd-g"),
        field(t("Blue"), B, "cd-b"),
        field(t("Hue"), H, "cd-h"),
        field(t("Saturation"), S, "cd-s"),
        field(t("Lightness"), L, "cd-l"),
      ),
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  paint();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    close();
    done(toHex(r, g, b));
  });
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT") {
      e.preventDefault();
      ok.click();
    }
  });
  hex.focus();
  hex.select();
}
