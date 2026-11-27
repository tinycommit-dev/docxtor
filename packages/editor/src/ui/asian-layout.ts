// The Asian layout boxes: Enclose characters, Two lines in one, and the document's line-breaking rules. The Phonetic guide
// box is in phonetic-guide.ts. Each writes through ops, so it is one step to undo.
import type { AsianSettings, LineBreakChars } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { combWidth } from "../editor/east-asian";
import type { Editor } from "../editor/editor";
import { h, modal, toast } from "./dom";

// ---------------------------------------------------------------------------
// Enclose characters
// ---------------------------------------------------------------------------

const ENCLOSURES: [string, string, string][] = [
  ["circle", msg("Circle"), "○"],
  ["square", msg("Square"), "□"],
  ["triangle", msg("Triangle"), "△"],
  ["diamond", msg("Diamond"), "◇"],
];

/** The enclosure at the caret, read from the `EQ \o\ac(shape,text)` field drawn there. */
function enclosureAt(ed: Editor): { at: { p: number; o: number }; shape: string; text: string } | null {
  const r = ed.range();
  if (!r || r[0].p !== r[1].p || r[0].o !== r[1].o) return null;
  const para = ed.paraElement(r[0].p);
  const fi = para?.querySelector<HTMLElement>(`.ghost.eq[data-o="${r[0].o}"]`)?.dataset.fi;
  const m = fi && /\\o\\ac\((.),((?:\\.|[^)\\])*)\)/.exec(fi);
  if (!m) return null;
  const shape = ENCLOSURES.find(([, , glyph]) => glyph === m[1])?.[0];
  return shape ? { at: r[0], shape, text: m[2].replace(/\\(.)/g, "$1") } : null;
}

/** Enclose one or two selected characters in a circle, square, triangle or diamond, or change or remove the enclosure at the caret. */
export function openEncloseCharacters(ed: Editor): boolean {
  const existing = enclosureAt(ed);
  const r = ed.range();
  let text = existing?.text ?? "";
  if (!existing) {
    if (!r || r[0].p !== r[1].p || r[0].o === r[1].o) return false;
    text = ed.engine.textRange(r[0], r[1]);
  }
  const count = [...text].length;
  if (count < 1 || count > 2) {
    toast(t("Select one or two characters to enclose."));
    return false;
  }
  let shape = existing?.shape ?? "circle";
  const glyphs = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Enclosure") });
  const preview = h("div", { class: "en-preview", "aria-hidden": "true" });
  const draw = () => {
    const glyph = ENCLOSURES.find(([id]) => id === shape)![2];
    preview.replaceChildren(h("span", { class: "en-shape" }, glyph), h("span", { class: "en-text" }, text));
    for (const b of Array.from(glyphs.children) as HTMLElement[]) b.classList.toggle("on", b.dataset.shape === shape);
  };
  for (const [id, label] of ENCLOSURES) {
    const b = h("button", { type: "button", role: "radio", "data-shape": id }, t(label));
    b.addEventListener("click", () => {
      shape = id;
      draw();
    });
    glyphs.append(b);
  }
  draw();
  const remove = h("button", { class: "btn ghost", type: "button" }, t("Remove enclosure"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Enclose characters") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Enclose characters"))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", {}, t("Enclosure")), glyphs), h("div", { class: "field" }, h("label", {}, t("Preview")), preview)),
    h("div", { class: "dialog-actions" }, existing ? remove : null, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  remove.addEventListener("click", () => {
    close();
    if (existing) ed.exec({ op: "east_asian", action: "clear_enclose", at: existing.at });
  });
  ok.addEventListener("click", () => {
    close();
    if (existing) {
      // The characters stay; the shape changes: take the old field away and enclose them again.
      if (shape === existing.shape) return;
      // One batch, so Undo gives the old shape back in one step.
      ed.exec([
        { op: "east_asian", action: "clear_enclose", at: existing.at },
        { op: "east_asian", action: "enclose", from: existing.at, to: { p: existing.at.p, o: existing.at.o + text.length }, symbol: shape },
      ]);
    } else if (r) {
      ed.exec({ op: "east_asian", action: "enclose", from: r[0], to: r[1], symbol: shape });
    }
  });
  return true;
}

// ---------------------------------------------------------------------------
// Two lines in one
// ---------------------------------------------------------------------------

const BRACKETS: [string, string][] = [
  ["none", msg("None")],
  ["round", msg("Round ( )")],
  ["square", msg("Square [ ]")],
  ["angle", msg("Angle < >")],
  ["curly", msg("Curly { }")],
];

/** Set the selected text in two half-height lines inside one character box, with brackets around it if wanted. */
export function openTwoLinesInOne(ed: Editor): boolean {
  const r = ed.range();
  if (!r || r[0].p !== r[1].p || r[0].o === r[1].o) return false;
  const text = ed.engine.textRange(r[0], r[1]);
  const layout = ed.format()?.run.eaLayout;
  const on = !!layout?.combine;
  const brackets = h("select", { id: "tl-brackets" }, ...BRACKETS.map(([v, label]) => h("option", { value: v, selected: v === (layout?.combineBrackets ?? "none") }, t(label))));
  const preview = h("div", { class: "tl-preview", "aria-hidden": "true" });
  const draw = () => {
    const b = brackets.value;
    const pair: Record<string, string> = { round: "()", square: "[]", angle: "<>", curly: "{}" };
    const mid = h("span", { class: "ea-comb", style: `font-size:18px;width:${combWidth(text)}` }, text);
    preview.replaceChildren(...(pair[b] ? [h("span", { class: "tl-br" }, pair[b][0]), mid, h("span", { class: "tl-br" }, pair[b][1])] : [mid]));
  };
  brackets.addEventListener("change", draw);
  draw();
  const remove = h("button", { class: "btn ghost", type: "button" }, t("Remove"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Two lines in one") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Two lines in one"))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "tl-brackets" }, t("Brackets")), brackets), h("div", { class: "field" }, h("label", {}, t("Preview")), preview)),
    h("div", { class: "dialog-actions" }, on ? remove : null, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  const set = (format: Record<string, unknown>) => {
    close();
    ed.exec({ op: "format_text", from: r[0], to: r[1], format });
  };
  remove.addEventListener("click", () => set({ eaLayout: { combine: false } }));
  ok.addEventListener("click", () => set({ eaLayout: { combine: true, combineBrackets: brackets.value } }));
  return true;
}

/** Turn text upright in a vertical line (horizontal in vertical, fitted into the line) on or off for the selection. */
export function toggleHorizontalInVertical(ed: Editor) {
  const r = ed.range();
  if (!r || r[0].p !== r[1].p || r[0].o === r[1].o) return;
  const on = !ed.format()?.run.eaLayout?.vert;
  ed.exec({ op: "format_text", from: r[0], to: r[1], format: { eaLayout: { vert: on, vertCompress: on } } });
}

// ---------------------------------------------------------------------------
// Line-breaking rules
// ---------------------------------------------------------------------------

const LANGS: [LineBreakChars["lang"], string][] = [
  ["ja-JP", msg("Japanese")],
  ["ko-KR", msg("Korean")],
  ["zh-CN", msg("Chinese (simplified)")],
  ["zh-TW", msg("Chinese (traditional)")],
];

/** The characters Word's strict rules keep off the start and the end of a line, which the Use the standard lists button fills in. */
const STANDARD_BEFORE = "!%),.:;?]}¢°’”‰′″℃、。々〉》」』】〕゛゜ゝゞ・ヽヾ！％），．：；？］｝｡｣､･ﾞﾟ";
const STANDARD_AFTER = "$([{£¥‘“〈《「『【〔＄（［｛｢￡￥";

/**
 * The rules for breaking East Asian lines (Format, Asian layout, Line breaking rules…): the standard or the strict level,
 * and lists of characters that may not start or end a line, one for each language. Paragraphs follow them unless they turn
 * the East Asian rules off in the Paragraph box.
 */
export function openLineBreakRules(ed: Editor) {
  const was: AsianSettings = ed.engine.asianSettings();
  const lists = new Map<string, LineBreakChars>(LANGS.map(([lang]) => [lang, { lang, before: "", after: "" }]));
  for (const c of was.custom ?? []) lists.set(c.lang, { ...c });
  const level = h("select", { id: "lb-level" }, h("option", { value: "normal", selected: !was.strict }, t("Standard")), h("option", { value: "strict", selected: !!was.strict }, t("Strict")));
  const lang = h("select", { id: "lb-lang" }, ...LANGS.map(([v, label]) => h("option", { value: v }, t(label))));
  const before = h("input", { id: "lb-before", type: "text", autocomplete: "off" });
  const after = h("input", { id: "lb-after", type: "text", autocomplete: "off" });
  let shown = LANGS[0][0] as string;
  const store = () => {
    const l = lists.get(shown)!;
    l.before = (before as HTMLInputElement).value;
    l.after = (after as HTMLInputElement).value;
  };
  const load = () => {
    shown = (lang as HTMLSelectElement).value;
    const l = lists.get(shown)!;
    (before as HTMLInputElement).value = l.before;
    (after as HTMLInputElement).value = l.after;
  };
  lang.addEventListener("change", () => {
    store();
    load();
  });
  load();
  const standard = h("button", { class: "btn ghost", type: "button" }, t("Use the standard lists"));
  standard.addEventListener("click", () => {
    (before as HTMLInputElement).value = STANDARD_BEFORE;
    (after as HTMLInputElement).value = STANDARD_AFTER;
  });
  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Line breaking rules") },
    h(
      "div",
      { class: "dialog-head" },
      h("h2", {}, t("Line breaking rules")),
      h("p", { class: "sub" }, t("Characters in the lists stay with the character beside them when a line breaks. Leave a list empty to use the standard rules.")),
    ),
    h(
      "div",
      { class: "dialog-body" },
      field(t("Rules"), level, "lb-level"),
      field(t("Language"), lang, "lb-lang"),
      field(t("Characters that can't start a line"), before, "lb-before"),
      field(t("Characters that can't end a line"), after, "lb-after"),
      standard,
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    store();
    const custom = [...lists.values()].filter((l) => l.before || l.after);
    close();
    const strict = (level as HTMLSelectElement).value === "strict";
    ed.exec({ op: "east_asian", action: "set_settings", settings: { strict, custom } });
  });
}
