// The documentation site (docs/, built by VitePress into /docs/): the pages it builds and the policy that serves them.
//   - the build passes (a dead link fails it) and writes a page for every Markdown file of docs/ and for the three root files;
//   - the sidebar lists exactly those files (config.mts finds the pages by walking docs/; the sidebar is the hand-ordered list, and a page
//     missing from it or a link in it to nothing fails);
//   - every top-level module of docxtor-core is named on one of the engine pages of docs/architecture/;
//   - VitePress writes two small inline scripts into every page (the theme switch, the Mac key labels). The /docs/* policy in
//     deploy/_headers allows exactly those by hash, never 'unsafe-inline' for scripts: this test hashes every inline script of every page
//     and fails when the policy's list and the pages' differ (a VitePress update that changes a script, a page that adds one);
//   - no page has an inline event handler, a javascript: link or a script, style sheet or font from another address.
// The site is built into scripts/.docs-tmp/site (ignored by git), so it never touches docs/.vitepress/dist or web/dist.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { before, describe, test } from "node:test";
import { fileURLToPath } from "node:url";
import { effectiveHeaders, parseHeaders } from "./headers.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(root, "scripts/.docs-tmp/site");
/** Every page of the built site, as a path under the site's folder ("ai/index.html"): the long pages have pages in a folder beside them. */
const pages = (dir = "") =>
  readdirSync(join(OUT, dir), { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? (e.name === "assets" ? [] : pages(join(dir, e.name))) : e.name.endsWith(".html") ? [join(dir, e.name)] : []));
const read = (f) => readFileSync(join(OUT, f), "utf8");
/** The text of every script with no src, as the browser hashes it. */
const inlineScripts = (html) => [...html.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g)].map((m) => m[1]);
const hash = (text) => `'sha256-${createHash("sha256").update(text, "utf8").digest("base64")}'`;

before(() => {
  rmSync(OUT, { recursive: true, force: true });
  const r = spawnSync(process.execPath, [join(root, "node_modules/vitepress/bin/vitepress.js"), "build", "docs"], {
    cwd: root,
    encoding: "utf8",
    timeout: 180_000,
    env: { ...process.env, DOCXTOR_DOCS_OUT: "scripts/.docs-tmp/site" },
  });
  assert.equal(r.status, 0, `the documentation build failed (a dead link, or a page VitePress cannot compile):\n${r.stdout}\n${r.stderr}`);
});

describe("the documentation site", () => {
  test("every Markdown file of docs/ and the three root files has a page", () => {
    const built = new Set(pages());
    /** The Markdown files of docs/ and its folders (the hidden folder of the site and the public files are not pages). */
    const markdown = (dir) => readdirSync(join(root, dir), { withFileTypes: true }).flatMap((e) => (e.name.startsWith(".") || e.name === "public" ? [] : e.isDirectory() ? markdown(`${dir}/${e.name}`) : e.name.endsWith(".md") ? [`${dir}/${e.name}`] : []));
    for (const f of markdown("docs")) {
      const name = f.slice("docs/".length).replace(/\.md$/, "").toLowerCase();
      // A page with pages under it (docs/ai/index.md and docs/ai/) is the folder's index.
      const own = existsSync(join(root, "docs", name)) && !name.includes("/") ? `${name}/index.html` : `${name}.html`;
      assert.ok(built.has(f === "docs/index.md" ? "index.html" : own), `${f} has no page (every Markdown file of docs/ is one: move the file out of docs/ if it should not be public)`);
    }
    for (const f of ["CONTRIBUTING", "PRIVACY", "SECURITY"]) assert.ok(built.has(`${f.toLowerCase()}.html`), `${f}.md has no page`);
    assert.ok(!built.has("readme.html") && !built.has("changelog.html"), "the README and the changelog are not pages of the site");
    // Nothing from the private working folders, however it is named: whole path segments only (a page called footnotes or release-notes is fine).
    for (const f of built) for (const part of f.replace(/\.html$/, "").split("/")) assert.ok(!/^(?:handoff|remaining|notes)$/i.test(part), f);
  });

  test("the sidebar lists the pages of the site: the files of docs/ and the three root files, nothing else", () => {
    const config = readFileSync(join(root, "docs/.vitepress/config.mts"), "utf8");
    // item("Title", "docs/X.md"), group("Title", "docs/X.md", [...]) and the ["Title", "docs/x/y.md"] pairs inside a group.
    const listed = [...config.matchAll(/(?:item|group)\("[^"]+", "([^"]+)"|\["[^"]+", "(docs\/[^"]+\.md)"\]/g)].map((m) => m[1] ?? m[2]);
    assert.equal(new Set(listed).size, listed.length, "a page is in the sidebar twice");
    for (const f of listed) assert.ok(existsSync(join(root, f)), `the sidebar names ${f}, which does not exist`);
    const markdown = (dir) => readdirSync(join(root, dir), { withFileTypes: true }).flatMap((e) => (e.name.startsWith(".") || e.name === "public" ? [] : e.isDirectory() ? markdown(`${dir}/${e.name}`) : e.name.endsWith(".md") ? [`${dir}/${e.name}`] : []));
    // docs/index.md is the front page (the logo and the nav reach it), the others are reached through the sidebar.
    const pages = [...markdown("docs").filter((f) => f !== "docs/index.md"), "CONTRIBUTING.md", "PRIVACY.md", "SECURITY.md"];
    for (const f of pages) assert.ok(listed.includes(f), `${f} is a page of the site and is not in the sidebar (docs/.vitepress/config.mts)`);
    for (const f of listed) assert.ok(pages.includes(f), `the sidebar names ${f}, which is not a page of the site`);
  });

  test("every top-level module of docxtor-core is named on an engine page", () => {
    const src = join(root, "crates/docxtor-core/src");
    const modules = readdirSync(src, { withFileTypes: true }).map((e) => e.name.replace(/\.rs$/, ""));
    const pages = readdirSync(join(root, "docs/architecture")).filter((n) => /^engine-.*\.md$/.test(n) && n !== "engine-concepts.md");
    assert.ok(pages.length >= 4, "the engine module pages are in docs/architecture/");
    const text = pages.map((n) => readFileSync(join(root, "docs/architecture", n), "utf8")).join("\n");
    // A module is named as `name`, `name/` or the start of a path such as `name/part.rs`.
    const named = new Set([...text.matchAll(/`([^`]+)`/g)].flatMap((m) => [m[1], m[1].split(/[/.:]/)[0]]));
    const missing = modules.filter((m) => !named.has(m));
    assert.deepEqual(missing, [], `crates/docxtor-core/src has modules no engine page names: add them to a page of docs/architecture/engine-*.md`);
  });

  test("the /docs/* policy allows exactly the inline scripts the pages have, by hash", () => {
    const rules = parseHeaders(readFileSync(join(root, "deploy/_headers"), "utf8"));
    const csp = effectiveHeaders(rules, "/docs/*").find(([k]) => k === "Content-Security-Policy")?.[1] ?? "";
    assert.ok(csp, "/docs/* has a Content-Security-Policy");
    assert.ok(!csp.includes(", "), "one policy, not the app's joined to the docs' (a `! Content-Security-Policy` line comes first)");
    const script = /(?:^|; )script-src ([^;]*)/.exec(csp)?.[1].split(" ") ?? [];
    assert.ok(!script.includes("'unsafe-inline'") && !script.includes("'unsafe-eval'") && !script.includes("'wasm-unsafe-eval'"), "no unsafe-inline, unsafe-eval or the engine's allowance for scripts");
    assert.ok(!/https?:/.test(csp), "no outside address");
    const allowed = script.filter((s) => s.startsWith("'sha256-")).sort();
    const found = new Set();
    for (const f of [...pages()]) for (const s of inlineScripts(read(f))) found.add(hash(s));
    assert.ok(found.size >= 1);
    assert.deepEqual([...found].sort(), allowed, "the hashes in deploy/_headers (/docs/*) are not the hashes of the inline scripts of the built pages: put these in the policy, then run npm run headers");
    for (const f of pages()) for (const s of inlineScripts(read(f))) assert.ok(allowed.includes(hash(s)), `${f} has an inline script the policy does not allow`);
  });

  test("no page has an inline handler, a javascript: link or anything loaded from another address", () => {
    for (const f of pages()) {
      const html = read(f);
      assert.ok(!/\son[a-z]+\s*=\s*["']/i.test(html.replace(/<pre[\s\S]*?<\/pre>/g, "").replace(/<code[\s\S]*?<\/code>/g, "")), `${f} has an inline event handler`);
      assert.ok(!/href=["']\s*javascript:/i.test(html), `${f} has a javascript: link`);
      for (const m of html.matchAll(/<(?:script|link|img|source|iframe)\b[^>]*\b(?:src|href)=["'](https?:)?\/\/([^"']+)["']/g)) {
        // A link to the repository is a link, not something the page loads.
        if (m[0].startsWith("<link") && !/rel=["'](?:stylesheet|preload|modulepreload|icon)/.test(m[0])) continue;
        assert.fail(`${f} loads ${m[0].slice(0, 120)} from another address`);
      }
    }
    for (const f of readdirSync(join(OUT, "assets")).filter((n) => n.endsWith(".css"))) {
      const css = readFileSync(join(OUT, "assets", f), "utf8");
      assert.ok(!/@import\s+url\(["']?https?:|url\(["']?https?:\/\/(?!www\.w3\.org)/.test(css), `${f} loads something from another address`);
    }
  });

  test("the fonts are the site's own", () => {
    const fonts = readdirSync(join(OUT, "assets")).filter((n) => n.endsWith(".woff2"));
    assert.ok(fonts.some((n) => n.startsWith("inter-latin-wght")), "Inter is in assets/");
    const css = readdirSync(join(OUT, "assets")).filter((n) => n.endsWith(".css")).map((n) => readFileSync(join(OUT, "assets", n), "utf8")).join("\n");
    assert.ok(!/fonts\.(googleapis|gstatic)\.com/.test(css));
  });

  test("a page of Markdown that is for GitHub keeps no front matter and no private path", () => {
    const files = (dir) => readdirSync(join(root, "docs", dir), { withFileTypes: true }).flatMap((e) => (e.name.startsWith(".") || e.name === "public" ? [] : e.isDirectory() ? files(join(dir, e.name)) : e.name.endsWith(".md") ? [join(dir, e.name)] : []));
    for (const f of files("")) {
      const text = readFileSync(join(root, "docs", f), "utf8");
      assert.ok(!text.startsWith("---"), `docs/${f} starts with front matter, which GitHub shows as a table`);
      assert.ok(!/\]\([^)]*(?:notes\/|CLAUDE\.md|\.impeccable)/.test(text), `docs/${f} links to a private file`);
    }
  });
});
