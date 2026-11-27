// Every warning the editor gives before a risky action, in one place (DESIGN.md, Risky actions). `ask` asks first
// and says what changes: for what can't be undone, or changes the whole document. `undo` does it, then offers Undo:
// for a local change the person sees happen. Small edits get neither.
//
// An id that is a command's is asked about by the host wherever the command runs from (menus, the palette, shortcuts,
// the toolbar, the assistant). Any other id is a button in a dialog, which asks with `editor.ask(id, ...)`; the
// arguments it passes are the ones the wording takes. The app's own warnings are in web/src/cautions.ts.
import { defineExtension, msg, t, tn, type Caution, type Extension } from "@docxtor/extensions";

/** The editor's cautions. Built when the extension starts, so the words are in the interface's language. */
export function editorCautions(): Caution[] {
  const name = (v: unknown) => String(v ?? "");
  const count = (v: unknown) => Number(v) || 0;
  return [
    // The whole document, or every page of a section.
    { id: "insert.removeHeader", kind: "ask", title: t("Remove the header?"), impact: t("It goes from every page of this section. Undo brings it back."), confirm: t("Remove header") },
    { id: "insert.removeFooter", kind: "ask", title: t("Remove the footer?"), impact: t("It goes from every page of this section. Undo brings it back."), confirm: t("Remove footer") },
    { id: "insert.removeCoverPage", kind: "ask", title: t("Remove the cover page?"), impact: t("The first page goes, with everything on it. Undo brings it back."), confirm: t("Remove cover page") },
    { id: "insert.removePageNumbers", kind: "ask", title: t("Remove the page numbers?"), impact: t("They go from the headers and footers of every page. Undo brings them back."), confirm: t("Remove page numbers") },
    { id: "insert.removeToc", kind: "ask", title: t("Remove the table of contents?"), impact: t("The table goes; the headings stay. Undo brings it back."), confirm: t("Remove table of contents") },
    { id: "watermark.remove", kind: "ask", title: t("Remove the watermark?"), impact: t("It goes from every page. Undo brings it back."), confirm: t("Remove watermark") },
    { id: "format.clearStyleInstances", kind: "ask", title: t("Clear this style from all its text?"), impact: t("Every paragraph with it goes back to Normal, and text with a character style loses it. Undo brings it back."), confirm: t("Clear formatting") },
    { id: "inspect.removeAll", kind: "ask", title: t("Remove everything found?"), impact: t("What the check found goes from the document: comments, personal information, hidden text and the rest. Undo brings it back until the file is saved."), confirm: t("Remove everything") },
    { id: "style.delete", kind: "ask", title: (s) => t("Delete the style “{style}”?", { style: name(s) }), impact: (s, base) => t("Text with “{style}” will use {base} instead.", { style: name(s), base: name(base) }), confirm: t("Delete") },
    { id: "tabs.delete", kind: "ask", title: (_, n) => t("Delete “{name}”?", { name: name(n) }), impact: t("The tab, its subtabs and everything in them go. Undo brings them back."), confirm: t("Delete") },
    { id: "mailMerge.makeLabels", kind: "ask", title: t("Make a sheet of labels"), impact: t("The sheet of labels replaces what is in this document. Make the labels?"), confirm: t("Make the labels") },
    { id: "mailMerge.makeEnvelope", kind: "ask", title: t("Make an envelope"), impact: t("The envelope replaces what is in this document. Make the envelope?"), confirm: t("Make the envelope") },
    // What can't be undone: kept for the person, outside the document.
    { id: "design.resetDefault", kind: "ask", title: t("Reset the default design?"), impact: t("New documents start with Word's own design again, and the design you set as the default is forgotten."), confirm: t("Reset"), permanent: true },
    { id: "shortcuts.resetAll", kind: "ask", title: t("Reset all shortcuts?"), impact: (n) => tn(count(n), "The shortcut you set goes back to Docxtor's.", "The {n} shortcuts you set go back to Docxtor's."), confirm: t("Reset all"), permanent: true },
    { id: "dictionary.remove", kind: "ask", title: (n) => t("Remove “{name}”?", { name: name(n) }), impact: (_, n) => tn(count(n), "Its word goes, and the spelling checker marks it again.", "Its {n} words go, and the spelling checker marks them again."), confirm: t("Remove dictionary"), permanent: true },
    { id: "quickParts.delete", kind: "ask", title: (n) => t("Delete “{name}”?", { name: name(n) }), impact: t("It goes from your quick parts. Text you already inserted stays."), confirm: t("Delete"), permanent: true },
    // Local changes the person sees: done at once, with Undo.
    { id: "table.deleteTable", kind: "undo", title: t("Table deleted.") },
    { id: "table.deleteRow", kind: "undo", title: t("Row deleted.") },
    { id: "table.deleteColumn", kind: "undo", title: t("Column deleted.") },
    { id: "shape.delete", kind: "undo", title: t("Shape deleted.") },
  ];
}

export function cautions(): Extension {
  return defineExtension({
    id: "docxtor.cautions",
    name: msg("Warnings before risky actions"),
    description: msg("Asks before what can't be undone or changes the whole document, and offers Undo after deleting a table, a row, a column or a shape."),
    activate(ctx) {
      for (const c of editorCautions()) ctx.cautions.register(c);
    },
  });
}
