// The list styles Docxtor offers, as the engine's presets (`set_list_style`): what the galleries and
// the Format menu show for each. The glyphs match what the engine writes for the first three levels.
import { msg } from "@docxtor/extensions";

export interface ListPreset {
  /** The engine's preset id. */
  id: string;
  /** Spoken name of the style (the gallery tile's label, the command's title). Translate with `t`. */
  title: string;
  /** Short name for menu lines. Translate with `t`. */
  name: string;
  /** What the first three levels start with. */
  levels: [string, string, string];
}

export const BULLET_PRESETS: ListPreset[] = [
  { id: "disc", title: msg("Disc bullets"), name: msg("Disc"), levels: ["●", "○", "■"] },
  { id: "diamond", title: msg("Diamond bullets"), name: msg("Diamond"), levels: ["❖", "➢", "■"] },
  { id: "arrow", title: msg("Arrow bullets"), name: msg("Arrow"), levels: ["➔", "◆", "●"] },
  { id: "star", title: msg("Star bullets"), name: msg("Star"), levels: ["★", "○", "■"] },
  { id: "arrowhead", title: msg("Arrowhead bullets"), name: msg("Arrowhead"), levels: ["➢", "○", "■"] },
  { id: "box", title: msg("Box bullets"), name: msg("Box"), levels: ["❑", "➢", "■"] },
];

export const NUMBER_PRESETS: ListPreset[] = [
  { id: "decimal", title: msg("Decimal numbering 1. a. i."), name: msg("1. a. i."), levels: ["1.", "a.", "i."] },
  { id: "paren", title: msg("Parenthesis numbering 1) a) i)"), name: msg("1) a) i)"), levels: ["1)", "a)", "i)"] },
  { id: "outline", title: msg("Outline numbering 1. 1.1. 1.1.1."), name: msg("1. 1.1. 1.1.1."), levels: ["1.", "1.1.", "1.1.1."] },
  { id: "upper", title: msg("Capital letter numbering A. a. i."), name: msg("A. a. i."), levels: ["A.", "a.", "i."] },
  { id: "roman", title: msg("Roman numeral numbering I. A. 1."), name: msg("I. A. 1."), levels: ["I.", "A.", "1."] },
  { id: "zero", title: msg("Leading zero numbering 01. a. i."), name: msg("01. a. i."), levels: ["01.", "a.", "i."] },
];

export const LIST_PRESETS: ListPreset[] = [...BULLET_PRESETS, ...NUMBER_PRESETS];
