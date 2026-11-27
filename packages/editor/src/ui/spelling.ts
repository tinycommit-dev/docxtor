// Tools > Spelling and grammar: the floating check panel that steps through the misspelled words and the
// grammar and style problems together (it talks to the spelling extension through `SpellingApi`), the Custom
// dictionaries dialog, and Readability statistics.
import type { MPos } from "@docxtor/core";
import { currentLocale, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import type { SpellHit } from "../editor/spell-layer";
import type { CustomDictionaries } from "../spelling/dictionaries";
import type { Readability } from "../spelling/readability";
import { h, modal, onChoose, svg, toast } from "./dom";
import { I } from "./icons";
import { languageChoices } from "./language";

export interface SpellingApi {
  /** Every misspelled word and every grammar or style problem in the text, in reading order. */
  errors(): Promise<SpellHit[]>;
  /** Up to five corrections for a word, or the checker's replacements for a grammar problem. */
  suggestions(hit: SpellHit): Promise<string[]>;
  /** Replace the words of a problem; false when the text there is no longer those words. */
  change(hit: SpellHit, text: string): boolean;
  /** Leave the word (or the grammar rule on those words) alone in this document. */
  ignore(hit: SpellHit): void;
  /** Accept the word in every document (it goes in the default custom dictionary). */
  add(word: string): void;
  /** The check went through to the end. */
  finished?(): void;
}

/** Whether position a comes before b in reading order (`order` lists the paragraph ids as they appear). */
const before = (order: Map<number, number>, a: MPos, b: MPos) => (order.get(a.p) ?? 0) < (order.get(b.p) ?? 0) || (a.p === b.p && a.o < b.o);

/**
 * The check panel, floating at the top right of the pages. It selects each misspelled word and each grammar
 * or style problem in the text, from the caret on, and offers its corrections. Returns a function that
 * closes it.
 */
export function openSpellPanel(ed: Editor, api: SpellingApi, host: HTMLElement): () => void {
  host.querySelector(".sp-panel")?.remove();
  let list: SpellHit[] = [];
  let index = 0;
  let suggestions: string[] = [];
  let chosen = 0;
  let closed = false;
  // Every look at the document may find the words moved: a token says which answer is the latest.
  let token = 0;

  const count = h("span", { class: "sp-count" });
  const closeBtn = h("button", { class: "tb", type: "button", title: t("Close"), "aria-label": t("Close") });
  closeBtn.append(svg(I.close));
  const label = h("p", { class: "sp-label" }, t("Not in dictionary"));
  const word = h("div", { class: "sp-word" });
  const message = h("p", { class: "sp-message", hidden: true });
  const sugs = h("div", { class: "sp-sugs", role: "listbox", "aria-label": t("Suggestions") });
  const change = h("button", { class: "btn primary", type: "button" }, t("Change"));
  const ignore = h("button", { class: "btn", type: "button" }, t("Ignore"));
  const add = h("button", { class: "btn", type: "button" }, t("Add to dictionary"));
  const prev = h("button", { class: "btn ghost", type: "button" }, t("Previous"));
  const next = h("button", { class: "btn ghost", type: "button" }, t("Next"));
  const panel = h(
    "div",
    { class: "sp-panel", role: "dialog", "aria-label": t("Spelling and grammar check") },
    h("div", { class: "sp-head" }, h("h2", {}, t("Spelling and grammar check")), count, closeBtn),
    h("div", { class: "sp-body" }, label, word, message, sugs),
    h("div", { class: "sp-actions" }, change, ignore, add),
    h("div", { class: "sp-nav" }, prev, next),
  );
  // Clicking the panel must not take the caret or the selection out of the text.
  panel.addEventListener("mousedown", (e) => {
    if (!(e.target as HTMLElement).closest("input")) e.preventDefault();
  });
  host.append(panel);

  // Escape closes the panel wherever the focus is (it stays in the text while the panel is used with the mouse).
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    close();
    ed.focus();
  };
  host.addEventListener("keydown", onKey, true);
  const close = () => {
    if (closed) return;
    closed = true;
    token++;
    host.removeEventListener("keydown", onKey, true);
    panel.remove();
  };
  const finish = () => {
    close();
    toast(t("No more spelling errors"));
    ed.focus();
    api.finished?.();
  };
  closeBtn.addEventListener("click", () => {
    close();
    ed.focus();
  });

  const paint = () => {
    const hit = list[index];
    count.textContent = list.length ? t("{part} of {whole}", { part: String(index + 1), whole: String(list.length) }) : "";
    word.textContent = hit?.word ?? "";
    // A grammar or style problem says what is wrong; a misspelled word can be added to the dictionary.
    const spelling = !hit?.kind || hit.kind === "spelling";
    label.textContent = spelling ? t("Not in dictionary") : hit.kind === "style" ? t("Style suggestion") : t("Grammar");
    message.textContent = spelling ? "" : (hit?.message ?? "");
    message.hidden = spelling || !hit?.message;
    add.hidden = !spelling;
    sugs.replaceChildren(
      ...(suggestions.length
        ? suggestions.map((s, i) => {
            const b = h("button", { class: i === chosen ? "sp-sug selected" : "sp-sug", type: "button", role: "option", "aria-selected": String(i === chosen) }, s);
            onChoose(
              b,
              () => {
                chosen = i;
                paint();
              },
              () => void doChange(),
            );
            return b;
          })
        : [h("p", { class: "sp-none" }, t("No suggestions"))]),
    );
    change.disabled = !suggestions.length || ed.readOnly;
    prev.disabled = index <= 0;
    next.disabled = !list.length;
  };

  /** Show the error at `i`: select it in the text and fetch its corrections. */
  const show = async (i: number) => {
    const mine = ++token;
    index = i;
    const hit = list[i];
    suggestions = [];
    chosen = 0;
    paint();
    ed.select({ anchor: { p: hit.p!, o: hit.from }, focus: { p: hit.p!, o: hit.to } });
    const el = ed.paraElement(hit.p!);
    if (el) {
      ed.ensureDrawn(el);
      const c = ed.canvas.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      if (r.top < c.top + 60 || r.bottom > c.bottom - 60) el.scrollIntoView({ block: "center" });
    }
    const found = await api.suggestions(hit);
    if (mine !== token || closed) return;
    suggestions = found;
    paint();
  };

  /** Look at the document again and show the first error at or after `from` (the first of all without it). */
  const reload = async (from?: MPos, wrap = false) => {
    const mine = ++token;
    list = await api.errors();
    if (mine !== token || closed) return;
    if (!list.length) return finish();
    const order = new Map(Array.from(ed.root.querySelectorAll<HTMLElement>("[data-p]"), (e, i) => [Number(e.dataset.p), i] as const));
    const at = from ? list.findIndex((e) => !before(order, { p: e.p!, o: e.from }, from)) : 0;
    // Past the last one: nothing is left after the caret (a first look starts again from the top instead).
    if (at < 0 && !wrap) return finish();
    void show(Math.max(0, at));
  };

  async function doChange() {
    const hit = list[index];
    const text = suggestions[chosen];
    if (!hit || !text || ed.readOnly) return;
    if (api.change(hit, text)) await reload({ p: hit.p!, o: hit.from + text.length });
    else await reload();
  }

  change.addEventListener("click", () => void doChange());
  ignore.addEventListener("click", () => {
    const hit = list[index];
    if (!hit) return;
    api.ignore(hit);
    void reload({ p: hit.p!, o: hit.from });
  });
  add.addEventListener("click", () => {
    const hit = list[index];
    if (!hit) return;
    api.add(hit.word);
    void reload({ p: hit.p!, o: hit.from });
  });
  prev.addEventListener("click", () => index > 0 && void show(index - 1));
  next.addEventListener("click", () => (index + 1 < list.length ? void show(index + 1) : finish()));

  // From the caret on (the start of the selection when text is selected).
  const r = ed.range();
  void api.errors().then((all) => {
    if (closed) return;
    if (!all.length) {
      close();
      toast(t("No spelling errors"));
      api.finished?.();
      return;
    }
    return reload(r?.[0], true);
  });
  return close;
}

/**
 * Tools > Spelling and grammar > Custom dictionaries: the lists of words the checker accepts. Each can be switched
 * off, be for one language or all, and one is the default that Add to dictionary fills. Below the list, the words of
 * the one chosen, to add to or take out of.
 */
export function openCustomDictionaries(ed: Editor, dicts: CustomDictionaries) {
  let chosen = dicts.defaultId;
  const list = h("div", { class: "dict-list", role: "group", "aria-label": t("Dictionaries") });
  const newBtn = h("button", { class: "btn", type: "button" }, t("New dictionary"));
  const defaultBtn = h("button", { class: "btn", type: "button" }, t("Make default"));
  const removeBtn = h("button", { class: "btn ghost", type: "button" }, t("Remove dictionary"));
  const name = h("input", { type: "text", class: "sp-input", id: "dict-name", autocomplete: "off", spellcheck: "false" });
  const lang = h("select", { id: "dict-lang" });
  lang.append(h("option", { value: "" }, t("All languages")));
  for (const c of languageChoices()) lang.append(h("option", { value: c.tag }, c.label));
  const input = h("input", { type: "text", class: "sp-input", placeholder: t("Add a word"), "aria-label": t("Add a word"), spellcheck: "false", autocomplete: "off" });
  const addBtn = h("button", { class: "btn primary", type: "button", disabled: true }, t("Add"));
  const words = h("div", { class: "sp-words" });
  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Custom dictionaries"), style: "width:min(var(--dialog-m),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Custom dictionaries")), h("p", { class: "sub" }, t("Words in these lists are spelled right in every document. Add to dictionary puts a word in the default one."))),
    h(
      "div",
      { class: "dialog-body" },
      list,
      h("div", { class: "dict-actions" }, newBtn, defaultBtn, h("span", { class: "spacer" }), removeBtn),
      h("div", { class: "dict-fields" }, h("label", { for: "dict-name" }, t("Name")), name, h("label", { for: "dict-lang" }, t("Language")), lang),
      h("div", { class: "sp-add" }, input, addBtn),
      words,
    ),
    h("div", { class: "dialog-actions" }, done),
  );
  const close = modal(dialog, () => ed.focus());
  done.addEventListener("click", close);

  const langLabel = (tag: string) => (tag ? (languageChoices().find((c) => c.tag.toLowerCase() === tag.toLowerCase())?.label ?? tag) : t("All languages"));
  const render = () => {
    const all = dicts.all();
    if (!dicts.find(chosen)) chosen = dicts.defaultId;
    list.replaceChildren(
      ...all.map((d) => {
        const use = h("input", { type: "checkbox", checked: d.on, "aria-label": t("Use {name}", { name: d.name }) });
        use.addEventListener("change", () => {
          dicts.update(d.id, { on: use.checked });
          render();
        });
        // The checkbox turns a dictionary on; the button beside it chooses which one the fields below edit. Two controls, side by side, not one inside the other.
        const pickBtn = h(
          "button",
          { class: "dict-pick", type: "button", "aria-current": String(d.id === chosen), "data-id": d.id },
          h("span", { class: "dict-name" }, d.name),
          h("span", { class: "dict-lang" }, langLabel(d.lang)),
          d.id === dicts.defaultId ? h("span", { class: "dict-default" }, t("Default")) : "",
        );
        const row = h("div", { class: `dict-row${d.id === chosen ? " on" : ""}` }, use, pickBtn);
        const pick = () => {
          chosen = d.id;
          render();
          list.querySelector<HTMLElement>(`.dict-pick[data-id="${CSS.escape(d.id)}"]`)?.focus();
        };
        pickBtn.addEventListener("click", pick);
        return row;
      }),
    );
    const d = dicts.find(chosen)!;
    if (document.activeElement !== name) name.value = d.name;
    lang.value = languageChoices().some((c) => c.tag === d.lang) ? d.lang : "";
    defaultBtn.disabled = d.id === dicts.defaultId;
    removeBtn.disabled = all.length <= 1;
    const sorted = [...d.words].sort((a, b) => a.localeCompare(b));
    if (!sorted.length) {
      words.replaceChildren(h("p", { class: "sp-empty" }, t("No words yet. Choose Add to dictionary on a misspelled word.")));
      return;
    }
    words.replaceChildren(
      ...sorted.map((w) => {
        const remove = h("button", { class: "btn ghost", type: "button", "aria-label": t("Remove {word}", { word: w }) }, t("Remove"));
        remove.addEventListener("click", () => {
          dicts.removeWord(chosen, w);
          render();
        });
        return h("div", { class: "sp-word-row" }, h("span", { class: "sp-word-text" }, w), remove);
      }),
    );
  };
  newBtn.addEventListener("click", () => {
    chosen = dicts.create(t("New dictionary {n}", { n: String(dicts.all().length) })).id;
    render();
    name.focus();
    name.select();
  });
  defaultBtn.addEventListener("click", () => {
    dicts.setDefault(chosen);
    render();
  });
  removeBtn.addEventListener("click", async () => {
    const d = dicts.find(chosen);
    if (!d || !(await ed.ask("dictionary.remove", d.name, d.words.length))) return;
    dicts.remove(chosen);
    chosen = dicts.defaultId;
    render();
  });
  name.addEventListener("change", () => {
    dicts.update(chosen, { name: name.value });
    render();
  });
  lang.addEventListener("change", () => {
    dicts.update(chosen, { lang: lang.value });
    render();
  });
  const add = () => {
    for (const w of input.value.split(/\s+/)) if (w) dicts.addWord(w, chosen);
    input.value = "";
    addBtn.disabled = true;
    render();
    input.focus();
  };
  input.addEventListener("input", () => (addBtn.disabled = !input.value.trim()));
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && input.value.trim()) {
      e.preventDefault();
      add();
    }
  });
  addBtn.addEventListener("click", add);
  render();
  input.focus();
}

/** Readability statistics, as Word shows them: counts, averages, and for English the passives and the Flesch scores. */
export function openReadability(ed: Editor, stats: Readability) {
  const fmt = (n: number) => n.toLocaleString(currentLocale());
  const row = (label: string, value: string) => h("div", { class: "rs-row" }, h("dt", {}, label), h("dd", {}, value));
  const group = (title: string, ...rows: HTMLElement[]) => h("section", { class: "rs-group" }, h("h3", { class: "pref-group" }, title), h("dl", {}, ...rows));
  const done = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Readability statistics"), style: "width:min(var(--dialog-s),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Readability statistics"))),
    h(
      "div",
      { class: "dialog-body" },
      group(t("Counts"), row(t("Words"), fmt(stats.words)), row(t("Characters"), fmt(stats.characters)), row(t("Paragraphs"), fmt(stats.paragraphs)), row(t("Sentences"), fmt(stats.sentences))),
      group(t("Averages"), row(t("Sentences per paragraph"), fmt(stats.sentencesPerParagraph)), row(t("Words per sentence"), fmt(stats.wordsPerSentence)), row(t("Characters per word"), fmt(stats.charactersPerWord))),
      stats.english
        ? group(t("Readability"), row(t("Passive sentences"), `${fmt(stats.english.passive)}%`), row(t("Flesch Reading Ease"), fmt(stats.english.readingEase)), row(t("Flesch-Kincaid Grade Level"), fmt(stats.english.gradeLevel)))
        : h("p", { class: "pref-help" }, t("The passive sentences and the Flesch scores are worked out for English text.")),
    ),
    h("div", { class: "dialog-actions" }, done),
  );
  const close = modal(dialog, () => ed.focus());
  done.addEventListener("click", close);
  done.focus();
}
