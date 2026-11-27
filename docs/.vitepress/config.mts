// The Docxtor documentation site: the Markdown in docs/ (plus CONTRIBUTING.md, PRIVACY.md and SECURITY.md at the root, which GitHub
// expects there), built into static files and served at https://docxtor.tinycommit.dev/docs/ on the same deploy as the app.
//   npm run docs:dev     the site with live reload
//   npm run docs:build   docs/.vitepress/dist (a dead link fails the build)
// The Markdown stays plain Markdown that GitHub shows too: no front matter, relative links to other .md files. The site is built from the
// repository root (srcDir "..") so every file keeps its real place and a relative link means the same thing on GitHub and here.
// A Markdown file in docs/ is a page at its own path; the sidebar below is the one hand-ordered list (scripts/docs-site.test.mjs fails when a
// page is missing from it or it names a file that does not exist). A topic that gets a second page becomes a folder with an index.md.
import { createHash } from "node:crypto";
import { existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitepress";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const REPO = "https://github.com/tinycommit-dev/docxtor";

/** The Markdown files of docs/ ("docs/use/index.md"), found by walking the folder: a file there is a page, never listed by hand. */
const walk = (dir: string): string[] =>
  readdirSync(join(root, dir)).sort().flatMap((name) => {
    if (name.startsWith(".") || name === "public") return [];
    const rel = `${dir}/${name}`;
    return statSync(join(root, rel)).isDirectory() ? walk(rel) : name.endsWith(".md") ? [rel] : [];
  });
/** The root files that GitHub expects at the top of the repository and the site shows as pages, with the address each gets. */
const ROOT_PAGES: Record<string, string> = { "PRIVACY.md": "privacy.md", "SECURITY.md": "security.md", "CONTRIBUTING.md": "contributing.md" };
/** The pages of the site: a source file (relative to the repository root) and the address it gets (lower case, no folder). */
const PAGES: Record<string, string> = {
  ...Object.fromEntries(walk("docs").map((file) => [file, file.slice("docs/".length)])),
  ...ROOT_PAGES,
};

/** Everything at the top of the repository except these is out of the site (a new folder or file never becomes a page by accident). */
const srcExclude = readdirSync(root)
  .filter((name) => name !== "docs" && !(name in PAGES))
  .map((name) => (statSync(join(root, name)).isDirectory() ? `${name}/**` : name));

const SOURCE_OF: Record<string, string> = Object.fromEntries(Object.entries(PAGES).map(([src, out]) => [out, src]));

/** The address of a page of the site (lower case, no .md, under the base). */
const addressOf = (file: string) => address(PAGES[file]);
/** "index.md" is the site's front page, "ai/index.md" is /ai/ (a page that has pages under it), anything else loses its .md. */
const address = (out: string) => (out === "index.md" ? "/" : out.endsWith("/index.md") ? `/${out.slice(0, -"index.md".length)}` : `/${out.replace(/\.md$/, "")}`);

function docxtorMarkdown(md: any) {
  // `{{x}}` in code is text, never a template: mark every inline code span as such (fenced blocks already are).
  const code = md.renderer.rules.code_inline;
  md.renderer.rules.code_inline = (...args: any[]) => code(...args).replace(/^<code/, "<code v-pre");
  // A relative link is resolved against the file where it is written (its real place in the repository, which is what GitHub does too).
  // A page of the site becomes the page's address; anything else (DESIGN.md, a source file, a folder) becomes a link to GitHub, so the
  // Markdown keeps the relative links that work on GitHub.
  md.core.ruler.push("docxtor_links", (state: any) => {
    const source = SOURCE_OF[state.env.relativePath] ?? state.env.relativePath;
    for (const block of state.tokens) {
      for (const tok of block.children ?? []) {
        if (tok.type !== "link_open") continue;
        const href = tok.attrGet("href");
        if (!href || /^([a-z][a-z0-9+.-]*:|#|\/)/i.test(href)) continue;
        const [path, hash] = href.split("#");
        const target = relative(root, resolve(root, dirname(source), decodeURIComponent(path))).split("\\").join("/");
        const tail = hash ? `#${hash}` : "";
        if (target in PAGES) tok.attrSet("href", `${addressOf(target)}${tail}`);
        else {
          const abs = join(root, target);
          const kind = existsSync(abs) && statSync(abs).isDirectory() ? "tree" : "blob";
          tok.attrSet("href", `${REPO}/${kind}/main/${target}${tail}`);
        }
      }
    }
  });
}

const item = (text: string, file: string) => ({ text, link: address(PAGES[file]) });
/** A page with pages under it: the overview is the group's own link, the others are its items. */
const group = (text: string, file: string, items: [string, string][]) => ({ text, link: address(PAGES[file]), collapsed: true, items: items.map(([t, f]) => item(t, f)) });

export default defineConfig({
  title: "Docxtor documentation",
  titleTemplate: ":title, Docxtor",
  description: "Docxtor is a free, open-source editor for Word (.docx) files in the browser. How to use it, embed it, host it and contribute to it.",
  lang: "en",
  base: "/docs/",
  srcDir: "..",
  // Where the site is written: docs/.vitepress/dist, unless a script says otherwise (scripts/add-docs.mjs puts it in web/dist/docs; the tests
  // each build their own copy, so two runs never write the same files).
  ...(process.env.DOCXTOR_DOCS_OUT ? { outDir: resolve(root, process.env.DOCXTOR_DOCS_OUT), cacheDir: resolve(root, "node_modules/.cache/docxtor-docs", createHash("sha1").update(process.env.DOCXTOR_DOCS_OUT).digest("hex").slice(0, 8)) } : {}),
  srcExclude,
  rewrites: PAGES,
  cleanUrls: true,
  lastUpdated: false,
  ignoreDeadLinks: [/^https?:\/\/localhost/],
  head: [["link", { rel: "icon", type: "image/svg+xml", href: "/favicon.svg" }]],
  markdown: { config: docxtorMarkdown, languageAlias: { caddyfile: "txt" }, theme: { light: "github-light", dark: "github-dark" } },
  themeConfig: {
    siteTitle: "Docxtor",
    nav: [
      { text: "Use Docxtor", link: "/use/" },
      { text: "Embed and extend", link: "/embedding" },
      { text: "Self-host", link: "/deploy" },
      { text: "Contribute", link: "/contributing" },
      { text: "Roadmap", link: "/roadmap" },
    ],
    sidebar: [
      {
        text: "Use Docxtor",
        items: [
          item("Start here", "docs/use/index.md"),
          item("Get started", "docs/use/getting-started.md"),
          item("Write and format", "docs/use/writing.md"),
          item("Tables, pictures and shapes", "docs/use/tables-pictures-shapes.md"),
          item("Notes, citations and contents", "docs/use/notes-citations-contents.md"),
          item("Check your writing", "docs/use/proofing.md"),
          item("Work with the assistant", "docs/use/assistant.md"),
          item("Save, download and share", "docs/use/saving-sharing.md"),
          item("Mail merge", "docs/use/mail-merge.md"),
          item("Shortcuts and the command palette", "docs/use/shortcuts.md"),
          item("Limits", "docs/use/limits.md"),
        ],
      },
      {
        text: "Embed and extend",
        items: [
          item("Embedding the editor", "docs/embedding.md"),
          group("Extensions", "docs/extensions/index.md", [
            ["Contribution points", "docs/extensions/contribution-points.md"],
            ["Files and review", "docs/extensions/files.md"],
            ["Design and content", "docs/extensions/document-look.md"],
            ["Mail merge, citations, fields", "docs/extensions/merge-citations-fields.md"],
            ["Language", "docs/extensions/language.md"],
          ]),
          group("AI, the CLI and MCP", "docs/ai/index.md", [
            ["Tools for text and layout", "docs/ai/tools-text.md"],
            ["Tools for parts and fields", "docs/ai/tools-parts.md"],
            ["Tools for content", "docs/ai/tools-content.md"],
            ["Raw ops", "docs/ai/tools-raw-ops.md"],
            ["Giving your users the AI", "docs/ai/your-users.md"],
          ]),
        ],
      },
      {
        text: "Self-host",
        items: [item("Putting Docxtor online", "docs/deploy.md"), item("Browsers", "docs/browsers.md"), item("Fidelity with Word", "docs/fidelity.md"), item("Privacy", "PRIVACY.md"), item("Security", "SECURITY.md")],
      },
      {
        text: "Contribute",
        items: [
          item("Contributing", "CONTRIBUTING.md"),
          group("Architecture", "docs/architecture/index.md", [
            ["How the engine works", "docs/architecture/engine-concepts.md"],
            ["Engine: files and the model", "docs/architecture/engine-files-and-model.md"],
            ["Engine: rendering and layout data", "docs/architecture/engine-rendering-and-layout.md"],
            ["Engine: content features", "docs/architecture/engine-content-features.md"],
            ["Engine: AI tools and fidelity", "docs/architecture/engine-ai-and-fidelity.md"],
            ["The editor package", "docs/architecture/editor.md"],
          ]),
          group("Performance", "docs/performance/index.md", [
            ["Profiling and comparing", "docs/performance/profiling.md"],
          ]),
        ],
      },
      item("Roadmap", "docs/roadmap.md"),
    ],
    socialLinks: [{ icon: "github", link: REPO }],
    search: { provider: "local" },
    // The pattern is a function the page runs in the browser, so it names the repository itself.
    editLink: { pattern: ({ filePath }: { filePath: string }) => `https://github.com/tinycommit-dev/docxtor/edit/main/${filePath}`, text: "Edit this page on GitHub" },
    outline: { level: [2, 3], label: "On this page" },
    docFooter: { prev: "Previous", next: "Next" },
    returnToTopLabel: "Back to top",
    sidebarMenuLabel: "Menu",
    darkModeSwitchLabel: "Theme",
    lightModeSwitchTitle: "Switch to the light theme",
    darkModeSwitchTitle: "Switch to the dark theme",
    footer: { message: "Docxtor is free software, Apache-2.0 or MIT.", copyright: `<a href="${REPO}">Source on GitHub</a>` },
  },
});
