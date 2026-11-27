import { Engine, loadEngine, type SniffedFormat } from "@docxtor/core";
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { describeOpenReport } from "../open-report";

// More file types to open. Each is its own extension, so it can be turned off in the Extensions dialog, and each is one
// call to the engine (the readers are in the Rust core, which the command-line tool uses too: `docxtor convert`).
// Opening makes a new .docx from the file; the person is told it was converted and what that leaves out.

function opener(id: string, name: string, description: string, label: () => string, extensions: string[], kinds: SniffedFormat[]): Extension {
  return defineExtension({
    id,
    name,
    description,
    activate(ctx) {
      ctx.importers.register({
        id: id.replace(/^docxtor\.open\./, ""),
        label: label(),
        extensions,
        detect: (bytes) => kinds.includes(Engine.sniff(bytes)),
        import: async (bytes, fileName) => {
          await loadEngine();
          const { docx, report } = Engine.convert(bytes, fileName);
          return { docx, notice: describeOpenReport(report) ?? undefined };
        },
      });
    },
  });
}

/** Open OpenDocument text (.odt, and the single-file .fodt) from LibreOffice and other editors. */
export function openOdt(): Extension {
  return opener("docxtor.open.odt", msg("Open OpenDocument text"), msg("Open .odt files from LibreOffice and other editors as new Word documents."), () => t("OpenDocument text"), ["odt", "ott", "fodt"], ["odt"]);
}

/** Open Rich Text (.rtf). */
export function openRtf(): Extension {
  return opener("docxtor.open.rtf", msg("Open Rich Text"), msg("Open .rtf files as new Word documents."), () => t("Rich Text"), ["rtf"], ["rtf"]);
}

/** Open web pages (.html, .htm) saved from a browser or written by Word. */
export function openHtml(): Extension {
  return opener("docxtor.open.html", msg("Open web pages"), msg("Open .html files as new Word documents."), () => t("Web page"), ["html", "htm", "xhtml"], ["html"]);
}

/** Open the binary Word format of Word 97 to 2003 (.doc). */
export function openDoc(): Extension {
  return opener("docxtor.open.doc", msg("Open Word 97 to 2003 files"), msg("Open old .doc files as new Word documents. Text, formatting, lists, tables and simple pictures come across."), () => t("Word 97 to 2003"), ["doc", "dot"], ["doc"]);
}

/** Open Word's XML files: Word 2003 XML and the single-file package (Flat OPC). */
export function openWordXml(): Extension {
  return opener("docxtor.open.xml", msg("Open Word XML files"), msg("Open Word 2003 XML and single-file Word packages (.xml) as new Word documents."), () => t("Word XML"), ["xml"], ["word-2003", "flat-opc"]);
}
