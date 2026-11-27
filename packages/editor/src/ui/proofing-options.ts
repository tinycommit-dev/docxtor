// Tools > Spelling and grammar > Proofing options: which words the spelling checker leaves alone, detecting the language,
// accents on French capitals, where corrections come from, which grammar rules run, readability statistics, hiding the
// marks in this document only, and checking the document again. The choices are Word's (File, Options, Proofing).
// The spelling and grammar groups are kept for the person; "In this document" is written to the file.
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import type { ProofingOptions, ProofingSettings } from "../spelling/options";
import { h, modal } from "./dom";

type Check = { key: keyof ProofingSettings; label: string; help: string };

const WORD_CHECKS: Check[] = [
  { key: "ignoreUppercase", label: msg("Ignore words in UPPERCASE"), help: msg("NASA, HTML and other words written in capitals.") },
  { key: "ignoreNumbers", label: msg("Ignore words that contain numbers"), help: msg("COVID19, file_name and the like.") },
  { key: "ignoreAddresses", label: msg("Ignore Internet and file addresses"), help: msg("Web addresses and e-mail addresses.") },
  { key: "detectLanguage", label: msg("Detect language automatically"), help: msg("A paragraph the file gives no language is checked in the language its words are in.") },
  { key: "frenchAccentedCapitals", label: msg("Enforce accented uppercase in French"), help: msg("ECOLE is a mistake for ÉCOLE. Off, French capitals may go without their accents.") },
  { key: "mainDictionaryOnly", label: msg("Suggest from main dictionary only"), help: msg("Corrections don't include the words in your custom dictionaries.") },
];

const GRAMMAR_CHECKS: Check[] = [
  { key: "repeatedWords", label: msg("Flag repeated words"), help: msg("The same word twice in a row, as in “the the”.") },
  { key: "confusedWords", label: msg("Frequently confused words"), help: msg("Their and there, its and it’s, then and than, and other pairs often mixed up, in English.") },
  { key: "showReadability", label: msg("Show readability statistics"), help: msg("When the spelling and grammar check is done: counts, averages and, for English, how easy the text is to read.") },
];

export function openProofingOptions(ed: Editor, options: ProofingOptions, recheck: () => void, dictionaries: () => void) {
  if (!ed.engine) return;
  const now = options.get();
  const hideSpelling = !!ed.pageInfo.hideSpellingErrors;
  const hideGrammar = !!ed.pageInfo.hideGrammaticalErrors;

  const row = (id: string, label: string, help: string, checked: boolean) => {
    const box = h("input", { type: "checkbox", id, checked });
    return { box, el: h("div", { class: "pref-check" }, h("label", { class: "check", for: id }, box, h("span", {}, label)), h("p", { class: "pref-help" }, help)) };
  };
  const words = WORD_CHECKS.map((c) => ({ key: c.key, ...row(`proof-${c.key}`, t(c.label), t(c.help), now[c.key]) }));
  const rules = GRAMMAR_CHECKS.map((c) => ({ key: c.key, ...row(`proof-${c.key}`, t(c.label), t(c.help), now[c.key]) }));
  const custom = h("button", { class: "btn", type: "button" }, t("Custom dictionaries…"));
  const spelling = row("proof-hide-spelling", t("Hide spelling errors in this document only"), t("The red marks go. Spelling and grammar check still finds the words. This is saved in the file."), hideSpelling);
  const grammar = row("proof-hide-grammar", t("Hide grammar errors in this document only"), t("The blue marks go. Spelling and grammar check still finds the problems. This is saved in the file."), hideGrammar);

  const again = h("button", { class: "btn", type: "button" }, t("Recheck document"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Proofing options"), style: "width:min(var(--dialog-m),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Proofing options")), h("p", { class: "sub" }, t("How spelling and grammar are checked."))),
    h(
      "div",
      { class: "dialog-body" },
      h("h3", { class: "pref-group" }, t("When checking spelling")),
      ...words.map((w) => w.el),
      h("div", { class: "pref-check" }, custom),
      h("h3", { class: "pref-group" }, t("When checking grammar")),
      ...rules.map((w) => w.el),
      h("h3", { class: "pref-group" }, t("In this document")),
      spelling.el,
      grammar.el,
      h("p", { class: "pref-help pref-flush" }, t("Words and rules you chose to ignore come back when you recheck the document.")),
    ),
    h("div", { class: "dialog-actions" }, again, h("span", { class: "spacer" }), cancel, apply),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  again.addEventListener("click", () => {
    close();
    recheck();
  });
  custom.addEventListener("click", () => {
    close();
    dictionaries();
  });
  apply.addEventListener("click", () => {
    close();
    options.set(Object.fromEntries([...words, ...rules].map((w) => [w.key, w.box.checked])) as Partial<ProofingSettings>);
    if (spelling.box.checked !== hideSpelling || grammar.box.checked !== hideGrammar) {
      ed.exec({ op: "set_layout_settings", hide_spelling_errors: spelling.box.checked, hide_grammatical_errors: grammar.box.checked });
      ed.reloadPageSetup();
    }
  });
  words[0].box.focus();
}
