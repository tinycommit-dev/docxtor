// What Google Docs copies keeps its look with Keep source formatting (the default): a heading keeps the font, size and
// black Google Docs gave it, not the document's blue heading style, and is still a Heading 1; list items and their
// bullets and numbers keep their black and font whatever the document's list style says; a checklist is a checklist.
import { expect, inPage, pasteData, test, type Page } from "./helpers";

const GDOCS = `<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1234"><h1 dir="ltr" style="line-height:1.38;margin-top:20pt;margin-bottom:6pt;"><span style="font-size:20pt;font-family:Calibri,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;">Project plan</span></h1><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Calibri,sans-serif;color:#000000;font-weight:400;">The first line.</span></p></b>`;

const li = (text: string, type = "disc", level = 1) =>
  `<li dir="ltr" style="list-style-type:${type};font-size:11pt;font-family:Calibri,sans-serif;color:#000000;" aria-level="${level}"><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:11pt;font-family:Calibri,sans-serif;color:#000000;">${text}</span></p></li>`;
const check = (text: string) =>
  `<li dir="ltr" role="checkbox" aria-checked="false" style="list-style-type:none;font-size:11pt;font-family:Calibri,sans-serif;color:#000000;" aria-level="1"><img width="11.37px" height="11.37px" alt="unchecked" aria-roledescription="checkbox" src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;" role="presentation"><span style="font-size:11pt;font-family:Calibri,sans-serif;color:#000000;">${text}</span></p></li>`;
const LISTS = `<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-5678"><ul style="margin-top:0;margin-bottom:0;padding-inline-start:48px;">${li("Bullet one")}${li("Bullet two")}<ul style="margin-top:0;margin-bottom:0;">${li("Nested", "circle", 2)}</ul></ul><ol style="margin-top:0;margin-bottom:0;">${li("Numbered", "decimal")}</ol><ul class="lst-kix_check" style="margin-top:0;margin-bottom:0;">${check("Task")}</ul></b>`;

async function pasteInto(page: Page, html = GDOCS, before?: () => Promise<void>) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w) => w.createIn(w.files.library, ""));
  await before?.();
  await page.locator(".flow [data-p]").first().click();
  await pasteData(page, { "text/html": html, "text/plain": "Project plan\nThe first line." });
}

const look = (page: Page, text: string) =>
  page.locator(".flow [data-p]", { hasText: text }).locator("span", { hasText: text }).first().evaluate((s) => ({ color: getComputedStyle(s).color, font: getComputedStyle(s).fontFamily }));

test("a Google Docs heading keeps its font and black colour, and is still a heading", async ({ page }) => {
  await pasteInto(page);
  await expect(page.locator(".flow")).toContainText("Project plan");
  const h = await look(page, "Project plan");
  expect(h.color).toBe("rgb(0, 0, 0)");
  expect(h.font).toMatch(/^Calibri/);
  expect(await page.locator(".flow [data-p]", { hasText: "Project plan" }).getAttribute("data-style")).toBe("Heading1");
  expect((await look(page, "The first line.")).font).toMatch(/^Calibri/);
});

test("Google Docs bullets, numbers and their text stay black in a document whose list style is blue, and a checklist is a checklist", async ({ page }) => {
  await pasteInto(page, LISTS, () =>
    inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { exec(op: unknown, o: unknown): unknown; sel: unknown };
      ed.exec({ op: "set_style_definition", style: "List Paragraph", def: { rpr: { color: "0000FF", font: "Corbel" } } }, { select: ed.sel, keepExtra: true });
    }),
  );
  await expect(page.locator(".flow")).toContainText("Task");
  for (const text of ["Bullet one", "Bullet two", "Nested", "Numbered"]) {
    const p = page.locator(".flow [data-p]", { hasText: text });
    expect(await p.locator(".lbl").evaluate((l) => getComputedStyle(l).color), text).toBe("rgb(0, 0, 0)");
    expect((await look(page, text)).color, text).toBe("rgb(0, 0, 0)");
    expect((await look(page, text)).font, text).toMatch(/^Calibri/);
  }
  const n = page.locator(".flow [data-p]", { hasText: "Numbered" }).locator(".lbl");
  await expect(n).toHaveText("1.");
  expect(await n.evaluate((l) => getComputedStyle(l).fontFamily)).toMatch(/^Calibri/);
  const task = page.locator(".flow [data-p]", { hasText: "Task" });
  await expect(task.locator(".lbl-check")).toHaveAttribute("aria-checked", "false");
  await expect(task.locator("img")).toHaveCount(0);
});

test("a Google Docs numbered list of 20 pt headings keeps its spacing, and the text of items 10 and up lines up with the rest", async ({ page }) => {
  const item = (t: string) =>
    `<li dir="ltr" style="list-style-type:decimal;font-size:20pt;font-family:Calibri,sans-serif;color:#000000;" aria-level="1"><h1 dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:20pt;font-family:Calibri,sans-serif;color:#000000;">${t}</span></h1></li>`;
  const items = Array.from({ length: 12 }, (_, i) => item(`Point ${i + 1}`)).join("");
  await pasteInto(page, `<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-9"><ol style="margin-top:0;margin-bottom:0;padding-inline-start:48px;">${items}</ol></b>`);
  await expect(page.locator(".flow")).toContainText("Point 12");
  const rows = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>(".flow [data-p]")]
      .filter((p) => p.textContent!.includes("Point"))
      .map((p) => {
        const text = [...p.querySelectorAll("span")].find((s) => !s.closest(".lbl") && s.children.length === 0 && s.textContent!.includes("Point"))!;
        return { style: p.dataset.style, x: Math.round(text.getClientRects()[0].left - p.getBoundingClientRect().left), top: parseFloat(getComputedStyle(p).marginTop) };
      }),
  );
  expect(rows).toHaveLength(12);
  expect(new Set(rows.map((r) => r.style))).toEqual(new Set(["Heading1"]));
  expect(new Set(rows.map((r) => r.x)).size, JSON.stringify(rows)).toBe(1);
  expect(rows.slice(1).every((r) => r.top === 0), JSON.stringify(rows)).toBe(true);
});
