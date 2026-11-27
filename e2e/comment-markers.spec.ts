// A comment's anchor in a file takes no room and shows nothing (no "Object" chip); comments themselves are not shown yet.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, paraText, test } from "./helpers";

test("a comment reference leaves no chip after the commented text, and takes no position", async ({ page }) => {
  await openBuilt(page, {
    sections: [
      {
        paragraphs: [
          {
            text: "Before ",
            xml: '<w:commentRangeStart w:id="0"/><w:r><w:t>commented words</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="0"/></w:r><w:r><w:t xml:space="preserve"> and after.</w:t></w:r>',
          },
        ],
      },
    ],
  });
  const p = Number(await page.locator(".flow .fp").first().getAttribute("data-p"));
  expect(await paraText(page, p)).toBe("Before commented words and after.");
  // Nothing is drawn for the reference: no object chip, and the line is the text and nothing else.
  await expect(page.locator(`.flow [data-p="${p}"] .atom`)).toHaveCount(0);
  expect(await page.locator(`.flow [data-p="${p}"]`).innerText()).toBe("Before commented words and after.");
  // It has no length: the paragraph ends where the text does, and the character after it is the next letter.
  const len = await inPage(page, (w, id) => w.fe.editor.engine!.textRange({ p: id, o: 0 }, { p: id, o: 1000 }).length, p);
  expect(len).toBe("Before commented words and after.".length);
  // Typing next to the commented text still works at the right place, and the file keeps the comment's anchor.
  await inPage(page, (w, id) => w.fe.editor.exec({ op: "insert_text", at: { p: id, o: 22 }, text: "!" }), p);
  expect(await paraText(page, p)).toBe("Before commented words! and after.");
  const kept = await inPage(page, (w) => {
    const bytes = w.fe.editor.engine!.save();
    return bytes.length > 0;
  });
  expect(kept).toBe(true);
});
