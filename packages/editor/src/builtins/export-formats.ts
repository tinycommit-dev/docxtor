import { defineExtension, msg, t, type Exporter, type Extension } from "@docxtor/extensions";

// More download formats. Each is its own extension, so it can be turned off in the Extensions dialog,
// and each is one call to the engine (the converters are in the Rust core, which the command-line
// tool and the MCP server use too).

function format(id: string, name: string, description: string, exporter: () => Exporter): Extension {
  return defineExtension({
    id,
    name,
    description,
    activate(ctx) {
      ctx.exporters.register(exporter());
    },
  });
}

/** Download as OpenDocument text (.odt), which LibreOffice and many other editors open. */
export function odtFormat(): Extension {
  return format("docxtor.format.odt", msg("OpenDocument text"), msg("Download the document as an .odt file for LibreOffice and other editors."), () => ({
    id: "odt",
    label: t("OpenDocument text"),
    extension: "odt",
    mime: "application/vnd.oasis.opendocument.text",
    export: (doc) => doc.exportOdt(),
  }));
}

/** Download as Rich Text (.rtf). */
export function rtfFormat(): Extension {
  return format("docxtor.format.rtf", msg("Rich Text"), msg("Download the document as a .rtf file."), () => ({
    id: "rtf",
    label: t("Rich Text"),
    extension: "rtf",
    mime: "application/rtf",
    export: (doc) => doc.exportRtf(),
  }));
}

/** Download as one web page (.html) with the pictures and styles inside. */
export function htmlFormat(): Extension {
  return format("docxtor.format.html", msg("Web page"), msg("Download the document as a single .html file with its pictures and styles inside."), () => ({
    id: "html",
    label: t("Web page"),
    extension: "html",
    mime: "text/html",
    export: (doc) => doc.exportHtml(),
  }));
}

/** Download as an EPUB book (.epub), one chapter for each Heading 1. */
export function epubFormat(): Extension {
  return format("docxtor.format.epub", msg("EPUB book"), msg("Download the document as an .epub e-book, with a chapter for each Heading 1."), () => ({
    id: "epub",
    label: t("EPUB book"),
    extension: "epub",
    mime: "application/epub+zip",
    export: (doc) => doc.exportEpub(),
  }));
}
