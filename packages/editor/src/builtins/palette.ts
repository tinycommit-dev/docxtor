import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

export function commandPalette({ fe }: Internals): Extension {
  return defineExtension({
    id: "docxtor.palette",
    name: msg("Command palette"),
    description: msg("Every command in one searchable list."),
    activate(ctx) {
      ctx.commands.register(
        "palette.show",
        async () => {
          const host = fe.extensions;
          const items = host.paletteCommands().map((c) => ({ label: c.title, detail: c.category, hint: host.shortcut(c.id), id: c.id }));
          const pick = await ctx.ui.pick(items, { placeholder: t("Type a command"), label: t("Command palette") });
          if (pick) await host.execute(pick.id);
        },
        { title: t("Show all commands"), keybinding: ["Mod+Shift+P", "Alt+/"], palette: false, icon: COMMAND_ICONS["palette.show"] },
      );
      // Every shortcut, searchable; picking one runs it.
      ctx.commands.register(
        "help.shortcuts",
        async () => {
          const host = fe.extensions;
          const items = host
            .paletteCommands()
            .filter((c) => host.shortcut(c.id))
            .map((c) => ({ label: c.title, detail: c.category, hint: host.shortcut(c.id), id: c.id }));
          const pick = await ctx.ui.pick(items, { placeholder: t("Search shortcuts"), label: t("Keyboard shortcuts") });
          if (pick) await host.execute(pick.id);
        },
        { title: t("Keyboard shortcuts"), category: t("Help"), icon: COMMAND_ICONS["help.shortcuts"] },
      );
      ctx.menus.register({ menu: "help", group: "1_find", command: "palette.show", title: t("Search the menus") });
      ctx.menus.register({ menu: "help", group: "1_find", command: "help.shortcuts" });
    },
  });
}
