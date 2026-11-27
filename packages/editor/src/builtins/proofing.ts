// Proofing options, as a built-in extension: which words the spelling checker leaves alone, flagging repeated words,
// hiding the marks in one document, and checking the document again. The choices live in `ProofingOptions`, which the
// Spelling and Grammar rules extensions read; this one keeps them between sessions and offers them under Tools.
// "Do not check spelling or grammar" is File, Language (ui/language.ts): it is a property of the text.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { ProofingOptions } from "../spelling/options";
import { openProofingOptions } from "../ui/proofing-options";
import { REVIEW_ICONS } from "../ui/review-icons";
import type { Internals } from "./index";

export function proofing({ fe, editor: e }: Internals, options: ProofingOptions): Extension {
  return defineExtension({
    id: "docxtor.proofing",
    name: msg("Proofing options"),
    description: msg("Which words the spelling checker leaves alone, detecting the language, accents on French capitals, which grammar rules run, readability statistics, hiding the spelling and grammar marks in one document, and checking again."),
    activate(ctx) {
      ctx.icons.register({ id: "proofing", svg: REVIEW_ICONS.proofing });
      options.load(ctx.state.get("settings", {}));
      ctx.subscriptions.push(options.onChange(() => ctx.state.set("settings", { ...options.get() })));

      // "Hide spelling errors in this document only" is in the file (settings.xml): the marks follow it.
      const syncHidden = () => e.spell.setHidden(!!fe.engine && !!e.pageInfo.hideSpellingErrors, !!fe.engine && !!e.pageInfo.hideGrammaticalErrors);
      ctx.subscriptions.push({ dispose: fe.on("layout", syncHidden) });
      ctx.subscriptions.push(ctx.document.onDidOpen(syncHidden));
      ctx.subscriptions.push({ dispose: () => e.spell.setHidden(false, false) });
      if (fe.engine) syncHidden();

      const open = () => !!fe.engine;
      ctx.commands.register("tools.proofingOptions", () => openProofingOptions(e, options, () => void ctx.commands.execute("tools.recheckDocument"), () => void ctx.commands.execute("tools.personalDictionary")), {
        title: t("Proofing options…"),
        category: t("Tools"),
        icon: "proofing",
        when: open,
      });
      const toggle = (id: string, title: string, key: "hideSpellingErrors" | "hideGrammaticalErrors", field: "hide_spelling_errors" | "hide_grammatical_errors") =>
        ctx.commands.register(
          id,
          () => {
            e.exec({ op: "set_layout_settings", [field]: !e.pageInfo[key] });
            e.reloadPageSetup();
          },
          { title, category: t("Tools"), when: () => open() && !fe.readOnly, toggled: () => !!e.pageInfo[key] },
        );
      toggle("tools.hideSpellingErrors", t("Hide spelling errors in this document"), "hideSpellingErrors", "hide_spelling_errors");
      toggle("tools.hideGrammarErrors", t("Hide grammar errors in this document"), "hideGrammaticalErrors", "hide_grammatical_errors");

      ctx.menus.register({ menu: "tools/spelling", group: "3", command: "tools.recheckDocument" });
      ctx.menus.register({ menu: "tools/spelling", group: "3", command: "tools.hideSpellingErrors" });
      ctx.menus.register({ menu: "tools/spelling", group: "3", command: "tools.hideGrammarErrors" });
      ctx.menus.register({ menu: "tools/spelling", group: "4", command: "tools.proofingOptions" });
    },
  });
}
