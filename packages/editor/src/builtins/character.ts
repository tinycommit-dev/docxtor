// Character formatting as commands: the Font dialog, changing case (and Shift+F3), hidden text and clearing a
// character style. The colour menus, the style picker and the toolbar buttons use the same engine ops.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { CASE_MODES, changeCase, clearCharStyle, cycleCase } from "../editor/charcmds";
import { CHARACTER_ICONS } from "../ui/character-icons";
import { openFont } from "../ui/font";
import type { Internals } from "./index";

export function characterSettings({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.character",
    name: msg("Character formatting"),
    description: msg("The Font dialog, changing case, hidden text and character styles."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(CHARACTER_ICONS)) ctx.icons.register({ id, svg });
      const open = () => !!fe.engine;
      const editable = () => open() && !fe.readOnly;
      ctx.commands.register("format.font", () => openFont(e), { title: t("Font…"), category: t("Format"), keybinding: "Mod+D", when: editable, icon: "font" });
      // The five cases are commands of their own, so the menu can list them and the assistant can run them.
      for (const { mode, title } of CASE_MODES)
        ctx.commands.register(`format.case.${mode}`, () => changeCase(e, mode), { title: t(title), category: t("Format"), when: editable, ai: true });
      ctx.commands.register("format.cycleCase", () => cycleCase(e), { title: t("Change case"), category: t("Format"), keybinding: "Shift+F3", when: editable, icon: "changeCase" });
      ctx.commands.register("format.clearCharStyle", () => clearCharStyle(e), {
        title: t("Clear character style"),
        category: t("Format"),
        when: () => editable() && !!e.format()?.charStyle,
        ai: true,
      });
      // Hidden text is not drawn unless this is on, or non-printing characters are shown (as in Word).
      ctx.commands.register(
        "view.hiddenText",
        () => {
          e.showHiddenText(!e.hiddenOwn);
          ctx.state.set("hiddenText", e.hiddenOwn);
        },
        { title: t("Show hidden text"), category: t("View"), when: () => true, icon: "hiddenText", toggled: () => e.hiddenShown },
      );
      e.showHiddenText(ctx.state.get("hiddenText", false));
      ctx.subscriptions.push({ dispose: () => e.showHiddenText(false) });
      ctx.menus.register({ menu: "context", group: "4_font", command: "format.font" });
    },
  });
}
