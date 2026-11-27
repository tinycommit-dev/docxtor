// The editor's own features, written as extensions: the commands behind the toolbar, find and
// replace, the command palette, and the Word, Markdown and plain-text formats. They use the same
// API as third-party extensions; only their UI (the find bar, the link editor) is internal.
import type { Extension } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import type { DocxtorEditor } from "../docxtor-editor";
import type { SpellingProvider } from "../spelling/types";
import type { ContextSource } from "../ui/context-menu";
import type { FindBar } from "../ui/find";
import type { ImageTool } from "../ui/image-tool";
import type { InkTool } from "../ui/ink-tool";
import type { ShapeTool } from "../ui/shape-tool";
import type { TableTool } from "../ui/table-tool";
import { align } from "./align";import type { ControlsTool } from "../ui/controls";
import type { ProtectionTool } from "../ui/protection";
import { arrange } from "./arrange";
import { characterSettings } from "./character";
import { cautions } from "./cautions";
import { contents } from "./contents";
import { charts } from "./charts";
import { contentControls } from "./controls";
import { contextMenu } from "./context-menu";
import { design } from "./design";
import { editingCommands } from "./editing";
import { eastAsian } from "./east-asian";
import { equations } from "./equations";
import { fields } from "./fields";
import { epubFormat, htmlFormat, odtFormat, rtfFormat } from "./export-formats";
import { findAndReplace } from "./find";
import { formats } from "./formats";
import { openDoc, openHtml, openOdt, openRtf, openWordXml } from "./import-formats";
import { openNotice } from "./open-notice";
import { grammar, grammarRules } from "./grammar";
import { icons, lightIcons } from "./icons";
import { lists } from "./lists";
import { mailMerge } from "./mail-merge";
import { missingFonts } from "./missing-fonts";
import { pageLayout } from "./page-layout";
import { documentTabs } from "./document-tabs";
import { smartChips } from "./smart-chips";
import { authorities } from "./authorities";
import { pageParts } from "./page-parts";
import { paragraphSettings } from "./paragraph";
import { commandPalette } from "./palette";
import { pasting } from "./paste";
import { pdf } from "./pdf";
import { selectionPane } from "./selection-pane";
import { pictures } from "./pictures";
import { references } from "./citations";
import { ProofingOptions } from "../spelling/options";
import { proofing as proofingOptionsExtension } from "./proofing";
import { englishThesaurus, thesaurus } from "./thesaurus";
import { inspectDocument } from "./inspect";
import { compareDocuments } from "./compare";
import { geometry } from "./geometry";
import { shapeFormat } from "./shape-format";
import { spelling } from "./spelling";
import { styles } from "./styles";
import { tableDesign } from "./table-design";
import { tableTools } from "./table-tools";
import { shortcuts } from "./shortcuts";
import { templateFiles } from "./templates";
import { views } from "./view";
import { screenReader } from "./screen-reader";
import { viewWindow } from "./view-window";

export interface Internals {
  fe: DocxtorEditor;
  editor: Editor;
  findBar: FindBar;
  tables: TableTool;
  images: ImageTool;
  shapes: ShapeTool;
  ink: InkTool;
  /** Content controls and legacy form fields on the page. */
  controls: ControlsTool;
  /** Document protection: the strip, the editable ranges, Restrict editing. */
  protection: ProtectionTool;
  /** Open the link editor (null without a toolbar). */
  openLink: (() => void) | null;
  /** The host's assistant, if any. */
  ask: ((prompt?: string) => void) | null;
  /** Hide the selection bubble. */
  hideBubble: () => void;
  /** The dictionary provider the `spelling` option gave (a shortcut for registering one with `ctx.spelling.dictionaries`), or null. */
  spelling: SpellingProvider | null;
  /** Docxtor's own spelling and grammar are on (false when the `spelling` option is `false`). */
  proofing: boolean;
  /** Lines that built-in extensions add at the top of the right-click menu (the spelling suggestions). */
  contextSources: ContextSource[];
}

export function builtinExtensions(x: Internals): Extension[] {
  const options = new ProofingOptions();
  const proofing = x.proofing ? [spelling(x, options), grammar(x), grammarRules(x, options), proofingOptionsExtension(x, options)] : [];
  return [icons(), cautions(), lightIcons(), editingCommands(x), pasting(x), findAndReplace(x), commandPalette(x), contextMenu(x), missingFonts(x), ...proofing, references(x), authorities(x), contents(x), fields(x), equations(x), charts(x), arrange(x), align(x), selectionPane(x), shapeFormat(x), geometry(x), pictures(x), paragraphSettings(x), characterSettings(x), pageLayout(x), pageParts(x), tableDesign(x), contentControls(x), smartChips(x), eastAsian(x), tableTools(x), lists(x), styles(x), design(x), thesaurus(x), englishThesaurus(), inspectDocument(x), compareDocuments(), views(x), screenReader(x), documentTabs(x), viewWindow(x), mailMerge(x), formats(), pdf(x), odtFormat(), rtfFormat(), htmlFormat(), epubFormat(), templateFiles(x), shortcuts(x), openOdt(), openRtf(), openHtml(), openDoc(), openWordXml(), openNotice(x)];
}
