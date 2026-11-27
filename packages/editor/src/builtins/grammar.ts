// Grammar, as two built-in extensions that work together and can be turned off one by one.
//
// "Grammar suggestions" is the part everyone shares: it asks every grammar checker that is on (through
// `ctx.grammar`), draws what they find as blue underlines on the editor's SpellLayer, puts the message and
// the replacements in the right-click menu, and adds Show grammar suggestions under Tools, Spelling and
// grammar. It does not check anything itself.
//
// "Grammar rules" is Docxtor's own checker, one contribution to `ctx.grammar`: plain functions on the
// paragraph's text (grammar/rules.ts), no network. Other extensions, such as LanguageTool in the app, add
// checkers beside it; their findings are merged.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { GrammarIssue } from "@docxtor/extensions";
import type { SpellHit } from "../editor/spell-layer";
import { checkText, LANGUAGES } from "../grammar/rules";
import type { ProofingOptions } from "../spelling/options";
import type { Internals } from "./index";

/** Docxtor's own grammar and style rules as a checker. */
export function grammarRules({ editor: e }: Internals, options: ProofingOptions): Extension {
  return defineExtension({
    id: "docxtor.grammar.rules",
    name: msg("Grammar rules"),
    description: msg("Docxtor's own grammar and style checker for English, Spanish, French and German. It runs in the page: no text leaves your device."),
    activate(ctx) {
      // Flagging repeated or confused words was turned on or off (Proofing options): the rules are asked again.
      const ruleOptions = () => ({ repeatedWords: options.get().repeatedWords, confusedWords: options.get().confusedWords });
      let was = JSON.stringify(ruleOptions());
      ctx.subscriptions.push(
        options.onChange(() => {
          const now = JSON.stringify(ruleOptions());
          if (now === was) return;
          was = now;
          e.spell.grammarChanged();
        }),
      );
      ctx.grammar.register({
        id: "docxtor.grammar.rules",
        label: msg("Docxtor grammar rules"),
        languages: LANGUAGES,
        async check({ paragraphs }) {
          const out: GrammarIssue[] = [];
          for (const p of paragraphs) {
            for (const r of checkText(p.text, p.lang, ruleOptions())) out.push({ paragraph: p.id, from: r.from, to: r.to, message: r.message, replacements: r.replacements, rule: r.rule, kind: r.kind });
          }
          return out;
        },
      });
    },
  });
}

/** Grammar underlines, the menu lines for them and the switch under Tools, Spelling and grammar. */
export function grammar({ fe, editor: e, contextSources }: Internals): Extension {
  return defineExtension({
    id: "docxtor.grammar",
    name: msg("Grammar suggestions"),
    description: msg("Underlines grammar and style problems found by the grammar checkers that are on, and offers fixes in the right-click menu."),
    activate(ctx) {
      const host = fe.extensions;
      let show = ctx.state.get<boolean>("suggestions", true);
      let lang = "";
      const documentLanguage = () => (e.pageInfo.lang || navigator.language || "en-US").replace(/_/g, "-");

      /** Start asking the checkers, or stop (turned off, or nothing open). */
      const start = () => {
        if (!show || !fe.engine) {
          e.spell.setGrammar(null);
          return;
        }
        lang = documentLanguage();
        e.spell.setGrammar({ lang, check: (paragraphs, partial) => host.checkGrammar(paragraphs, partial) });
      };
      ctx.subscriptions.push({ dispose: () => e.spell.setGrammar(null) });
      // A checker was turned on or off: what the others said stays, but everything is asked again.
      ctx.subscriptions.push(host.grammarCheckers.onChange(() => e.spell.grammarChanged()));
      ctx.subscriptions.push(
        ctx.document.onDidOpen(() => {
          e.spell.resetIgnoredGrammar();
          start();
        }),
      );
      // File, Language changed the document's language (it lays the pages out again).
      ctx.subscriptions.push({
        dispose: fe.on("layout", () => {
          if (fe.engine && show && documentLanguage() !== lang) start();
        }),
      });
      if (fe.engine) start();

      // -- the right-click menu -----------------------------------------------------------------------

      const change = (hit: SpellHit, text: string): boolean => {
        if (hit.p === null || !e.engine) return false;
        const from = { p: hit.p, o: hit.from };
        const to = { p: hit.p, o: hit.to };
        // The words may have been edited since they were found.
        if (e.engine.textRange(from, to) !== hit.word) return false;
        return e.replaceText(from, to, text);
      };
      const source: (typeof contextSources)[number] = (at) => {
        if (!e.spell.grammarActive || e.readOnly) return [];
        // A misspelled word under the pointer is for the spelling lines.
        if (e.spell.active && (at ? e.spell.errorAtPoint(at.x, at.y) : e.spell.errorAtCaret())) return [];
        const hit = at ? e.spell.grammarAtPoint(at.x, at.y) : e.spell.grammarAtCaret();
        // A header, footer or note that isn't being edited can't be changed from here.
        if (!hit || hit.p === null) return [];
        const fixes = (hit.replacements ?? []).slice(0, 5).map((s) => ({ label: s, strong: true, run: () => change(hit, s) }));
        return [[{ label: hit.message ?? "", note: true, run: () => {} }], fixes, [{ label: t("Ignore"), run: () => e.spell.ignoreGrammar(hit) }]];
      };
      contextSources.push(source);
      ctx.subscriptions.push({ dispose: () => void contextSources.splice(contextSources.indexOf(source), 1) });

      // -- Tools, Spelling and grammar ----------------------------------------------------------------

      ctx.commands.register(
        "tools.grammarSuggestions",
        () => {
          show = !show;
          ctx.state.set("suggestions", show);
          start();
        },
        { title: t("Show grammar suggestions"), category: t("Tools"), when: () => !!fe.engine, toggled: () => show },
      );
      ctx.menus.register({ menu: "tools/spelling", group: "2", command: "tools.grammarSuggestions" });
    },
  });
}
