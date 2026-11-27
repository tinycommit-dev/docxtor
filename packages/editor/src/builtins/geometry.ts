// Shape geometry, as an extension: curves, freeforms and scribbles, edit points, and connectors that stay attached to
// shapes. The shape picker (Insert, Drawing, Shape, Lines) starts the drawing tools; Format, Shape has Edit points, and so
// does the strip over a shape and the right-click menu. Each is a command, so it is also in the palette.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { ConnectorTool } from "../ui/connector-tool";
import { EditPoints } from "../ui/edit-points";
import { FreeformTool } from "../ui/freeform-tool";
import { h } from "../ui/dom";
import { GEOMETRY_ICONS, type GeometryTool } from "../ui/geometry-icons";
import type { Internals } from "./index";

export function geometry({ fe, editor, shapes }: Internals): Extension {
  return defineExtension({
    id: "docxtor.geometry",
    name: msg("Shape geometry"),
    description: msg("Curves, freeforms and scribbles, edit points, and connectors that stay attached to shapes."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(GEOMETRY_ICONS)) ctx.icons.register({ id, svg });
      const free = new FreeformTool(editor, shapes);
      const connectors = new ConnectorTool(editor, shapes);
      const points = new EditPoints(editor, shapes);
      const editable = () => !!fe.engine && !fe.readOnly;
      const drawing = () => editable() && !editor.story && !free.active && !connectors.active;

      // The picker hands over the tools it lists.
      shapes.onTool = (tool: GeometryTool, arrow: boolean) => {
        if (tool === "curve" || tool === "freeform" || tool === "scribble") free.start(tool);
        else connectors.start(tool, arrow);
      };
      const tools: [string, string, string, () => void][] = [
        ["insert.curve", t("Curve"), "curve", () => free.start("curve")],
        ["insert.freeform", t("Freeform"), "freeform", () => free.start("freeform")],
        ["insert.scribble", t("Scribble"), "scribble", () => free.start("scribble")],
        ["insert.straightConnector", t("Straight connector"), "connector", () => connectors.start("straight")],
        ["insert.elbowConnector", t("Elbow connector"), "elbow", () => connectors.start("elbow")],
        ["insert.curvedConnector", t("Curved connector"), "curvedConnector", () => connectors.start("curved")],
      ];
      for (const [id, title, icon, run] of tools) ctx.commands.register(id, run, { title, category: t("Insert"), when: drawing, icon });

      // Edit points on the selected shape (its outline becomes a custom one with the first change).
      ctx.commands.register("shape.editPoints", () => (points.active ? points.exit() : void points.start()), {
        title: t("Edit points"),
        category: t("Shape"),
        when: () => editable() && (points.active || points.possible),
        toggled: () => points.active,
        icon: "editPoints",
      });
      ctx.menus.register({ menu: "format/shape", group: "1", command: "shape.editPoints" });
      ctx.menus.register({ menu: "context", group: "5_geometry", command: "shape.editPoints" });
      // And a button in the strip over a shape.
      shapes.stripExtras.push(() => {
        if (!points.possible) return [];
        const label = t("Edit points");
        const b = h("button", { class: "tb", type: "button", title: label, "aria-label": label });
        b.innerHTML = GEOMETRY_ICONS.editPoints;
        b.addEventListener("mousedown", (e) => e.preventDefault());
        b.addEventListener("click", () => void points.start());
        return [b];
      });
    },
  });
}
