// Styles, as an extension: the commands behind the style gallery and the dialogs that edit styles (Modify style, New
// style, Create a style from the selection, Update to match, Manage styles), selecting and clearing every instance of
// a style, and the style gallery switches. The Styles panel and Reveal formatting need the app's navigator, so they are
// registered there (`web/src/builtins.ts`); the lists they draw are `ui/styles-list.ts` and `ui/reveal-panel.ts`.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { Op } from "@docxtor/core";
import { DESIGN_ICONS } from "../ui/design-icons";
import { openCreateStyle } from "../ui/create-style";
import { openStyleDialog } from "../ui/modify-style";
import { openManageStyles } from "../ui/styles-list";
import { styleTitle } from "../ui/style-preview";
import type { Internals } from "./index";

export function styles({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.styles",
    name: msg("Styles"),
    description: msg("Change a style for every paragraph that uses it, make a style from the selection, update one to match, delete one, and manage them all. Styles are written as Word writes them."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(DESIGN_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly;
      const host = () => fe.extensions;
      const engine = () => fe.engine!;
      /** A command argument that names a style, else the style the caret's paragraph has. */
      const styleArg = (arg: unknown) => (typeof arg === "string" && arg ? arg : (e.format()?.style ?? "Normal"));
      const title = (id: string) => {
        const s = engine().styles().find((x) => x.id === id);
        return s ? styleTitle(s.name, s.custom) : id;
      };

      ctx.commands.register("format.createStyle", () => openCreateStyle(e, host()), { title: t("Create a style from the selection…"), category: t("Format"), when: editable, icon: "styleAdd" });
      ctx.commands.register("format.newStyle", () => openStyleDialog({ ed: e, host: host() }), { title: t("New style…"), category: t("Format"), when: editable, icon: "styleAdd" });
      ctx.commands.register("format.modifyStyle", (arg) => openStyleDialog({ ed: e, host: host(), style: styleArg(arg) }), { title: t("Modify style…"), category: t("Format"), when: editable, icon: "styleEdit" });
      ctx.commands.register(
        "format.updateStyle",
        (arg) => {
          const r = e.range();
          if (!r) return;
          const id = styleArg(arg);
          e.exec({ op: "update_style", style: id, from: r[0], to: r[1] }, { select: e.sel, keepExtra: true, onError: (m) => void ctx.ui.notify(m) });
          e.focus();
        },
        { title: t("Update style to match the selection"), category: t("Format"), when: editable, icon: "styleRefresh" },
      );
      ctx.commands.register("format.manageStyles", () => openManageStyles(e, host()), { title: t("Manage styles…"), category: t("Format"), when: editable, icon: "styles" });
      ctx.commands.register(
        "format.applyStyle",
        (arg) => {
          if (typeof arg !== "string") return;
          const look = engine().styleGallery().find((l) => l.id === arg || l.name === arg);
          if (look?.kind === "character") e.setRun({ style: look.id });
          else e.setStyle(look?.id ?? arg);
        },
        { title: t("Apply a style"), category: t("Format"), when: editable, palette: false },
      );
      ctx.commands.register(
        "format.selectStyleInstances",
        (arg) => {
          const uses = engine().styleUses(styleArg(arg));
          if (!uses.length) return;
          if (e.selectRanges(uses.map((u) => ({ from: { p: u.p, o: u.from }, to: { p: u.p, o: u.to } }))) && e.sel) e.scrollParaIntoView(e.sel.focus.p);
        },
        { title: t("Select all text with this style"), category: t("Format"), when: editable, palette: false },
      );
      ctx.commands.register(
        "format.clearStyleInstances",
        (arg) => {
          const id = styleArg(arg);
          const uses = engine().styleUses(id);
          if (!uses.length) return;
          const info = engine().styles().find((s) => s.id === id);
          const ops: Op[] =
            info?.kind === "character"
              ? uses.map((u) => ({ op: "format_text", from: { p: u.p, o: u.from }, to: { p: u.p, o: u.to }, format: { style: "" } }))
              : [{ op: "set_style", ids: uses.map((u) => u.p), style: "Normal" }];
          e.exec(ops, { select: e.sel });
        },
        { title: t("Clear formatting of all text with this style"), category: t("Format"), when: editable, palette: false },
      );
      ctx.commands.register(
        "format.styleInGallery",
        (arg, on) => {
          if (typeof arg !== "string") return;
          e.exec({ op: "set_style_definition", style: arg, def: { quick: on !== false } }, { select: e.sel, keepExtra: true });
        },
        { title: t("Show or hide a style in the gallery"), category: t("Format"), when: editable, palette: false },
      );
      ctx.commands.register(
        "format.renameStyle",
        async (arg) => {
          const id = styleArg(arg);
          const info = engine().styles().find((s) => s.id === id);
          if (!info?.custom) {
            void ctx.ui.notify(t("Word's own styles keep their names."));
            return;
          }
          const name = await ctx.ui.prompt(t("Rename the style"), { value: info.name, confirm: t("Rename") });
          if (name === null || !name.trim() || name.trim() === info.name) return;
          e.exec({ op: "set_style_definition", style: id, def: { name: name.trim() } }, { select: e.sel, keepExtra: true, onError: (m) => void ctx.ui.notify(m) });
        },
        { title: t("Rename a style"), category: t("Format"), when: editable, icon: "pencil", palette: false },
      );
      ctx.commands.register(
        "format.deleteStyle",
        async (arg) => {
          const id = styleArg(arg);
          const info = engine().styles().find((s) => s.id === id);
          if (!info?.custom) {
            void ctx.ui.notify(t("Word's own styles stay in the document. Change one instead."));
            return;
          }
          const base = info.basedOn ? title(info.basedOn) : t("Normal");
          if (!(await ctx.ask("style.delete", title(id), base))) return;
          e.exec({ op: "delete_style", style: id }, { select: e.sel, keepExtra: true, onError: (m) => void ctx.ui.notify(m) });
        },
        { title: t("Delete a style"), category: t("Format"), when: editable, icon: "trash", palette: false },
      );
    },
  });
}
