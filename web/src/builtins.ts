// The app's own features as extensions, on the same API third parties use: the navigator's
// panels, the File/View/Help commands, and the document templates. The AI providers live in
// ai/providers.ts.
import { atMenuMatches, defineExtension, h, msg, t, type CommandOptions, type Extension, type FileRef, type FontContribution } from "@docxtor/editor";
import { routeHash, type Route } from "./route";
import { DOC_EXT, recents, type Recent } from "./files/fs";
import type { App } from "./main";
import { spellingProvider } from "./spelling";

// Zotero as a source for the Citations panel (see zotero.ts); main.ts adds it with the other app extensions.
export { zotero } from "./zotero";

/** Files and History in the navigator (the Navigation pane comes with the editor), and the Accessibility tool beside them once opened. */
export function navigator(app: App): Extension {
  return defineExtension({
    id: "docxtor.navigator",
    name: msg("Navigator"),
    description: msg("The Files and History views."),
    activate(ctx) {
      ctx.panels.register({ id: "files", title: t("Files"), order: 10, render: (el) => void el.append(app.files.el), onShow: () => void app.files.refresh() });
      ctx.panels.register({ id: "history", title: t("History"), order: 30, render: (el) => void el.append(app.history.el), onShow: () => void app.history.render() });
      ctx.panels.register({ id: "accessibility", title: t("Accessibility"), order: 40, tool: true, render: (el) => void el.append(app.a11y.el), onShow: () => app.a11y.update() });
      // The find bar's list of results, with each match and its context, lives here while the app is open.
      const results = h("div", { class: "find-dock" });
      app.fe.dockFindResults({ el: results, show: () => app.showPanel("find") });
      ctx.panels.register({ id: "find", title: t("Find in"), order: 50, tool: true, render: (el) => void el.append(results) });
    },
  });
}

/** Each app command's icon (ids from the editor's set, which icon packs can replace). */
const COMMAND_ICONS: Record<string, string> = {
  "file.new": "filePlus",
  "file.newFromTemplate": "template",
  "file.open": "open",
  "file.openFolder": "folderPlus",
  "file.home": "home",
  "file.save": "save",
  "file.saveAs": "saveAs",
  "file.makeCopy": "copy",
  "file.rename": "pencil",
  "file.details": "file",
  "file.pin": "bookmark",
  "file.trash": "trash",
  "file.recovered": "history",
  "file.protect": "lock",
  "file.history": "history",
  "file.autosave": "autosave",
  "tools.accessibility": "accessibility",
  "tools.compare": "compare",
  "tools.combine": "combine",
  "view.assistant": "caret",
  "view.navigator": "sidebar",
  "view.outline": "outline",
  "view.theme": "theme",
  "view.iconPack": "iconPack",
  "view.language": "globe",
  "help.extensions": "puzzle",
  "help.tour": "compass",
};

/** File, View, Extensions and Help commands, and where they sit in the menu bar. */
export function appCommands(app: App): Extension {
  return defineExtension({
    id: "docxtor.app",
    name: msg("App commands"),
    description: msg("New, open, save, download, the assistant, themes and extensions."),
    activate(ctx) {
      const doc = () => !!app.engine;
      const cmd = (id: string, title: string, category: string, run: (...args: unknown[]) => unknown, keybinding?: string | string[], when?: () => boolean, extra: Partial<CommandOptions> = {}) =>
        ctx.commands.register(id, run, { title, category, keybinding, when, icon: COMMAND_ICONS[id], ...extra });
      // File chips and the @ menu's Files: the recent documents and the Library, opened at their address in the app.
      ctx.files.register({
        id: "docxtor.app.files",
        label: t("Your documents"),
        search: async (q) => {
          const out: FileRef[] = [];
          const seen = new Set<string>([app.docKey() ?? ""]);
          for (const r of await recents()) {
            if (seen.has(r.key) || !atMenuMatches(q, r.name)) continue;
            seen.add(r.key);
            const route: Route = r.backend === "library" ? { kind: "library", path: r.path } : r.backend === "folder" ? { kind: "folder", folder: r.folder ?? "", path: r.path } : r.backend === "storage" ? { kind: "storage", provider: r.provider ?? "", path: r.path } : { kind: "file", name: r.name };
            // A full address: saved in the file, a `#…` alone would be read as a bookmark (by Word too).
            out.push({ title: r.name.replace(DOC_EXT, ""), url: `${location.origin}${location.pathname}${routeHash(route)}`, detail: app.recentPlace(r) });
          }
          return out.slice(0, 20);
        },
      });
      cmd("file.new", t("New document"), t("File"), () => app.createIn(app.files.library, ""), "Mod+Alt+N");
      cmd("file.newFromTemplate", t("New from a template…"), t("File"), () => app.newFromTemplate());
      cmd("file.open", t("Open…"), t("File"), () => app.openPicker(), "Mod+O");
      cmd("file.openFolder", t("Open a folder…"), t("File"), () => app.files.connectFolder().then(() => app.sidebar.show("files")), undefined, () => app.canUseFolders);
      cmd("file.save", t("Save"), t("File"), () => app.save(true), "Mod+S", doc);
      cmd("file.saveAs", t("Save as…"), t("File"), () => app.saveAs(), "Mod+Shift+S", () => doc() && app.canPickFiles);
      cmd("file.makeCopy", t("Make a copy"), t("File"), () => app.makeCopy(), undefined, () => app.canManageFile);
      cmd("file.rename", t("Rename…"), t("File"), () => app.renameDocument(), undefined, () => app.canManageFile);
      cmd("file.openRecent", t("Open recent"), t("File"), (key: unknown) => app.openRecentKey(String(key)), undefined, undefined, { palette: false });
      cmd("file.details", t("Document details…"), t("File"), () => app.showDetails(), undefined, doc);
      cmd("file.protect", t("Protect with password…"), t("File"), () => app.protect(), undefined, () => doc() && !!app.ref);
      // A pinned document is listed first on the start screen and in Open recent.
      cmd("file.pin", t("Pin to the start screen"), t("File"), () => app.togglePin(), undefined, () => doc() && !!app.ref, { toggled: () => app.pinned });
      cmd("file.trash", t("Trash"), t("File"), () => app.showTrash());
      cmd("file.recovered", t("Recovered documents…"), t("File"), () => app.showRecovered(), undefined, () => app.recoverable > 0);
      cmd("file.download", t("Download a copy…"), t("File"), () => app.downloadAs(), undefined, doc);
      // A backup of everything kept in this browser, as one .zip file.
      cmd("file.downloadAll", t("Download all documents"), t("File"), () => void app.downloadAll());
      cmd("file.home", t("Your documents"), t("File"), () => (location.hash = "#/"));
      cmd("file.export", t("Download as…"), t("File"), (id: unknown) => app.exportAs(String(id)), undefined, doc, { palette: false });
      cmd("file.downloadPdf", t("Download as PDF"), t("File"), () => app.exportAs("pdf"), undefined, () => doc() && !!app.fe.extensions.exporter("pdf"));
      cmd("file.history", t("Version history"), t("File"), () => app.showPanel("history"), "Mod+Alt+Shift+H", doc);
      cmd("file.autosave", t("Autosave files on disk"), t("File"), () => app.toggleAutosave(), undefined, undefined, { toggled: () => app.prefs.autosaveFiles });
      cmd("tools.compare", t("Compare documents…"), t("Tools"), () => app.compareDocuments(), undefined, doc);
      cmd("tools.combine", t("Combine documents…"), t("Tools"), () => app.compareDocuments("combine"), undefined, doc);
      cmd("view.newWindow", t("New window"), t("View"), () => app.newWindow(), undefined, doc);
      cmd("view.sideBySide", t("View side by side…"), t("View"), () => app.viewSideBySide(), undefined, doc, { toggled: () => app.side.on });
      cmd("view.syncScroll", t("Synchronous scrolling"), t("View"), () => app.side.setSync(!app.side.sync), undefined, () => app.side.on, { toggled: () => app.side.on && app.side.sync });
      cmd("view.resetWindow", t("Reset window position"), t("View"), () => app.side.reset(), undefined, () => app.side.on);
      cmd("tools.accessibility", t("Accessibility check"), t("Tools"), () => app.showPanel("accessibility"), undefined, doc);
      cmd("view.assistant", t("Assistant"), t("View"), () => app.ai.askAbout(), "Mod+/", doc, { toggled: () => app.ai.open });
      cmd("view.navigator", t("Show navigator"), t("View"), () => app.sidebar.toggle(), undefined, undefined, { toggled: () => app.sidebar.open });
      cmd("view.outline", t("Show navigation pane"), t("View"), () => app.showPanel("navigation"), undefined, doc);
      cmd("view.theme", t("Theme…"), t("View"), () => app.pickTheme());
      cmd("view.iconPack", t("Icons…"), t("View"), () => app.pickIconPack(), undefined, () => app.fe.iconPacks().length > 1);
      cmd("view.language", t("Display language…"), t("View"), () => app.pickLanguage(), undefined, () => app.fe.extensions.languages().length > 0);
      cmd("help.extensions", t("Manage extensions…"), t("Extensions"), () => app.manageExtensions());
      cmd("help.tour", t("Open the tour in a new tab"), t("Help"), () => app.sample());

      const item = (menu: string, group: string, command: string, title?: string) => ctx.menus.register({ menu, group, command, title });
      item("file", "1_new", "file.new");
      item("file", "1_new", "file.newFromTemplate");
      item("file", "2_open", "file.open");
      ctx.menus.register({ menu: "file", group: "2_open", submenu: "file/recent", title: t("Open recent") });
      item("file", "2_open", "file.openFolder");
      item("file", "2_open", "file.home");
      item("file", "3_save", "file.save");
      item("file", "3_save", "file.saveAs");
      item("file", "3_save", "file.makeCopy");
      item("file", "3_save", "file.rename");
      ctx.menus.register({ menu: "file", group: "4_download", submenu: "file/download", title: t("Download"), icon: "download" });
      item("file", "5_history", "file.history");
      item("file", "5_history", "file.autosave");
      item("file", "6_details", "file.details");
      item("file", "6_details", "file.protect");
      item("file", "6_details", "file.pin");
      item("file", "5_history", "file.trash");
      item("file", "5_history", "file.recovered");
      ctx.menus.register({ menu: "file/download", group: "9_all", command: "file.downloadAll" });
      item("tools", "3_review", "tools.compare");
      item("tools", "3_review", "tools.combine");
      item("tools", "3_review", "tools.accessibility");
      item("view", "2_panels", "view.navigator");
      item("view", "2_panels", "view.outline");
      item("view", "2_panels", "view.assistant");
      item("view", "7_appearance", "view.theme");
      item("view", "7_appearance", "view.iconPack");
      item("view", "7_appearance", "view.language");
      item("extensions", "1_manage", "help.extensions");
      item("help", "3_learn", "help.tour");

      // Open recent ▸ the last few documents (not the open one), following the list of recents.
      let recentItems: { dispose(): void }[] = [];
      let syncing = 0;
      const syncRecent = async () => {
        const turn = ++syncing;
        const open = app.docKey();
        const list: Recent[] = (await recents()).filter((r) => r.key !== open).slice(0, 8);
        if (turn !== syncing) return;
        const stems = list.map((r) => r.name.replace(DOC_EXT, ""));
        recentItems.forEach((d) => d.dispose());
        recentItems = list.map((r, i) => {
          const name = stems[i]!;
          const title = stems.filter((s) => s === name).length > 1 ? t("{name}, {place}", { name, place: app.recentPlace(r) }) : name;
          return ctx.menus.register({ menu: "file/recent", group: "1", command: "file.openRecent", args: [r.key], title });
        });
      };
      void syncRecent();
      ctx.subscriptions.push(app.onRecentsChange(() => void syncRecent()), {
        dispose: () => {
          syncing++;
          recentItems.forEach((d) => d.dispose());
        },
      });

      // Download ▸ one line per format, following the exporters that are turned on.
      let formats: { dispose(): void }[] = [];
      const syncFormats = () => {
        formats.forEach((d) => d.dispose());
        formats = app.fe.extensions.exporters.list().map((x) => ctx.menus.register({ menu: "file/download", group: "1", command: "file.export", args: [x.id], title: `${x.label} (.${x.extension})` }));
      };
      syncFormats();
      ctx.subscriptions.push(app.fe.extensions.exporters.onChange(syncFormats), { dispose: () => formats.forEach((d) => d.dispose()) });
    },
  });
}

const LETTER = msg(`[Your name]

[Street address], [City, postcode]

[Date]

[Recipient's name]

[Organisation]

[Street address], [City, postcode]

Dear [Recipient's name],

[Why you are writing, in a sentence or two.]

[The details. Keep each paragraph to one point.]

[What you would like to happen next.]

Yours sincerely,

[Your name]`);

const MEETING = msg(`#! Meeting notes

[Date], [Place or call link]

Attendees: [Names]

## Agenda

1. [Topic]
2. [Topic]

## Notes

[What was said, briefly.]

## Decisions

- [Decision]

## Action items

| Owner | Action | Due |
|---|---|---|
| [Name] | [What needs doing] | [Date] |`);

const REPORT = msg(`#! [Report title]

[Author], [Date]

## Summary

[Two or three sentences a busy reader can stop after.]

## Background

[What prompted this report.]

## Findings

[What you found, with the evidence.]

## Recommendations

1. [Recommendation]
2. [Recommendation]

## Appendix

[Supporting tables and data.]`);

const PAPER = msg(`#! [Title]

[Author], [Institution]

[Course or venue], [Date]

## Abstract

[One paragraph: the question, how you approached it, and what you found.]

## Introduction

[The question and why it matters.]

## Method

[How you went about answering it.]

## Results

[What you found.]

## Discussion

[What the results mean, and their limits.]

## References

1. [Author, title, year]`);

/** A few starting points; extensions can add their own. */
export const templates = defineExtension({
  id: "docxtor.templates",
  name: msg("Templates"),
  description: msg("Letter, meeting notes, report and paper."),
  activate(ctx) {
    const md = (id: string, name: string, description: string, markdown: string) =>
      ctx.templates.register({ id, name, description, fileName: `${name}.docx`, create: () => ({ markdown: t(markdown) }) });
    md("letter", t("Letter"), t("Addresses, date, greeting and sign-off"), LETTER);
    md("meeting", t("Meeting notes"), t("Agenda, notes, decisions and action items"), MEETING);
    md("report", t("Report"), t("Summary, findings and recommendations"), REPORT);
    md("paper", t("Paper"), t("Abstract to references, for coursework and research"), PAPER);
  },
});

/**
 * Lets the assistant run commands that opted in (`ai: true`): headings, lists, page numbers, and
 * any extension's commands, like a person using the command palette. It can select a paragraph,
 * or words in it, first.
 */
export const aiCommands = defineExtension({
  id: "docxtor.ai-commands",
  name: msg("Commands for the assistant"),
  description: msg("Lets the assistant run commands that need no further input, like headings, lists and page numbers."),
  activate(ctx) {
    const allowed = () => ctx.commands.list().filter((c) => c.ai);
    ctx.ai.tools.register({
      name: "run_command",
      // Read when a conversation starts, so it lists the commands available then.
      get description() {
        const list = allowed()
          .map((c) => `${c.id} (${c.title})`)
          .join(", ");
        return (
          "Run an editor command, the way a person would from the command palette. " +
          "Optionally select a paragraph (or some exact text in it) first; commands act on the selection. " +
          "Prefer the editing tools for text changes; use this for commands they don't cover. " +
          `Available commands: ${list || "none"}.`
        );
      },
      get input_schema() {
        return {
          type: "object",
          properties: {
            command: { type: "string", enum: allowed().map((c) => c.id) },
            paragraph: { type: "string", description: "Paragraph id to select first, e.g. p12" },
            text: { type: "string", description: "Exact text within that paragraph to select (default: the whole paragraph)" },
          },
          required: ["command"],
        };
      },
      async run(input) {
        const id = String(input.command ?? "");
        const cmd = allowed().find((c) => c.id === id);
        if (!cmd) throw new Error(`Unknown or unavailable command "${id}". Available: ${allowed().map((c) => c.id).join(", ")}`);
        if (input.paragraph !== undefined) {
          const p = Number(String(input.paragraph).replace(/^p/i, ""));
          const engine = ctx.document.engine;
          if (!engine || !Number.isFinite(p)) throw new Error(`No paragraph ${input.paragraph}`);
          const text = engine.textRange({ p, o: 0 }, { p, o: Number.MAX_SAFE_INTEGER });
          let from = 0;
          let to = text.length;
          if (typeof input.text === "string" && input.text) {
            from = text.indexOf(input.text);
            if (from < 0) throw new Error(`"${input.text}" isn't in ${input.paragraph}. Its text is: ${text.slice(0, 200)}`);
            to = from + input.text.length;
          }
          ctx.document.select({ p, o: from }, { p, o: to });
        }
        const ran = await ctx.commands.execute(id);
        if (ran.status === "declined") return `The person was asked and said no, so "${cmd.title}" did not run. Nothing changed.`;
        if (ran.status === "unavailable") throw new Error(`"${cmd.title}" is not available here. Nothing changed.`);
        return `Ran "${cmd.title}".`;
      },
    });
  },
});

const GOOGLE_FONTS: [string, FontContribution["category"]][] = [
  ["Lato", "sans"],
  ["Roboto", "sans"],
  ["Open Sans", "sans"],
  ["Montserrat", "sans"],
  ["Source Sans 3", "sans"],
  ["Noto Sans", "sans"],
  ["Nunito", "sans"],
  ["Work Sans", "sans"],
  ["Merriweather", "serif"],
  ["Lora", "serif"],
  ["Playfair Display", "serif"],
  ["EB Garamond", "serif"],
  ["Source Serif 4", "serif"],
  ["Noto Serif", "serif"],
  ["Libre Baskerville", "serif"],
  ["Crimson Pro", "serif"],
  ["Roboto Mono", "mono"],
  ["JetBrains Mono", "mono"],
  ["Oswald", "display"],
  ["Caveat", "handwriting"],
];

/**
 * Popular families from Google Fonts in the font menu. Each loads from Google only when a document
 * (or the menu's preview) uses it; turn the extension off to keep every request on your own servers.
 */
export const googleFonts = defineExtension({
  id: "docxtor.google-fonts",
  name: msg("Google Fonts"),
  description: msg("Twenty popular font families in the font menu, loaded from Google Fonts when used. Word shows its own fallback on computers without them."),
  activate(ctx) {
    for (const [family, category] of GOOGLE_FONTS) {
      ctx.fonts.register({
        family,
        category,
        stylesheet: `https://fonts.googleapis.com/css2?family=${family.replace(/ /g, "+")}:ital,wght@0,400;0,700;1,400;1,700&display=swap`,
      });
    }
  },
});

/**
 * The Hunspell dictionaries Docxtor's spelling checker uses in the app: English (US and UK), Spanish, French and
 * German, each fetched the first time a document or paragraph in that language is checked. They are an extension
 * like any other source of dictionaries (see web/src/spelling.ts): turn it off and spelling has none, and a
 * different extension can bring its own.
 */
export const spellingDictionaries = defineExtension({
  id: "docxtor.dictionaries",
  name: msg("Spelling dictionaries: English, Spanish, French, German"),
  description: msg("The word lists for Docxtor's spelling checker. Each loads the first time text in that language is checked."),
  activate(ctx) {
    ctx.spelling.dictionaries.register({
      id: "docxtor.dictionaries",
      label: msg("English, Spanish, French and German"),
      languages: ["en", "es", "fr", "de"],
      load: (lang) => spellingProvider.load(lang),
    });
  },
});
