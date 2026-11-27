// Frames and drop caps (`w:framePr`): a drop cap is the first letter in a frame of its own, as tall as the lines it spans,
// with the text wrapping beside it; other frames float to the side of the text, or sit on the page.
import { openBuilt, type SectionSpec } from "./support/docx-builder";
import { expect, inPage, test, type Locator, type Page } from "./helpers";

const near = (a: number, b: number, tolerance = 1.5) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);
const TW = 1 / 15;
const LONG = "orem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";

const DROP = (kind: string, lines = 3) => `<w:framePr w:dropCap="${kind}" w:lines="${lines}" w:wrap="around" w:vAnchor="text" w:hAnchor="text"/><w:spacing w:after="0" w:line="${Math.round(lines * 274.5)}" w:lineRule="exact"/><w:textAlignment w:val="baseline"/>`;
const LETTER = (ch: string, size = 110) => `<w:r><w:rPr><w:position w:val="-8"/><w:sz w:val="${size}"/></w:rPr><w:t>${ch}</w:t></w:r>`;

const dropCapSection = (kind = "drop", lines = 3): SectionSpec => ({ paragraphs: [{ text: "", ppr: DROP(kind, lines), xml: LETTER("L") }, `${LONG} ${LONG}`, "Second paragraph."] });

/** The box of a paragraph's first n lines: the left edge of each line of text. */
const lineLefts = (el: Locator) =>
  el.evaluate((p) => {
    const range = document.createRange();
    const lines = new Map<number, number>();
    const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const t = n as Text;
      for (let i = 0; i < t.data.length; i++) {
        range.setStart(t, i);
        range.setEnd(t, i + 1);
        const r = range.getClientRects()[0];
        if (!r || r.width === 0) continue;
        const top = Math.round(r.top);
        lines.set(top, Math.min(lines.get(top) ?? Infinity, r.left));
      }
    }
    return [...lines.entries()].sort((a, b) => a[0] - b[0]).map(([top, left]) => ({ top, left }));
  });

test.describe("drop caps", () => {
  test("the letter is as tall as three lines and the text wraps beside it", async ({ page }) => {
    await openBuilt(page, { sections: [dropCapSection()] });
    const cap = page.locator(".flow .fp.dropcap");
    await expect(cap).toHaveCount(1);
    await page.screenshot({ path: "test-results/drop-cap.png" });
    const text = page.locator(".flow .fp").nth(1);
    const lefts = await lineLefts(text);
    const capBox = (await cap.boundingBox())!;
    const textBox = (await text.boundingBox())!;
    // The first three lines start to the right of the letter; the fourth is back at the margin.
    expect(lefts.length).toBeGreaterThan(4);
    for (const l of lefts.slice(0, 3)) expect(l.left).toBeGreaterThanOrEqual(capBox.x + capBox.width - 1);
    near(lefts[3].left, textBox.x, 1);
    // The letter starts where the text does, and spans three lines.
    near(capBox.y, textBox.y, 2);
    const pitch = lefts[1].top - lefts[0].top;
    near(capBox.height, 3 * pitch, 4);
    await page.screenshot({ path: "test-results/drop-cap.png" });
  });

  test("a drop cap in the margin hangs out to the left of the text", async ({ page }) => {
    await openBuilt(page, { sections: [dropCapSection("margin")] });
    const cap = page.locator(".flow .fp.dropcap");
    const text = page.locator(".flow .fp").nth(1);
    const capBox = (await cap.boundingBox())!;
    const textBox = (await text.boundingBox())!;
    expect(capBox.x + capBox.width).toBeLessThanOrEqual(textBox.x + 1);
    const lefts = await lineLefts(text);
    // Every line starts at the margin: nothing is indented.
    for (const l of lefts.slice(0, 4)) near(l.left, textBox.x, 1);
    await page.screenshot({ path: "test-results/drop-cap-margin.png" });
  });

  test("the letter is a paragraph of its own that can be edited", async ({ page }) => {
    await openBuilt(page, { sections: [dropCapSection()] });
    const cap = page.locator(".flow .fp.dropcap");
    expect(await cap.textContent()).toBe("L");
    const frame = await inPage(page, (w) => {
      const r = w.fe.editor.engine!.renderAll();
      return (r.blocks[0] as unknown as { fmt: { frame?: { dropCap: string; lines: number } } }).fmt.frame;
    });
    expect(frame).toMatchObject({ dropCap: "drop", lines: 3 });
  });
});

test.describe("frames", () => {
  const FRAME = (attrs: string) => `<w:framePr w:w="2400" w:h="1200" w:hRule="exact" w:wrap="around" w:hAnchor="margin" w:vAnchor="text" ${attrs}/>`;

  test("a frame aligned right floats there and the text wraps round it", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Sidebar note", ppr: FRAME('w:xAlign="right"') }, LONG + LONG] }] });
    const frame = page.locator(".flow .fp.pfloat");
    const text = page.locator(".flow .fp").nth(1);
    const f = (await frame.boundingBox())!;
    const t = (await text.boundingBox())!;
    near(f.width, 2400 * TW, 1);
    near(f.height, 1200 * TW, 1);
    near(f.x + f.width, t.x + t.width, 2);
    // The first line of the text is narrower than the column, ending before the frame.
    const right = await text.evaluate((p) => {
      const range = document.createRange();
      range.selectNodeContents(p);
      const first = range.getClientRects()[0];
      return first.right;
    });
    expect(right).toBeLessThanOrEqual(f.x + 2);
    await page.screenshot({ path: "test-results/frame-right.png" });
  });

  test("a frame that doesn't let text beside it sits on its own line", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before", { text: "Boxed", ppr: FRAME('w:xAlign="center"').replace('w:wrap="around"', 'w:wrap="notBeside"') }, "After the frame"] }] });
    const f = (await page.locator(".flow .fp.pfloat").boundingBox())!;
    const after = (await page.locator(".flow .fp").nth(2).boundingBox())!;
    expect(after.y).toBeGreaterThanOrEqual(f.y + f.height - 1);
  });

  test("a frame placed from the page sits at that spot on the page", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ paragraphs: [{ text: "Page frame", ppr: '<w:framePr w:w="2000" w:hAnchor="page" w:vAnchor="page" w:x="1440" w:y="720" w:wrap="around"/>' }, "Body text of the page"] }],
    });
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const sheet = (await page.locator(".sheet").first().boundingBox())!;
    const f = (await page.locator(".flow .fp.abs-frame").boundingBox())!;
    near((f.x - sheet.x) / zoom, 1440 * TW, 2);
    near((f.y - sheet.y) / zoom, 720 * TW, 2);
  });

  test("the page setup reports a frame's settings", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Sidebar note", ppr: FRAME('w:xAlign="right" w:hSpace="144"') }, "Text"] }] });
    const f = await inPage(page, (w) => (w.fe.editor.engine!.renderAll().blocks[0] as unknown as { fmt: { frame: Record<string, unknown> } }).fmt.frame);
    expect(f).toMatchObject({ w: 2400, h: 1200, hRule: "exact", wrap: "around", hAnchor: "margin", vAnchor: "text", xAlign: "right", hSpace: 144 });
  });
});

/** A paragraph's line count, for checks that the text still flows. */
export async function lineCount(el: Locator): Promise<number> {
  return (await lineLefts(el)).length;
}
export type { Page };
