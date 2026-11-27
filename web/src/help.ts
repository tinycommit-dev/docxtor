// Help: written help pages in the app (they work offline), what's new, and links to report a problem and to the source.
// Each is a command in the Help menu, so a self-hosted deployment or an extension can leave them out or add its own.
import { defineExtension, h, modal, msg, t, type Extension } from "@docxtor/editor";
import pkg from "../package.json";
import type { App } from "./main";

/**
 * The repository's address. A build can name its own (`VITE_DOCXTOR_REPO=https://github.com/owner/name npm run build`), or leave the
 * links out with an empty one (`VITE_DOCXTOR_REPO=`): then there is no Report a problem or Source code.
 */
export const REPO: string = ((import.meta.env.VITE_DOCXTOR_REPO as string | undefined) ?? "https://github.com/tinycommit-dev/docxtor").replace(/\/+$/, "");
/** The documentation's address, the same way (`VITE_DOCXTOR_DOCS`; empty leaves the link out). */
export const DOCS: string = (import.meta.env.VITE_DOCXTOR_DOCS as string | undefined) ?? "https://docxtor.tinycommit.dev/docs/";
export const VERSION: string = pkg.version;

interface Topic {
  id: string;
  title: string;
  body: string[];
}

const TOPICS: Topic[] = [
  {
    id: "start",
    title: msg("Getting started"),
    body: [
      msg("Open a Word file with File, Open, or drop it on the page. Docxtor shows it on real pages and saves it back the way Word wrote it."),
      msg("Documents in your Library save by themselves a moment after you stop typing. Documents on your computer save when you press the save key, or by themselves if you turn that on under Tools, Preferences."),
      msg("Press the key for Show all commands to search every command by name."),
    ],
  },
  {
    id: "documents",
    title: msg("Your documents"),
    body: [
      msg("The Library is kept in this browser. Open a folder to work on the Word files in a folder of your computer."),
      msg("Clearing this site's data in the browser deletes the Library. File, Download, Download all documents keeps a copy of every document in one .zip file."),
      msg("Pin a document to keep it first on the start screen. Documents you delete wait in the Trash for 30 days. If the page closes before changes are saved, Docxtor offers them back as recovered documents next time."),
      msg("Every document has its own address, so the back button and bookmarks work."),
    ],
  },
  {
    id: "templates",
    title: msg("Templates"),
    body: [
      msg("File, Save as template keeps the open document as a template. File, New from a template starts a document from one, from a template file on your computer, or from one that comes with Docxtor."),
      msg("A template is a Word template file (.dotx), so Word opens your templates too. Download as a Word template saves one as a file."),
    ],
  },
  {
    id: "word",
    title: msg("Working with Word"),
    body: [
      msg("Docxtor keeps everything it doesn't show, so a file you open and save again keeps its parts. The status bar says how much of the file came through unchanged."),
      msg("Document properties, tracked changes, comments, fields, styles and page setup are written the way Word writes them."),
    ],
  },
  {
    id: "assistant",
    title: msg("The assistant"),
    body: [
      msg("The assistant edits through the same operations you do, so every change is shown and can be undone. Connect a model with your own key under the assistant's settings."),
      msg("Your key stays in this browser and goes only to the provider you choose."),
    ],
  },
  {
    id: "offline",
    title: msg("Working offline"),
    body: [
      msg("After the first visit Docxtor works without a connection, with its own copy of the interface fonts. You can install it from the address bar or the browser's menu."),
      msg("Things that need the internet, such as the assistant, the dictionary, DOI lookups and LanguageTool, wait until you are back online."),
    ],
  },
  {
    id: "privacy",
    title: msg("Privacy"),
    body: [
      msg("Docxtor has no server of its own. Your documents, your settings and your assistant key stay in this browser, and nothing is sent anywhere when you open, edit or save a document."),
      msg("A few things ask another site, and only when you use them: a font from Google Fonts sends the font's name to Google when a document or the font menu shows it; the dictionary sends the word you look up to Wiktionary; a DOI lookup asks doi.org; Zotero asks your own Zotero; the assistant sends your request, with the parts of the document it needs, to the AI provider you connect."),
      msg("The Datamuse thesaurus and LanguageTool are off until you turn them on. Extensions, Manage extensions turns any of these on or off. Docxtor has no analytics and no tracking."),
    ],
  },
];

interface News {
  title: string;
  date: string;
  items: string[];
}

const NEWS: News[] = [
  {
    title: msg("Properties, templates, offline and more"),
    date: "2026-10-14",
    items: [
      msg("Document details lists the properties Word keeps: author, subject, keywords, category, company and your own custom properties."),
      msg("Save a document as a template and start new ones from it. Download as a Word template (.dotx)."),
      msg("Docxtor installs as an app and works offline."),
      msg("The Undo button has a list of what you did, so you can go back to any point."),
      msg("Print only the selection. Downloaded PDFs can be tagged for screen readers, or made as PDF/A for archives."),
      msg("Pin documents, restore deleted ones from the Trash, and get back changes that were not saved."),
      msg("Preferences has your default font, how soon documents save, exceptions for automatic capitals and your own keyboard shortcuts."),
    ],
  },
];

/** The browser and language, for a problem report. Nothing from the document goes in. */
function environment(): string {
  const ua = navigator.userAgent;
  const browser = /Edg\/([\d.]+)/.exec(ua)?.[0] ?? /Chrome\/([\d.]+)/.exec(ua)?.[0] ?? /Firefox\/([\d.]+)/.exec(ua)?.[0] ?? /Version\/([\d.]+).*Safari/.exec(ua)?.[0] ?? "";
  return `Docxtor ${VERSION}, ${browser || "an unknown browser"}, ${navigator.language}`;
}

/** The address that opens a new issue with a short form filled in. */
export function reportUrl(): string {
  const body = ["What happened:", "", "", "What you expected:", "", "", "Steps to see it:", "", "", `(${environment()})`].join("\n");
  return `${REPO}/issues/new?body=${encodeURIComponent(body)}`;
}

/** Open an address in a tab of its own. */
export function visit(url: string) {
  window.open(url, "_blank", "noopener,noreferrer");
}

function openTopics(app: App, first?: string) {
  const nav = h("div", { class: "help-nav", role: "tablist", "aria-label": t("Help topics") });
  const page = h("div", { class: "help-page", role: "tabpanel" });
  const buttons = new Map<string, HTMLButtonElement>();
  const show = (id: string) => {
    const topic = TOPICS.find((x) => x.id === id) ?? TOPICS[0]!;
    for (const [k, b] of buttons) {
      b.setAttribute("aria-selected", String(k === topic.id));
      b.classList.toggle("on", k === topic.id);
    }
    page.replaceChildren(h("h3", {}, t(topic.title)), ...topic.body.map((p) => h("p", {}, t(p))));
  };
  for (const topic of TOPICS) {
    const b = h("button", { class: "help-tab", type: "button", role: "tab" }, t(topic.title));
    b.addEventListener("click", () => show(topic.id));
    buttons.set(topic.id, b);
    nav.append(b);
  }
  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const links = h(
    "div",
    { class: "help-links" },
    ...(DOCS ? [h("button", { class: "btn ghost", type: "button", onclick: () => visit(DOCS) }, t("Documentation"))] : []),
    ...(REPO
      ? [
          h("button", { class: "btn ghost", type: "button", onclick: () => visit(reportUrl()) }, t("Report a problem")),
          h("button", { class: "btn ghost", type: "button", onclick: () => visit(REPO) }, t("Source code")),
        ]
      : []),
  );
  const dialog = h(
    "div",
    { class: "dialog help-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Help") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Help")), h("p", { class: "sub" }, t("Docxtor {version}", { version: VERSION }))),
    h("div", { class: "dialog-body help-body" }, nav, page),
    h("div", { class: "dialog-actions" }, links, h("span", { style: "flex:1" }), done),
  );
  const close = modal(dialog, () => app.engine && app.editor.focus());
  done.addEventListener("click", close);
  show(first ?? TOPICS[0]!.id);
  buttons.get(first ?? TOPICS[0]!.id)?.focus();
}

function openNews(app: App) {
  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const dialog = h(
    "div",
    { class: "dialog help-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("What's new") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("What's new")), h("p", { class: "sub" }, t("Docxtor {version}", { version: VERSION }))),
    h(
      "div",
      { class: "dialog-body help-news" },
      ...NEWS.flatMap((n) => [h("h3", {}, t(n.title)), h("ul", {}, ...n.items.map((i) => h("li", {}, t(i))))]),
    ),
    h("div", { class: "dialog-actions" }, done),
  );
  const close = modal(dialog, () => app.engine && app.editor.focus());
  done.addEventListener("click", close);
  done.focus();
}

export function helpLinks(app: App): Extension {
  return defineExtension({
    id: "docxtor.help",
    name: msg("Help"),
    description: msg("Help pages, what's new, and links to the documentation, to report a problem and to the source code."),
    activate(ctx) {
      const cmd = (id: string, title: string, run: () => void, extra: { keybinding?: string; icon?: string } = {}) => ctx.commands.register(id, run, { title, category: t("Help"), ...extra });
      cmd("help.topics", t("Help"), () => openTopics(app), { keybinding: "F1", icon: "book" });
      cmd("help.whatsNew", t("What's new"), () => openNews(app), { icon: "sparkle" });
      const item = (command: string) => ctx.menus.register({ menu: "help", group: "3_learn", command });
      item("help.topics");
      item("help.whatsNew");
      if (DOCS) {
        cmd("help.docs", t("Documentation"), () => visit(DOCS), { icon: "book" });
        ctx.menus.register({ menu: "help", group: "4_links", command: "help.docs" });
      }
      if (REPO) {
        cmd("help.report", t("Report a problem"), () => visit(reportUrl()), { icon: "alert" });
        cmd("help.source", t("Source code"), () => visit(REPO), { icon: "code" });
        ctx.menus.register({ menu: "help", group: "4_links", command: "help.report" });
        ctx.menus.register({ menu: "help", group: "4_links", command: "help.source" });
      }
    },
  });
}
