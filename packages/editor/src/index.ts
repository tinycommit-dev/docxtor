// @docxtor/editor — Docxtor's document editor.
//
//   import { DocxtorEditor } from "@docxtor/editor";
//   import "@docxtor/editor/style.css";
//   const editor = new DocxtorEditor({ onSave: (bytes) => upload(bytes) }).mount(document.getElementById("editor")!);
//   await editor.open(await (await fetch("/report.docx")).arrayBuffer(), "report.docx");
//
// or, with no JavaScript framework at all: <docxtor-editor src="/report.docx"></docxtor-editor>
export { docxFrom, DocxtorEditor, type ExportedFile, type DocxtorEditorEvents, type DocxtorEditorOptions, type ThemeOption } from "./docxtor-editor";
export { DocxtorEditorElement } from "./element";
export { DEFAULT_FONTS_URL, ensureFonts, fontStatus } from "./fonts";
export { Editor, type EditorHooks, type EquationAtCaret, type StoryEdit, type ListKind } from "./editor/editor";
export { mathNode } from "./editor/math";
export type { PageBox } from "./editor/layout";
export { renderBlock, renderBlocks, tw, type RenderEnv } from "./editor/render";
export { domToModel, modelToDom, readSelection, writeSelection, type ModelSel } from "./editor/selection";
export type { SpellingDictionary, SpellingProvider } from "./spelling/types";
export { quickPick } from "./ui/quick-pick";
export { describeOpenError, describeOpenReport, formatLabel } from "./open-report";
export { openFailedDialog, openNoticeDialog } from "./ui/open-notice";
export { ZoomControl } from "./ui/zoom-control";
export { clampZoom, fitPageZoom, fitWidthZoom, pagesZoom, pinchZoom, stepZoom, wheelZoom, zoomLabel, ZOOM_MAX, ZOOM_MIN, ZOOM_PRESETS, type ZoomMode } from "./editor/zoom";
export { sourceAuthors, sourceDetail, sourceTitle } from "./ui/citations";
export { languageName } from "./ui/language";
export { StylesList, openManageStyles } from "./ui/styles-list";
export { RevealPanel } from "./ui/reveal-panel";
export { openStyleDialog } from "./ui/modify-style";
export { MENU_BAR, MenuBar, type MenuBarOptions } from "./ui/menu-bar";
export { applyTheme, applyTokens, coalesce, confirmDialog, escapeHtml, h, keepFocusOnFrame, modal, overlayLayer, popover, promptDialog, setOverlayTheme, svg, toast, type Theme } from "./ui/dom";
export { I } from "./ui/icons";
export * from "@docxtor/core";
export * from "@docxtor/extensions";
export { makeZip, type ZipEntry } from "./mailmerge/zip";
export { downloadBytes } from "./mailmerge/download";
