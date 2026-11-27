// Colours of a chart in the Edit chart dialog: the document theme's swatches (as Word's colour picker lays them out),
// the standard colours, and any hex. A colour is `RRGGBB`, or a theme colour written `scheme:accent2:lumMod=60000:lumOff=40000`
// (what the engine writes as `a:schemeClr`), so a chart keeps following the theme when the theme changes.

export interface Swatch {
  /** The colour as the chart stores it: `RRGGBB` or a `scheme:` setting. */
  spec: string;
  hex: string;
  label: string;
}


/** Word's standard colours. */
export const STANDARD: string[] = ["C00000", "FF0000", "FFC000", "FFFF00", "92D050", "00B050", "00B0F0", "0070C0", "002060", "7030A0"];

const OFFICE: Record<string, string> = { lt1: "FFFFFF", dk1: "000000", lt2: "E7E6E6", dk2: "44546A", accent1: "4472C4", accent2: "ED7D31", accent3: "A5A5A5", accent4: "FFC000", accent5: "5B9BD5", accent6: "70AD47" };

/** The scheme names a chart uses, by the theme's own name for the colour. */
const SCHEME: Record<string, string> = { bg1: "lt1", tx1: "dk1", bg2: "lt2", tx2: "dk2" };

function toRgb(hex: string): [number, number, number] {
  const n = parseInt(hex, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex(r: number, g: number, b: number): string {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
  return `${c(r)}${c(g)}${c(b)}`.toUpperCase();
}

function toHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (Math.abs(max - min) < 1e-9) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = Math.abs(max - r) < 1e-9 ? (g - b) / d + (g < b ? 6 : 0) : Math.abs(max - g) < 1e-9 ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h / 6, s, l];
}

function fromHsl(h: number, s: number, l: number): [number, number, number] {
  if (Math.abs(s) < 1e-9) return [l * 255, l * 255, l * 255];
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const f = (t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    const v = t < 1 / 6 ? p + (q - p) * 6 * t : t < 0.5 ? q : t < 2 / 3 ? p + (q - p) * (2 / 3 - t) * 6 : p;
    return v * 255;
  };
  return [f(h + 1 / 3), f(h), f(h - 1 / 3)];
}

/** A colour setting as `RRGGBB`, with the theme's colours and the luminance changes applied. */
export function resolveColor(spec: string | undefined, theme: Record<string, string> | undefined): string | undefined {
  if (!spec) return undefined;
  if (!spec.startsWith("scheme:")) return /^[0-9a-f]{6}$/i.test(spec) ? spec.toUpperCase() : undefined;
  const [, name, ...mods] = spec.split(":");
  const key = SCHEME[name!] ?? name!;
  const base = theme?.[key] ?? OFFICE[key];
  if (!base) return undefined;
  const m: Record<string, number> = {};
  for (const p of mods) {
    const [k, v] = p.split("=");
    if (k && v) m[k] = Number(v) / 100000;
  }
  let [r, g, b] = toRgb(base);
  if (m.lumMod !== undefined || m.lumOff !== undefined) {
    const [h, s, l] = toHsl(r, g, b);
    [r, g, b] = fromHsl(h, s, Math.max(0, Math.min(1, l * (m.lumMod ?? 1) + (m.lumOff ?? 0))));
  }
  return toHex(r, g, b);
}

/** The theme part of the picker: ten theme colours across, and lighter and darker shades of each below. */
export function themeSwatches(theme: Record<string, string> | undefined, label: (name: string, shade: string) => string): Swatch[][] {
  const order = ["bg1", "tx1", "bg2", "tx2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6"];
  const shades: [string, string][] = [
    ["", ""],
    [":lumMod=20000:lumOff=80000", "lighter 80%"],
    [":lumMod=40000:lumOff=60000", "lighter 60%"],
    [":lumMod=60000:lumOff=40000", "lighter 40%"],
    [":lumMod=75000", "darker 25%"],
    [":lumMod=50000", "darker 50%"],
  ];
  return shades.map(([mods, shade]) =>
    order.map((name) => {
      const spec = `scheme:${name}${mods}`;
      return { spec, hex: resolveColor(spec, theme) ?? "000000", label: label(name, shade) };
    }),
  );
}

