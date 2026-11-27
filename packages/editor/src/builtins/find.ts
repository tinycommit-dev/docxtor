import { defineExtension, msg, t, tn, type Extension } from "@docxtor/extensions";
import { toast } from "../ui/dom";
import { openGoto, stepPlace, type GoKind } from "../ui/goto";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

/** The note commands: Insert, References, Next and Previous footnote or endnote. */
const NOTE_STEPS: { id: string; title: string; kind: GoKind; d: 1 | -1; key?: string }[] = [
  { id: "references.nextFootnote", title: msg("Next footnote"), kind: "footnote", d: 1, key: "Mod+Alt+]" },
  { id: "references.previousFootnote", title: msg("Previous footnote"), kind: "footnote", d: -1, key: "Mod+Alt+[" },
  { id: "references.nextEndnote", title: msg("Next endnote"), kind: "endnote", d: 1 },
  { id: "references.previousEndnote", title: msg("Previous endnote"), kind: "endnote", d: -1 },
];

export function findAndReplace({ fe, editor, findBar }: Internals): Extension {
  return defineExtension({
    id: "docxtor.find",
    name: msg("Find and replace"),
    description: msg("Search the document and replace matches, go to a page, heading or note, and select text that is formatted alike."),
    activate(ctx) {
      const doc = () => !!fe.engine;
      ctx.commands.register("find.open", () => findBar.show(), { title: t("Find and replace"), category: t("Edit"), keybinding: ["Mod+F", "Mod+Shift+H"], when: doc, icon: COMMAND_ICONS["find.open"] });
      ctx.commands.register("find.next", () => findBar.stepFromOutside(1), { title: t("Find next"), category: t("Edit"), keybinding: "F3", when: doc, icon: COMMAND_ICONS["find.open"] });
      ctx.commands.register("find.previous", () => findBar.stepFromOutside(-1), { title: t("Find previous"), category: t("Edit"), keybinding: "Mod+Shift+G", when: doc, icon: COMMAND_ICONS["find.open"] });
      ctx.commands.register("find.results", () => findBar.showResults(), { title: t("List all results"), category: t("Edit"), when: doc, icon: COMMAND_ICONS["find.open"] });
      ctx.commands.register("edit.goto", () => openGoto(editor), { title: t("Go to…"), category: t("Edit"), keybinding: ["Mod+G", "F5"], when: doc, icon: "goto" });
      ctx.commands.register("edit.selectSimilar", () => selectSimilar(), { title: t("Select text with similar formatting"), category: t("Edit"), when: () => doc() && !editor.story, icon: "selectSimilar" });
      for (const n of NOTE_STEPS) {
        ctx.commands.register(
          n.id,
          () => {
            if (!stepPlace(editor, n.kind, n.d)) toast(n.kind === "footnote" ? t("There are no footnotes.") : t("There are no endnotes."));
          },
          { title: t(n.title), category: t("References"), keybinding: n.key, when: doc },
        );
      }
      ctx.menus.register({ menu: "edit", group: "4_find", command: "find.open" });
      ctx.menus.register({ menu: "edit", group: "4_find", command: "find.next" });
      ctx.menus.register({ menu: "edit", group: "4_find", command: "find.previous" });
      ctx.menus.register({ menu: "edit", group: "4_find", command: "find.results" });
      ctx.menus.register({ menu: "edit", group: "4_find", command: "edit.goto" });
      ctx.menus.register({ menu: "edit", group: "3_select", command: "edit.selectSimilar" });
      // One line in References that opens the four, so the menu stays short.
      ctx.menus.register({ menu: "insert/references", group: "4_notes", submenu: "insert/references/notes", title: t("Go to a note"), icon: "footnote" });
      for (const n of NOTE_STEPS) ctx.menus.register({ menu: "insert/references/notes", group: "1", command: n.id });
      // A document that replaces the one on screen leaves the matches stale.
      ctx.subscriptions.push({ dispose: fe.on("change", () => findBar.refresh()) }, { dispose: fe.on("open", () => findBar.refresh()) }, { dispose: () => findBar.hide(false) });

      /** Select every stretch of text that is formatted like the selection (or like the text at the caret). */
      function selectSimilar() {
        const r = editor.range();
        if (!r) return;
        const at = editor.collapsed() ? r[0] : { p: r[0].p, o: r[0].o + 1 };
        const format = editor.engine.findFormatAt(at.p, at.o);
        if (!format) return;
        const found = editor.engine.find("", { find_format: format, main_only: true });
        const ranges = found.hits.filter((m) => !m.story && m.p === m.endP).map((m) => ({ from: { p: m.p, o: m.o }, to: { p: m.endP, o: m.endO } }));
        if (!ranges.length) {
          toast(t("No other text is formatted like this."));
          return;
        }
        const first = ranges.findIndex((x) => x.from.p === r[0].p && x.from.o <= r[0].o && x.to.o >= r[0].o);
        const got = editor.selectRanges(ranges, Math.max(0, first));
        toast(got ? tn(got, "{n} place selected", "{n} places selected") : t("No other text is formatted like this."));
      }
    },
  });
}
