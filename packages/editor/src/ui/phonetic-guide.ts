// The Phonetic guide box (Format, Asian layout, Phonetic guide…): small text over East Asian text, such as the reading
// of a kanji. With text selected it puts a guide over it; double-click a guide (or Enter on a selected one) opens it
// for editing, and Remove guide takes it away and leaves the base text. A preview draws the guide the way the page will.
import type { RRuby, RSeg } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { rubyAtom } from "../editor/east-asian";
import type { Editor, RubyAtCaret } from "../editor/editor";
import { fontStack, runStyle } from "../editor/render";
import { EAST_ASIAN_FONTS } from "./asian-typography";
import { h, modal } from "./dom";

const ALIGNS: [RRuby["align"], string][] = [
  ["center", msg("Centered")],
  ["left", msg("Left")],
  ["right", msg("Right")],
  ["distributeSpace", msg("Spread with space at the ends")],
  ["distributeLetter", msg("Spread to the edges")],
];

/** Word's usual size for a guide: half the base text. */
const half = (base: number) => Math.max(1, Math.round(base * 5) / 10);

/**
 * Open the box. With `existing` it edits that guide; otherwise the selected text becomes the base of a new one (nothing
 * happens when no text is selected).
 */
export function openPhoneticGuide(ed: Editor, existing?: RubyAtCaret | null, anchor?: HTMLElement | null, suggest?: (text: string, lang: string) => Promise<{ text: string; source: string } | null>) {
  const range = ed.range();
  const at = existing?.at;
  let from = range?.[0];
  let to = range?.[1];
  if (existing) {
    from = existing.at;
    to = { p: existing.at.p, o: existing.at.o + 1 };
  }
  if (!from || !to || from.p !== to.p || from.o === to.o) return false;
  const base = existing ? existing.ruby.base : ed.engine.textRange(from, to);
  if (!base) return false;
  const was = existing?.ruby;
  const baseSize = was?.baseSize ?? ed.format()?.run.size ?? 11;

  const check = (label: string, id: string, input: HTMLElement) => h("div", { class: "field" }, h("label", { for: id }, label), input);
  const baseBox = h("input", { id: "pg-base", type: "text", value: base, readonly: true });
  const text = h("input", { id: "pg-text", type: "text", value: was?.text ?? "", autocomplete: "off", lang: was?.lang ?? "ja" });
  const align = h("select", { id: "pg-align" }, ...ALIGNS.map(([v, label]) => h("option", { value: v, selected: v === (was?.align ?? "center") }, t(label))));
  const raise = h("input", { id: "pg-raise", type: "number", min: "0", max: "1638", step: "0.5", value: String(was?.raise ?? Math.round(baseSize * 0.85 * 2) / 2) });
  const size = h("input", { id: "pg-size", type: "number", min: "1", max: "1638", step: "0.5", value: String(was?.size ?? half(baseSize)) });
  const fonts = [...new Set([...(was?.font ? [was.font] : []), ...EAST_ASIAN_FONTS])];
  const font = h("select", { id: "pg-font" }, h("option", { value: "", selected: !was?.font }, t("Same as the text")), ...fonts.map((f) => h("option", { value: f, selected: f === was?.font }, f)));

  const hint = h("p", { class: "sub pg-hint", role: "status" });
  const suggestBtn = h("button", { class: "btn", type: "button" }, t("Suggest"));
  /** Ask the reading providers what the base text says; fill the guide in when it is empty or `force` is set. */
  const ask = async (force: boolean) => {
    if (!suggest) return;
    hint.textContent = "";
    const got = await suggest(base, was?.lang ?? "ja").catch(() => null);
    if (!got) {
      if (force) hint.textContent = t("No reading found for this text.");
      return;
    }
    if (force || !(text as HTMLInputElement).value) {
      (text as HTMLInputElement).value = got.text;
      draw();
    }
    hint.textContent = t("Suggested by {source}.", { source: got.source });
  };
  suggestBtn.addEventListener("click", () => void ask(true));
  const preview = h("div", { class: "pg-preview", "aria-hidden": "true" });
  const draw = () => {
    const sz = Math.max(1, Number(size.value) || half(baseSize));
    const seg: RSeg = {
      o: 0,
      k: "ruby",
      f: { size: Math.min(baseSize, 28), fontEa: font.value || undefined, eaText: true },
      ruby: { base, text: text.value, align: align.value as RRuby["align"], size: sz, baseSize, rtF: { size: Math.min(sz, 14), fontEa: font.value || undefined, eaText: true }, lang: was?.lang },
    };
    const el = rubyAtom(seg, fontStack, runStyle);
    el.setAttribute("style", runStyle(seg.f));
    preview.replaceChildren(el);
  };

  const remove = h("button", { class: "btn ghost", type: "button" }, t("Remove guide"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog pg-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Phonetic guide") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Phonetic guide"))),
    h(
      "div",
      { class: "dialog-body" },
      check(t("Base text"), "pg-base", baseBox),
      check(t("Guide text"), "pg-text", h("div", { class: "pg-row" }, text, suggest ? suggestBtn : "")),
      suggest ? hint : "",
      h("div", { class: "pd-grid" }, check(t("Alignment"), "pg-align", align), check(t("Font"), "pg-font", font), check(t("Offset (pt)"), "pg-raise", raise), check(t("Size (pt)"), "pg-size", size)),
      h("div", { class: "field" }, h("label", {}, t("Preview")), preview),
    ),
    h("div", { class: "dialog-actions" }, existing ? remove : null, cancel, ok),
  );
  dialog.addEventListener("input", draw);
  dialog.addEventListener("change", draw);
  draw();
  const close = modal(dialog, () => ed.focus());
  (text as HTMLInputElement).focus();
  void anchor;
  if (!was) void ask(false);

  cancel.addEventListener("click", close);
  remove.addEventListener("click", () => {
    close();
    if (at) ed.exec({ op: "east_asian", action: "clear_ruby", at });
  });
  ok.addEventListener("click", () => {
    const guide = (text as HTMLInputElement).value.trim();
    if (!guide) {
      (text as HTMLInputElement).focus();
      return;
    }
    const num = (el: HTMLInputElement, fallback: number) => (Number.isFinite(parseFloat(el.value)) ? parseFloat(el.value) : fallback);
    close();
    ed.exec(
      {
        op: "east_asian",
        action: "set_ruby",
        from,
        to,
        text: guide,
        align: align.value,
        size: Math.max(0.5, num(size as HTMLInputElement, half(baseSize))),
        raise: Math.max(0, num(raise as HTMLInputElement, baseSize * 0.85)),
        baseSize,
        font: (font as HTMLSelectElement).value,
        ...(was?.lang ? {} : { lang: "ja-JP" }),
      },
      { select: { anchor: { p: from.p, o: Math.min(from.o, to.o) }, focus: { p: from.p, o: Math.min(from.o, to.o) + 1 } } },
    );
  });
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT") {
      e.preventDefault();
      ok.click();
    }
  });
  return true;
}
