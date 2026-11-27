// Paragraph settings as commands: the Paragraph and Tabs dialogs, distributed alignment and the text direction.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { openParagraph } from "../ui/paragraph";
import { PARAGRAPH_ICONS } from "../ui/paragraph-icons";
import { openTabs } from "../ui/tabs-dialog";
import type { Internals } from "./index";

export function paragraphSettings({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.paragraph",
    name: msg("Paragraph settings"),
    description: msg("The Paragraph and Tabs dialogs, distributed text and the direction text runs in."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(PARAGRAPH_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly;
      const para = () => e.format()?.para;
      ctx.commands.register("format.paragraph", () => openParagraph(e), { title: t("Paragraph…"), category: t("Format"), when: editable, icon: "pilcrow" });
      ctx.commands.register("format.tabs", () => openTabs(e), { title: t("Tabs…"), category: t("Format"), when: editable, icon: "tabStops" });
      ctx.commands.register("format.alignDistribute", () => e.setAlign("distribute"), {
        title: t("Distributed"),
        category: t("Format"),
        when: editable,
        icon: "alignDistribute",
        ai: true,
        toggled: () => para()?.align === "distribute",
      });
      // Switching the direction keeps the paragraph's alignment word, so a right-aligned paragraph becomes left-aligned
      // on screen, as it does in Word.
      ctx.commands.register("format.ltr", () => e.setDirection(false), {
        title: t("Left-to-right"),
        category: t("Format"),
        when: editable,
        icon: "textLtr",
        ai: true,
        toggled: () => !para()?.bidi,
      });
      ctx.commands.register("format.rtl", () => e.setDirection(true), {
        title: t("Right-to-left"),
        category: t("Format"),
        when: editable,
        icon: "textRtl",
        ai: true,
        toggled: () => !!para()?.bidi,
      });
      ctx.menus.register({ menu: "context", group: "5_paragraph", command: "format.paragraph" });
    },
  });
}
