// The colour palettes of the Change colors gallery: Word's own (colourful 1 to 4, monochromatic 1 to 13, which the
// engine builds from the document theme and writes as `colorsN.xml`), and any an extension registers.
import { t, type ChartPaletteContribution } from "@docxtor/extensions";

export interface PaletteInfo {
  id: string;
  label: () => string;
  /** An extension's palette: its colours (the engine's own are made by the engine). */
  colors?: string[];
}

export function palettes(extra: ChartPaletteContribution[] = []): PaletteInfo[] {
  return [
    ...[1, 2, 3, 4].map((n) => ({ id: `colorful${n}`, label: () => t("Colorful palette {n}", { n }) })),
    ...Array.from({ length: 13 }, (_, k) => k + 1).map((n) => ({ id: `mono${n}`, label: () => t("Monochromatic palette {n}", { n }) })),
    ...extra.map((p) => ({ id: `ext:${p.id}`, label: () => p.label, colors: p.colors })),
  ];
}
