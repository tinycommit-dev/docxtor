// Templates: File, Save as template (into any template store that is on), and Download as a Word template (.dotx).
// Where saved templates are kept is a contribution (`ctx.templates.stores`); the app brings one that keeps them in the
// browser, and another extension can keep them on a server or a shared drive.
import { packageAsTemplate } from "@docxtor/core";
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { openSaveTemplate } from "../ui/save-template";
import { toast } from "../ui/dom";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

export function templateFiles({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.template-files",
    name: msg("Templates"),
    description: msg("Save a document as a template, and download it as a Word template (.dotx)."),
    activate(ctx) {
      ctx.exporters.register({
        id: "dotx",
        label: t("Word template"),
        extension: "dotx",
        mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.template",
        export: (doc) => packageAsTemplate(doc.savePlain()),
      });

      const stores = () => fe.extensions.templateStores.list();
      ctx.commands.register(
        "file.saveAsTemplate",
        () => {
          const engine = fe.engine;
          const list = stores();
          if (!engine || !list.length) return;
          const stem = (engine.fileName || t("Untitled")).replace(/\.(docx|docm|dotx|dotm)$/i, "");
          openSaveTemplate({
            name: stem,
            stores: list.map((s) => ({ id: s.id, label: s.label, note: s.note })),
            save: async ({ name, description, store }) => {
              const target = list.find((s) => s.id === store) ?? list[0]!;
              await target.save({ name, description: description || undefined }, packageAsTemplate(engine.savePlain()));
              toast(t("Saved the template “{name}”.", { name }));
            },
            onClose: () => e.focus(),
          });
        },
        { title: t("Save as template…"), category: t("File"), when: () => !!fe.engine && stores().length > 0, icon: COMMAND_ICONS["file.saveAsTemplate"] },
      );
      ctx.menus.register({ menu: "file", group: "3_save", command: "file.saveAsTemplate" });
    },
  });
}
