// Lists a person defines, and sorting, as an extension: Define new bullet, number format and multilevel list
// (Format, Bullets and numbering, and the list galleries), Set numbering value, and Sort paragraphs (Tools).
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { LIST_ICONS } from "../ui/list-icons";
import { openDefineList } from "../ui/define-list";
import { openNumberingValue } from "../ui/numbering-value";
import { openSort } from "../ui/sort";
import type { Internals } from "./index";

export function lists({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.lists",
    name: msg("Lists and sorting"),
    description: msg("Define your own bullets, number formats and multilevel lists, set a list item's number, and sort paragraphs."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(LIST_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly;
      const fonts = () => fe.extensions.fonts.list().map((f) => f.family);
      const define = (id: string, title: string, mode: "bullet" | "number" | "multilevel", icon: string) =>
        ctx.commands.register(id, () => openDefineList(e, { mode, fonts: fonts() }), { title, category: t("Format"), when: editable, icon });
      define("format.defineBullet", t("Define new bullet…"), "bullet", "bullets");
      define("format.defineNumberFormat", t("Define new number format…"), "number", "numbers");
      define("format.defineMultilevel", t("Define new multilevel list…"), "multilevel", "outline");
      // A list the document already has (a gallery's "In this document" tile): the selected paragraphs join it.
      ctx.commands.register(
        "format.applyList",
        (numId) => {
          const ids = e.selectedParagraphs();
          if (typeof numId !== "number" || !ids.length) return;
          e.exec({ op: "apply_list", ids, num_id: numId }, { select: e.sel, keepExtra: true });
        },
        { title: t("List from this document"), category: t("Format"), when: editable, palette: false },
      );
      ctx.commands.register("format.setNumberingValue", () => openNumberingValue(e), {
        title: t("Set numbering value…"),
        category: t("Format"),
        when: () => editable() && e.format()?.list === "decimal",
      });
      ctx.commands.register("format.sortParagraphs", () => openSort(e), {
        title: t("Sort paragraphs…"),
        category: t("Tools"),
        when: () => editable() && !e.collapsed() && e.selectedParagraphs().length > 1,
        icon: "sort",
      });

      ctx.menus.register({ menu: "format/lists", group: "3", command: "format.setNumberingValue" });
      for (const c of ["format.defineBullet", "format.defineNumberFormat", "format.defineMultilevel"]) ctx.menus.register({ menu: "format/lists", group: "5", command: c });
      ctx.menus.register({ menu: "tools", group: "4_text", command: "format.sortParagraphs" });
    },
  });
}
