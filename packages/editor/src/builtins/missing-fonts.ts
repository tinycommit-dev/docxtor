import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { MissingFontsBar } from "../ui/missing-fonts";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

export function missingFonts({ fe, editor }: Internals): Extension {
  return defineExtension({
    id: "docxtor.missing-fonts",
    name: msg("Missing fonts"),
    description: msg("Says when a document uses fonts this device can't show, and replaces them."),
    activate(ctx) {
      const offered = () => fe.extensions.fonts.list().map((f) => f.family);
      const bar = new MissingFontsBar(editor, offered);
      fe.surface.prepend(bar.el);
      ctx.subscriptions.push({ dispose: () => bar.dispose() });
      ctx.subscriptions.push(ctx.document.onDidOpen(() => bar.reset()));
      // Fonts extensions add (or take away) change what's missing.
      ctx.subscriptions.push(fe.extensions.fonts.onChange(() => bar.check()));
      // Undo, redo, a paste or a font replaced can bring a font in or take one out: look again once
      // a burst of them pauses. (Not on every edit: checking renders the whole document.)
      let timer = 0;
      ctx.subscriptions.push(
        {
          dispose: fe.on("fonts-may-change", () => {
            clearTimeout(timer);
            timer = window.setTimeout(() => bar.check(), 300);
          }),
        },
        { dispose: () => clearTimeout(timer) },
      );
      if (fe.engine) bar.reset();
      ctx.commands.register("format.replaceMissingFonts", () => bar.openReplace(), {
        title: t("Replace missing fonts…"),
        category: t("Format"),
        icon: COMMAND_ICONS["format.replaceMissingFonts"],
        when: () => !fe.readOnly && bar.missing.length > 0,
      });
      ctx.menus.register({ menu: "format", group: "9_clear", command: "format.replaceMissingFonts" });
    },
  });
}
