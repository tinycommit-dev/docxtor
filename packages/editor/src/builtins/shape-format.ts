// Shaping an object, as an extension: Format shape (the panel for fill, line, effects, size and text options), and flipping
// or turning the selected shapes. Each is a command, so it is in the menus, the right-click menu, the palette and the strip
// over a shape. The panel itself is in ui/format-shape.ts.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { formatShapePanel } from "../ui/format-shape";
import { SHAPE_ICONS } from "../ui/shape-icons";
import type { Internals } from "./index";

export function shapeFormat({ fe, editor, shapes }: Internals): Extension {
  return defineExtension({
    id: "docxtor.shapeFormat",
    name: msg("Format shape"),
    description: msg("The Format shape panel (fill, line, effects, size and position, text box options), and flipping and turning shapes."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(SHAPE_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly;
      const panel = () => formatShapePanel(editor, shapes);
      // The strip over a shape has a Format button; it opens the same panel.
      shapes.openFormat = () => panel().open();

      const onShape = () => editable() && shapes.selected;
      ctx.commands.register("shape.format", () => panel().open(), {
        title: t("Format shape…"),
        category: t("Shape"),
        when: onShape,
        toggled: () => panel().isOpen,
        icon: "formatShape",
      });
      ctx.commands.register("arrange.flipHorizontal", () => shapes.flip("h"), { title: t("Flip horizontal"), category: t("Arrange"), when: onShape, toggled: () => !!shapes.current?.flipH, icon: "flipHorizontal" });
      ctx.commands.register("arrange.flipVertical", () => shapes.flip("v"), { title: t("Flip vertical"), category: t("Arrange"), when: onShape, toggled: () => !!shapes.current?.flipV, icon: "flipVertical" });
      ctx.commands.register("arrange.rotateRight", () => shapes.rotateBy(90), { title: t("Rotate right 90°"), category: t("Arrange"), when: onShape, icon: "rotateRight" });
      ctx.commands.register("arrange.rotateLeft", () => shapes.rotateBy(-90), { title: t("Rotate left 90°"), category: t("Arrange"), when: onShape, icon: "rotateLeft" });

      ctx.menus.register({ menu: "format/shape", group: "0", command: "shape.format" });
      for (const id of ["arrange.rotateRight", "arrange.rotateLeft", "arrange.flipHorizontal", "arrange.flipVertical"]) {
        ctx.menus.register({ menu: "format/arrange", group: "4", command: id });
        ctx.menus.register({ menu: "context", group: "5_arrange", command: id });
      }
    },
  });
}
