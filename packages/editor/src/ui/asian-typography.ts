// The Asian typography tab of the Paragraph dialog: the rules for breaking lines in East Asian text, spacing between
// Asian and Latin text, grid snapping and font alignment. It reads the paragraph's formatting (Word's defaults where the
// file says nothing) and gives back a patch with only what the person changed.
import type { ParaFmt, ParaPatch } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { h } from "./dom";

/** East Asian fonts the Font dialog offers beside the ones a document uses (the browser falls back to a system font for those a device lacks). */
export const EAST_ASIAN_FONTS = [
  "MS Mincho",
  "MS Gothic",
  "Yu Mincho",
  "Yu Gothic",
  "Meiryo",
  "SimSun",
  "SimHei",
  "Microsoft YaHei",
  "PMingLiU",
  "Microsoft JhengHei",
  "Malgun Gothic",
  "Batang",
  "Gulim",
  "Hiragino Mincho ProN",
  "Hiragino Sans",
  "PingFang SC",
  "Noto Serif CJK JP",
  "Noto Sans CJK JP",
];

const ALIGNMENTS: [string, string][] = [
  ["auto", msg("Automatic")],
  ["top", msg("Top")],
  ["center", msg("Centered")],
  ["baseline", msg("Baseline")],
  ["bottom", msg("Bottom")],
];

/** The switches as the dialog shows them: Word's defaults where the paragraph says nothing. */
interface Switches {
  kinsoku: boolean;
  latinBreak: boolean;
  hang: boolean;
  compress: boolean;
  spaceDE: boolean;
  spaceDN: boolean;
  grid: boolean;
  align: string;
}

function read(p: ParaFmt): Switches {
  return {
    kinsoku: p.kinsoku !== false,
    // The file says "break Latin words only at their ends" (word wrap); the box is the opposite.
    latinBreak: p.wordWrap === false,
    hang: p.overflowPunct !== false,
    compress: !!p.topLinePunct,
    spaceDE: p.autoSpaceDe !== false,
    spaceDN: p.autoSpaceDn !== false,
    grid: p.snapToGrid !== false,
    align: p.textAlignment ?? "auto",
  };
}

/** The tab's content, and the patch for what differs from the paragraph's own formatting. */
export function asianTypography(p: ParaFmt): { pane: HTMLElement; patch: () => ParaPatch } {
  const was = read(p);
  const check = (id: string, label: string, on: boolean) => {
    const input = h("input", { id, type: "checkbox", checked: on });
    return { input, row: h("label", { class: "check", for: id }, input, h("span", {}, label)) };
  };
  const kinsoku = check("ad-kinsoku", t("Follow the East Asian rules for the first and last character of a line"), was.kinsoku);
  const latinBreak = check("ad-latin-break", t("Allow Latin text to wrap in the middle of a word"), was.latinBreak);
  const hang = check("ad-hang", t("Allow punctuation to hang past the margin"), was.hang);
  const compress = check("ad-compress", t("Compress punctuation at the start of a line"), was.compress);
  const spaceDE = check("ad-space-de", t("Adjust the space between Asian and Latin text"), was.spaceDE);
  const spaceDN = check("ad-space-dn", t("Adjust the space between Asian text and numbers"), was.spaceDN);
  const grid = check("ad-grid", t("Snap to the document grid"), was.grid);
  const align = h("select", { id: "ad-align" }, ...ALIGNMENTS.map(([v, label]) => h("option", { value: v, selected: v === was.align }, t(label))));
  const group = (title: string, ...body: HTMLElement[]) => h("div", { class: "pd-group" }, h("h3", {}, title), ...body);
  const pane = h(
    "div",
    { class: "pd-pane", role: "tabpanel", id: "pd-pane-3", "aria-labelledby": "pd-tab-3", hidden: true },
    group(t("Line breaks"), kinsoku.row, latinBreak.row, hang.row, compress.row),
    group(t("Spacing"), spaceDE.row, spaceDN.row, grid.row),
    group(t("Font alignment"), h("div", { class: "field" }, h("label", { for: "ad-align" }, t("Align text of different sizes to the")), align)),
  );
  const patch = (): ParaPatch => {
    const out: ParaPatch = {};
    if (kinsoku.input.checked !== was.kinsoku) out.kinsoku = kinsoku.input.checked;
    if (latinBreak.input.checked !== was.latinBreak) out.wordWrap = !latinBreak.input.checked;
    if (hang.input.checked !== was.hang) out.overflowPunct = hang.input.checked;
    if (compress.input.checked !== was.compress) out.topLinePunct = compress.input.checked;
    if (spaceDE.input.checked !== was.spaceDE) out.autoSpaceDe = spaceDE.input.checked;
    if (spaceDN.input.checked !== was.spaceDN) out.autoSpaceDn = spaceDN.input.checked;
    if (grid.input.checked !== was.grid) out.snapToGrid = grid.input.checked;
    if (align.value !== was.align) out.textAlignment = align.value;
    return out;
  };
  return { pane, patch };
}
