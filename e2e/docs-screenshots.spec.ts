// The screenshots of the user guide (docs/use/images/): taken from the real app, on a short paper built with the app's own tools,
// light theme, at a laptop size, each cropped to what its page talks about. They are not part of a normal run.
//
//   DOCXTOR_DOCS_SHOTS=1 npx playwright test e2e/docs-screenshots.spec.ts --project=chromium
//
// Needs: the engine built (`npm run wasm`), network for the interface fonts (the shots use the real fonts, not the test stand-ins),
// and `node tools/mock-ai.mjs` running on port 8787 for the two assistant shots (the scripted model stands in for a provider).
// If ImageMagick (`magick`) is on the PATH each picture is reduced to 128 colours, which takes a screenshot of this interface from about
// 250 KB to under 100; the spec fails when a picture is over 150 KB or the set is over 3 MB. Then read the pictures before you commit them.
import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test, type Locator, type Page } from "@playwright/test";

const OUT = join(process.cwd(), "docs/use/images");
const MD = `Water use in urban gardens

A short paper for Environmental Studies 201

# Abstract

Urban gardens grow food in the middle of cities. This paper compares how much water ten community gardens used in a dry summer and asks which habits saved the most.

# Introduction

Community gardens give city residents a place to grow vegetables, but they also compete for a limited supply of water. Earlier surveys counted gardens and harvests; few measured the water.

# Method

We read the water meters of ten gardens every week from June to August. Each garden also told us how it watered.

- Hand watering with a can or hose
- Drip lines on a timer
- Rain barrels

# Results

| Garden | Method | Litres per square metre |
|---|---|---|
| Riverside | Drip lines | 212 |
| Elm Street | Hand watering | 341 |
| The Allotments | Rain barrels | 158 |
| Station Yard | Drip lines | 197 |

# Discussion

Gardens with drip lines or rain barrels used about a third less water than gardens that watered by hand. The difference held in the driest weeks.
`;

test.skip(!process.env.DOCXTOR_DOCS_SHOTS, "set DOCXTOR_DOCS_SHOTS=1 to take the guide's screenshots");
test.use({ viewport: { width: 1280, height: 800 }, colorScheme: "light" });

type W = { docxtor: any };

/** Open the app and wait for the engine. */
async function start(page: Page) {
  await page.goto("/");
  await page.waitForFunction(() => (window as unknown as W).docxtor?.files?.library, null, { timeout: 60_000 });
}

/** A document from Markdown in the Library, opened. */
async function create(page: Page, name: string, md: string) {
  await page.evaluate(async ([n, m]) => {
    const w = (window as unknown as W).docxtor;
    await w.createIn(w.files.library, "", n, m);
  }, [name, md]);
  await page.waitForSelector(".flow .fp");
}

/** The sample paper: a title, a contents list, a footnote, a citation and a reference list, a captioned table, a header and a footer. */
async function paper(page: Page) {
  await create(page, "Water use in urban gardens.docx", MD);
  await page.evaluate(() => {
    const ed = (window as unknown as W).docxtor.fe.editor;
    const raw = () => ed.engine.raw;
    const tool = (name: string, input: object) => {
      const out = raw().callTool(name, JSON.stringify(input));
      ed.refresh({ keepScroll: true });
      return out as string;
    };
    const blocks = () => [...(raw().callTool("read_document", "{}") as string).matchAll(/\[(p\d+)([^\]]*)\] (.*)/g)].map((m) => ({ id: m[1].slice(1), text: m[3] }));
    const find = (text: string) => blocks().find((p) => p.text.startsWith(text))!.id;
    tool("set_paragraph_style", { ids: [find("Water use")], style: "Title" });
    tool("set_paragraph_style", { ids: [find("A short paper")], style: "Subtitle" });
    tool("insert_table_of_contents", { after: find("A short paper"), levels: 2 });
    // The space goes in first, so the citation sits before the full stop: "water (Okafor & Lind, 2022)."
    const intro = find("Community gardens give");
    const text = blocks().find((p) => p.id === intro)!.text;
    const at = text.indexOf("few measured the water") + "few measured the water".length;
    ed.exec({ op: "insert_text", at: { p: Number(intro), o: at }, text: " " });
    tool("add_source", { bibtex: "@article{okafor2022, author={Okafor, Ngozi and Lind, Mats}, title={Counting the water in community gardens}, journal={Journal of Urban Agriculture}, year={2022}, volume={14}, pages={101--118}}\n@book{hale2019, author={Hale, Priya}, title={The City Allotment}, publisher={Meadow Press}, year={2019}}" });
    const tag = /\b([A-Z][A-Za-z]+\d{2})\b/.exec(tool("list_sources", {}))![1];
    tool("cite", { id: intro, after_text: "few measured the water ", tags: [tag] });
    tool("insert_footnote", { id: intro, after_text: "limited supply of water", text: "Metered use in the city's gardens was not public before 2023." });
    const table = /\[(t\d+) table/.exec(raw().callTool("read_document", "{}") as string)![1];
    tool("insert_caption", { before: table, label: "Table", text: "Water used by four of the ten gardens" });
    tool("insert_bibliography", {});
    tool("set_page_layout", { margins_pt: { top: 54, bottom: 54 } });
    tool("set_header_footer", { kind: "header", text: "Water use in urban gardens" });
    tool("set_header_footer", { kind: "footer", text: "Page {PAGE} of {NUMPAGES}" });
  });
  await page.evaluate(async () => {
    await (window as unknown as W).docxtor.fe.extensions.execute("insert.updateTocPages");
  });
  await page.waitForTimeout(1200);
}

const menuItem = (page: Page, label: string) => page.locator('.menu-drop [role="menuitem"]').filter({ has: page.locator(".menu-label").filter({ hasText: new RegExp(`^${label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`) }) });
async function choose(page: Page, top: string, ...path: string[]) {
  await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: top, exact: true }).click();
  for (const label of path) await menuItem(page, label).click();
  await page.waitForTimeout(400);
}
const leave = async (page: Page) => {
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press("Escape");
    await page.waitForTimeout(120);
  }
};

/** Write a picture, reduced when ImageMagick is there. */
async function save(page: Page, name: string, clip?: { x: number; y: number; width: number; height: number } | Locator) {
  mkdirSync(OUT, { recursive: true });
  const file = join(OUT, `${name}.png`);
  const bytes = clip && "screenshot" in clip ? await clip.screenshot() : await page.screenshot(clip ? { clip } : {});
  writeFileSync(file, bytes);
  try {
    execFileSync("magick", [file, "-strip", "-colors", "128", `PNG8:${file}`]);
  } catch {
    /* no ImageMagick: keep the full picture */
  }
}
const box = async (page: Page, selector: string, pad = 0) => {
  const b = (await page.locator(selector).first().boundingBox())!;
  return { x: Math.max(0, b.x - pad), y: Math.max(0, b.y - pad), width: b.width + pad * 2, height: b.height + pad * 2 };
};

test.describe("the guide's screenshots", () => {
  test.describe.configure({ mode: "serial" });

  test("the start screen, the editor, the page, the table and the panels", async ({ page }) => {
    test.setTimeout(240_000);
    await start(page);
    await paper(page);
    // The editor, from the top.
    await page.getByRole("tab", { name: "Files" }).click();
    await save(page, "editor");
    // The page with its header, footer and footnote.
    // A tall window, so the whole sheet is on screen.
    await page.setViewportSize({ width: 1280, height: 1300 });
    await page.waitForTimeout(600);
    await page.locator(".sheet").first().scrollIntoViewIfNeeded();
    await save(page, "header-footer", page.locator(".sheet").first());
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.waitForTimeout(600);
    // The table with its tools.
    const cell = page.locator(".flow table td").nth(4);
    await cell.evaluate((el) => el.scrollIntoView({ block: "center" }));
    await cell.click();
    await page.waitForTimeout(500);
    const table = (await page.locator(".flow table").first().boundingBox())!;
    const bar = (await page.locator(".tbl-strip").first().boundingBox())!;
    const left = Math.min(table.x, bar.x) - 16;
    const top = Math.min(table.y, bar.y) - 16;
    await save(page, "table", { x: Math.max(0, left), y: Math.max(0, top), width: Math.max(table.x + table.width, bar.x + bar.width) + 16 - left, height: table.y + table.height + 60 - top });
    // Page setup.
    await choose(page, "File", "Page setup…");
    await save(page, "page-setup", await box(page, '[role="dialog"]', 12));
    await leave(page);
    // Word count.
    await choose(page, "Tools", "Word count");
    await save(page, "word-count", await box(page, '[role="dialog"]', 12));
    await leave(page);
    // Find.
    await page.locator(".flow").first().scrollIntoViewIfNeeded();
    await page.keyboard.press("ControlOrMeta+f");
    await page.keyboard.type("water");
    await page.waitForTimeout(500);
    await save(page, "find", { x: 330, y: 114, width: 940, height: 330 });
    await leave(page);
    // The citations tab.
    await choose(page, "Insert", "Citation…");
    await save(page, "citations", { x: 0, y: 114, width: 250, height: 650 });
    // The command palette.
    await page.keyboard.press("ControlOrMeta+Shift+p");
    await page.keyboard.type("heading");
    await page.waitForTimeout(400);
    await save(page, "command-palette", { x: 250, y: 0, width: 700, height: 440 });
    await leave(page);
    // History.
    await page.getByRole("tab", { name: "History" }).click();
    await page.waitForTimeout(300);
    await save(page, "history", { x: 0, y: 114, width: 250, height: 300 });
    // The start screen, with the paper among the recent documents.
    await page.goto("/#/");
    await page.waitForSelector(".start");
    await page.waitForTimeout(800);
    await save(page, "start-screen", { x: 0, y: 0, width: 880, height: 560 });
  });

  test("spelling", async ({ page }) => {
    await start(page);
    await create(page, "Notes.docx", "Notes for Thursday\n\nThe garden commitee meets at six. Bring the water readings and the minutes from last month.");
    const point = await page.evaluate(() => {
      const el = [...document.querySelectorAll(".flow .fp")].find((p) => p.textContent?.includes("commitee"))!;
      const node = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = node.nextNode(); n; n = node.nextNode()) {
        const i = n.textContent!.indexOf("commitee");
        if (i >= 0) {
          const r = document.createRange();
          r.setStart(n, i + 3);
          r.setEnd(n, i + 5);
          const b = r.getBoundingClientRect();
          return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
        }
      }
      return null;
    });
    await page.waitForTimeout(1500);
    await page.mouse.click(point!.x, point!.y, { button: "right" });
    await page.locator(".ctx-menu").waitFor();
    await page.waitForTimeout(500);
    const menu = (await page.locator(".ctx-menu").boundingBox())!;
    const left = Math.max(0, Math.min(point!.x, menu.x) - 120);
    const top = Math.max(0, Math.min(menu.y, point!.y) - 20);
    await save(page, "spelling", { x: left, y: top, width: menu.x + menu.width + 20 - left, height: Math.min(Math.max(260, point!.y + 50 - top), 800 - top) });
  });

  test("the assistant", async ({ page }) => {
    test.setTimeout(120_000);
    await start(page);
    await create(page, "Product update.docx", "Product update\n\n# Highlights\n\n- Revenue grew 18% year over year\n- Two new enterprise customers\n- Churn down to 2.1%\n\n# Plan\n\n1. Ship the editor\n2. Publish fidelity scores\n");
    await page.getByRole("button", { name: "Assistant" }).click();
    await page.getByRole("button", { name: "Connect a model" }).first().click();
    await page.getByLabel("Provider").selectOption({ label: "Custom (OpenAI-compatible)" });
    await page.getByLabel("API key").fill("sk-example");
    await page.getByLabel("Base URL").fill("http://localhost:8787/v1");
    await page.getByLabel("Model", { exact: true }).fill("example-model");
    await page.waitForTimeout(700);
    await save(page, "assistant-connect", await box(page, '[role="dialog"]', 12));
    await page.getByRole("button", { name: "Save" }).click();
    await page.getByPlaceholder(/Ask anything/).fill("Update the churn figure to 1.9% and add a Next steps section.");
    await page.getByRole("button", { name: "Send" }).click();
    await page.waitForSelector(".review", { timeout: 20_000 });
    await page.waitForTimeout(800);
    await save(page, "assistant-review", { x: 250, y: 114, width: 1030, height: 540 });
  });

  test("the pictures are small", () => {
    const files = readdirSync(OUT).filter((f) => f.endsWith(".png"));
    expect(files.length).toBeGreaterThan(10);
    let total = 0;
    for (const f of files) {
      const size = statSync(join(OUT, f)).size;
      total += size;
      expect(size, `${f} is ${Math.round(size / 1024)} KB`).toBeLessThan(150 * 1024);
    }
    expect(total).toBeLessThan(3 * 1024 * 1024);
  });
});
