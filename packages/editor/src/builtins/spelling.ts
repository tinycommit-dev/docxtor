// Docxtor's own spelling checker, as an extension. The dictionaries come from `ctx.spelling.dictionaries`
// (the `spelling` option of DocxtorEditor registers one as a shortcut); with none for the document's
// language it adds nothing and the browser's spellcheck keeps working.
//
// The underlines are drawn by the editor's SpellLayer (editor/spell-layer.ts); the words are looked up
// by the checker (spelling/checker.ts, spellbook in WebAssembly). This extension decides when to check, which words to leave
// alone (the custom dictionaries, the ones ignored in this document) and what the menus offer. Each
// paragraph is checked in its own language, and a paragraph in a language without a dictionary is left alone.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { SpellChecker, SpellHit } from "../editor/spell-layer";
import { SpellClient } from "../spelling/client";
import { browserDetector, detectByWords } from "../spelling/detect";
import { CustomDictionaries, loadDictionaries, sameWord } from "../spelling/dictionaries";
import type { ProofingOptions } from "../spelling/options";
import { readability } from "../spelling/readability";
import type { SpellingProvider } from "../spelling/types";
import { openCustomDictionaries, openReadability, openSpellPanel, type SpellingApi } from "../ui/spelling";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

/** How long the right-click menu waits for suggestions before it opens without them. */
const SUGGEST_WAIT = 700;

const same = sameWord;

/** The same language, whatever the region: `fr-CA` and `fr` are. */
const base = (lang: string) => lang.toLowerCase().split(/[-_]/)[0];
/** How many paragraphs' languages are remembered. */
const MAX_DETECTED = 4000;

export function spelling({ fe, editor: e, spelling: option, contextSources }: Internals, options: ProofingOptions): Extension {
  return defineExtension({
    id: "docxtor.spelling",
    name: msg("Spelling"),
    description: msg("Underlines misspelled words and offers corrections in the right-click menu and under Tools, Spelling and grammar."),
    activate(ctx) {
      // The `spelling` option is a shortcut for registering one provider.
      if (option) ctx.spelling.dictionaries.register({ id: "docxtor.spelling.option", label: msg("Dictionaries from the editor's spelling option"), languages: "*", load: (lang) => option.load(lang) });
      const host = fe.extensions;
      // Dictionaries come from whichever extension has the language (the latest registered wins).
      const fromHost: SpellingProvider = { load: (lang) => host.spellingDictionary(lang) };
      let client = new SpellClient(fromHost);
      ctx.subscriptions.push({ dispose: () => client.destroy() });

      let show = ctx.state.get<boolean>("suggestions", true);
      // The custom dictionaries; before there were several, the words were one list kept as "personal".
      const dictionaries = new CustomDictionaries(loadDictionaries(ctx.state.get("dictionaries", null), ctx.state.get("personal", []), t("Personal")), (st) => ctx.state.set("dictionaries", st), t("Personal"));
      ctx.subscriptions.push(
        dictionaries.onChange(() => {
          suggestions.clear();
          e.spell.refilter();
        }),
      );

      // Detect language automatically: the browser's detector when it is ready, else Docxtor's own (asked latest first).
      ctx.spelling.languageDetectors.register({ id: "docxtor.detect.words", label: msg("Docxtor's language detector"), detect: (text) => detectByWords(text) });
      const fromBrowser = browserDetector();
      ctx.spelling.languageDetectors.register({ id: "docxtor.detect.browser", label: msg("The browser's language detector"), detect: (text) => fromBrowser.detect(text) });
      /** What language each paragraph's text was found to be in (null: none Docxtor can check), and what is still being asked. */
      const detected = new Map<string, string | null>();
      const asking = new Set<string>();
      const langFor = (text: string): string | null => {
        const words = text.match(/\p{L}+/gu);
        if (!words || words.length < 6) return null;
        const k = text.length > 600 ? text.slice(0, 600) : text;
        const known = detected.get(k);
        // Compared with the document's language now, which may have changed since: its own language keeps its region (en-GB stays en-GB).
        if (known !== undefined) return known && base(known) !== base(documentLanguage()) ? known : null;
        if (!asking.has(k)) {
          asking.add(k);
          void host.detectLanguage(k).then(async (found) => {
            // Only a language Docxtor has a dictionary for: otherwise the paragraph would go unchecked.
            const lang = found && (await client.ready(found.lang)) ? found.lang : null;
            asking.delete(k);
            if (detected.size > MAX_DETECTED) detected.clear();
            detected.set(k, lang);
            if (lang && base(lang) !== base(documentLanguage())) e.spell.languagesChanged();
          });
        }
        return null;
      };
      /** Words ignored in the open document. */
      const ignored = new Set<string>();
      /** The language checked now, and whether Docxtor has a dictionary for it. */
      let lang = "";
      /** The document language the checker was last started for (set at once, where `lang` waits for the dictionary). */
      let started = "";
      let available = true;
      let sync = 0;
      const suggestions = new Map<string, string[]>();

      const documentLanguage = () => (e.pageInfo.lang || navigator.language || "en-US").replace(/_/g, "-");

      // The browser's own underlines would double ours: they are off while Docxtor's checker is on.
      e.spell.onActive = (on) => e.setNativeSpellcheck(!on);
      ctx.subscriptions.push({
        dispose: () => {
          e.spell.onActive = null;
          e.spell.setChecker(null);
          e.setNativeSpellcheck(true);
        },
      });

      /** Start checking, or stop (turned off, nothing open, or no dictionary for the document's language). Paragraphs in other languages use their own dictionary. */
      const start = async () => {
        const mine = ++sync;
        if (!show || !fe.engine) {
          e.spell.setChecker(null);
          return;
        }
        const docLang = documentLanguage();
        started = docLang;
        const ok = await client.ready(docLang);
        if (mine !== sync) return;
        lang = docLang;
        available = ok;
        if (!ok) {
          e.spell.setChecker(null);
          return;
        }
        const checker: SpellChecker = {
          lang,
          ready: (l) => client.ready(l),
          // French capitals may go without their accents (ECOLE for ÉCOLE), unless the person asks for them.
          check: (l, words) => client.check(l, words, base(l) === "fr" && !options.get().frenchAccentedCapitals),
          ignored: (w, l) => ignored.has(same(w)) || dictionaries.has(w, l),
          options: () => options.words(),
        };
        suggestions.clear();
        e.spell.setChecker(checker);
        e.spell.setDetector(options.get().detectLanguage ? langFor : null);
      };

      // Proofing options: which kinds of word are left alone (the words are looked at again, not looked up again);
      // detecting languages or the accents on French capitals change what the words are checked against: start again.
      const lookups = () => `${options.get().detectLanguage}|${options.get().frenchAccentedCapitals}`;
      let lookedUp = lookups();
      ctx.subscriptions.push(
        options.onChange(() => {
          // Corrections may come from other places now (Suggest from main dictionary only).
          suggestions.clear();
          if (lookups() !== lookedUp) {
            lookedUp = lookups();
            void start();
          } else e.spell.refilter();
        }),
      );
      // A dictionary was added or taken away (an extension turned on or off): start again with a fresh worker.
      ctx.subscriptions.push(
        host.spellingDictionaries.onChange(() => {
          client.destroy();
          client = new SpellClient(fromHost);
          void start();
        }),
      );
      // File, Language changed the document's language (it lays the pages out again).
      ctx.subscriptions.push({
        dispose: fe.on("layout", () => {
          if (fe.engine && show && documentLanguage() !== started) void start();
        }),
      });

      let panel: (() => void) | null = null;
      const closePanel = () => {
        panel?.();
        panel = null;
      };
      ctx.subscriptions.push({ dispose: closePanel });
      ctx.subscriptions.push(
        ctx.document.onDidOpen(() => {
          ignored.clear();
          closePanel();
          void start();
        }),
      );
      if (fe.engine) void start();

      // -- what the menus and the panel use -----------------------------------------------------------

      /** Corrections for a misspelled word (in the language its paragraph was checked in) or a grammar issue (the checker's own). */
      const suggest = async (hit: SpellHit): Promise<string[]> => {
        if (hit.kind && hit.kind !== "spelling") return (hit.replacements ?? []).slice(0, 5);
        const l = hit.lang || lang;
        const k = `${l}\u0000${hit.word}`;
        let found = suggestions.get(k);
        if (!found) {
          // The custom dictionaries' words come first, unless suggestions are from the main dictionary only.
          const own = options.get().mainDictionaryOnly ? [] : dictionaries.similar(hit.word, l, 2);
          found = [...own, ...(await client.suggest(l, hit.word))].filter((s, i, all) => same(s) !== same(hit.word) && all.findIndex((x) => same(x) === same(s)) === i).slice(0, 5);
          suggestions.set(k, found);
        }
        return found;
      };
      const change = (hit: SpellHit, text: string): boolean => {
        if (hit.p === null || !e.engine) return false;
        const from = { p: hit.p, o: hit.from };
        const to = { p: hit.p, o: hit.to };
        // The word may have been edited since it was found.
        if (e.engine.textRange(from, to) !== hit.word) return false;
        return e.replaceText(from, to, text);
      };
      const ignore = (hit: SpellHit) => {
        if (hit.kind && hit.kind !== "spelling") return e.spell.ignoreGrammar(hit);
        ignored.add(same(hit.word));
        e.spell.refilter();
      };
      const add = (word: string) => dictionaries.addWord(word);
      const stats = () => {
        if (!e.engine) return;
        openReadability(e, readability(e.engine.text(), documentLanguage()));
      };
      const api: SpellingApi = {
        errors: () => e.spell.allErrors(),
        suggestions: suggest,
        change,
        ignore,
        add,
        finished: () => {
          if (options.get().showReadability) stats();
        },
      };

      // -- the right-click menu -----------------------------------------------------------------------

      const waitFor = <T>(p: Promise<T>, fallback: T) => Promise.race([p, new Promise<T>((r) => setTimeout(() => r(fallback), SUGGEST_WAIT))]);
      const source: (typeof contextSources)[number] = async (at) => {
        if (!e.spell.active || e.readOnly) return [];
        const hit = at ? e.spell.errorAtPoint(at.x, at.y) : e.spell.errorAtCaret();
        // A header, footer or note that isn't being edited can't be changed from here.
        if (!hit || hit.p === null) return [];
        const found = await waitFor(suggest(hit), [] as string[]);
        const fixes = found.length
          ? found.map((s) => ({ label: s, strong: true, run: () => change(hit, s) }))
          : [{ label: t("No suggestions"), disabled: true, run: () => {} }];
        return [fixes, [{ label: t("Ignore"), run: () => ignore(hit) }, { label: t("Add to dictionary"), run: () => add(hit.word) }]];
      };
      contextSources.push(source);
      ctx.subscriptions.push({ dispose: () => void contextSources.splice(contextSources.indexOf(source), 1) });

      // -- Tools, Spelling and grammar ----------------------------------------------------------------

      const open = () => !!fe.engine;
      ctx.commands.register(
        "tools.spellingSuggestions",
        () => {
          show = !show;
          ctx.state.set("suggestions", show);
          if (!show) closePanel();
          void start();
        },
        { title: t("Show spelling suggestions"), category: t("Tools"), when: open, toggled: () => show },
      );
      ctx.commands.register(
        "tools.spellCheck",
        () => {
          // Grammar alone is enough to open the panel when some checker covers the language.
          const grammar = e.spell.grammarActive && host.grammarCheckersFor(documentLanguage()).length > 0;
          if (!e.spell.active && !grammar) {
            if (!show) return ctx.ui.notify(t("Spelling suggestions are off. Turn them on under Tools, Spelling and grammar."));
            return ctx.ui.notify(available ? t("Spelling is still loading. Try again in a moment.") : t("Spelling isn't available for this language"));
          }
          closePanel();
          panel = openSpellPanel(e, api, fe.surface);
        },
        { title: t("Spelling and grammar check"), category: t("Tools"), keybinding: "Mod+Alt+X", when: () => open() && !fe.readOnly, icon: COMMAND_ICONS["tools.spellCheck"] },
      );
      ctx.commands.register("tools.personalDictionary", () => openCustomDictionaries(e, dictionaries), { title: t("Custom dictionaries…"), category: t("Tools"), when: open });
      ctx.commands.register("tools.readability", stats, { title: t("Readability statistics…"), category: t("Tools"), when: open });
      // Words and rules the person chose to ignore in this document come back, and everything is looked at again.
      ctx.commands.register(
        "tools.recheckDocument",
        () => {
          ignored.clear();
          e.spell.resetIgnoredGrammar();
          e.spell.refilter();
          e.spell.grammarChanged();
          ctx.ui.notify(t("Spelling and grammar will be checked again."));
        },
        { title: t("Recheck document"), category: t("Tools"), when: open },
      );
      ctx.menus.register({ menu: "tools", group: "1_spelling", submenu: "tools/spelling", title: t("Spelling and grammar"), icon: "spellcheck" });
      ctx.menus.register({ menu: "tools/spelling", group: "1", command: "tools.spellCheck" });
      ctx.menus.register({ menu: "tools/spelling", group: "2", command: "tools.spellingSuggestions" });
      ctx.menus.register({ menu: "tools/spelling", group: "2", order: 1, command: "tools.personalDictionary" });
      ctx.menus.register({ menu: "tools/spelling", group: "2", order: 2, command: "tools.readability" });
    },
  });
}
