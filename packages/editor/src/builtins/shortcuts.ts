// Keyboard shortcuts of your own: Tools, Keyboard shortcuts. The choices are kept with the editor's other settings and
// handed to the extension host, which runs a command from the key the person gave it.
import { defineExtension, msg, t, type Extension, type TakenKey } from "@docxtor/extensions";
import { openShortcuts } from "../ui/shortcuts";
import type { Internals } from "./index";

export function shortcuts({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.shortcuts",
    name: msg("Keyboard shortcuts"),
    description: msg("Give any command a keyboard shortcut of your own, or take one away."),
    activate(ctx) {
      const host = fe.extensions;
      host.loadShortcutOverrides(ctx.state.get<Record<string, string[]>>("overrides", {}), ctx.state.get<TakenKey[]>("taken", []));
      // The record of who took which key is kept with the choices, so a key still goes back after a reload.
      const save = (overrides: Record<string, string[]>, taken: TakenKey[]) => {
        ctx.state.set("overrides", overrides);
        ctx.state.set("taken", taken);
      };
      ctx.subscriptions.push({ dispose: () => host.loadShortcutOverrides({}) });
      ctx.commands.register(
        "tools.shortcuts",
        () => openShortcuts({ host, save, onClose: () => fe.engine && e.focus() }),
        { title: t("Customize keyboard shortcuts…"), category: t("Tools"), icon: "keyboard" },
      );
      ctx.menus.register({ menu: "tools", group: "8_settings", command: "tools.shortcuts" });
    },
  });
}
