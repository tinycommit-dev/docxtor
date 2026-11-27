// Pasting as an extension: Docxtor's own readers for what is on the clipboard (its own format, HTML from Word,
// Google Docs and web pages, RTF, plain text), Paste special, ways to paste with or without formatting, and the
// format painter. Other extensions add readers through `ctx.clipboard.converters`.
import { defineExtension, formatChord, msg, t, type Extension } from "@docxtor/extensions";
import { pasteFromClipboard } from "../editor/clipboard";
import { BUILTIN_CONVERTERS } from "../editor/paste-convert";
import { openPasteSpecial } from "../ui/paste-special";
import type { Internals } from "./index";

export function pasting({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.paste",
    name: msg("Pasting"),
    description: msg("Reads Docxtor, Word, Google Docs, web pages, RTF and plain text from the clipboard, with Paste special and the format painter. Extensions can add formats."),
    activate(ctx) {
      for (const c of BUILTIN_CONVERTERS) ctx.clipboard.converters.register(c);
      e.pasteFlow.converters = () => fe.extensions.pasteConverters.list();
      ctx.subscriptions.push({ dispose: () => (e.pasteFlow.converters = () => []) });

      const open = () => !!fe.engine;
      const editable = () => open() && !fe.readOnly;
      const key = () => formatChord("Mod+V", fe.extensions.mac);
      ctx.commands.register("edit.pasteSpecial", () => openPasteSpecial(e, key()), { title: t("Paste special…"), category: t("Edit"), when: editable, icon: "paste" });
      ctx.commands.register("edit.pasteKeepSource", () => pasteFromClipboard(e, false, key(), "keep"), { title: t("Paste and keep source formatting"), category: t("Edit"), when: editable, icon: "paste" });
      ctx.commands.register("edit.pasteMerge", () => pasteFromClipboard(e, false, key(), "merge"), { title: t("Paste and merge formatting"), category: t("Edit"), when: editable, icon: "paste" });

      // The brush: one use, or until Escape or a second click when switched on with a double-click.
      ctx.commands.register("format.painter", () => e.painter.toggle(), { title: t("Format painter"), category: t("Format"), when: editable, icon: "brush", toggled: () => e.painter.on });
      ctx.commands.register("format.painterKeep", () => e.painter.toggle(true), { title: t("Format painter, keep on"), category: t("Format"), when: editable, icon: "brush", palette: false });
    },
  });
}
