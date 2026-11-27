// File > Language: the language of the document's text, which hyphenation, screen readers and (later)
// spelling follow. A searchable list of common languages, named in the reader's own interface language.
import { currentLocale, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal, onChoose, svg } from "./dom";
import { I } from "./icons";

/** BCP 47 tag written to the document, and the tag whose name the list shows (Chinese is named by its script). */
interface Choice {
  tag: string;
  named: string;
}

const COMMON: Choice[] = [
  { tag: "en-US", named: "en-US" },
  { tag: "en-GB", named: "en-GB" },
  { tag: "es-ES", named: "es-ES" },
  { tag: "es-MX", named: "es-MX" },
  { tag: "fr-FR", named: "fr-FR" },
  { tag: "de-DE", named: "de-DE" },
  { tag: "it-IT", named: "it-IT" },
  { tag: "pt-BR", named: "pt-BR" },
  { tag: "pt-PT", named: "pt-PT" },
  { tag: "nl-NL", named: "nl-NL" },
  { tag: "sv-SE", named: "sv-SE" },
  { tag: "da-DK", named: "da-DK" },
  { tag: "nb-NO", named: "nb-NO" },
  { tag: "fi-FI", named: "fi-FI" },
  { tag: "pl-PL", named: "pl-PL" },
  { tag: "cs-CZ", named: "cs-CZ" },
  { tag: "tr-TR", named: "tr-TR" },
  { tag: "el-GR", named: "el-GR" },
  { tag: "ru-RU", named: "ru-RU" },
  { tag: "uk-UA", named: "uk-UA" },
  { tag: "ar-SA", named: "ar-SA" },
  { tag: "he-IL", named: "he-IL" },
  { tag: "hi-IN", named: "hi-IN" },
  { tag: "ja-JP", named: "ja-JP" },
  { tag: "ko-KR", named: "ko-KR" },
  { tag: "zh-CN", named: "zh-Hans" },
  { tag: "zh-TW", named: "zh-Hant" },
];

/** A language's name in the interface language, e.g. "Spanish (Mexico)". Falls back to the tag. */
export function languageName(tag: string, named = tag): string {
  const locales = [currentLocale(), "en"].filter((l) => l && l !== "en-XA");
  try {
    const names = new Intl.DisplayNames(locales, { type: "language", languageDisplay: "standard" });
    return names.of(named) ?? tag;
  } catch {
    return tag;
  }
}

const same = (a: string | undefined, b: string | undefined) => !!a && !!b && a.toLowerCase() === b.toLowerCase();

/** The languages the dialog offers, named and sorted for the reader. The document's own, if it isn't one of them, comes first. */
export function languageChoices(current?: string): { tag: string; label: string }[] {
  const list = COMMON.map((c) => ({ tag: c.tag, label: languageName(c.tag, c.named) }));
  let sorted: { tag: string; label: string }[];
  try {
    const collator = new Intl.Collator(currentLocale() === "en-XA" ? "en" : currentLocale());
    sorted = list.sort((a, b) => collator.compare(a.label, b.label));
  } catch {
    sorted = list.sort((a, b) => a.label.localeCompare(b.label));
  }
  if (current && !sorted.some((c) => same(c.tag, current))) sorted.unshift({ tag: current, label: languageName(current) });
  return sorted;
}

export function openLanguage(ed: Editor) {
  if (!ed.engine) return;
  const current = ed.engine.pageSetup().lang;
  const choices = languageChoices(current);
  const ids = ed.collapsed() ? [] : ed.selectedParagraphs();
  let chosen = choices.find((c) => same(c.tag, current))?.tag ?? "";
  let visible = choices;

  const search = h("input", {
    type: "text",
    id: "lang-search",
    role: "combobox",
    "aria-controls": "lang-list",
    "aria-expanded": "true",
    "aria-autocomplete": "list",
    placeholder: t("Search languages"),
    "aria-label": t("Search languages"),
    spellcheck: "false",
    autocomplete: "off",
  });
  const list = h("div", { class: "lang-list", id: "lang-list", role: "listbox", "aria-label": t("Languages") });
  const empty = h("div", { class: "empty-note", hidden: true }, t("Nothing matches"));
  const only = h("input", { type: "checkbox", id: "lang-only" });
  const onlyRow = ids.length
    ? h("div", { class: "pref-check" }, h("label", { class: "check", for: "lang-only" }, only, h("span", {}, t("Apply to the selected text only"))), h("p", { class: "pref-help" }, t("The rest of the document keeps its language.")))
    : null;
  // "Do not check spelling or grammar" is a property of the text (`w:noProof`), for the selection or the text typed next.
  const wasNoProof = !!ed.format()?.run.noProof;
  const noProof = h("input", { type: "checkbox", id: "lang-noproof", checked: wasNoProof });
  const noProofRow = h(
    "div",
    { class: "pref-check" },
    h("label", { class: "check", for: "lang-noproof" }, noProof, h("span", {}, t("Do not check spelling or grammar"))),
    h("p", { class: "pref-help" }, ed.collapsed() ? t("Applies to the text you type next.") : t("Applies to the selected text. Code, names and quotations in other languages are often better left alone.")),
  );
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));

  const paint = () => {
    list.replaceChildren(
      ...visible.map((c) => {
        const row = h(
          "div",
          { class: `lang-item${c.tag === chosen ? " on" : ""}`, role: "option", id: `lang-${c.tag}`, "aria-selected": String(c.tag === chosen), "data-lang": c.tag },
          h("span", { class: "lang-mark" }, same(c.tag, current) ? svg(I.check) : null),
          h("span", { class: "lang-name" }, c.label),
          same(c.tag, current) ? h("span", { class: "lang-current" }, t("Current")) : null,
        );
        onChoose(
          row,
          () => {
            chosen = c.tag;
            paint();
          },
          () => apply.click(),
        );
        return row;
      }),
    );
    empty.hidden = visible.length > 0;
    // Applying is possible with a language chosen, or with only the spelling choice changed.
    apply.disabled = !chosen && noProof.checked === wasNoProof;
    if (chosen) search.setAttribute("aria-activedescendant", `lang-${chosen}`);
    else search.removeAttribute("aria-activedescendant");
    list.querySelector(".lang-item.on")?.scrollIntoView({ block: "nearest" });
  };
  const filter = () => {
    const words = search.value.toLowerCase().split(/\s+/).filter(Boolean);
    visible = choices.filter((c) => words.every((w) => c.label.toLowerCase().includes(w) || c.tag.toLowerCase().includes(w)));
    if (visible.length && !visible.some((c) => c.tag === chosen)) chosen = visible[0].tag;
    paint();
  };
  search.addEventListener("input", filter);
  search.addEventListener("keydown", (e) => {
    const i = visible.findIndex((c) => c.tag === chosen);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!visible.length) return;
      chosen = visible[(Math.max(0, i) + (e.key === "ArrowDown" ? 1 : -1) + visible.length) % visible.length].tag;
      paint();
    } else if (e.key === "Enter" && !e.isComposing) {
      e.preventDefault();
      apply.click();
    }
  });

  const dialog = h(
    "div",
    { class: "dialog lang-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Language") },
    h(
      "div",
      { class: "dialog-head" },
      h("h2", {}, t("Language")),
      h("p", { class: "sub" }, current ? t("The document's language is {language}. Hyphenation and screen readers follow it.", { language: languageName(current) }) : t("The document has no language set. Hyphenation and screen readers follow it.")),
    ),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, search, list, empty), onlyRow, noProofRow),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog, () => ed.focus());
  paint();
  search.focus();
  cancel.addEventListener("click", close);
  noProof.addEventListener("change", paint);
  apply.addEventListener("click", () => {
    if (apply.disabled) return;
    const selectionOnly = ids.length > 0 && only.checked;
    close();
    if (noProof.checked !== wasNoProof) ed.setRun({ noProof: noProof.checked });
    if (!chosen || (!selectionOnly && same(chosen, current))) return;
    ed.exec({ op: "set_language", lang: chosen, ...(selectionOnly ? { ids } : {}) });
    ed.reloadPageSetup();
  });
}
