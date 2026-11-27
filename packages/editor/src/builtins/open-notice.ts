import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { OpenNoticeBar, openNoticeDialog } from "../ui/open-notice";
import type { Internals } from "./index";

/** Says when a file was converted from another kind, repaired, or read as Strict Open XML, and what that changed. */
export function openNotice({ fe, editor }: Internals): Extension {
  return defineExtension({
    id: "docxtor.open-notice",
    name: msg("File notices"),
    description: msg("Says when a file was converted, repaired or read as Strict Open XML, and what that changed."),
    activate(ctx) {
      const bar = new OpenNoticeBar(() => editor.focus());
      fe.surface.prepend(bar.el);
      ctx.subscriptions.push({ dispose: () => bar.el.remove() });
      ctx.subscriptions.push(ctx.document.onDidOpen(() => bar.show(fe.openNotice)));
      if (fe.engine) bar.show(fe.openNotice);
      ctx.commands.register(
        "file.openNotes",
        () => {
          const n = fe.openNotice;
          if (n) openNoticeDialog(n);
        },
        { title: t("About this file…"), category: t("File"), when: () => !!fe.openNotice },
      );
      ctx.menus.register({ menu: "file", group: "6_details", command: "file.openNotes" });
    },
  });
}
