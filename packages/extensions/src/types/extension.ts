// The extension, its context and the paste converters
import type { AIProvider, AITool } from "./ai.ts";
import type { Caution, CommandsApi, DocumentApi, StateApi, UiApi } from "./api.ts";
import type { FieldType } from "./fields.ts";
import type { Exporter, Importer, Template, TemplateStore } from "./files.ts";
import type { DocumentComparer, Inspector } from "./inspect.ts";
import type { GrammarChecker, Hyphenator, LanguageDetector, ReadingProvider, SpellingDictionaryProvider, Thesaurus } from "./language.ts";
import type { ChartPaletteContribution, CitationStyleContribution, ContentControlContribution, DocumentThemeContribution, IconContribution, IconPack, SettingDefinition, StyleSetContribution, TableDesignContribution, ThemeContribution } from "./look.ts";
import type { EmailSender, LabelProduct, MailMergeDataSource, PictureSource } from "./mail.ts";
import type { FontContribution, SnippetStore, SourceLibrary, SourceProvider } from "./sources.ts";
import type { StorageProvider } from "./storage.ts";
import type { MenuItem, Panel, ToolbarItem } from "./ui.ts";
import type { LanguagePack } from "../l10n.ts";
import type { AtMenuProvider, DocBuildingBlock, DropdownPreset, FileProvider, LinkPreviewProvider, PeopleProvider } from "../smart.ts";
import type { ViewMode } from "../views.ts";
import type { Engine, Fragment } from "@docxtor/core";

/** Something to release: a registration, a listener, a subscription. */
export interface Disposable {
  dispose(): void;
}

export interface Extension {
  /** Unique and stable: `publisher.name`, e.g. `acme.citations`. Built-ins use `docxtor.*`. */
  id: string;
  /** Shown in the extensions list. */
  name: string;
  version?: string;
  description?: string;
  publisher?: string;
  /**
   * Contributions that are plain data, read when the extension is added and before anything runs.
   * Language packs live here so the interface can be built in the right language.
   */
  contributes?: { languages?: LanguagePack[] };
  /** Set to false for an extension that is installed but off until the person turns it on (Extensions dialog). Default true. */
  enabledByDefault?: boolean;
  /**
   * For an extension that sends the document's text somewhere: one plain sentence saying what goes where.
   * The Extensions dialog shows it, and asks before the extension is turned on.
   */
  privacy?: string;
  /**
   * Called once when the extension is enabled. Register contributions through `ctx`; everything
   * registered there is removed again when the extension is disabled.
   */
  activate(ctx: ExtensionContext): void | Promise<void>;
  /** Called when the extension is disabled, after its registrations are removed. */
  deactivate?(): void | Promise<void>;
}

/** Identity for typed extension definitions: `export default defineExtension({ … })`. */
export function defineExtension(ext: Extension): Extension {
  return ext;
}

export interface ExtensionContext {
  readonly extension: { readonly id: string; readonly name: string };
  /** The open document. */
  readonly document: DocumentApi;
  /** Small pieces of UI the host provides: notices, confirmations, pickers. */
  readonly ui: UiApi;
  /**
   * Ask before a risky action that isn't a command (a button in a dialog), by the id of its caution (see `cautions`):
   * true to go ahead. Commands with a caution are asked about by the host.
   */
  ask(id: string, ...args: unknown[]): Promise<boolean>;
  /** Settings and data the extension keeps between sessions (per extension). */
  readonly state: StateApi;
  readonly commands: CommandsApi;
  readonly toolbar: ContributionApi<ToolbarItem>;
  readonly menus: ContributionApi<MenuItem>;
  readonly panels: ContributionApi<Panel>;
  readonly exporters: ContributionApi<Exporter>;
  readonly importers: ContributionApi<Importer>;
  readonly templates: ContributionApi<Template> & {
    /** Places the person's own templates are kept (Save as template, New from a template). */
    readonly stores: ContributionApi<TemplateStore>;
  };
  /** Settings Preferences lists besides the editor's own: a toggle, a choice, a number, a text or a font. */
  readonly settings: ContributionApi<SettingDefinition>;
  readonly themes: ContributionApi<ThemeContribution>;
  readonly ai: {
    readonly providers: ContributionApi<AIProvider>;
    readonly tools: ContributionApi<AITool>;
  };
  readonly storage: ContributionApi<StorageProvider>;
  readonly fonts: ContributionApi<FontContribution>;
  readonly icons: ContributionApi<IconContribution>;
  readonly spelling: {
    /** Hunspell dictionaries for Docxtor's spelling checker. When several can serve a language, the latest registered wins. */
    readonly dictionaries: ContributionApi<SpellingDictionaryProvider>;
    /**
     * What language a paragraph is in, for Detect language automatically: a paragraph the file gives no language is
     * checked in the language found. Asked latest first; the first sure answer wins.
     */
    readonly languageDetectors: ContributionApi<LanguageDetector>;
  };
  /** Grammar and style checkers. Any number can be on at once; their findings are merged. */
  readonly grammar: ContributionApi<GrammarChecker>;
  /** Where words may be broken with a hyphen, for Hyphenate manually. The latest registered that has an answer for a language wins. */
  readonly hyphenation: ContributionApi<Hyphenator>;
  /** Readings of East Asian text, for the Phonetic guide box to suggest a guide. The latest registered that has an answer for a language wins. */
  readonly readings: ContributionApi<ReadingProvider>;
  /** Where synonyms come from, for Review, Thesaurus. Every thesaurus that has the word answers; the results are listed by source. */
  readonly thesaurus: ContributionApi<Thesaurus>;
  /** Checks for what a file carries besides what the page shows, for Review, Inspect document. Every inspector runs; each can offer to remove what it found. */
  readonly inspectors: ContributionApi<Inspector>;
  /** Ways to compare two versions of a document, for Review, Compare and Combine. The latest registered comparer that has what is needed wins. */
  readonly comparers: ContributionApi<DocumentComparer>;
  /** A named set of icons the person can choose between under View, Icons. */
  readonly iconPacks: ContributionApi<IconPack>;
  /** Field types: the names Insert, Field offers, with their options, and optionally how to work out their result. */
  readonly fields: ContributionApi<FieldType>;
  /** Table designs the Table design gallery offers besides Word's built-in ones (a company's table styles, say). */
  readonly tableDesigns: ContributionApi<TableDesignContribution>;
  /** Colour palettes the Edit chart dialog's Change colors gallery offers besides Word's own (a company's brand colours, say). */
  readonly chartPalettes: ContributionApi<ChartPaletteContribution>;  /** Kinds of content control: draw or fill in a control Docxtor doesn't know (a rating, a signature box), or take over one it does. */
  readonly contentControls: ContributionApi<ContentControlContribution>;
  /** Lines for the @ menu (typing @ in the text): people, dates, files, drop-downs, building blocks and inserts, or your own (a Jira issue, a GitHub pull request). */
  readonly atMenu: ContributionApi<AtMenuProvider>;
  /** Where the people in person chips come from. Docxtor's own is local; a directory or server accounts can be another. */
  readonly people: ContributionApi<PeopleProvider>;
  /** Where the documents in file chips come from (the app's files list, a drive, a wiki). */
  readonly files: ContributionApi<FileProvider>;
  /** Ready-made drop-down chips (Status, Project status, or your own). */
  readonly dropdowns: ContributionApi<DropdownPreset>;
  /** Docs-style building blocks (meeting notes, an email draft, a roadmap): Markdown with chip tokens. */
  readonly buildingBlocks: ContributionApi<DocBuildingBlock>;
  /** Looks a link up for its title and description when the person points at it. Needs the network, so it belongs in an extension that is off by default. */
  readonly linkPreviews: ContributionApi<LinkPreviewProvider>;
  /**
   * Warnings for risky actions (see `Caution`): the host asks before a command that has one, or offers Undo after it,
   * wherever the command runs from. Docxtor's own are all in one extension (`docxtor.cautions`).
   */
  readonly cautions: ContributionApi<Caution>;
  /**
   * Document themes for the Design menu: whole themes (colours and fonts), or colour schemes or font schemes on their own.
   * Docxtor's own (Office, Facet, Integral...) are registered the same way.
   */
  readonly documentThemes: ContributionApi<DocumentThemeContribution>;
  /** Style sets for the Design menu (Title, Heading 1 to 9, Normal and Quote written together), beside Docxtor's own. */
  readonly styleSets: ContributionApi<StyleSetContribution>;
  readonly clipboard: {
    /** Readers for formats on the clipboard (`text/html`, `text/rtf`, or one of your own). The first to return content wins. */
    readonly converters: ContributionApi<PasteConverter>;
  };
  readonly citations: {
    /** Citation styles the Citations panel offers. */
    readonly styles: ContributionApi<CitationStyleContribution>;
    /** Places the "Add a source" part of the Citations panel can bring sources from. */
    readonly sources: ContributionApi<SourceProvider>;
    /** Places a list of sources shared between documents is kept (Word's master list). */
    readonly libraries: ContributionApi<SourceLibrary>;
  };
  readonly snippets: {
    /** Places saved snippets (quick parts, AutoText) are kept. */
    readonly stores: ContributionApi<SnippetStore>;
  };
  readonly pictures: {
    /** Places the Insert, Image menu can bring pictures from. */
    readonly sources: ContributionApi<PictureSource>;
  };
  /** View modes under View, Layout: ways to show the document besides Docxtor's print layout, pageless, reading and outline. */
  readonly views: ContributionApi<ViewMode>;
  readonly mailMerge: {
    /** Places a list of recipients can come from (Mailings, Select recipients): a file, a spreadsheet service, a CRM. */
    readonly dataSources: ContributionApi<MailMergeDataSource>;
    /** Label sheets the Labels dialog lists besides Docxtor's own (a vendor's products). */
    readonly labelProducts: ContributionApi<LabelProduct>;
    /** Ways to send the merged messages (Mailings, Finish and merge, Send email messages): a file of messages, the person's own mail app, a mail service. */
    readonly mailSenders: ContributionApi<EmailSender>;
  };
  /** Everything here is disposed when the extension is disabled. Registrations add themselves. */
  readonly subscriptions: Disposable[];
}

// ---------------------------------------------------------------------------
// The clipboard
// ---------------------------------------------------------------------------

/** What a paste converter makes from a clipboard format. */
export type PasteContent =
  /** Runs, paragraph properties, styles, lists, tables, pictures and links (see `Engine.htmlToFragment`). Pastes with the person's choice of source or merged formatting. */
  | { fragment: Fragment }
  /** Markdown, which becomes headings, lists, tables and emphasis. */
  | { markdown: string }
  /** Plain text, kept as typed. */
  | { text: string };

/** What a converter is given. */
export interface PasteInput {
  /** The clipboard's data in the converter's format, as text. */
  data: string;
  /** Another format on the clipboard (the plain text next to the HTML, say); undefined when it isn't there. */
  get(type: string): string | undefined;
  /** Every format on the clipboard. */
  types: readonly string[];
  /** The engine, for turning HTML and RTF into content the way Docxtor does. */
  engine: Engine;
  /** Pictures the page fetched, by the address the markup names. A converter fills this for the ones it wants. */
  fetchImage(address: string): Promise<Blob | null>;
}

/** Reads one clipboard format when something is pasted. Docxtor's own readers (its fragment, HTML, RTF and plain text) are converters too. */
export interface PasteConverter {
  id: string;
  label: string;
  /** The format it reads: `text/html`, `text/rtf`, `application/x-docxtor-fragment`, or your own. */
  mime: string;
  /** Converters are tried from the highest priority down. Docxtor's own run from 100 (its fragment) to 10 (plain text). */
  priority?: number;
  /** Return null to leave the paste to the next converter. */
  convert(input: PasteInput): PasteContent | null | Promise<PasteContent | null>;
}

export interface ContributionApi<T> {
  /** Add a contribution; dispose the result to take it away again. */
  register(item: T): Disposable;
}

// ---------------------------------------------------------------------------
// The document
// ---------------------------------------------------------------------------

