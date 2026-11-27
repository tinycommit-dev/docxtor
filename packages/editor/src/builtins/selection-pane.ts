// The Selection pane, as an extension: Format, Arrange, Selection pane lists every object of the document with an
// eye to hide it, a name to change, and the stacking order to drag. See ui/selection-pane.ts.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { dockOf } from "../ui/dock";
import { objectSetOf } from "../ui/object-set";
import { SelectionPane } from "../ui/selection-pane";
import type { Internals } from "./index";

export function selectionPane({ fe, editor, shapes, images }: Internals): Extension {
  return defineExtension({
    id: "docxtor.selection-pane",
    name: msg("Selection pane"),
    description: msg("A list of the objects in the document, to select, rename, hide and put in order."),
    activate(ctx) {
      const pane = new SelectionPane(editor, objectSetOf(editor, shapes, images), shapes, fe);
      ctx.subscriptions.push({ dispose: () => pane.close() });
      ctx.commands.register("arrange.selectionPane", () => (pane.isOpen ? pane.close() : pane.open()), {
        title: t("Selection pane"),
        category: t("Arrange"),
        keybinding: "Alt+F10",
        when: () => !!fe.engine && !editor.story,
        toggled: () => pane.isOpen && !!dockOf("selection-pane"),
        icon: "selectionPane",
      });
      ctx.menus.register({ menu: "format/arrange", group: "6", command: "arrange.selectionPane" });
    },
  });
}
