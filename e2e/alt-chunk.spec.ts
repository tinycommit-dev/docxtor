// Imported content chunks (w:altChunk): a web page a report generator put inside a document is shown in place, with
// its headings, lists and table; the file saves as it came; typing inside it makes it ordinary text, in one undo step.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, LINE_START, test } from "./helpers";

const HTML = `<html><body><h1>Quarterly report</h1><p>Sales grew <b>twelve</b> percent.</p><ul><li>North</li><li>South</li></ul><table border="1"><tr><td>Region</td><td>Units</td></tr></table></body></html>`;

async function open(page: import("@playwright/test").Page) {
  await openBuilt(page, {
    sections: [{ paragraphs: ["Before the chunk", { body: '<w:altChunk r:id="rChunk"/>' }, "After the chunk"] }],
    extraRels: [{ id: "rChunk", type: "aFChunk", target: "afchunk.htm" }],
    files: [{ name: "afchunk.htm", contentType: "text/html", data: HTML }],
  });
}

/** The saved file as text: the zip's part names are readable in it (the part of a chunk is word/afchunk.htm). */
const savedFile = (page: import("@playwright/test").Page) => inPage(page, (w) => new TextDecoder("latin1").decode(w.fe.editor.engine!.save()));

test.describe("imported content chunks", () => {
  test("a web page inside the document is shown in place, formatted", async ({ page }) => {
    await open(page);
    const flow = page.locator(".flow");
    await expect(flow).toContainText("Quarterly report");
    await expect(flow).toContainText("Sales grew twelve percent.");
    const text = await flow.innerText();
    expect(text.indexOf("Before the chunk")).toBeLessThan(text.indexOf("Quarterly report"));
    expect(text.indexOf("Region")).toBeLessThan(text.indexOf("After the chunk"));
    // A heading, bold text, a bulleted list and a table.
    await expect(page.locator(".flow [data-p]", { hasText: "Quarterly report" })).toHaveCSS("font-size", /^(1[6-9]|2\d)/);
    expect(await page.locator(".flow b, .flow [style*='font-weight: 700'], .flow [style*='font-weight:700']", { hasText: "twelve" }).count()).toBeGreaterThan(0);
    await expect(page.locator(".flow [data-p]", { hasText: "North" }).locator(".lbl")).toHaveCount(1);
    await expect(page.locator(".flow table", { hasText: "Region" })).toHaveCount(1);
    await page.screenshot({ path: "test-results/alt-chunk.png" });
  });

  test("saving untouched keeps the chunk; typing in it makes it ordinary text, and undo brings the chunk back", async ({ page }) => {
    await open(page);
    expect(await savedFile(page)).toContain("word/afchunk.htm");
    await page.locator(".flow [data-p]", { hasText: "Sales grew" }).click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Draft: ");
    await expect(page.locator(".flow")).toContainText("Draft: Sales grew twelve percent.");
    expect(await savedFile(page)).not.toContain("word/afchunk.htm");
    // Typing undoes a word at a time; the first word took the chunk with it.
    for (let k = 0; k < 4 && (await page.locator(".flow").innerText()).includes("Draft"); k++) await page.keyboard.press("ControlOrMeta+z");
    await expect(page.locator(".flow")).not.toContainText("Draft");
    expect(await savedFile(page)).toContain("word/afchunk.htm");
  });
});
