// Tools > Dictionary: looks a word up on Wiktionary and shows its definitions in the navigator. It is
// an extension of its own because it is the one place the app asks another server something: the word
// goes to Wikipedia's servers, so a deployment that keeps every request at home turns it off under
// Extensions, Manage extensions.
import { timeLimit, defineExtension, h, languageName, msg, t, type Extension, type ExtensionContext } from "@docxtor/editor";
import type { App } from "../main";

const ENDPOINT = "https://en.wiktionary.org/api/rest_v1/page/definition/";
const ENTRY = "https://en.wiktionary.org/wiki/";

interface Sense {
  definition?: string;
  examples?: string[];
  parsedExamples?: { example?: string }[];
}
interface Entry {
  partOfSpeech?: string;
  language?: string;
  definitions?: Sense[];
}
type Reply = Record<string, Entry[]>;

type Outcome = { kind: "found"; word: string; reply: Reply } | { kind: "missing" } | { kind: "offline" } | { kind: "error"; status: number };

/** Plain text from a piece of HTML: parsed in an inert document and read as text, never inserted. */
export function plain(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  return (doc.body.textContent ?? "").replace(/\s+/g, " ").trim();
}

/** The word under the caret, or the selected words (a short phrase is fine; Wiktionary has those). */
export function wordAt(ctx: ExtensionContext): string {
  const sel = ctx.document.selection();
  const engine = ctx.document.engine;
  if (!sel || !engine) return "";
  const phrase = (s: string) => s.replace(/\s+/g, " ").trim();
  if (sel.text && phrase(sel.text)) {
    const words = phrase(sel.text).split(" ");
    return words.slice(0, 4).join(" ").replace(/^[\p{P}\p{S}]+|[\p{P}\p{S}]+$/gu, "");
  }
  const { p, o } = sel.from;
  const text = engine.textRange({ p, o: 0 }, { p, o: Number.MAX_SAFE_INTEGER });
  const segmenter = new Intl.Segmenter(undefined, { granularity: "word" });
  for (const s of segmenter.segment(text)) {
    if (s.isWordLike && o >= s.index && o <= s.index + s.segment.length) return s.segment;
  }
  return "";
}

class DictionaryPanel {
  readonly el: HTMLElement;
  private input: HTMLInputElement;
  private results: HTMLElement;
  private notice: HTMLElement | null = null;
  private abort: AbortController | null = null;

  constructor(
    ctx: ExtensionContext,
    private language: () => string,
  ) {
    this.input = h("input", { type: "search", class: "dict-input", placeholder: t("Look up a word"), "aria-label": t("Look up a word"), spellcheck: "false", autocomplete: "off" });
    const go = h("button", { class: "btn sm primary", type: "submit" }, t("Look up"));
    const form = h("form", { class: "dict-form", role: "search" }, this.input, go);
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      void this.lookup(this.input.value);
    });
    this.results = h("div", { class: "dict-results", "aria-live": "polite" }, h("p", { class: "empty-note" }, t("Select a word in the document, or type one above.")));
    this.el = h("div", { class: "dict" }, h("div", { class: "panel-title" }, h("span", {}, t("Dictionary"))), form);
    if (!ctx.state.get("noticeSeen", false)) {
      const ok = h("button", { class: "btn sm ghost", type: "button" }, t("Got it"));
      ok.addEventListener("click", () => {
        ctx.state.set("noticeSeen", true);
        this.notice?.remove();
        this.notice = null;
      });
      this.notice = h("div", { class: "dict-notice" }, h("p", {}, t("Looks words up on Wiktionary. The word is sent to Wikipedia's servers; nothing else is.")), ok);
      this.el.append(this.notice);
    }
    this.el.append(this.results);
  }

  focusInput() {
    this.input.focus();
    this.input.select();
  }

  private async fetchWord(word: string, signal: AbortSignal): Promise<Outcome> {
    try {
      const res = await fetch(ENDPOINT + encodeURIComponent(word), { signal: timeLimit(15000, signal), headers: { Accept: "application/json" } });
      if (res.status === 404) return { kind: "missing" };
      if (!res.ok) return { kind: "error", status: res.status };
      const reply = (await res.json()) as Reply;
      return reply && typeof reply === "object" && Object.keys(reply).length ? { kind: "found", word, reply } : { kind: "missing" };
    } catch (e) {
      if ((e as Error).name === "AbortError") throw e;
      return { kind: "offline" };
    }
  }

  async lookup(raw: string) {
    const word = raw.replace(/\s+/g, " ").trim();
    if (!word) return;
    this.input.value = word;
    this.abort?.abort();
    const abort = (this.abort = new AbortController());
    this.results.replaceChildren(h("p", { class: "empty-note" }, t("Looking up “{word}”…", { word })));
    try {
      let out = await this.fetchWord(word, abort.signal);
      // A word at the start of a sentence is capitalised; entries are not.
      const lower = word.toLocaleLowerCase();
      if (out.kind === "missing" && lower !== word) out = await this.fetchWord(lower, abort.signal);
      this.show(word, out);
    } catch {
      // Replaced by a newer lookup.
    }
  }

  private show(word: string, out: Outcome) {
    if (out.kind === "missing") return this.say(t("Wiktionary has no entry for “{word}”.", { word }));
    if (out.kind === "offline") return this.say(t("Couldn't reach Wiktionary. Check your connection and try again."));
    if (out.kind === "error") return this.say(t("Wiktionary didn't answer (error {status}). Try again later.", { status: out.status }));
    const lang = this.language();
    const primary = lang.split("-")[0].toLowerCase();
    const keys = Object.keys(out.reply);
    const mine = keys.filter((k) => k.toLowerCase() === primary || (primary === "nb" && k === "no"));
    const sections = (mine.length ? mine : keys).map((k) => ({ code: k, entries: out.reply[k] ?? [] })).filter((s) => s.entries.length);
    if (!sections.length) return this.say(t("Wiktionary has no entry for “{word}”.", { word }));
    const body: HTMLElement[] = [h("h3", { class: "dict-word" }, out.word)];
    if (!mine.length && lang) body.push(h("p", { class: "dict-note" }, t("No {language} entry. These are the languages Wiktionary has for this word.", { language: languageName(primary) })));
    for (const s of sections) {
      if (sections.length > 1) body.push(h("h4", { class: "dict-lang" }, s.entries[0]?.language || s.code));
      for (const entry of s.entries) {
        const defs = (entry.definitions ?? []).map((d) => ({ text: plain(d.definition ?? ""), example: plain(d.parsedExamples?.[0]?.example ?? d.examples?.[0] ?? "") })).filter((d) => d.text);
        if (!defs.length) continue;
        body.push(
          h("h4", { class: "dict-pos" }, entry.partOfSpeech || t("Definitions")),
          h("ol", { class: "dict-defs" }, ...defs.map((d) => h("li", {}, h("span", {}, d.text), d.example ? h("span", { class: "dict-ex" }, d.example) : null))),
        );
      }
    }
    body.push(
      h(
        "p",
        { class: "dict-source" },
        h("a", { href: ENTRY + encodeURIComponent(out.word.replace(/ /g, "_")), target: "_blank", rel: "noopener noreferrer" }, t("Open the full entry on Wiktionary")),
        h("span", {}, t("Definitions from Wiktionary, shared under CC BY-SA.")),
      ),
    );
    this.results.replaceChildren(...body);
    this.results.scrollTop = 0;
  }

  private say(text: string) {
    this.results.replaceChildren(h("p", { class: "dict-message" }, text));
  }
}

/** The Dictionary: a navigator panel, Tools, Dictionary, and ⇧⌘Y. */
export function dictionary(app: App): Extension {
  return defineExtension({
    id: "docxtor.dictionary",
    name: msg("Dictionary"),
    description: msg("Looks up words on Wiktionary. The word you look up is sent to Wikipedia's servers; turn this off to keep every request on your own servers."),
    activate(ctx) {
      const panel = new DictionaryPanel(ctx, () => ctx.document.engine?.pageSetup().lang ?? "en");
      ctx.panels.register({
        id: "dictionary",
        title: t("Dictionary"),
        order: 60,
        tool: true,
        render: (el) => void el.append(panel.el),
      });
      ctx.commands.register(
        "tools.dictionary",
        async () => {
          const word = wordAt(ctx);
          app.showPanel("dictionary");
          if (word) await panel.lookup(word);
          else panel.focusInput();
        },
        { title: t("Dictionary"), category: t("Tools"), keybinding: "Mod+Shift+Y", icon: "book", when: () => !!app.fe.engine },
      );
      ctx.menus.register({ menu: "tools", group: "3_review", command: "tools.dictionary" });
    },
  });
}
