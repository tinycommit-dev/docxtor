// Aligning and distributing objects, as an extension: Format, Arrange, Align (left, center, right, top, middle,
// bottom, and spreading three or more evenly), relative to the selected objects, the margins or the page. It works on
// any mix of shapes, text boxes, pictures and charts that float; the same lines are in the right-click menu on a
// selection of several. The maths and the ops are in ui/align.ts.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { ALIGN_ICONS } from "../ui/align-icons";
import { planAlign, skippedMessage, type AlignAction, type AlignTo } from "../ui/align";
import { toast } from "../ui/dom";
import { objectSetOf } from "../ui/object-set";
import type { Internals } from "./index";

export function align({ fe, editor, shapes, images }: Internals): Extension {
  return defineExtension({
    id: "docxtor.align",
    name: msg("Align and distribute"),
    description: msg("Line up shapes, text boxes, pictures and charts with each other, the margins or the page, and spread them evenly."),
    activate(ctx) {
      // Built here, not at import: the text is translated in the language the editor was opened in.
      const EDGES = [
        ["left", "arrange.align.left", t("Align left"), "alignObjectsLeft"],
        ["center", "arrange.align.center", t("Align center"), "alignObjectsCenter"],
        ["right", "arrange.align.right", t("Align right"), "alignObjectsRight"],
        ["top", "arrange.align.top", t("Align top"), "alignObjectsTop"],
        ["middle", "arrange.align.middle", t("Align middle"), "alignObjectsMiddle"],
        ["bottom", "arrange.align.bottom", t("Align bottom"), "alignObjectsBottom"],
      ] as const;

      const TARGETS: [AlignTo, string, string][] = [
        ["objects", "arrange.alignTo.objects", t("Align to selected objects")],
        ["margin", "arrange.alignTo.margin", t("Align to margin")],
        ["page", "arrange.alignTo.page", t("Align to page")],
      ];

      for (const [id, svg] of Object.entries(ALIGN_ICONS)) ctx.icons.register({ id, svg });
      const set = objectSetOf(editor, shapes, images);
      // The ones outside the shape tool's own selection follow the selection and the document.
      ctx.subscriptions.push({ dispose: fe.on("selection", () => set.refresh()) }, { dispose: fe.on("change", () => set.refresh()) }, { dispose: fe.on("open", () => set.clear()) });
      let to: AlignTo = ctx.state.get<AlignTo>("alignTo", "objects");
      const editable = () => !!fe.engine && !fe.readOnly && !editor.story;
      const count = () => (editable() ? set.all().length : 0);
      // Distributing needs something to spread: three objects among themselves, two across the margins or the page.
      const spread = () => count() >= (to === "objects" ? 3 : 2);

      const run = (action: AlignAction) => {
        const chosen = set.all();
        if (!chosen.length) return;
        const plan = planAlign(editor, chosen, action, to);
        if (!plan.ops.length) {
          if (plan.skipped) toast(skippedMessage(plan.skipped));
          return;
        }
        const first = chosen[0];
        editor.exec(plan.ops, { select: { anchor: first, focus: { p: first.p, o: first.o + 1 } } });
        set.select(chosen);
        if (plan.skipped) toast(skippedMessage(plan.skipped));
      };

      for (const [edge, id, title, icon] of EDGES) ctx.commands.register(id, () => run(edge), { title, category: t("Arrange"), when: () => count() >= 1, icon });
      ctx.commands.register("arrange.distribute.h", () => run("distribute-h"), { title: t("Distribute horizontally"), category: t("Arrange"), when: spread, icon: "distributeObjectsH" });
      ctx.commands.register("arrange.distribute.v", () => run("distribute-v"), { title: t("Distribute vertically"), category: t("Arrange"), when: spread, icon: "distributeObjectsV" });
      for (const [value, id, title] of TARGETS) {
        ctx.commands.register(
          id,
          () => {
            to = value;
            ctx.state.set("alignTo", value);
          },
          { title, category: t("Arrange"), when: () => editable(), toggled: () => to === value },
        );
      }

      ctx.menus.register({ menu: "format/arrange", group: "5", submenu: "format/arrange/align", title: t("Align"), icon: "alignObjects" });
      for (const [, id] of EDGES.slice(0, 3)) ctx.menus.register({ menu: "format/arrange/align", group: "1", command: id });
      for (const [, id] of EDGES.slice(3)) ctx.menus.register({ menu: "format/arrange/align", group: "2", command: id });
      ctx.menus.register({ menu: "format/arrange/align", group: "3", command: "arrange.distribute.h" });
      ctx.menus.register({ menu: "format/arrange/align", group: "3", command: "arrange.distribute.v" });
      for (const [, id] of TARGETS) ctx.menus.register({ menu: "format/arrange/align", group: "4", command: id });
      // The right-click menu on a selection of several: the same commands under their own ids, which only apply to two or more.
      const several = () => count() >= 2;
      for (const [edge, id, title, icon] of EDGES) {
        ctx.commands.register(`${id}.many`, () => run(edge), { title, category: t("Arrange"), when: several, icon, palette: false });
        ctx.menus.register({ menu: "context", group: "6_align", command: `${id}.many` });
      }
      for (const [id, action, title, icon] of [
        ["arrange.distribute.h.many", "distribute-h", t("Distribute horizontally"), "distributeObjectsH"],
        ["arrange.distribute.v.many", "distribute-v", t("Distribute vertically"), "distributeObjectsV"],
      ] as const) {
        ctx.commands.register(id, () => run(action), { title, category: t("Arrange"), when: spread, icon, palette: false });
        ctx.menus.register({ menu: "context", group: "6_align", command: id });
      }
    },
  });
}
