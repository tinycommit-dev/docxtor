// References that list or number what is in the text, as an extension: the Table of contents and Index dialogs,
// update page numbers only, remove the table, Add to table of contents, AutoMark, AutoCaption and citation
// placeholders; and Quick parts (saved snippets and the document's own building blocks). They sit in Insert,
// References and Insert, Quick parts.
import { defineExtension, msg, t, type CommandOptions, type Extension } from "@docxtor/extensions";
import { insertCitationPlaceholder, removeToc, updateTocPages, type AutoCaptions } from "../editor/lists";
import { expandAutoText } from "../editor/quick-parts";
import { openAutoCaption } from "../ui/auto-caption";
import { openAutoMark } from "../ui/automark";
import { toast } from "../ui/dom";
import { openIndexDialog } from "../ui/index-dialog";
import { openQuickParts, openSaveQuickPart } from "../ui/quick-parts";
import { openTocDialog } from "../ui/toc-dialog";
import { openTocEntry } from "../ui/toc-entry";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

export function contents({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.contents",
    name: msg("Contents, index and quick parts"),
    description: msg("The table of contents and index dialogs, AutoMark, AutoCaption, citation placeholders and Quick parts."),
    activate(ctx) {
      const open = () => !!fe.engine;
      const editable = () => open() && !fe.readOnly;
      const inBody = () => editable() && !e.story;
      const cmd = (id: string, title: string, category: string, run: (...args: unknown[]) => unknown, keybinding?: string | string[], when = inBody, extra: Partial<CommandOptions> = {}) =>
        ctx.commands.register(id, run, { title, category, keybinding, when, icon: COMMAND_ICONS[id], ...extra });

      // The table of contents.
      cmd("insert.tocOptions", t("Table of contents options…"), t("Insert"), () => openTocDialog(e));
      cmd("insert.updateTocPages", t("Update page numbers only"), t("Insert"), () => void updateTocPages(e), undefined, () => inBody() && e.hasToc(), { ai: true });
      cmd("insert.removeToc", t("Remove table of contents"), t("Insert"), () => void removeToc(e), undefined, () => inBody() && e.hasToc(), { ai: true });
      cmd("insert.tocEntry", t("Add to table of contents…"), t("Insert"), () => openTocEntry(e), "Mod+Alt+Shift+O");

      // The index and the marks that feed it.
      cmd("insert.indexOptions", t("Index options…"), t("Insert"), () => openIndexDialog(e));
      cmd("insert.autoMark", t("AutoMark…"), t("Insert"), () => openAutoMark(e));

      // Captions that add themselves. The settings last between sessions.
      const saved = ctx.state.get<AutoCaptions>("autoCaptions", {});
      e.autoCaptions = saved;
      e.autoCaptionsSave = (a) => ctx.state.set("autoCaptions", a);
      cmd("insert.autoCaption", t("AutoCaption…"), t("Insert"), () => openAutoCaption(e));

      // A source with only a tag so far, cited now and filled in later.
      cmd("insert.citationPlaceholder", t("Add new placeholder…"), t("Insert"), async () => {
        const taken = new Set(e.engine.sources().map((x) => x.tag));
        let n = 1;
        while (taken.has(`Placeholder${n}`)) n++;
        const tag = await ctx.ui.prompt(t("Placeholder name"), { placeholder: t("Name"), value: `Placeholder${n}`, confirm: t("Add") });
        if (tag === null) return;
        const made = insertCitationPlaceholder(e, tag.trim() || undefined);
        if (made) toast(t("Cited {tag}. Fill in the source in the Citations panel.", { tag: made }));
      }, undefined, inBody);

      // Quick parts: saved snippets in the stores extensions register, and the document's own building blocks.
      cmd("quickParts.save", t("Save selection to Quick parts…"), t("Insert"), () => openSaveQuickPart(e, fe.extensions), "Alt+F3", () => inBody() && !e.collapsed());
      cmd("quickParts.open", t("Quick parts…"), t("Insert"), () => void openQuickParts(e, fe.extensions), undefined, () => editable() && !e.story);
      cmd("quickParts.expandAutoText", t("Expand AutoText"), t("Insert"), async () => {
        if (!(await expandAutoText(fe.extensions, e))) toast(t("No AutoText has that name"));
      }, "Mod+Alt+F3", () => inBody() && e.collapsed());
    },
  });
}
