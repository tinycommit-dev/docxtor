// Tools > Thesaurus: a small panel by the page that looks up the word at the caret (or the selected words) in every
// thesaurus that is on, lists the meanings with their synonyms and opposites, and replaces the word with the one
// you pick. The thesauruses are extensions (`ctx.thesaurus.register`); this panel only asks the host.
import { msg, t, type ThesaurusAnswer } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { matchCase } from "../thesaurus/english";
import { h, onChoose, svg, toast } from "./dom";
import { I } from "./icons";
import { languageName } from "./language";
import { languageAt, wordAtCaret, type WordTarget } from "./word-at";

export interface ThesaurusApi {
  /** Look a word up in a language. */
  lookup(word: string, lang: string): Promise<ThesaurusAnswer>;
  /** Whether any thesaurus that is on serves the language. */
  serves(lang: string): boolean;
}

const POS_LABEL: Record<string, string> = {
  noun: msg("noun"),
  verb: msg("verb"),
  adjective: msg("adjective"),
  adverb: msg("adverb"),
};

const sentence = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/**
 * The thesaurus panel, floating at the top right of the pages. It looks up the word at the caret first, and
 * returns functions to close it and to look up the word the caret is in now.
 */
export function openThesaurus(ed: Editor, api: ThesaurusApi, host: HTMLElement): { close: () => void; lookup: () => void } {
  host.querySelector(".th-panel")?.remove();
  let target: WordTarget | null = wordAtCaret(ed);
  let lang = target ? languageAt(ed, target.p) : (ed.pageInfo.lang || navigator.language || "en-US").replace(/_/g, "-");
  let chosen: string | null = null;
  let closed = false;
  let token = 0;
  /** Words looked up before this one, for Back. */
  const trail: string[] = [];
  let shown = "";

  const closeBtn = h("button", { class: "tb", type: "button", title: t("Close"), "aria-label": t("Close") });
  closeBtn.append(svg(I.close));
  const input = h("input", { type: "text", class: "sp-input", "aria-label": t("Word to look up"), placeholder: t("Word to look up"), spellcheck: "false", autocomplete: "off" });
  const go = h("button", { class: "btn", type: "submit" }, t("Look up"));
  const form = h("form", { class: "sp-add", role: "search" }, input, go);
  const status = h("p", { class: "sp-label", role: "status", "aria-live": "polite" });
  const results = h("div", { class: "th-results", role: "listbox", "aria-label": t("Synonyms") });
  const replace = h("button", { class: "btn primary", type: "button" }, t("Replace"));
  const lookUp = h("button", { class: "btn", type: "button" }, t("Look up"));
  const back = h("button", { class: "btn ghost", type: "button" }, t("Back"));
  const panel = h(
    "div",
    { class: "sp-panel th-panel", role: "dialog", "aria-label": t("Thesaurus") },
    h("div", { class: "sp-head" }, h("h2", {}, t("Thesaurus")), closeBtn),
    form,
    status,
    results,
    h("div", { class: "sp-actions" }, replace, lookUp, back),
  );
  // Clicking the panel must not take the caret or the selection out of the text.
  panel.addEventListener("mousedown", (e) => {
    if (!(e.target as HTMLElement).closest("input")) e.preventDefault();
  });
  host.append(panel);

  const close = () => {
    if (closed) return;
    closed = true;
    token++;
    host.removeEventListener("keydown", onKey, true);
    panel.remove();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    close();
    ed.focus();
  };
  host.addEventListener("keydown", onKey, true);
  closeBtn.addEventListener("click", () => {
    close();
    ed.focus();
  });

  const options = () => Array.from(results.querySelectorAll<HTMLButtonElement>(".th-word"));
  const paint = () => {
    for (const o of options()) {
      const on = o.dataset.word === chosen;
      o.classList.toggle("selected", on);
      o.setAttribute("aria-selected", String(on));
    }
    const canReplace = !!target && !!chosen && !ed.readOnly;
    replace.disabled = !canReplace;
    lookUp.disabled = !chosen;
    back.disabled = !trail.length;
  };
  const word = (text: string, inflected?: string) => {
    const b = h("button", { class: "sp-sug th-word", type: "button", role: "option", "data-word": text, "aria-selected": "false" }, text);
    onChoose(
      b,
      () => {
        chosen = text;
        paint();
      },
      () => doReplace(text),
    );
    b.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        if (e.shiftKey) void show(text, true);
        else doReplace(text);
      } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const all = options();
        all[(all.indexOf(b) + (e.key === "ArrowDown" ? 1 : -1) + all.length) % all.length]?.focus();
      }
    });
    if (inflected) b.title = inflected;
    return b;
  };

  const render = (answer: ThesaurusAnswer) => {
    const sources = answer.results.length;
    const parts: HTMLElement[] = [];
    for (const r of answer.results) {
      if (sources > 1) parts.push(h("h3", { class: "th-source" }, r.label));
      for (const s of r.senses) {
        const pos = s.pos ? (POS_LABEL[s.pos] ? t(POS_LABEL[s.pos]) : s.pos) : "";
        // The base word of an inflected form (stop for stopping), named when it isn't the word that was asked about.
        const base = s.word ?? (r.word.toLowerCase() !== answer.word.toLowerCase() ? r.word : "");
        const caption = base ? t("{word} ({kind})", { word: `“${base}”`, kind: pos || t("word") }) : pos ? sentence(pos) : t("Synonyms");
        parts.push(
          h(
            "div",
            { class: "th-sense", role: "group", "aria-label": s.meaning ? `${caption}: ${s.meaning}` : caption },
            h("p", { class: "th-caption" }, s.meaning ? t("{caption}, like “{meaning}”", { caption, meaning: s.meaning }) : caption),
            h("div", { class: "th-list" }, ...s.synonyms.map((w) => word(w))),
            s.antonyms?.length ? h("p", { class: "th-caption th-opposites" }, t("Opposites")) : null,
            s.antonyms?.length ? h("div", { class: "th-list" }, ...s.antonyms.map((w) => word(w))) : null,
          ),
        );
      }
    }
    results.replaceChildren(...parts);
  };

  /** Look a word up and show what the thesauruses say. `remember` keeps the word shown before it for Back. */
  async function show(text: string, remember = false) {
    const asked = text.trim();
    if (!asked) return;
    const mine = ++token;
    if (remember && shown && shown !== asked) trail.push(shown);
    input.value = asked;
    chosen = null;
    results.replaceChildren();
    if (!api.serves(lang)) {
      shown = asked;
      status.textContent = t("No thesaurus is on for {language}. Turn one on under Extensions, Manage extensions.", { language: languageName(lang) });
      paint();
      return;
    }
    status.textContent = t("Looking up “{word}”…", { word: asked });
    paint();
    const answer = await api.lookup(asked, lang);
    if (mine !== token || closed) return;
    shown = asked;
    if (!answer.results.length) {
      status.textContent = t("No synonyms found for “{word}”.", { word: asked });
    } else {
      status.textContent = t("Meanings of “{word}”", { word: asked });
      render(answer);
      chosen = null;
    }
    paint();
  }

  /** Put a synonym in place of the word: it takes the capitals the word had. */
  function doReplace(text: string | null = chosen) {
    if (!text || ed.readOnly) return;
    // The word may have moved since the panel opened: look at where the caret is now when this one is gone.
    if (!target || !ed.engine || ed.engine.textRange({ p: target.p, o: target.from }, { p: target.p, o: target.to }) !== target.text) {
      target = wordAtCaret(ed);
      if (!target) {
        toast(t("Select the word to replace first."));
        return;
      }
    }
    const next = matchCase(target.text, text);
    const at = target;
    if (!ed.replaceText({ p: at.p, o: at.from }, { p: at.p, o: at.to }, next)) return;
    target = { p: at.p, from: at.from, to: at.from + next.length, text: next };
    ed.select({ anchor: { p: at.p, o: at.from }, focus: { p: at.p, o: at.from + next.length } });
    status.textContent = t("Replaced with “{word}”.", { word: next });
    paint();
  }

  form.addEventListener("submit", (e) => {
    e.preventDefault();
    // Looking up a word that is not the one in the text: the text is left alone until a synonym is picked.
    void show(input.value, true);
  });
  replace.addEventListener("click", () => doReplace());
  lookUp.addEventListener("click", () => chosen && void show(chosen, true));
  back.addEventListener("click", () => {
    const prev = trail.pop();
    if (prev) void show(prev);
  });

  paint();
  if (target) void show(target.text);
  else {
    status.textContent = t("Select a word in the document, or type one above.");
    input.focus();
  }
  return {
    close,
    // The command again, with the caret on another word: look that one up.
    lookup: () => {
      target = wordAtCaret(ed);
      if (target) {
        lang = languageAt(ed, target.p);
        void show(target.text, true);
      } else input.focus();
    },
  };
}
