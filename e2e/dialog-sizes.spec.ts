// Dialogs come in four widths (440, 600, 800 and 1040px) and one height limit (720px, or the window less 48px), as
// DESIGN.md says. The title and the buttons stay in place and only the middle scrolls, so OK and Cancel are always on
// screen; a dialog with tabs keeps its height when a shorter tab is chosen.
import { chooseMenu, expect, openFixture, test, type Page } from "./helpers";

const SIZES = [440, 600, 800, 1040];
const DIALOGS: [string, ...string[]][] = [
  ["File", "Page setup…"],
  ["Format", "Font…"],
  ["Format", "Paragraph…"],
  ["Format", "Paragraph styles", "Modify style…"],
  ["Format", "Paragraph styles", "Manage styles…"],
  ["Format", "Tabs…"],
  ["Tools", "Word count"],
  ["Tools", "Preferences…"],
];

const top = (page: Page) => page.locator(".scrim .dialog").last();
const measure = (page: Page) =>
  top(page).evaluate((d) => {
    const r = d.getBoundingClientRect();
    const actions = d.querySelector(".dialog-actions")!.getBoundingClientRect();
    return { w: Math.round(r.width), h: Math.round(r.height), top: r.top, bottom: r.bottom, actionsBottom: actions.bottom, view: window.innerHeight };
  });

test("every dialog has one of the four widths, fits a short window, and keeps its buttons on screen", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 640 });
  await openFixture(page, "rich.docx");
  for (const [menu, ...path] of DIALOGS) {
    await chooseMenu(page, menu, ...path);
    await expect(top(page)).toBeVisible();
    const m = await measure(page);
    const name = [menu, ...path].join(" > ");
    expect(SIZES, `${name}: ${m.w}`).toContain(m.w);
    expect(m.h, name).toBeLessThanOrEqual(Math.min(720, m.view - 48) + 1);
    expect(m.top, name).toBeGreaterThanOrEqual(0);
    expect(m.actionsBottom, name).toBeLessThanOrEqual(m.view);
    expect(m.actionsBottom, name).toBeCloseTo(m.bottom, 0);
    await page.keyboard.press("Escape");
    await expect(page.locator(".scrim")).toHaveCount(0);
  }
});

test("a dialog with tabs keeps its height when a shorter tab is chosen", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await chooseMenu(page, "File", "Page setup…");
  await expect(top(page)).toBeVisible();
  const tabs = top(page).getByRole("tab");
  const heights: number[] = [];
  for (const name of ["Margins", "Paper", "Layout", "Margins"]) {
    await tabs.filter({ hasText: name }).click();
    await page.waitForTimeout(50);
    heights.push((await measure(page)).h);
  }
  // Once it has been as tall as its tallest tab, it stays that tall.
  const tallest = Math.max(...heights.slice(0, 3));
  expect(heights[3]).toBeGreaterThanOrEqual(tallest - 1);
});
