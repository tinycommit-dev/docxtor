import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { textToMarkdown } from "../editor/paste";

export function formats(): Extension {
  return defineExtension({
    id: "docxtor.formats",
    name: msg("Word, Markdown and plain text"),
    description: msg("Export as .docx, Markdown or plain text; open Markdown and text files."),
    activate(ctx) {
      ctx.exporters.register({
        id: "docx",
        label: t("Word document"),
        extension: "docx",
        mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        export: (doc) => doc.save(),
      });
      ctx.exporters.register({ id: "markdown", label: t("Markdown"), extension: "md", mime: "text/markdown", export: (doc) => doc.markdown() });
      ctx.exporters.register({ id: "text", label: t("Plain text"), extension: "txt", mime: "text/plain", export: (doc) => doc.text() });
      const decode = (b: Uint8Array) => new TextDecoder().decode(b).replace(/^﻿/, "");
      ctx.importers.register({ id: "markdown", label: t("Markdown"), extensions: ["md", "markdown"], import: (b) => ({ markdown: decode(b) }) });
      ctx.importers.register({ id: "text", label: t("Plain text"), extensions: ["txt"], import: (b) => ({ markdown: textToMarkdown(decode(b)) }) });
    },
  });
}
