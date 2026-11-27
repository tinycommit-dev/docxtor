// What opening a file did, and why a file could not be opened, in the person's language. The engine reports codes
// (see `OpenReport` in @docxtor/core); this is where they become sentences.
import { OpenError, type OpenFormat, type OpenNote, type OpenReport } from "@docxtor/core";
import { t, tn, type OpenNotice } from "@docxtor/extensions";

/** The name of a kind of file, as the person knows it. */
export function formatLabel(format: OpenFormat): string {
  switch (format) {
    case "odt":
      return t("OpenDocument text");
    case "rtf":
      return t("Rich Text");
    case "html":
      return t("HTML");
    case "doc":
      return t("Word 97 to 2003");
    case "word2003":
      return t("Word 2003 XML");
    case "flat-opc":
      return t("Word XML package");
    default:
      return t("Word");
  }
}

/** What a conversion keeps and leaves out, said once for each kind of file. */
function caveat(format: OpenFormat): string | null {
  switch (format) {
    case "odt":
      return t("Docxtor read the text and its formatting, headings, lists, tables, links, notes and pictures. Headers and footers, text boxes, drawings and charts are not carried over.");
    case "rtf":
      return t("Docxtor read the text and its formatting, lists, tables, links, notes and pictures. Headers and footers, page setup beyond size and margins, text boxes and drawings are not carried over.");
    case "html":
      return t("Docxtor read the text and its formatting, headings, lists, tables, links and embedded pictures. Page layout, scripts and pictures stored beside the page are not carried over.");
    case "doc":
      return t("Docxtor read the text, character and paragraph formatting, lists, tables, links, notes and simple pictures. Styles are turned into direct formatting, and headers and footers, text boxes, drawings, equations and objects are not carried over.");
    case "word2003":
      return t("Docxtor read the text and its formatting, lists, tables, links and pictures. Headers and footers, comments, text boxes and drawings are not carried over.");
    case "flat-opc":
      return t("Everything in the package was kept. It is now an ordinary .docx file.");
    default:
      return null;
  }
}

function noteText(n: OpenNote): string | null {
  const c = n.count;
  switch (n.code) {
    case "zip-directory":
      return tn(c, "The file's index was damaged. Docxtor rebuilt it and found {n} part.", "The file's index was damaged. Docxtor rebuilt it and found {n} parts.");
    case "zip-damaged":
      return tn(c, "{n} part was cut short or damaged and was read as far as it could be.", "{n} parts were cut short or damaged and were read as far as they could be.");
    case "xml-recovered":
      return tn(c, "{n} part had damaged markup. Docxtor kept what it could read.", "{n} parts had damaged markup. Docxtor kept what it could read.");
    case "xml-unreadable":
      return tn(c, "{n} part could not be read and was left as it is.", "{n} parts could not be read and were left as they are.");
    case "text-cut":
      return t("The text of the document ends early. Anything that came after the damage is lost.");
    case "parts-missing":
      return tn(c, "{n} picture or other part the document refers to is missing.", "{n} pictures or other parts the document refers to are missing.");
    case "rels-restored":
      return t("Missing links between the file's parts were put back.");
    case "types-restored":
      return t("The file's list of part types was incomplete. Docxtor completed it.");
    case "picture-unreadable":
      return tn(c, "{n} picture could not be read and was left out.", "{n} pictures could not be read and were left out.");
    case "picture-unsupported":
      return tn(c, "{n} picture is in a format Docxtor can't show and was left out.", "{n} pictures are in a format Docxtor can't show and were left out.");
    case "pictures-linked":
      return tn(c, "{n} picture is stored outside the page and was left out.", "{n} pictures are stored outside the page and were left out.");
    case "chunk-objects":
      return tn(c, "{n} chart or embedded object in imported content is not shown, and editing that content removes it.", "{n} charts or embedded objects in imported content are not shown, and editing that content removes them.");
    case "object-dropped":
      return tn(c, "{n} drawing or embedded object was left out.", "{n} drawings or embedded objects were left out.");
    case "lost-drawings":
      return tn(c, "{n} drawn shape or floating picture was left out.", "{n} drawn shapes or floating pictures were left out.");
    case "lost-text-boxes":
      return t("Text boxes became ordinary paragraphs or were left out.");
    case "lost-headers":
      return t("Headers and footers were left out.");
    case "lost-comments":
      return t("Comments were left out.");
    case "doc-word6":
      return t("This is a Word 6 or Word 95 file, so only its text was read.");
    case "nothing-found":
      return t("No text was found in the file.");
    default:
      return null;
  }
}

/**
 * The notice to show after opening, or null when there is nothing to say. A conversion always has one: the person
 * hears that the file became a new Word document and what that leaves out. A repair says what was fixed. Strict Open XML
 * says it was read as the standard kind.
 */
export function describeOpenReport(report: OpenReport | null | undefined): OpenNotice | null {
  if (!report) return null;
  const lines = report.notes.filter((n) => n.code !== "strict").map(noteText).filter((s): s is string => !!s);
  const strict = report.notes.some((n) => n.code === "strict");
  const repaired = report.notes.some((n) => ["zip-directory", "zip-damaged", "xml-recovered", "xml-unreadable", "text-cut", "rels-restored", "types-restored", "parts-missing"].includes(n.code));
  if (report.converted) {
    const label = formatLabel(report.format);
    const notes = [t("Your original file has not been changed. Docxtor works on a new Word document made from it, and saves it as a .docx."), caveat(report.format), ...lines].filter((s): s is string => !!s);
    return { title: t("This {format} file was converted to a Word document.", { format: label }), notes };
  }
  if (repaired) {
    const notes = strict ? [t("The file also uses Strict Open XML. Docxtor read it as the standard kind and will save it as one."), ...lines] : lines;
    return { title: t("This file was damaged. Docxtor repaired it as far as it could."), notes: [...notes, t("Check the document before you rely on it. Saving writes the repaired version.")] };
  }
  if (strict) {
    return {
      title: t("This file uses Strict Open XML."),
      notes: [t("Docxtor read it as the standard kind of Word file, and will save it as one. Word opens both kinds, so nothing in the text or its formatting changes.")],
    };
  }
  return null;
}

/** Why a file could not be opened, in plain words. Any other error keeps its own message. */
export function describeOpenError(e: unknown): string {
  if (!(e instanceof OpenError)) return e instanceof Error ? e.message : String(e);
  switch (e.code) {
    case "empty":
      return t("The file is empty.");
    case "not-word":
      switch (e.detail) {
        case "spreadsheet":
          return t("This is a spreadsheet, not a Word document. Open it in a spreadsheet app.");
        case "presentation":
          return t("This is a presentation, not a Word document. Open it in a presentation app.");
        case "pdf":
          return t("This is a PDF, and Docxtor can't open PDFs yet.");
        case "epub":
          return t("This is an e-book (EPUB), and Docxtor can't open those.");
        case "image":
          return t("This is a picture, not a document. Insert it into a document instead.");
        case "archive":
          return t("This is a zip archive that doesn't hold a Word document.");
        default:
          return t("This isn't a kind of file Docxtor can open.");
      }
    case "doc-encrypted":
      return t("This old Word file (.doc) is protected with a password, and Docxtor can't unlock those. Open it in Word, save it as a .docx, and open that. A .docx can have a password and Docxtor asks for it.");
    case "too-big":
      return t("This file is too large or too deeply nested to open.");
    case "damaged":
      return t("This file is too damaged to open. Nothing in it could be read.");
    case "no-text":
      return t("Docxtor couldn't find the text of this document. The file may be damaged.");
    case "encrypted":
      return t("This document is protected with a password.");
    default:
      return t("Docxtor doesn't recognise this kind of file.");
  }
}
