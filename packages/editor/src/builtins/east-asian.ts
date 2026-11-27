// East Asian typography as an extension: Format, Asian layout holds the Phonetic guide, Enclose characters, Two lines in
// one, Horizontal in vertical and the document's Line breaking rules. The Asian typography tab of the Paragraph box, the
// East Asian font in the Font box and the document grid in Page setup are in those boxes; the page drawing is in
// `editor/east-asian.ts`.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { hiraganaOf } from "../editor/east-asian";
import { openEncloseCharacters, openLineBreakRules, openTwoLinesInOne, toggleHorizontalInVertical } from "../ui/asian-layout";
import { EAST_ASIAN_ICONS } from "../ui/east-asian-icons";
import { openPhoneticGuide } from "../ui/phonetic-guide";
import type { Internals } from "./index";

export function eastAsian({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.eastAsian",
    name: msg("East Asian typography"),
    description: msg("Phonetic guides, enclosed characters, two lines in one, text upright in vertical lines and line breaking rules: Format, Asian layout."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(EAST_ASIAN_ICONS)) ctx.icons.register({ id, svg });
      // Docxtor's own reading: kana read as hiragana. A dictionary of kanji, or a service, is another provider beside it.
      ctx.readings.register({ id: "docxtor.kana", label: msg("Kana"), languages: ["ja"], reading: (text) => hiraganaOf(text) });
      const suggest = (text: string, lang: string) => fe.extensions.readingFor(text, lang);
      const editable = () => !!fe.engine && !fe.readOnly;
      /** Text is selected within one paragraph. */
      const selected = () => {
        const r = editable() ? e.range() : null;
        return !!r && r[0].p === r[1].p && r[0].o !== r[1].o;
      };
      const cmd = (id: string, title: string, run: () => unknown, icon: string, when: () => boolean, extra: { toggled?: () => boolean } = {}) =>
        ctx.commands.register(id, run, { title, category: t("Format"), when, icon, ...extra });
      cmd(
        "format.phoneticGuide",
        t("Phonetic guide…"),
        () => {
          const guide = e.rubyAtCaret();
          openPhoneticGuide(e, guide, guide ? e.rubyElement(guide.at) : null, suggest);
        },
        "phoneticGuide",
        () => selected() || (editable() && !!e.rubyAtCaret()),
      );
      cmd("format.encloseCharacters", t("Enclose characters…"), () => openEncloseCharacters(e), "encloseCharacters", () => editable());
      cmd("format.twoLinesInOne", t("Two lines in one…"), () => openTwoLinesInOne(e), "twoLinesInOne", selected);
      cmd("format.horizontalInVertical", t("Horizontal in vertical"), () => toggleHorizontalInVertical(e), "horizontalInVertical", selected, {
        toggled: () => !!e.format()?.run.eaLayout?.vert,
      });
      cmd("format.lineBreakRules", t("Line breaking rules…"), () => openLineBreakRules(e), "lineBreakRules", editable);
    },
  });
}
