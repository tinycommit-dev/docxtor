// Equations, as an extension: Insert, Equation opens the equation editor at the caret, and "Edit equation…" opens it on
// the equation that is selected (double-click and Enter on one do the same). The editor and its palette are in
// `ui/equation.ts`; the engine does the reading and the drawing.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { openEquationEditor } from "../ui/equation";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

export function equations({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.equations",
    name: msg("Equations"),
    description: msg("Write equations in LaTeX with a live preview and a palette, and edit the ones Word made: Insert, Equation."),
    activate(ctx) {
      const editable = () => !!fe.engine && !fe.readOnly;
      // Alt+= is Word's shortcut for an equation. Ctrl/Cmd+Alt+= is zoom in.
      ctx.commands.register("insert.equation", () => openEquationEditor(e), {
        title: t("Equation…"),
        category: t("Insert"),
        keybinding: "Alt+=",
        when: editable,
        icon: COMMAND_ICONS["insert.equation"],
      });
      ctx.commands.register(
        "equation.edit",
        () => {
          const eq = e.equationAtCaret();
          if (eq) openEquationEditor(e, { edit: eq });
        },
        { title: t("Edit equation…"), category: t("Insert"), when: () => editable() && !!e.equationAtCaret(), icon: COMMAND_ICONS["insert.equation"] },
      );
    },
  });
}
