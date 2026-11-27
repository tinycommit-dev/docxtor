// The Paragraph dialog (Format, Paragraph…): alignment, outline level and direction; indents, spacing and line
// spacing; and where the paragraph may fall on the page. Lengths follow the unit setting (see units.ts); spacing is
// in points, as in Word. Only what the person changed is written, so a paragraph keeps what it had otherwise.
import type { ParaPatch } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { visualAlign, type VisualAlign } from "../editor/align";
import type { Editor } from "../editor/editor";
import { paraIndents, paraSpace } from "../editor/render";
import { formatUnits, parseLength, unitName } from "../units";
import { h, modal } from "./dom";
import { selectionTarget, type FormatTarget } from "./format-target";
import { openTabs } from "./tabs-dialog";
import { asianTypography } from "./asian-typography";

const ALIGNS: [VisualAlign, string][] = [
  ["left", msg("Left")],
  ["center", msg("Centered")],
  ["right", msg("Right")],
  ["both", msg("Justified")],
  ["distribute", msg("Distributed")],
];

const SPECIAL: [string, string][] = [
  ["none", msg("(none)")],
  ["first", msg("First line")],
  ["hanging", msg("Hanging")],
];

const LINES: [string, string][] = [
  ["single", msg("Single")],
  ["1.5", msg("1.5 lines")],
  ["double", msg("Double")],
  ["atLeast", msg("At least")],
  ["exactly", msg("Exactly")],
  ["multiple", msg("Multiple")],
];

const LEVELS = [msg("Level 1"), msg("Level 2"), msg("Level 3"), msg("Level 4"), msg("Level 5"), msg("Level 6"), msg("Level 7"), msg("Level 8"), msg("Level 9")];

type Special = "none" | "first" | "hanging";
type LineKind = "single" | "1.5" | "double" | "atLeast" | "exactly" | "multiple";

/** What the dialog shows, and the values to compare with to find what changed. */
interface State {
  align: VisualAlign;
  outline: number;
  rtl: boolean;
  left: number;
  right: number;
  special: Special;
  by: number;
  mirror: boolean;
  before: number;
  beforeAuto: boolean;
  after: number;
  afterAuto: boolean;
  kind: LineKind;
  at: number;
  contextual: boolean;
  widow: boolean;
  keepNext: boolean;
  keepLines: boolean;
  pageBreak: boolean;
  noLineNumbers: boolean;
  noHyphen: boolean;
}

const pt = (twips: number) => Math.round((twips / 20) * 100) / 100;

function read(target: FormatTarget): State {
  const { para: p, run } = target.state();
  const ind = paraIndents(p, run);
  const space = paraSpace(p, run);
  const px = (v: number) => Math.round(v * 15);
  let kind: LineKind = "single";
  let at = 1;
  const rule = p.lineRule ?? "auto";
  if (p.line && rule === "exact") (kind = "exactly"), (at = pt(p.line));
  else if (p.line && rule === "atLeast") (kind = "atLeast"), (at = pt(p.line));
  else if (p.line) {
    at = Math.round((p.line / 240) * 100) / 100;
    kind = Math.abs(at - 1) < 0.005 ? "single" : Math.abs(at - 1.5) < 0.005 ? "1.5" : Math.abs(at - 2) < 0.005 ? "double" : "multiple";
  }
  return {
    align: visualAlign(p.align, p.bidi),
    outline: p.outlineLvl !== undefined && p.outlineLvl < 9 ? p.outlineLvl : 9,
    rtl: !!p.bidi,
    // The fields are named by side of the text (start and end, or inside and outside), as the engine's indLeft and indRight are.
    left: px(ind.start),
    right: px(ind.end),
    special: ind.hang ? "hanging" : ind.first ? "first" : "none",
    by: px(ind.hang || ind.first),
    mirror: !!p.mirrorIndents,
    before: space.before,
    beforeAuto: !!p.beforeAuto,
    after: space.after,
    afterAuto: !!p.afterAuto,
    kind,
    at,
    contextual: !!p.contextualSpacing,
    widow: p.widowControl !== false,
    keepNext: !!p.keepNext,
    keepLines: !!p.keepLines,
    pageBreak: !!p.pageBreakBefore,
    noLineNumbers: !!p.suppressLineNumbers,
    noHyphen: !!p.suppressAutoHyphens,
  };
}

/** The dialog formats the selection, or whatever `target` stands for (a style being edited). */
export function openParagraph(ed: Editor, target: FormatTarget = selectionTarget(ed)) {
  const was = read(target);

  const select = (id: string, items: [string, string][], value: string) => h("select", { id }, ...items.map(([v, label]) => h("option", { value: v, selected: v === value }, t(label))));
  const check = (id: string, label: string, on: boolean) => {
    const input = h("input", { id, type: "checkbox", checked: on });
    return { input, row: h("label", { class: "check", for: id }, input, h("span", {}, label)) };
  };
  const length = (id: string, twips: number) => h("input", { id, type: "text", inputmode: "decimal", autocomplete: "off", value: formatUnits(twips) });
  const points = (id: string, twips: number) => h("input", { id, type: "number", min: "0", max: "1584", step: "1", value: String(pt(twips)) });
  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);

  // General
  const align = select("pd-align", ALIGNS, was.align);
  const outline = select("pd-outline", [["9", msg("Body text")], ...LEVELS.map((label, i): [string, string] => [String(i), label])], String(was.outline));
  const direction = select("pd-direction", [["ltr", msg("Left-to-right")], ["rtl", msg("Right-to-left")]], was.rtl ? "rtl" : "ltr");

  // Indentation
  const left = length("pd-left", was.left);
  const right = length("pd-right", was.right);
  const special = select("pd-special", SPECIAL, was.special);
  const by = length("pd-by", was.by);
  const mirror = check("pd-mirror", t("Mirror indents"), was.mirror);
  const edited = new Set<HTMLInputElement>();
  for (const c of [left, right, by]) c.addEventListener("input", () => edited.add(c));
  const leftLabel = h("label", { for: "pd-left" });
  const rightLabel = h("label", { for: "pd-right" });
  const sides = () => {
    const unit = unitName();
    const rtl = direction.value === "rtl";
    leftLabel.textContent = mirror.input.checked ? t("Inside ({unit})", { unit }) : rtl ? t("Before text ({unit})", { unit }) : t("Left ({unit})", { unit });
    rightLabel.textContent = mirror.input.checked ? t("Outside ({unit})", { unit }) : rtl ? t("After text ({unit})", { unit }) : t("Right ({unit})", { unit });
    by.disabled = special.value === "none";
  };

  // Spacing
  const before = points("pd-before", was.before);
  const beforeAuto = check("pd-before-auto", t("Auto"), was.beforeAuto);
  const after = points("pd-after", was.after);
  const afterAuto = check("pd-after-auto", t("Auto"), was.afterAuto);
  const line = select("pd-line", LINES, was.kind);
  const at = h("input", { id: "pd-at", type: "number", min: "0", step: "0.05", value: String(was.at) });
  const contextual = check("pd-contextual", t("Don't add space between paragraphs of the same style"), was.contextual);
  const spacing = () => {
    before.disabled = beforeAuto.input.checked;
    after.disabled = afterAuto.input.checked;
    const k = line.value as LineKind;
    at.disabled = k === "single" || k === "1.5" || k === "double";
    at.step = k === "atLeast" || k === "exactly" ? "1" : "0.05";
    if (k === "single") at.value = "1";
    else if (k === "1.5") at.value = "1.5";
    else if (k === "double") at.value = "2";
  };

  // Line and page breaks
  const widow = check("pd-widow", t("Widow/orphan control"), was.widow);
  const keepNext = check("pd-keep-next", t("Keep with next"), was.keepNext);
  const keepLines = check("pd-keep-lines", t("Keep lines together"), was.keepLines);
  const pageBreak = check("pd-page-break", t("Page break before"), was.pageBreak);
  const noNumbers = check("pd-no-numbers", t("Suppress line numbers"), was.noLineNumbers);
  const noHyphen = check("pd-no-hyphen", t("Don't hyphenate"), was.noHyphen);

  const group = (title: string, ...body: (HTMLElement | null)[]) => h("div", { class: "pd-group" }, h("h3", {}, title), ...body);
  const grid = (...cells: HTMLElement[]) => h("div", { class: "pd-grid" }, ...cells);
  const asian = asianTypography(target.state().para);

  const indents = h(
    "div",
    { class: "pd-pane", role: "tabpanel", id: "pd-pane-1", "aria-labelledby": "pd-tab-1" },
    group(t("General"), grid(field(t("Alignment"), align, "pd-align"), field(t("Outline level"), outline, "pd-outline"), field(t("Direction"), direction, "pd-direction"))),
    group(
      t("Indentation"),
      grid(
        h("div", { class: "field" }, leftLabel, left),
        h("div", { class: "field" }, rightLabel, right),
        field(t("Special"), special, "pd-special"),
        field(t("By ({unit})", { unit: unitName() }), by, "pd-by"),
      ),
      mirror.row,
    ),
    group(
      t("Spacing"),
      grid(
        h("div", { class: "field" }, h("label", { for: "pd-before" }, t("Before (pt)")), before, beforeAuto.row),
        h("div", { class: "field" }, h("label", { for: "pd-after" }, t("After (pt)")), after, afterAuto.row),
        field(t("Line spacing"), line, "pd-line"),
        field(t("At"), at, "pd-at"),
      ),
      contextual.row,
    ),
  );
  const breaks = h(
    "div",
    { class: "pd-pane", role: "tabpanel", id: "pd-pane-2", "aria-labelledby": "pd-tab-2", hidden: true },
    group(t("Pagination"), widow.row, keepNext.row, keepLines.row, pageBreak.row),
    group(t("Formatting exceptions"), noNumbers.row, noHyphen.row),
  );

  const tab1 = h("button", { type: "button", role: "tab", id: "pd-tab-1", "aria-controls": "pd-pane-1", "aria-selected": "true" }, t("Indents and spacing"));
  const tab2 = h("button", { type: "button", role: "tab", id: "pd-tab-2", "aria-controls": "pd-pane-2", "aria-selected": "false" }, t("Line and page breaks"));
  const tab3 = h("button", { type: "button", role: "tab", id: "pd-tab-3", "aria-controls": "pd-pane-3", "aria-selected": "false" }, t("Asian typography"));
  const show = (n: 1 | 2 | 3) => {
    tab1.classList.toggle("on", n === 1);
    tab2.classList.toggle("on", n === 2);
    tab3.classList.toggle("on", n === 3);
    tab1.setAttribute("aria-selected", String(n === 1));
    tab2.setAttribute("aria-selected", String(n === 2));
    tab3.setAttribute("aria-selected", String(n === 3));
    indents.hidden = n !== 1;
    breaks.hidden = n !== 2;
    asian.pane.hidden = n !== 3;
  };
  tab1.addEventListener("click", () => show(1));
  tab2.addEventListener("click", () => show(2));
  tab3.addEventListener("click", () => show(3));
  show(1);

  const tabs = h("button", { class: "btn ghost pd-tabs", type: "button" }, t("Tabs…"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog para-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Paragraph") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Paragraph"))),
    h("div", { class: "pd-tablist seg", role: "tablist" }, tab1, tab2, tab3),
    h("div", { class: "dialog-body" }, indents, breaks, asian.pane),
    h("div", { class: "dialog-actions" }, tabs, cancel, ok),
  );
  for (const c of [mirror.input, direction, special]) c.addEventListener("change", sides);
  for (const c of [beforeAuto.input, afterAuto.input, line]) c.addEventListener("change", spacing);
  sides();
  spacing();
  const close = modal(dialog);

  /** The patch for what changed, and the alignment if it did. */
  const collect = (): { patch: ParaPatch; align?: VisualAlign } => {
    const patch: ParaPatch = {};
    // A field nobody typed in keeps its exact twips: the text shown is rounded, and must not rewrite the file.
    const len = (input: HTMLInputElement, was: number) => {
      if (!edited.has(input)) return was;
      const v = parseLength(input.value);
      return v === null ? was : Math.max(0, v);
    };
    const rtl = direction.value === "rtl";
    if (rtl !== was.rtl) patch.bidi = rtl;
    const lvl = Number(outline.value);
    if (lvl !== was.outline) patch.outlineLvl = lvl;
    const l = len(left, was.left);
    const r = len(right, was.right);
    if (l !== was.left) patch.indLeft = l;
    if (r !== was.right) patch.indRight = r;
    const sp = special.value as Special;
    const amount = sp === "none" ? 0 : len(by, was.by);
    const first = sp === "first" ? amount : sp === "hanging" ? -amount : 0;
    const wasFirst = was.special === "first" ? was.by : was.special === "hanging" ? -was.by : 0;
    if (first !== wasFirst) patch.indFirst = first;
    if (mirror.input.checked !== was.mirror) patch.mirrorIndents = mirror.input.checked;
    const space = (input: HTMLInputElement, auto: HTMLInputElement, wasValue: number, wasAuto: boolean, key: "spaceBefore" | "spaceAfter", autoKey: "beforeAuto" | "afterAuto") => {
      if (auto.checked) {
        if (!wasAuto) patch[autoKey] = true;
        return;
      }
      const v = Math.max(0, Math.round((Number(input.value) || 0) * 20));
      if (wasAuto || v !== wasValue) {
        patch[key] = v;
        if (wasAuto) patch[autoKey] = false;
      }
    };
    space(before, beforeAuto.input, was.before, was.beforeAuto, "spaceBefore", "beforeAuto");
    space(after, afterAuto.input, was.after, was.afterAuto, "spaceAfter", "afterAuto");
    const kind = line.value as LineKind;
    const value = Number(at.value) || (kind === "atLeast" || kind === "exactly" ? 12 : 1);
    if (kind !== was.kind || (kind !== "single" && kind !== "1.5" && kind !== "double" && value !== was.at)) {
      if (kind === "atLeast" || kind === "exactly") {
        patch.line = Math.max(1, Math.round(value * 20));
        patch.lineRule = kind === "exactly" ? "exact" : "atLeast";
      } else patch.lineMultiple = kind === "single" ? 1 : kind === "1.5" ? 1.5 : kind === "double" ? 2 : value;
    }
    if (contextual.input.checked !== was.contextual) patch.contextualSpacing = contextual.input.checked;
    if (widow.input.checked !== was.widow) patch.widowControl = widow.input.checked;
    if (keepNext.input.checked !== was.keepNext) patch.keepNext = keepNext.input.checked;
    if (keepLines.input.checked !== was.keepLines) patch.keepLines = keepLines.input.checked;
    if (pageBreak.input.checked !== was.pageBreak) patch.pageBreakBefore = pageBreak.input.checked;
    if (noNumbers.input.checked !== was.noLineNumbers) patch.suppressLineNumbers = noNumbers.input.checked;
    if (noHyphen.input.checked !== was.noHyphen) patch.suppressAutoHyphens = noHyphen.input.checked;
    Object.assign(patch, asian.patch());
    const visual = align.value as VisualAlign;
    return { patch, align: visual !== was.align ? visual : undefined };
  };

  const apply = () => {
    const { patch, align: a } = collect();
    close();
    if (Object.keys(patch).length || a) target.setPara(patch, a);
  };
  ok.addEventListener("click", () => {
    apply();
    target.done();
  });
  cancel.addEventListener("click", close);
  tabs.addEventListener("click", () => {
    // What is set here is applied first, then the Tabs dialog opens.
    apply();
    openTabs(ed, target);
  });
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && (e.target as HTMLInputElement).type !== "checkbox") {
      e.preventDefault();
      ok.click();
    }
  });
}
