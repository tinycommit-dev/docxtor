// The thesaurus, as two built-in extensions.
//
// "Thesaurus" is the part everyone shares: the command (Tools, Thesaurus), its panel and the line in the right-click
// menu. It does not know a single synonym; it asks every thesaurus that is on, through `ctx.thesaurus`.
//
// "English thesaurus" is Docxtor's own source, one contribution to `ctx.thesaurus`: a compact list that ships with
// Docxtor and is loaded the first time a word is looked up. Nothing leaves the device. Other extensions add sources
// beside it (the app has Datamuse, which is off until someone turns it on); the answers are listed by source.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { EnglishThesaurus } from "../thesaurus/english";
import { REVIEW_ICONS } from "../ui/review-icons";
import { openThesaurus } from "../ui/thesaurus";
import { languageAt, wordAtCaret } from "../ui/word-at";
import type { Internals } from "./index";

/** Docxtor's own English synonyms. */
export function englishThesaurus(): Extension {
  return defineExtension({
    id: "docxtor.thesaurus.english",
    name: msg("English thesaurus"),
    description: msg("Synonyms and opposites for English, from a list that comes with Docxtor. It runs in the page: no word leaves your device."),
    activate(ctx) {
      // The list is a chunk of its own, fetched when the first word is looked up.
      let loaded: Promise<EnglishThesaurus> | null = null;
      const load = () => (loaded ??= import("../thesaurus/english-data").then((m) => new EnglishThesaurus(m.ENGLISH_GROUPS)));
      ctx.thesaurus.register({
        id: "docxtor.thesaurus.english",
        label: msg("Docxtor English thesaurus"),
        languages: ["en"],
        async lookup(word) {
          return (await load()).lookup(word);
        },
      });
    },
  });
}

/** Tools, Thesaurus: the panel and the right-click line. */
export function thesaurus({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.thesaurus",
    name: msg("Thesaurus"),
    description: msg("Looks up synonyms for the word at the caret and replaces it with the one you pick. The synonyms come from the thesauruses that are on."),
    activate(ctx) {
      ctx.icons.register({ id: "thesaurus", svg: REVIEW_ICONS.thesaurus });
      const host = fe.extensions;
      let panel: ReturnType<typeof openThesaurus> | null = null;
      const closePanel = () => {
        panel?.close();
        panel = null;
      };
      ctx.subscriptions.push({ dispose: closePanel });
      ctx.subscriptions.push(ctx.document.onDidOpen(closePanel));

      const open = () => {
        // The panel is already there: look up the word the caret is in now.
        if (panel && fe.surface.querySelector(".th-panel")) return panel.lookup();
        panel = openThesaurus(e, { lookup: (word, lang) => host.thesaurusLookup(word, lang), serves: (lang) => host.thesaurusesFor(lang).length > 0 }, fe.surface);
      };
      ctx.commands.register("tools.thesaurus", open, { title: t("Thesaurus"), category: t("Tools"), keybinding: "Shift+F7", icon: "thesaurus", when: () => !!fe.engine });
      ctx.menus.register({ menu: "tools", group: "3_review", command: "tools.thesaurus" });

      // Right-click on a word: Synonyms… opens the panel on it (only where there is a word and a thesaurus for its language).
      ctx.commands.register("tools.synonyms", open, {
        title: t("Synonyms…"),
        category: t("Tools"),
        icon: "thesaurus",
        palette: false,
        when: () => {
          if (!fe.engine || e.readOnly || e.story) return false;
          const w = wordAtCaret(e);
          return !!w && host.thesaurusesFor(languageAt(e, w.p)).length > 0;
        },
      });
      ctx.menus.register({ menu: "context", group: "8_lookup", command: "tools.synonyms" });
    },
  });
}
