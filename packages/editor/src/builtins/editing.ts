import { currentLocale, defineExtension, formatChord, msg, t, tn, type CommandOptions, type Extension } from "@docxtor/extensions";
import { normalizeSettings, type AutoformatSettings } from "../editor/autoformat";
import { copyAsMarkdown, pasteFromClipboard, pasteFromMarkdown } from "../editor/clipboard";
import { normalizeChoice } from "../editor/paste-flow";
import { h, toast } from "../ui/dom";
import { addBookmark, openBookmarks } from "../ui/bookmarks";
import { openBordersShading } from "../ui/borders-shading";
import { openColumns } from "../ui/columns";
import { openLineNumbers } from "../ui/line-numbers";
import { isLandscape, openPageSetup, setOrientation } from "../ui/page-setup";
import { openCaption } from "../ui/caption";
import { openCrossReference } from "../ui/cross-reference";
import { openIndexEntry } from "../ui/index-entry";
import { openLanguage } from "../ui/language";
import { LIST_PRESETS } from "../ui/list-styles";
import { openNoteOptions } from "../ui/note-options";
import { openPageNumbers } from "../ui/page-numbers";
import { openPreferences, type UnitChoice } from "../ui/preferences";
import { openSpecialCharacters } from "../ui/special-chars";
import { keepRecentColorsIn } from "../ui/swatches";
import { openTableOfFigures } from "../ui/table-of-figures";
import { openWatermark } from "../ui/watermark";
import { openWordCount } from "../ui/word-count";
import { storyLink, toggleLinkToPrevious } from "../editor/story-links";
import { MENU_FONTS } from "../fonts";
import { localeUnitSystem, setUnitSystem } from "../units";
import { COMMAND_ICONS } from "./icons";
import { editorMenus } from "./menus";
import type { Internals } from "./index";

export function editingCommands({ fe, editor: e, openLink, tables, images, shapes }: Internals): Extension {
  return defineExtension({
    id: "docxtor.editing",
    name: msg("Editing commands"),
    description: msg("Formatting, inserting and viewing, as commands for the palette and shortcuts."),
    activate(ctx) {
      const open = () => !!fe.engine;
      const editable = () => open() && !fe.readOnly;
      const cmd = (id: string, title: string, category: string, run: (...args: unknown[]) => unknown, keybinding?: string | string[], when = editable, extra: Partial<CommandOptions> = {}) =>
        ctx.commands.register(id, run, { title, category, keybinding, when, icon: COMMAND_ICONS[id], ...extra });
      // Formatting, tables and simple inserts need no further input, so the assistant may run them too.
      const aiCmd = (id: string, title: string, category: string, run: (...args: unknown[]) => unknown, keybinding?: string | string[], when = editable, extra: Partial<CommandOptions> = {}) =>
        cmd(id, title, category, run, keybinding, when, { ai: true, ...extra });
      const hasSelection = () => open() && !e.collapsed();
      const mod = (k: string) => formatChord(k, fe.extensions.mac);

      cmd("edit.undo", t("Undo"), t("Edit"), () => e.undo(), "Mod+Z");
      cmd("edit.redo", t("Redo"), t("Edit"), () => e.redo(), ["Mod+Shift+Z", "Mod+Y"]);
      // The browser does cut and copy itself (their shortcuts stay native); the menu items ask it to, and paste
      // reads the clipboard when the browser allows. Select all and the start and end of the document are set from
      // the model (the browser would stop at the blocks it has laid out); text fields keep the browser's own.
      cmd("edit.cut", t("Cut"), t("Edit"), () => document.execCommand("cut"), undefined, () => editable() && !e.collapsed(), { shortcutHint: "Mod+X" });
      cmd("edit.copy", t("Copy"), t("Edit"), () => document.execCommand("copy"), undefined, hasSelection, { shortcutHint: "Mod+C" });
      cmd("edit.paste", t("Paste"), t("Edit"), () => pasteFromClipboard(e, false, mod("Mod+V")), undefined, editable, { shortcutHint: "Mod+V" });
      cmd("edit.pastePlain", t("Paste as plain text"), t("Edit"), () => pasteFromClipboard(e, true, mod("Mod+Shift+V")), undefined, editable, { shortcutHint: "Mod+Shift+V" });
      cmd("edit.selectAll", t("Select all"), t("Edit"), () => e.selectAll(), "Mod+A", open);
      const mac = fe.extensions.mac;
      cmd("edit.documentStart", t("Go to start of document"), t("Edit"), () => e.moveToBoundary(false), mac ? ["Mod+Up", "Mod+Home"] : "Mod+Home", open);
      cmd("edit.documentEnd", t("Go to end of document"), t("Edit"), () => e.moveToBoundary(true), mac ? ["Mod+Down", "Mod+End"] : "Mod+End", open);
      cmd("edit.selectToDocumentStart", t("Select to start of document"), t("Edit"), () => e.moveToBoundary(false, true), mac ? ["Mod+Shift+Up", "Mod+Shift+Home"] : "Mod+Shift+Home", open);
      cmd("edit.selectToDocumentEnd", t("Select to end of document"), t("Edit"), () => e.moveToBoundary(true, true), mac ? ["Mod+Shift+Down", "Mod+Shift+End"] : "Mod+Shift+End", open);
      // Needs text selected within one paragraph; the matches are selected next to it, and formatting, deleting, typing and copying then apply to all of them.
      cmd("edit.selectAllMatching", t("Select all matching text"), t("Edit"), () => {
        const n = e.selectAllMatching();
        if (n) toast(tn(n, "{n} match selected", "{n} matches selected"));
      }, "Mod+Alt+A", () => {
        const r = open() && !e.story && !e.collapsed() ? e.range() : null;
        return !!r && r[0].p === r[1].p;
      });
      cmd("edit.delete", t("Delete"), t("Edit"), () => e.deleteSelection(), undefined, () => editable() && !e.collapsed());
      // Markdown in and out through the clipboard, for chat apps, issue trackers and notes.
      cmd("edit.copyMarkdown", t("Copy as Markdown"), t("Edit"), () => copyAsMarkdown(e), undefined, hasSelection);
      cmd("edit.pasteMarkdown", t("Paste from Markdown"), t("Edit"), () => pasteFromMarkdown(e, mod("Mod+V")));
      aiCmd("format.copyFormat", t("Copy formatting"), t("Format"), () => e.copyFormat(), "Mod+Alt+C", open);
      aiCmd("format.pasteFormat", t("Paste formatting"), t("Format"), () => e.pasteFormat(), "Mod+Alt+V", () => editable() && e.hasCopiedFormat);

      aiCmd("format.bold", t("Bold"), t("Format"), () => e.toggleMark("bold"), "Mod+B");
      aiCmd("format.italic", t("Italic"), t("Format"), () => e.toggleMark("italic"), "Mod+I");
      aiCmd("format.underline", t("Underline"), t("Format"), () => e.toggleMark("underline"), "Mod+U");
      aiCmd("format.strike", t("Strikethrough"), t("Format"), () => e.toggleMark("strike"), "Mod+Shift+X");
      aiCmd("format.superscript", t("Superscript"), t("Format"), () => e.toggleVertAlign("superscript"), "Mod+.", editable, { toggled: () => e.formatWithPending()?.run.vertAlign === "superscript" });
      aiCmd("format.subscript", t("Subscript"), t("Format"), () => e.toggleVertAlign("subscript"), "Mod+,", editable, { toggled: () => e.formatWithPending()?.run.vertAlign === "subscript" });
      aiCmd("format.smallCaps", t("Small caps"), t("Format"), () => e.toggleMark("smallCaps"), "Alt+Shift+K", editable, { toggled: () => !!e.formatWithPending()?.run.smallCaps });
      aiCmd("format.allCaps", t("All caps"), t("Format"), () => e.toggleMark("caps"), undefined, editable, { toggled: () => !!e.formatWithPending()?.run.caps });
      aiCmd("format.sizeUp", t("Increase font size"), t("Format"), () => e.stepFontSize(1), "Mod+Shift+.");
      aiCmd("format.sizeDown", t("Decrease font size"), t("Format"), () => e.stepFontSize(-1), "Mod+Shift+,");
      aiCmd("format.clear", t("Clear formatting"), t("Format"), () => e.clearFormatting(), "Mod+\\");
      aiCmd("format.normal", t("Normal text"), t("Format"), () => e.setStyle("Normal"), "Mod+Alt+0");
      aiCmd("format.title", t("Title"), t("Format"), () => e.setStyle("Title"));
      aiCmd("format.subtitle", t("Subtitle"), t("Format"), () => e.setStyle("Subtitle"));
      for (const n of [1, 2, 3, 4, 5, 6]) aiCmd(`format.heading${n}`, t("Heading {n}", { n }), t("Format"), () => e.setStyle(`Heading ${n}`), `Mod+Alt+${n}`);
      aiCmd("format.quote", t("Quote"), t("Format"), () => e.setStyle("Quote"));
      aiCmd("format.alignLeft", t("Align left"), t("Format"), () => e.setAlign("left"), "Mod+Shift+L");
      aiCmd("format.alignCenter", t("Center"), t("Format"), () => e.setAlign("center"), "Mod+Shift+E");
      aiCmd("format.alignRight", t("Align right"), t("Format"), () => e.setAlign("right"), "Mod+Shift+R");
      aiCmd("format.justify", t("Justify"), t("Format"), () => e.setAlign("both"), "Mod+Shift+J");
      aiCmd("format.bullets", t("Bulleted list"), t("Format"), () => e.toggleList("bullet"), "Mod+Shift+8");
      aiCmd("format.numbers", t("Numbered list"), t("Format"), () => e.toggleList("decimal"), "Mod+Shift+7");
      aiCmd("format.checklist", t("Checklist"), t("Format"), () => e.toggleChecklist(), "Mod+Shift+9");
      // The galleries and the menus pick a style by its preset id. A menu line can't pass an
      // argument and show a check mark for its own style, so each preset is also a command of its own.
      ctx.commands.register("format.listStyle", (style) => typeof style === "string" && e.setListStyle(style), { title: t("List style"), category: t("Format"), when: editable, palette: false });
      for (const p of LIST_PRESETS)
        cmd(`format.listStyle.${p.id}`, t(p.title), t("Format"), () => e.setListStyle(p.id), undefined, editable, { palette: false, toggled: () => e.format()?.listStyle === p.id });
      const numbered = () => editable() && e.format()?.list === "decimal";
      cmd("format.restartNumbering", t("Restart numbering"), t("Format"), () => e.restartNumbering(), undefined, numbered);
      cmd("format.continueNumbering", t("Continue previous numbering"), t("Format"), () => e.continueNumbering(), undefined, numbered);
      cmd("format.toggleChecked", t("Done"), t("Format"), () => e.setChecked(), "Mod+Alt+Enter", () => editable() && e.format()?.list === "checklist", { toggled: () => !!e.format()?.checked });
      aiCmd("format.indent", t("Increase indent"), t("Format"), () => e.indent(1), "Mod+]");
      const line = () => {
        const p = e.format()?.para;
        return p?.line && (p.lineRule ?? "auto") === "auto" ? p.line / 240 : 1;
      };
      // The multiples are numbers in the reader's own format ("1,15" in Spanish).
      const number = (() => {
        try {
          return new Intl.NumberFormat(currentLocale());
        } catch {
          return new Intl.NumberFormat();
        }
      })();
      for (const [m, title] of [
        [1, t("Single")],
        [1.15, number.format(1.15)],
        [1.5, number.format(1.5)],
        [2, t("Double")],
      ] as const)
        aiCmd(`format.line${String(m).replace(".", "_")}`, title, t("Format"), () => e.setPara({ lineMultiple: m }), undefined, editable, { toggled: () => Math.abs(line() - m) < 0.02 });
      const before = () => (e.format()?.para.spaceBefore ?? 0) > 0;
      const after = () => (e.format()?.para.spaceAfter ?? 0) > 0;
      aiCmd("format.spaceBefore", t("Space before paragraph"), t("Format"), () => e.setPara({ spaceBefore: before() ? 0 : 240 }), undefined, editable, { toggled: before });
      aiCmd("format.spaceAfter", t("Space after paragraph"), t("Format"), () => e.setPara({ spaceAfter: after() ? 0 : 160 }), undefined, editable, { toggled: after });
      aiCmd("format.outdent", t("Decrease indent"), t("Format"), () => e.indent(-1), "Mod+[");
      // Where a paragraph may fall on the page. Absent widow control counts as on, as it does in Word.
      const para = () => e.format()?.para;
      aiCmd("format.keepWithNext", t("Keep with next"), t("Format"), () => e.setPara({ keepNext: !para()?.keepNext }), undefined, editable, { toggled: () => !!para()?.keepNext });
      aiCmd("format.keepLinesTogether", t("Keep lines together"), t("Format"), () => e.setPara({ keepLines: !para()?.keepLines }), undefined, editable, { toggled: () => !!para()?.keepLines });
      aiCmd("format.preventSingleLines", t("Prevent single lines"), t("Format"), () => e.setPara({ widowControl: para()?.widowControl === false }), undefined, editable, { toggled: () => open() && para()?.widowControl !== false });
      aiCmd("format.pageBreakBefore", t("Add page break before"), t("Format"), () => e.setPara({ pageBreakBefore: !para()?.pageBreakBefore }), undefined, editable, { toggled: () => !!para()?.pageBreakBefore });
      cmd("format.bordersShading", t("Borders and shading…"), t("Format"), () => openBordersShading(e));
      // Columns apply to the caret's section (to the whole document when it has one section); More options also offers the whole document.
      const columns = () => (open() ? e.caretColumns().num : 1);
      const inBody = () => editable() && !e.story;
      for (const [n, title] of [[1, t("One column")], [2, t("Two columns")], [3, t("Three columns")]] as const)
        aiCmd(`format.columns${n}`, title, t("Format"), () => e.setColumns(n), undefined, inBody, { toggled: () => columns() === n });
      cmd("format.columnsMore", t("More column options…"), t("Format"), () => openColumns(e), undefined, inBody);

      if (openLink) cmd("insert.link", t("Link"), t("Insert"), openLink, "Mod+K");
      const picker = h("input", { type: "file", accept: "image/*", hidden: true });
      picker.addEventListener("change", () => {
        const f = picker.files?.[0];
        if (f) void e.insertImage(f);
        picker.value = "";
      });
      e.canvas.append(picker);
      ctx.subscriptions.push({ dispose: () => picker.remove() });
      cmd("insert.image", t("Image…"), t("Insert"), () => picker.click());
      // The last characters picked are remembered across documents and sessions.
      const recent = { get: () => ctx.state.get<string[]>("recentChars", []), set: (list: string[]) => ctx.state.set("recentChars", list) };
      // So are the colours picked lately in the colour menus.
      keepRecentColorsIn({ get: () => ctx.state.get<string[]>("recentColors", []), set: (list) => ctx.state.set("recentColors", list) });
      ctx.subscriptions.push({ dispose: () => keepRecentColorsIn(null) });
      cmd("insert.specialCharacters", t("Special characters…"), t("Insert"), () => void openSpecialCharacters(e, { recent }));
      cmd("insert.emoji", t("Emoji…"), t("Insert"), () => void openSpecialCharacters(e, { category: "emoji", recent }));
      aiCmd("insert.table", t("Table"), t("Insert"), () => e.insertTable(3, 3));
      aiCmd("insert.horizontalLine", t("Horizontal line"), t("Insert"), () => e.insertHorizontalLine());
      // A bookmark needs a name, so the assistant has its own tools for them.
      cmd("insert.bookmark", t("Add bookmark"), t("Insert"), () => addBookmark(e));
      cmd("insert.bookmarks", t("Bookmarks…"), t("Insert"), () => openBookmarks(e), undefined, open);
      // The Remove buttons of the bookmark popover and dialog: they name the bookmark.
      cmd("bookmark.remove", t("Remove bookmark"), t("Insert"), (name) => (typeof name === "string" ? e.removeBookmark(name) : false), undefined, editable, { palette: false });
      cmd("insert.watermark", t("Watermark…"), t("Insert"), () => openWatermark(e));
      // The Remove button of the watermark dialog (it asks first: see cautions.ts).
      cmd("watermark.remove", t("Remove watermark"), t("Insert"), () => {
        if (!e.exec({ op: "set_watermark", text: null })) return false;
        e.reloadPageSetup();
      }, undefined, editable, { palette: false });
      aiCmd("insert.pageBreak", t("Page break"), t("Insert"), () => e.insertPageBreak(), "Mod+Enter");
      aiCmd("insert.columnBreak", t("Column break"), t("Insert"), () => e.insertBreak("column"));
      // A section is a stretch of the document with its own page setup; it can't start inside a table or a header, footer or note.
      const canSection = () => editable() && !e.story && !e.format()?.inTable;
      aiCmd("insert.sectionNextPage", t("Section break (next page)"), t("Insert"), () => e.insertBreak("next_page"), undefined, canSection);
      aiCmd("insert.sectionContinuous", t("Section break (continuous)"), t("Insert"), () => e.insertBreak("continuous"), undefined, canSection);
      aiCmd("insert.sectionEvenPage", t("Section break (even page)"), t("Insert"), () => e.insertBreak("even_page"), undefined, canSection);
      aiCmd("insert.sectionOddPage", t("Section break (odd page)"), t("Insert"), () => e.insertBreak("odd_page"), undefined, canSection);
      aiCmd("insert.footnote", t("Footnote"), t("Insert"), () => e.insertFootnote(), "Mod+Alt+F");
      aiCmd("insert.endnote", t("Endnote"), t("Insert"), () => e.insertEndnote(), "Mod+Alt+D");
      cmd("insert.noteOptions", t("Footnote and endnote options…"), t("Insert"), () => openNoteOptions(e), undefined, editable);
      // Captions, cross-references and the index need choices, so each opens a dialog; the assistant has tools of its own for them.
      cmd("insert.caption", t("Caption…"), t("Insert"), () => openCaption(e), undefined, () => editable() && !e.story);
      cmd("insert.crossReference", t("Cross-reference…"), t("Insert"), () => openCrossReference(e), undefined, () => editable() && !e.story);
      cmd("insert.indexEntry", t("Mark index entry…"), t("Insert"), () => openIndexEntry(e), "Mod+Alt+Shift+X", () => editable() && !e.story);
      aiCmd("insert.index", t("Index"), t("Insert"), () => e.insertIndex(), undefined, () => editable() && !e.story);
      cmd("insert.tableOfFigures", t("Table of figures…"), t("Insert"), () => openTableOfFigures(e), undefined, () => editable() && !e.story);
      // F9 as in Word. Everything that is computed from the rest of the document: caption numbers, cross-references, page numbers, the contents, tables of figures and the index.
      aiCmd("edit.updateFields", t("Update fields"), t("Edit"), () => {
        const n = e.updateFields();
        toast(n ? tn(n, "Updated {n} field", "Updated {n} fields") : t("All fields are up to date"));
      }, "F9");
      aiCmd("insert.pageNumbers", t("Page numbers"), t("Insert"), () => e.addPageNumbers());
      cmd("insert.pageNumberOptions", t("Page number options…"), t("Insert"), () => openPageNumbers(e));
      aiCmd("insert.toc", t("Table of contents"), t("Insert"), () => e.insertToc());
      aiCmd("insert.updateToc", t("Update table of contents"), t("Insert"), () => e.updateToc(), undefined, () => editable() && e.hasToc());
      // A view without pages shows no headers or footers: Print layout first, as Word does.
      const editPart = (zone: "header" | "footer") => {
        if (e.continuous) fe.setView("print");
        e.enterStory(zone, Math.max(0, e.currentPage() - 1));
      };
      cmd("insert.header", t("Edit header"), t("Insert"), () => editPart("header"));
      cmd("insert.footer", t("Edit footer"), t("Insert"), () => editPart("footer"));
      // Only while a header or footer of a section after the first is being edited; checked while it shows the section before's.
      cmd("insert.linkToPrevious", t("Link to previous"), t("Insert"), () => toggleLinkToPrevious(e), undefined, () => editable() && !!storyLink(e)?.linkable, { toggled: () => !!storyLink(e)?.linked });

      const inTable = () => editable() && !!e.format()?.inTable;
      const table: [string, string, string][] = [
        ["rowAbove", t("Insert row above"), "row_above"],
        ["rowBelow", t("Insert row below"), "row_below"],
        ["columnLeft", t("Insert column left"), "col_left"],
        ["columnRight", t("Insert column right"), "col_right"],
        ["deleteRow", t("Delete row"), "delete_row"],
        ["deleteColumn", t("Delete column"), "delete_col"],
        ["deleteTable", t("Delete table"), "delete_table"],
      ];
      for (const [id, title, action] of table) aiCmd(`table.${id}`, title, t("Table"), () => e.tableAction(action), undefined, inTable);
      aiCmd("table.merge", t("Merge cells"), t("Table"), () => tables.merge(), undefined, () => inTable() && tables.canMerge());
      aiCmd("table.split", t("Unmerge cells"), t("Table"), () => tables.split(), undefined, () => inTable() && tables.canSplit());
      aiCmd("table.distributeColumns", t("Distribute columns"), t("Table"), () => tables.distributeColumns(), undefined, inTable);
      aiCmd("table.headerRow", t("Repeat header row on each page"), t("Table"), () => tables.toggleHeaderRow(), undefined, inTable, { toggled: () => inTable() && tables.headerRow() });
      const onImage = () => editable() && images.selected;
      cmd("image.altText", t("Alt text…"), t("Image"), () => images.editAlt(), "Mod+Alt+Y", onImage);
      cmd("image.replace", t("Replace image…"), t("Image"), () => images.replace(), undefined, () => onImage() && !images.isChart);
      // Select an image by where it is (the accessibility check goes to images this way), so its frame and strip show.
      ctx.commands.register("image.select", (p, o) => typeof p === "number" && typeof o === "number" && images.selectAt(p, o), { title: t("Select image"), category: t("Image"), when: open, palette: false });

      // Text boxes and shapes. A text box needs no input, so the assistant may insert one; the shape picker is a choice, so the
      // assistant has its own tool for shapes.
      const inBodyText = () => editable() && !e.story;
      aiCmd("insert.textBox", t("Text box"), t("Insert"), () => shapes.insertTextBox(), undefined, inBodyText);
      cmd("insert.shape", t("Shape"), t("Insert"), () => shapes.openPicker(), undefined, inBodyText);
      const onShape = () => editable() && shapes.selected;
      cmd("shape.editText", t("Edit text"), t("Shape"), () => shapes.enterText(), undefined, () => onShape() && shapes.hasText && !shapes.typing);
      cmd("shape.fill", t("Fill…"), t("Shape"), () => shapes.pickFill(), undefined, onShape);
      cmd("shape.outline", t("Outline…"), t("Shape"), () => shapes.pickOutline(), undefined, onShape);
      for (const [id, title] of [
        ["inline", t("In line with text")],
        ["square", t("Text wraps around")],
        ["topBottom", t("Text above and below")],
        ["behind", t("Behind text")],
        ["front", t("In front of text")],
      ] as const)
        cmd(`shape.wrap.${id}`, title, t("Shape"), () => shapes.setWrap(id), undefined, onShape, { toggled: () => (shapes.wrap === "tight" ? id === "square" : shapes.wrap === id) });
      cmd("shape.altText", t("Alt text…"), t("Shape"), () => shapes.editAlt(), undefined, onShape);
      cmd("shape.delete", t("Delete shape"), t("Shape"), () => shapes.remove(), undefined, onShape);
      // Select a shape by where it is (the accessibility check goes to shapes this way), so its frame and strip show.
      ctx.commands.register("shape.select", (p, o) => typeof p === "number" && typeof o === "number" && shapes.selectAt(p, o), { title: t("Select shape"), category: t("Shape"), when: open, palette: false });

      // Zoom, the layouts and compact mode are in view.ts.
      cmd("view.ruler", t("Show ruler"), t("View"), () => {
        e.showRuler(!e.rulerShown);
        ctx.state.set("ruler", e.rulerShown);
      }, undefined, () => true, { toggled: () => e.rulerShown });
      e.showRuler(ctx.state.get("ruler", true));
      ctx.subscriptions.push({ dispose: () => e.showRuler(true) });
      // Marks for the characters that don't print: ¶, spaces, tabs and breaks. Alt-Mod-P (P for pilcrow): Word's Ctrl+Shift+8 is the bullets here.
      cmd("view.nonPrinting", t("Show non-printing characters"), t("View"), () => {
        e.showMarks(!e.marksShown);
        ctx.state.set("nonPrinting", e.marksShown);
      }, "Mod+Alt+P", () => true, { toggled: () => e.marksShown });
      e.showMarks(ctx.state.get("nonPrinting", false));
      ctx.subscriptions.push({ dispose: () => e.showMarks(false) });
      cmd("view.fullScreen", t("Full screen"), t("View"), () => (document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen()), undefined, () => !!document.fullscreenEnabled, { toggled: () => !!document.fullscreenElement });
      // Viewing is one reason to be read-only among others; Editing lets go of it and nothing else, so it never undoes the
      // window lock, a document marked as final or a view that locks the text.
      let releaseViewing: (() => void) | null = null;
      const whyReadOnly = (reason: string) =>
        reason === "window-lock"
          ? t("This document is open in another window, so it can't be edited here.")
          : reason === "marked as final"
            ? t("This document is marked as final. Use Edit anyway to change it.")
            : reason === "mail merge preview"
              ? t("A merged record is shown. Stop previewing the results to edit.")
              : reason.startsWith("view:") || reason === "line focus" || reason === "page grid"
                ? t("Leave this view to edit the document.")
                : t("This document can't be edited here.");
      cmd("view.editing", t("Editing"), t("View"), () => {
        releaseViewing?.();
        releaseViewing = null;
        const left = fe.lockReasons();
        if (left.length) toast(whyReadOnly(left[0]), 6000);
      }, undefined, () => open() && !fe.lockReasons().includes("window-lock"), { toggled: () => !fe.readOnly });
      cmd("view.viewing", t("Viewing"), t("View"), () => {
        if (!releaseViewing) releaseViewing = fe.lock("viewing");
      }, undefined, open, { toggled: () => fe.readOnly });
      ctx.subscriptions.push({ dispose: () => releaseViewing?.() });
      cmd("file.pageSetup", t("Page setup…"), t("File"), () => openPageSetup(e));
      cmd("file.language", t("Language…"), t("File"), () => openLanguage(e), undefined, open);
      aiCmd("format.portrait", t("Portrait"), t("Format"), () => setOrientation(e, false), undefined, editable, { toggled: () => !isLandscape(e) });
      aiCmd("format.landscape", t("Landscape"), t("Format"), () => setOrientation(e, true), undefined, editable, { toggled: () => isLandscape(e) });
      cmd("file.print", t("Print…"), t("File"), () => e.print(), "Mod+P", open);
      // Only the selected text, laid out on pages of its own (Word's Print selection).
      cmd("file.printSelection", t("Print selection"), t("File"), () => void e.printSelection(), undefined, () => open() && !e.story && !e.collapsed());
      cmd("tools.wordCount", t("Word count"), t("Tools"), () => openWordCount(e, { extras: ctx.state.get<boolean>("wordCountExtras", true), onExtras: (on) => ctx.state.set("wordCountExtras", on) }), "Mod+Shift+C", open);
      cmd("tools.lineNumbers", t("Line numbers…"), t("Tools"), () => openLineNumbers(e));
      aiCmd("format.suppressLineNumbers", t("Suppress line numbers"), t("Format"), () => e.setPara({ suppressLineNumbers: !para()?.suppressLineNumbers }), undefined, editable, { toggled: () => !!para()?.suppressLineNumbers });
      aiCmd("tools.hyphenation", t("Hyphenation"), t("Tools"), () => {
        e.exec({ op: "set_hyphenation", on: !e.pageInfo.hyphenation });
        e.reloadPageSetup();
      }, undefined, editable, { toggled: () => open() && !!e.pageInfo.hyphenation });

      // What a plain Paste does with the source's formatting: each person's choice, set under Tools, Preferences.
      e.pasteFlow.choice = normalizeChoice(ctx.state.get<{ mode?: string }>("paste", {}).mode);

      // The measuring unit: the language's own (centimetres outside the United States) unless a person chose one.
      const applyUnits = (mode: UnitChoice) => setUnitSystem(mode === "auto" ? localeUnitSystem() : mode);
      applyUnits(ctx.state.get<UnitChoice>("units", "auto"));

      // Typing substitutions: each person's choices are kept with the editor's other settings. A host that
      // embeds the editor with `autoformat: false` gets neither the substitutions nor the dialog.
      if (fe.autoformatEnabled) {
        const load = (): AutoformatSettings => normalizeSettings(ctx.state.get("autoformat", undefined));
        e.setAutoformat(load());
        ctx.subscriptions.push({ dispose: () => e.setAutoformat(null) });
        cmd("tools.preferences", t("Preferences…"), t("Tools"), () => {
          openPreferences({
            settings: load(),
            save: (settings) => {
              ctx.state.set("autoformat", settings);
              e.setAutoformat(settings);
            },
            onClose: () => open() && e.focus(),
            extra: fe.extensions.settings.list(),
            fonts: () => [...new Set([...MENU_FONTS, ...fe.extensions.fonts.list().map((f) => f.family)])],
            shortcuts: () => void ctx.commands.execute("tools.shortcuts"),
            paste: {
              mode: e.pasteFlow.choice,
              save: (mode) => {
                e.pasteFlow.choice = mode;
                ctx.state.set("paste", { mode });
              },
            },
            units: {
              mode: ctx.state.get<UnitChoice>("units", "auto"),
              save: (mode) => {
                ctx.state.set("units", mode);
                applyUnits(mode);
                // The rulers and the open dialogs' fields measure in the new unit.
                if (open()) e.reloadPageSetup();
              },
            },
          });
        }, undefined, () => true);
      }

      editorMenus(ctx, fe.autoformatEnabled);
    },
  });
}
