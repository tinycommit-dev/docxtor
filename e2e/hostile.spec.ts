// Hostile documents: script in every place text can hide (e2e/hostile-docx.ts builds the files). For each one:
//   - open it in the app: no script runs (a `window.__pwned` trap every payload sets, a dialog listener, a record of every node the
//     page adds that could run script), and no request leaves the page for any address but the app's own;
//   - open the panels and dialogs that show the document's names, details and notes, and do the same checks;
//   - click every link: nothing navigates to a script address;
//   - download it as a web page, EPUB, Markdown, PDF, OpenDocument text and Rich Text: the files carry no live script and
//     no address that loads something from outside.
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { EVIL_HOST, hostileDocx, hostileSet, type Hostile } from "./support/hostile-docx";
import { chooseMenu, dialog, documentReady, expect, test, type Page } from "./helpers";
import { partNames, partOf } from "./support/zip-read";

/** Everything that happened to the page while a hostile document was open. */
interface Trap {
  requests: string[];
  dialogs: string[];
  popups: string[];
}

/** Install the traps before the page loads: the script trap, and a record of risky nodes the page adds. */
async function armPage(page: Page): Promise<Trap> {
  const trap: Trap = { requests: [], dialogs: [], popups: [] };
  await page.addInitScript(() => {
    const w = window as unknown as { __pwned: string[]; __risky: string[] };
    w.__pwned = [];
    w.__risky = [];
    const BAD_URL = /^\s*(javascript|vbscript|data:text\/html|data:application\/xhtml)/i;
    const OUTSIDE = /^\s*(https?:|\/\/|file:|ftp:)/i;
    // `url(...)` or `@import` in CSS that points anywhere but a place in the page or an inline picture.
    const CSS_LOADS_RE = /@import|url\(\s*['"]?\s*(?!data:image|blob:|#|data:font)/i;
    // A quoted string in CSS is text, whatever it says (a font name `x; background: url(...)` in quotes loads nothing).
    // A picture the page draws itself (`url("data:image/svg+xml,...")`, what a pattern shading paints) loads nothing either.
    const OWN_PICTURE = /url\(\s*(?:"data:(?:image|font)[^"]*"|'data:(?:image|font)[^']*'|data:(?:image|font)[^)]*)\s*\)/gi;
    const CSS_LOADS = { test: (css: string) => CSS_LOADS_RE.test(css.replace(OWN_PICTURE, "url(#own)").replace(/'(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"/g, "''")) };
    const check = (el: Element) => {
      const tag = el.localName.toLowerCase();
      // The app's own scripts (the dev server adds some) are modules from its own origin.
      const ownScript = tag === "script" && el.getAttribute("type") === "module" && /^(\/|\.\/|http:\/\/localhost)/.test(el.getAttribute("src") ?? "x");
      if (["script", "iframe", "frame", "object", "embed", "applet", "base", "form"].includes(tag) && !ownScript) w.__risky.push(`<${tag}> added: ${(el.outerHTML || "").slice(0, 100)}`);
      if (tag === "meta" && /refresh/i.test(el.getAttribute("http-equiv") ?? "")) w.__risky.push("<meta refresh> added");
      if (tag === "link" && /stylesheet|import|preload|prefetch/i.test(el.getAttribute("rel") ?? "") && !/fonts\.googleapis\.com/.test(el.getAttribute("href") ?? "")) w.__risky.push(`<link ${el.getAttribute("rel")}> added: ${el.getAttribute("href")}`);
      if (tag === "foreignobject") w.__risky.push("<foreignObject> added");
      // (The dev server puts the app's own style sheets in <style data-vite-dev-id> elements.)
      if (tag === "style" && !el.hasAttribute("data-vite-dev-id") && CSS_LOADS.test(el.textContent ?? "")) w.__risky.push(`<style> loads from outside: ${(el.textContent ?? "").slice(0, 100)}`);
      for (const a of Array.from(el.attributes)) {
        if (/^on/i.test(a.name)) w.__risky.push(`<${tag} ${a.name}> added`);
        if (/^(href|src|xlink:href|action|formaction|data|poster|background|srcset)$/i.test(a.name)) {
          if (BAD_URL.test(a.value)) w.__risky.push(`<${tag} ${a.name}="${a.value.slice(0, 40)}"> added`);
          // Something the page loads by itself: not a link the reader follows.
          const loads = /^(src|srcset|poster|data|background)$/i.test(a.name) || (tag === "link" && a.name === "href") || ((tag === "use" || tag === "image") && /href/i.test(a.name));
          if (loads && OUTSIDE.test(a.value)) w.__risky.push(`<${tag} ${a.name}> loads ${a.value.slice(0, 80)}`);
        }
        if (a.name === "style" && CSS_LOADS.test(a.value)) w.__risky.push(`<${tag} style> loads from outside: ${a.value.slice(0, 700)}`);
      }
    };
    const walk = (n: Node) => {
      if (n.nodeType !== 1) return;
      check(n as Element);
      n.childNodes.forEach(walk);
    };
    new MutationObserver((list) => {
      for (const m of list) {
        if (m.type === "attributes") check(m.target as Element);
        m.addedNodes.forEach(walk);
      }
    }).observe(document, { subtree: true, childList: true, attributes: true });
  });
  page.on("dialog", (d) => {
    trap.dialogs.push(`${d.type()}: ${d.message()}`);
    void d.dismiss();
  });
  page.on("popup", (p) => {
    trap.popups.push(p.url());
    void p.close();
  });
  // Every request to an address that is not the app's own is recorded and refused (the fixture's own Google Fonts stand-in is
  // put aside: a document's names must never reach it, which the recorded list shows).
  const refuse = (route: import("@playwright/test").Route) => {
    const url = route.request().url();
    let own = false;
    try {
      const u = new URL(url);
      own = u.origin === `http://localhost:${process.env.DOCXTOR_E2E_PORT ?? 5180}` || u.protocol === "data:" || u.protocol === "blob:";
    } catch {
      /* not an address */
    }
    if (own) return route.fallback();
    trap.requests.push(`${route.request().method()} ${url}`);
    return route.abort();
  };
  await page.route("**/*", refuse);
  // A window the page opens is not this page: the same rule for it.
  await page.context().route("**/*", refuse);
  return trap;
}

/** Open the document in the app the way a person does: the page is up with another document, then this one replaces it. */
async function openHostile(page: Page, doc: Hostile) {
  await page.goto("/?open=simple.docx");
  await documentReady(page);
  await page.evaluate(async (b) => {
    const w = window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } };
    await w.docxtor.fe.open(new Uint8Array(b), "hostile.docx");
  }, Array.from(doc.bytes));
  await expect(page.locator(".flow .fp").first()).toContainText(doc.marker);
  await page.evaluate(() => document.fonts.ready);
}

/** What ran, what the page added that could run, and what was asked of the network. */
async function expectQuiet(page: Page, trap: Trap, where: string, followed: string[] = []) {
  const seen = await page.evaluate(() => ({ pwned: (window as unknown as { __pwned: string[] }).__pwned, risky: (window as unknown as { __risky: string[] }).__risky, url: location.href }));
  expect(seen.pwned, `${where}: script ran`).toEqual([]);
  expect(seen.risky, `${where}: the page added something that can run script or load from outside`).toEqual([]);
  expect(trap.dialogs, `${where}: a dialog opened`).toEqual([]);
  // A web address the test followed on purpose opens a window to it (aborted here): that one request is the person's own doing.
  const unexpected = trap.requests.filter((r) => !followed.some((f) => r === `GET ${f}`));
  expect(unexpected, `${where}: a request left the page`).toEqual([]);
}

/**
 * The colours slot puts a value that is not a colour in every place the engine reads one. The colour the file meant must not matter: the
 * shape has no fill, the text its default. So nothing of the value is on the page, whatever the page did with it.
 */
async function expectNoColourTraces(page: Page, doc: Hostile, where: string) {
  if (!doc.slots.includes("colours")) return;
  const found = await page.evaluate(() => {
    const html = document.documentElement.outerHTML;
    return ["evil.test/colours", "__pwned.push('colours:", "a;b:c)", "FF0000;background"].filter((m) => html.includes(m));
  });
  expect(found, `${where}: a value that is not a colour is on the page`).toEqual([]);
}

/** Open each surface that shows names, details and notes from a document. Each is looked for by what the person sees. */
async function visitSurfaces(page: Page, trap: Trap, doc: Hostile) {
  const visited: string[] = [];
  const settle = () => page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  // File, Document details
  await chooseMenu(page, "File", "Document details…");
  await expect(dialog(page, "Document details")).toBeVisible();
  visited.push("Document details");
  await settle();
  await expectQuiet(page, trap, "Document details");
  await page.keyboard.press("Escape");
  // The navigator's tabs
  for (const tab of ["Navigation", "History", "Files"]) {
    await page.locator(".sidebar").getByRole("tab", { name: tab, exact: true }).click();
    visited.push(tab);
    await settle();
  }
  await expectQuiet(page, trap, "the navigator");
  // The app's Google Fonts extension asks Google for the families it lists (a request this test refuses and PRIVACY.md describes); here it
  // is off, so what the panels draw is the document's own.
  await page.evaluate(() => (window as unknown as { docxtor: { fe: { extensions: { disable(id: string): Promise<void> } } } }).docxtor.fe.extensions.disable("docxtor.google-fonts"));
  // The panes and dialogs that list a document's styles, fonts, objects and fields, opened by their commands with the caret in the text
  await page.locator(".flow .fp").first().click();
  const commands: [string, () => Promise<unknown>][] = [
    ["format.stylesPane", () => expect(page.getByRole("tab", { name: /^Styles/ }).or(page.locator(".styles-panel, .sty-panel")).first()).toBeVisible()],
    ["format.manageStyles", () => expect(page.getByRole("dialog", { name: /styles/i }).first()).toBeVisible()],
    ["format.font", () => expect(page.getByRole("dialog", { name: "Font" }).first()).toBeVisible()],
    ["design.fonts", () => expect(page.getByRole("dialog").or(page.locator(".design-gallery")).first()).toBeVisible()],
    ["arrange.selectionPane", () => expect(page.locator(".selection-pane, .sel-pane, .sidebar").first()).toBeVisible()],
    ["tools.accessibility", () => expect(page.locator(".a11y-panel, .acc-panel, .sidebar").first()).toBeVisible()],
    ["insert.field", () => expect(page.getByRole("dialog", { name: /field/i }).first()).toBeVisible()],
    ["insert.property.title", async () => undefined],
    ["view.fieldCodes", async () => undefined],
    ["insert.toc", async () => undefined],
  ];
  for (const [id, shown] of commands) {
    const result = await page.evaluate(async (c) => {
      const w = window as unknown as { docxtor: { fe: { extensions: { execute(id: string): Promise<{ status: string }> } } } };
      return (await w.docxtor.fe.extensions.execute(c)).status;
    }, id);
    expect(result, `${id} ran`).toBe("done");
    await shown();
    visited.push(id);
    await settle();
    await expectQuiet(page, trap, id);
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
  }
  // The font menu in the toolbar, which lists the document's fonts
  await page.locator(".font-pick").first().click();
  await expect(page.locator(".menu-item").first()).toBeVisible();
  visited.push("font menu");
  await settle();
  await expectQuiet(page, trap, "the font menu");
  await page.keyboard.press("Escape");
  expect(visited.length).toBeGreaterThan(10);
  return visited;
}

/** Follow every link: the page stays where it is, no script runs, and the only window that opens is for a web or mail address. */
async function clickLinks(page: Page, trap: Trap, doc: Hostile): Promise<string[]> {
  const before = page.url();
  const spans = page.locator(".flow [data-href]");
  const n = await spans.count();
  expect(n, "the document has links to follow").toBeGreaterThan(0);
  const refused = /only opens web and email addresses/;
  for (let i = 0; i < n; i++) {
    const span = spans.nth(i);
    const href = (await span.getAttribute("data-href")) ?? "";
    await page.keyboard.press("Escape");
    await span.scrollIntoViewIfNeeded();
    await span.click({ modifiers: ["ControlOrMeta"], timeout: 5000 });
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    expect(page.url(), `link ${i} changed the page's address`).toBe(before);
    // The click reaches the editor's one place that opens addresses: a web address opens a window (refused here, and recorded), anything
    // else is refused with a notice.
    if (/^(https?|mailto):/i.test(href)) {
      if (/^https?:/i.test(href)) await expect.poll(() => trap.requests.includes(`GET ${href}`), { message: `a click on ${href} opens a window for it` }).toBe(true);
    } else if (!href.startsWith("#")) {
      await expect(page.getByText(refused).first(), `a click on ${href.slice(0, 40)} is refused with a notice`).toBeVisible();
    }
  }
  for (const p of trap.popups) expect(p, "a window opened for something that is not a web or mail address").not.toMatch(/^(javascript|vbscript|data|file|blob):/i);
  // The addresses that were web addresses are the ones a window may be opened for.
  return doc.links.map((l) => l.href).filter((h) => /^https?:\/\//i.test(h));
}

// ---------------------------------------------------------------------------------------------------------------- checkers

/** Live script and loads from outside in an HTML document (a web page, or a chapter of an EPUB), found by parsing it. */
async function scanHtml(page: Page, html: string, label: string): Promise<string[]> {
  return page.evaluate(
    ([source, where]) => {
      const out: string[] = [];
      const doc = new DOMParser().parseFromString(source, "text/html");
      const BAD_URL = /^[\s\u0000-\u001f]*(javascript|vbscript|data:text\/html|data:application\/xhtml|data:image\/svg)/i;
      const LOADS = /^(src|poster|data|background|srcset|xlink:href|href|action|formaction|manifest|ping|longdesc|cite|usemap)$/i;
      const all = Array.from(doc.querySelectorAll("*"));
      for (const el of all) {
        const tag = el.localName.toLowerCase();
        if (["script", "iframe", "frame", "frameset", "object", "embed", "applet", "base", "form", "foreignobject", "audio", "video", "source", "track", "portal", "dialog"].includes(tag)) out.push(`${where}: <${tag}>`);
        if (tag === "meta" && /refresh|set-cookie/i.test(el.getAttribute("http-equiv") ?? "")) out.push(`${where}: <meta http-equiv=${el.getAttribute("http-equiv")}>`);
        // A style sheet inside the book, by a relative address, is the book's own.
        if (tag === "link" && !(/^stylesheet$/i.test(el.getAttribute("rel") ?? "") && /^[^:/][^:]*$/.test(el.getAttribute("href") ?? ":"))) out.push(`${where}: <link rel=${el.getAttribute("rel")} href=${el.getAttribute("href")}>`);
        if ((tag === "use" || tag === "image") && Array.from(el.attributes).some((a) => /href$/i.test(a.name) && !/^\s*(#|data:image\/(png|jpe?g|gif|webp))/i.test(a.value))) out.push(`${where}: <${tag}> reaches outside the file`);
        for (const a of Array.from(el.attributes)) {
          if (/^on/i.test(a.name)) out.push(`${where}: <${tag} ${a.name}>`);
          if (LOADS.test(a.name) && BAD_URL.test(a.value) && !(tag === "img" && /^data:image\/svg/i.test(a.value))) out.push(`${where}: <${tag} ${a.name}="${a.value.slice(0, 50)}">`);
          // Something the page loads by itself (not a link the reader follows): a web or file address.
          if (/^(src|poster|data|background|srcset|manifest|ping)$/i.test(a.name) && /^[\s]*(https?:|\/\/|file:|ftp:)/i.test(a.value) && !(tag === "img" && a.value.includes("/external/linked-picture"))) out.push(`${where}: <${tag} ${a.name}> loads ${a.value.slice(0, 60)}`);
          if (/^style$/i.test(a.name) && /url\(\s*['"]?(?!data:)|expression\(|@import|javascript:/i.test(a.value)) out.push(`${where}: <${tag} style="${a.value.slice(0, 60)}">`);
        }
        if (tag === "style" && /url\(\s*['"]?(?!data:)|@import|expression\(|javascript:/i.test(el.textContent ?? "")) out.push(`${where}: <style> loads or runs something: ${(el.textContent ?? "").slice(0, 80)}`);
      }
      return out;
    },
    [html, label] as const,
  );
}

/** Markdown: no raw HTML that would run, and no link that is not a web, mail or phone address or a place in the document. */
function scanMarkdown(md: string): string[] {
  const out: string[] = [];
  // A backslash escape is text: what it escapes cannot start a tag or a link.
  const flat = md.replace(/\\[\s\S]/g, "__");
  // The exporter writes underline as <u>; any other tag is raw HTML a renderer may run.
  for (const m of flat.matchAll(/<\/?([a-zA-Z][\w:-]*)/g)) if (!["u"].includes(m[1].toLowerCase())) out.push(`raw HTML <${m[1]}> in Markdown`);
  for (const m of flat.matchAll(/!?\[[^\]]*\]\(\s*<?([^)\s>]*)/g)) {
    const target = m[1];
    if (/^(javascript|vbscript|data|file|blob):/i.test(target.trim())) out.push(`Markdown link or image to ${target.slice(0, 50)}`);
    if (m[0].startsWith("!") && /^(https?:)?\/\//i.test(target) && !target.includes("/external/linked-picture")) out.push(`Markdown image loads ${target.slice(0, 50)}`);
  }
  for (const m of flat.matchAll(/^\s*\[[^\]]+\]:\s*(\S+)/gm)) if (/^(javascript|vbscript|data|file|blob):/i.test(m[1])) out.push(`Markdown reference to ${m[1].slice(0, 50)}`);
  return out;
}

/** A PDF: no action that runs code, opens a file or sends a form, and links only to addresses a reader may follow. */
function scanPdf(bytes: Buffer): string[] {
  const out: string[] = [];
  // The objects' dictionaries and the trailer: the file without the bodies of its streams (page content, fonts, pictures).
  const all = bytes.toString("latin1").replace(/stream\r?\n[\s\S]*?endstream/g, "stream endstream");
  for (const key of ["/JavaScript", "/JS ", "/JS(", "/JS<", "/Launch", "/OpenAction", "/AA ", "/AA<", "/EmbeddedFile", "/SubmitForm", "/ImportData", "/GoToR", "/GoToE", "/RichMedia", "/Movie", "/Sound", "/XFA", "/AcroForm", "/Names <<"]) {
    if (all.includes(key)) out.push(`the PDF has ${key.trim()}`);
  }
  for (const m of all.matchAll(/\/URI\s*\(((?:\\.|[^)\\])*)\)/g)) if (!/^(https?:|mailto:|tel:|ftp:)/i.test(m[1])) out.push(`the PDF links to ${m[1].slice(0, 60)}`);
  return out;
}

async function download(page: Page, label: string): Promise<{ name: string; bytes: Buffer }> {
  const [file] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", label)]);
  return { name: file.suggestedFilename(), bytes: await readFile((await file.path())!) };
}

// A trap that catches nothing proves nothing: each one is shown to catch what it is for.
test("the traps catch script, a risky node, a dialog and a request to another address", async ({ page }) => {
  const trap = await armPage(page);
  await page.goto("/?open=simple.docx");
  await documentReady(page);
  await page.evaluate(() => {
    document.body.insertAdjacentHTML("beforeend", `<img src=x onerror="__pwned.push('control')"><div style="background:url(http://evil.test/control)"></div><a href="javascript:void(0)">x</a>`);
    setTimeout(() => alert("control"), 0);
    void fetch("http://evil.test/control").catch(() => undefined);
  });
  await expect.poll(() => trap.dialogs.length).toBeGreaterThan(0);
  await expect.poll(() => trap.requests.length).toBeGreaterThan(0);
  // the picture that cannot load says so a moment later, on a busy machine too
  await expect.poll(() => page.evaluate(() => (window as unknown as { __pwned: string[] }).__pwned.includes("control"))).toBe(true);
  const seen = await page.evaluate(() => ({ pwned: (window as unknown as { __pwned: string[] }).__pwned, risky: (window as unknown as { __risky: string[] }).__risky }));
  expect(seen.pwned).toContain("control");
  expect(seen.risky.join("\n")).toMatch(/onerror/);
  expect(seen.risky.join("\n")).toMatch(/style> loads from outside/);
  expect(seen.risky.join("\n")).toMatch(/javascript:void/);
  // and the scanners find what they are for
  expect(await scanHtml(page, `<p onclick="x()">a</p><script>1</script><a href="javascript:x()">l</a><img src="http://evil.test/p.png"><style>p{background:url(http://evil.test/x)}</style>`, "control")).toHaveLength(5);
  expect(scanMarkdown("a <script>x</script> [l](javascript:x) ![i](file:///etc/passwd)")).toHaveLength(4);
  expect(scanPdf(Buffer.from("%PDF-1.7\n1 0 obj\n<< /Type /Annot /A << /S /JavaScript /JS (app.alert(1)) >> >>\nendobj\ntrailer\n<< >>\n%%EOF"))).not.toEqual([]);
  expect(trap.requests.every((r) => r.includes(EVIL_HOST))).toBe(true);
  // and so does the check for a value that is not a colour, on a page that has one
  await page.evaluate(() => document.body.insertAdjacentHTML("beforeend", `<i data-colour="a;b:c)"></i>`));
  await expect(expectNoColourTraces(page, { slots: ["colours"] } as Hostile, "control")).rejects.toThrow();
  await page.unroute("**/*");
});

// DOCXTOR_HOSTILE_OUT=dir writes the files, to try them with the command-line program (`docxtor export x.docx --format html`).
test("the files can be written out for the command-line program", async () => {
  test.skip(!process.env.DOCXTOR_HOSTILE_OUT, "set DOCXTOR_HOSTILE_OUT=<dir> to write the hostile files");
  const dir = process.env.DOCXTOR_HOSTILE_OUT!;
  await mkdir(dir, { recursive: true });
  for (const { name, slots } of hostileSet()) await writeFile(join(dir, `${name}.docx`), hostileDocx(slots).bytes);
});

for (const { name, slots } of hostileSet()) {
  test.describe(`hostile document: ${name}`, () => {
    test.setTimeout(180_000);
    const doc = hostileDocx(slots);

    test("opening it runs nothing and asks the network for nothing", async ({ page }) => {
      const trap = await armPage(page);
      await openHostile(page, doc);
      await expectQuiet(page, trap, "opening");
      await expectNoColourTraces(page, doc, "opening");
      await visitSurfaces(page, trap, doc);
      await expectQuiet(page, trap, "the panels");
      await expectNoColourTraces(page, doc, "the panels");
    });

    test("following every link opens no script address", async ({ page }) => {
      test.skip(doc.links.length === 0, "this document has no links");
      const trap = await armPage(page);
      await openHostile(page, doc);
      const followed = await clickLinks(page, trap, doc);
      await expectQuiet(page, trap, "links", followed);
    });

    test("its downloads carry no live script and load nothing from outside", async ({ page }) => {
      const trap = await armPage(page);
      await openHostile(page, doc);
      const found: string[] = [];

      const html = await download(page, "Web page (.html)");
      found.push(...(await scanHtml(page, html.bytes.toString("utf8"), "web page")));

      const md = await download(page, "Markdown (.md)");
      found.push(...scanMarkdown(md.bytes.toString("utf8")).map((f) => `markdown: ${f}`));

      const epub = await download(page, "EPUB book (.epub)");
      for (const part of partNames(epub.bytes)) {
        if (/\.(x?html?|svg)$/i.test(part)) found.push(...(await scanHtml(page, partOf(epub.bytes, part) ?? "", `epub ${part}`)));
        else if (/\.(opf|ncx|xml)$/i.test(part)) {
          const text = partOf(epub.bytes, part) ?? "";
          if (/properties="[^"]*scripted/.test(text)) found.push(`epub ${part}: declares scripted content`);
          if (/<script\b|javascript:/i.test(text)) found.push(`epub ${part}: script`);
        } else if (/\.(js|mjs)$/i.test(part)) found.push(`epub has a script file: ${part}`);
        else if (/\.css$/i.test(part) && /url\(\s*['"]?(?!data:)|@import/i.test(partOf(epub.bytes, part) ?? "")) found.push(`epub ${part}: loads from outside`);
      }

      const pdf = await download(page, "PDF document (.pdf)");
      found.push(...scanPdf(pdf.bytes));

      const odt = await download(page, "OpenDocument text (.odt)");
      for (const part of ["content.xml", "styles.xml", "meta.xml", "settings.xml"]) {
        const text = partOf(odt.bytes, part) ?? "";
        if (/<office:scripts\b[^>]*>\s*<office:script\b|<script:event-listener|xlink:href="\s*(javascript|vbscript|data:text\/html|file):/i.test(text)) found.push(`odt ${part}: script or a script address`);
      }
      const rtf = await download(page, "Rich Text (.rtf)");
      const rtfText = rtf.bytes.toString("latin1");
      const instructions = [...rtfText.matchAll(/\\fldinst\{+([^}]*)\}/g)].map((m) => m[1]);
      if (instructions.some((i) => /^\s*(HYPERLINK\s+"?\s*(javascript|vbscript|data:text\/html|file):|INCLUDETEXT|INCLUDEPICTURE|DDE|DDEAUTO|LINK|IMPORT)/i.test(i)) || /\\objdata|\\objupdate|\\objlink/i.test(rtfText)) found.push("rtf: a script address, an include field or an object");

      expect(found, "live script or outside loads in the downloads").toEqual([]);
      await expectQuiet(page, trap, "downloads");
    });
  });
}
