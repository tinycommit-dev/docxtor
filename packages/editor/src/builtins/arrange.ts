// Arranging objects, as an extension: where a shape, text box, picture or chart sits in the stack of objects
// (bring to front, send to back), behind the text or in front of it, group and ungroup for shapes, and Insert,
// Drawing, Draw for freehand drawing. The strip over a selected object has the same lines in its Arrange menu.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { ARRANGE_ICONS } from "../ui/arrange-icons";
import type { Internals } from "./index";

export function arrange({ fe, shapes, images, ink }: Internals): Extension {
  return defineExtension({
    id: "docxtor.arrange",
    name: msg("Arrange"),
    description: msg("Bring to front, send to back, behind text, group and ungroup, and drawing by hand."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(ARRANGE_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly;
      // Whichever object is selected: a shape or text box, or a picture or chart.
      const target = () => (shapes.selected ? shapes : images.selected ? images : null);
      const floating = () => editable() && !!target()?.floating;
      const stack = [
        ["front", "arrange.front", t("Bring to front"), "bringToFront"],
        ["forward", "arrange.forward", t("Bring forward"), "bringForward"],
        ["backward", "arrange.backward", t("Send backward"), "sendBackward"],
        ["back", "arrange.back", t("Send to back"), "sendToBack"],
      ] as const;
      for (const [how, id, title, icon] of stack) ctx.commands.register(id, () => target()?.order(how), { title, category: t("Arrange"), when: floating, icon });
      ctx.commands.register("arrange.behindText", () => target()?.setBehind(true), {
        title: t("Behind text"),
        category: t("Arrange"),
        when: () => editable() && !!target(),
        toggled: () => target()?.wrap === "behind",
        icon: "behindText",
      });
      ctx.commands.register("arrange.inFrontOfText", () => target()?.setBehind(false), {
        title: t("In front of text"),
        category: t("Arrange"),
        when: () => editable() && !!target(),
        toggled: () => target()?.wrap === "front",
        icon: "inFrontOfText",
      });
      ctx.commands.register("arrange.group", () => shapes.group(), { title: t("Group"), category: t("Arrange"), when: () => editable() && shapes.canGroup, icon: "group" });
      ctx.commands.register("arrange.ungroup", () => shapes.ungroup(), { title: t("Ungroup"), category: t("Arrange"), when: () => editable() && shapes.canUngroup, icon: "ungroup" });
      ctx.commands.register("insert.draw", () => ink.start(), { title: t("Draw"), category: t("Insert"), when: () => editable() && !fe.editor.story && !ink.active, icon: "draw" });

      ctx.menus.register({ menu: "insert/drawing", group: "1", command: "insert.draw" });
      ctx.menus.register({ menu: "format", group: "4_objects", submenu: "format/arrange", title: t("Arrange"), icon: "bringToFront" });
      for (const [, id] of stack) ctx.menus.register({ menu: "format/arrange", group: "1", command: id });
      ctx.menus.register({ menu: "format/arrange", group: "2", command: "arrange.behindText" });
      ctx.menus.register({ menu: "format/arrange", group: "2", command: "arrange.inFrontOfText" });
      ctx.menus.register({ menu: "format/arrange", group: "3", command: "arrange.group" });
      ctx.menus.register({ menu: "format/arrange", group: "3", command: "arrange.ungroup" });
      // The right-click menu on an object.
      for (const [, id] of stack) ctx.menus.register({ menu: "context", group: "5_arrange", command: id });
      ctx.menus.register({ menu: "context", group: "5_arrange", command: "arrange.group" });
      ctx.menus.register({ menu: "context", group: "5_arrange", command: "arrange.ungroup" });
    },
  });
}
