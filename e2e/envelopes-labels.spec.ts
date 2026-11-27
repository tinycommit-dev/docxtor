// Envelopes and Labels: the Envelopes dialog (addresses, sizes, fonts, a preview) and "Add to document" as a new first section with the
// envelope's page size; the Labels dialog (vendors and products, the full page of one label or a single label, your own size) and the
// sheet it makes in a new document, measured on the page; label products an extension adds; and both as the main document of a merge.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, menuItem, openMenu, test, type Page } from "./helpers";
import { bodyText, caretAtEndOf, chooseCsv, eng, fields, fileName, info, line, mergeField, pagesOf, parts, run, strip } from "./support/mail-merge-helpers";

const PLAIN = { sections: [{ paragraphs: ["Dear Ada,", "Thank you for your letter."] }] };
const NAMES = "First Name,Last Name\nAda,Lovelace\nGrace,Hopper\nAlan,Turing\nEdsger,Dijkstra\n";

/** Mailings, Start mail merge, and a line of its submenu (Labels and Envelopes are also in the menu above it). */
async function startMerge(page: Page, label: string) {
  await openMenu(page, "Mailings");
  await menuItem(page, "Start mail merge").click();
  await menuItem(page, label).last().click();
}

const sections = (page: Page) => eng(page, (e) => e.pageSetup().sections);

/** The page boxes and the boxes of some elements inside the first page, as fractions of that page's width and height. */
async function onFirstPage(page: Page, selector: string) {
  return page.evaluate((sel) => {
    const sheet = document.querySelector<HTMLElement>(".sheets .sheet")!.getBoundingClientRect();
    return Array.from(document.querySelectorAll<HTMLElement>(sel)).map((el) => {
      const r = el.getBoundingClientRect();
      // Where the text itself starts (an indent moves it, not the box).
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, { acceptNode: (n) => ((n as Text).data.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT) });
      const node = walker.nextNode();
      const range = document.createRange();
      if (node) range.selectNodeContents(node);
      const t = node ? range.getBoundingClientRect() : r;
      return { x: (r.left - sheet.left) / sheet.width, y: (r.top - sheet.top) / sheet.height, w: r.width / sheet.width, h: r.height / sheet.height, tx: (t.left - sheet.left) / sheet.width, ty: (t.top - sheet.top) / sheet.height, text: el.textContent ?? "" };
    });
  }, selector);
}

test.describe("Envelopes", () => {
  test("the dialog lists Word's sizes and draws where the addresses go", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Envelopes…");
    const box = dialog(page, "Envelopes");
    const sizes = await box.getByLabel("Envelope size").locator("option").allTextContents();
    for (const want of ["Size 10 (4 1/8 x 9 1/2 in)", "Size 9", "Size 6 3/4", "Monarch", "DL (110 x 220 mm)", "C5 (162 x 229 mm)", "C6 (114 x 162 mm)", "Custom size…"]) expect(sizes.some((s) => s.startsWith(want))).toBe(true);
    await box.getByLabel("Delivery address", { exact: true }).fill("Ada Lovelace\n12 Crumb Rd\nLondon");
    await box.getByLabel("Return address", { exact: true }).fill("Grace Hopper\n1 Navy Way");
    const to = box.locator(".mm-env-text.to");
    const from = box.locator(".mm-env-text.from");
    await expect(to).toContainText("12 Crumb Rd");
    await expect(from).toContainText("1 Navy Way");
    // Size 10 is 9.5 by 4.125 inches: the picture has that shape, and the delivery address starts 44% of the way across.
    const env = await box.locator(".mm-envelope").boundingBox();
    expect(env!.width / env!.height).toBeCloseTo(9.5 / 4.125, 1);
    expect(Number(await to.getAttribute("data-x"))).toBe(Math.round(13680 * 0.44));
    // Leaving the return address out takes it off the picture.
    await box.getByLabel("Leave out the return address").check();
    await expect(from).toHaveCount(0);
    await box.getByLabel("Envelope size").selectOption("dl");
    expect(Number(await to.getAttribute("data-x"))).toBe(Math.round(12472 * 0.44));
  });

  test("Add to document makes the envelope a new first section with its page size", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Envelopes…");
    const box = dialog(page, "Envelopes");
    await box.getByLabel("Delivery address", { exact: true }).fill("Ada Lovelace\n12 Crumb Rd\nLondon");
    await box.getByLabel("Return address", { exact: true }).fill("Grace Hopper\n1 Navy Way");
    await box.getByRole("button", { name: "Add to document" }).click();
    await expect(box).toBeHidden();
    const s = await sections(page);
    expect(s).toHaveLength(2);
    expect([s[0].width, s[0].height]).toEqual([13680, 5940]);
    // The letter is still on letter paper in its own section.
    expect([s[1].width, s[1].height]).toEqual([12240, 15840]);
    expect(await bodyText(page)).toMatch(/^Grace Hopper\n1 Navy Way\nAda Lovelace\n12 Crumb Rd\nLondon\nDear Ada,/);
    await expect.poll(() => pagesOf(page)).toBe(2);
    // On the page: the sheet has the envelope's shape, the return address is in the corner and the delivery address a little over
    // two fifths of the way across and under the middle line, as Word places them.
    const sheets = await page.evaluate(() => Array.from(document.querySelectorAll<HTMLElement>(".sheets .sheet")).map((el) => el.getBoundingClientRect().width / el.getBoundingClientRect().height));
    expect(sheets[0]).toBeCloseTo(13680 / 5940, 2);
    expect(sheets[1]).toBeCloseTo(12240 / 15840, 2);
    const [ret, , to] = await onFirstPage(page, ".flow .fp");
    expect(ret.text).toBe("Grace Hopper");
    expect(ret.tx).toBeCloseTo(360 / 13680, 2);
    expect(ret.ty).toBeCloseTo(360 / 5940, 2);
    expect(to.text).toBe("Ada Lovelace");
    expect(to.tx).toBeCloseTo(0.44, 2);
    expect(Math.abs(to.ty - 0.46)).toBeLessThan(0.02);
    await page.screenshot({ path: "test-results/envelope-first-page.png" });
    // One undo step takes the envelope out again.
    await page.keyboard.press("ControlOrMeta+z");
    expect(await sections(page)).toHaveLength(1);
  });

  test("adding an envelope again changes the one that is there; a custom size and fonts are written", async ({ page }) => {
    await openBuilt(page, PLAIN);
    for (const [size, name] of [["10", "Ada"], ["custom", "Alan"]] as const) {
      await chooseMenu(page, "Mailings", "Envelopes…");
      const box = dialog(page, "Envelopes");
      await box.getByLabel("Delivery address", { exact: true }).fill(`${name} Lovelace`);
      await box.getByLabel("Envelope size").selectOption(size);
      if (size === "custom") {
        await box.getByLabel("Width in inches").fill("6");
        await box.getByLabel("Height in inches").fill("3");
        await box.getByLabel("Delivery address font").selectOption("Georgia");
        await box.getByLabel("Delivery address font").locator("../..").getByLabel("Size in points").fill("14");
      }
      await box.getByRole("button", { name: "Add to document" }).click();
      await expect(box).toBeHidden();
    }
    const s = await sections(page);
    expect(s).toHaveLength(2);
    expect([s[0].width, s[0].height]).toEqual([8640, 4320]);
    expect(await bodyText(page)).toMatch(/^Alan Lovelace\nDear Ada,/);
    const xml = (await parts(page)).get("word/document.xml")!;
    expect(xml).toContain('w:ascii="Georgia"');
    expect(xml).toContain('<w:sz w:val="28"/>');
  });

  test("the delivery address starts as the selected text, and a missing one is asked about", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await line(page, 0).click({ clickCount: 3 });
    await chooseMenu(page, "Mailings", "Envelopes…");
    const box = dialog(page, "Envelopes");
    await expect(box.getByLabel("Delivery address", { exact: true })).toHaveValue("Dear Ada,");
    await box.getByLabel("Delivery address", { exact: true }).fill("");
    await box.getByRole("button", { name: "Add to document" }).click();
    const ask = page.getByRole("alertdialog", { name: "Add the envelope" });
    await expect(ask).toContainText("There is no delivery address");
    await ask.getByRole("button", { name: "Cancel" }).click();
    expect(await sections(page)).toHaveLength(1);
  });

  test("the return address is remembered for the next envelope", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Envelopes…");
    let box = dialog(page, "Envelopes");
    await box.getByLabel("Delivery address", { exact: true }).fill("Ada");
    await box.getByLabel("Return address", { exact: true }).fill("Grace Hopper\n1 Navy Way");
    await box.getByRole("button", { name: "Add to document" }).click();
    await chooseMenu(page, "Mailings", "Envelopes…");
    box = dialog(page, "Envelopes");
    await expect(box.getByLabel("Return address", { exact: true })).toHaveValue("Grace Hopper\n1 Navy Way");
  });
});

test.describe("Labels", () => {
  test("the vendors and their products", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Labels…");
    const box = dialog(page, "Labels");
    const vendors = await box.getByLabel("Label vendor").locator("option").allTextContents();
    expect(vendors).toEqual(["Avery US Letter", "Avery A4 and A5", "Plain paper", "Other (your own size)"]);
    const products = () => box.getByLabel("Product number").locator("option").allTextContents();
    expect(await products()).toEqual(expect.arrayContaining(["5160 Address labels", "5161 Address labels", "5162 Address labels", "5163 Shipping labels", "5164 Shipping labels", "5167 Return address labels", "8160 Address labels"]));
    await box.getByLabel("Label vendor").selectOption("Avery A4 and A5");
    expect(await products()).toEqual(expect.arrayContaining(["L7160 Address labels", "L7161 Address labels", "L7162 Address labels", "L7163 Address labels", "L7173 Shipping labels", "J8160 Address labels"]));
    await box.getByLabel("Product number").selectOption({ label: "L7160 Address labels" });
    await expect(box).toContainText("21 labels on a sheet, 3 across and 7 down. Each is 63.5 mm by 38.1 mm.");
    await box.getByLabel("Label vendor").selectOption("Avery US Letter");
    await box.getByLabel("Product number").selectOption({ label: "5160 Address labels" });
    await expect(box).toContainText("30 labels on a sheet, 3 across and 10 down. Each is 2.625 in by 1 in.");
  });

  test("a full page of the same label is a table with Word's geometry", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Labels…");
    const box = dialog(page, "Labels");
    await box.getByLabel("Product number").selectOption({ label: "5160 Address labels" });
    await box.getByLabel("Address").fill("Ada Lovelace\n12 Crumb Rd");
    await box.getByRole("button", { name: "New document" }).click();
    await expect.poll(() => fileName(page)).toMatch(/Labels/);
    // Letter paper; margins that put the first label 0.5 in down and 0.1875 in across.
    const s = await sections(page);
    expect(s).toHaveLength(1);
    expect([s[0].width, s[0].height, ...s[0].margin]).toEqual([12240, 15840, 720, 270, 660, 270]);
    await expect.poll(() => pagesOf(page)).toBe(1);
    // Thirty labels, each with the address.
    const cells = page.locator(".flow table td").filter({ hasText: "Ada Lovelace" });
    await expect(cells).toHaveCount(30);
    // Measured on the page: the first label's corner, its size, and the next label's place, as fractions of the sheet.
    const boxes = await onFirstPage(page, ".flow table td");
    const labels = boxes.filter((b) => b.text.includes("Ada Lovelace"));
    const near = (a: number, b: number) => expect(Math.abs(a - b)).toBeLessThan(0.004);
    near(labels[0].x, 270 / 12240);
    near(labels[0].y, 720 / 15840);
    near(labels[0].w, 3780 / 12240);
    near(labels[0].h, 1440 / 15840);
    near(labels[1].x, (270 + 3960) / 12240);
    near(labels[3].y, (720 + 1440) / 15840);
    near(labels[29].x, (270 + 2 * 3960) / 12240);
    near(labels[29].y, (720 + 9 * 1440) / 15840);
    await page.screenshot({ path: "test-results/labels-5160.png" });
  });

  test("a single label at a row and column; the other labels are empty", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Labels…");
    const box = dialog(page, "Labels");
    await box.getByLabel("Address").fill("Only me");
    await box.getByRole("radio", { name: "A single label" }).click();
    await box.getByLabel("Row").fill("3");
    await box.getByLabel("Column").fill("2");
    await box.getByRole("button", { name: "New document" }).click();
    await expect.poll(() => fileName(page)).toMatch(/Labels/);
    await expect(page.locator(".flow table td").filter({ hasText: "Only me" })).toHaveCount(1);
    const boxes = await onFirstPage(page, ".flow table td");
    const one = boxes.find((b) => b.text.includes("Only me"))!;
    expect(Math.abs(one.x - (270 + 3960) / 12240)).toBeLessThan(0.004);
    expect(Math.abs(one.y - (720 + 2 * 1440) / 15840)).toBeLessThan(0.004);
  });

  test("an A4 sheet has A4 paper", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Labels…");
    const box = dialog(page, "Labels");
    await box.getByLabel("Label vendor").selectOption("Avery A4 and A5");
    await box.getByLabel("Product number").selectOption({ label: "L7163 Address labels" });
    await box.getByLabel("Address").fill("Ada");
    await box.getByRole("button", { name: "New document" }).click();
    await expect.poll(() => fileName(page)).toMatch(/Labels/);
    const s = await sections(page);
    expect([s[0].width, s[0].height]).toEqual([11906, 16838]);
    await expect(page.locator(".flow table td").filter({ hasText: "Ada" })).toHaveCount(14);
  });

  test("your own size: the numbers of the sheet", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await chooseMenu(page, "Mailings", "Labels…");
    const box = dialog(page, "Labels");
    await box.getByLabel("Label vendor").selectOption("custom");
    await box.getByLabel("Labels across").fill("2");
    await box.getByLabel("Labels down").fill("4");
    await box.getByLabel("Label width").fill("3.5");
    await box.getByLabel("Label height").fill("2");
    await box.getByLabel("Horizontal pitch").fill("4");
    await box.getByLabel("Vertical pitch").fill("2.5");
    await box.getByLabel("Top margin").fill("0.5");
    await box.getByLabel("Side margin").fill("0.25");
    await expect(box).toContainText("8 labels on a sheet, 2 across and 4 down. Each is 3.5 in by 2 in.");
    await box.getByLabel("Address").fill("Hello");
    await box.getByRole("button", { name: "New document" }).click();
    await expect.poll(() => fileName(page)).toMatch(/Labels/);
    await expect(page.locator(".flow table td").filter({ hasText: "Hello" })).toHaveCount(8);
  });

  test("an extension adds a label product, and it is listed under its vendor", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await inPage(page, async (w) => {
      await (w.fe as unknown as { use(ext: unknown): Promise<void> }).use({
        id: "acme.labels",
        name: "Acme labels",
        activate(ctx: { mailMerge: { labelProducts: { register(p: unknown): void } } }) {
          ctx.mailMerge.labelProducts.register({ id: "acme.40x20", vendor: "Acme", name: "40 x 20 mm labels", unit: "mm", pageWidth: 210, pageHeight: 297, top: 13, left: 5, labelWidth: 40, labelHeight: 20, pitchH: 40, pitchV: 20, across: 5, down: 14 });
        },
      });
    });
    await chooseMenu(page, "Mailings", "Labels…");
    const box = dialog(page, "Labels");
    await box.getByLabel("Label vendor").selectOption("Acme");
    await expect(box.getByLabel("Product number").locator("option")).toHaveText(["40 x 20 mm labels"]);
    await expect(box).toContainText("70 labels on a sheet, 5 across and 14 down. Each is 40 mm by 20 mm.");
    await box.getByLabel("Address").fill("Hi");
    await box.getByRole("button", { name: "New document" }).click();
    await expect.poll(() => fileName(page)).toMatch(/Labels/);
    await expect(page.locator(".flow table td").filter({ hasText: "Hi" })).toHaveCount(70);
    expect((await sections(page))[0].width).toBe(11906);
  });
});

test.describe("labels and envelopes as the main document of a merge", () => {
  test("Start mail merge, Labels: the sheet, Next Record fields, Update labels, preview and merge", async ({ page }) => {
    await openBuilt(page, PLAIN);
    await startMerge(page, "Labels…");
    const labels = dialog(page, "Labels");
    await labels.getByLabel("Product number").selectOption({ label: "5160 Address labels" });
    await labels.getByRole("button", { name: "OK" }).click();
    // The document has text, so the sheet asks before it replaces it.
    await page.getByRole("alertdialog", { name: "Make a sheet of labels" }).getByRole("button", { name: "Make the labels" }).click();
    expect(await info(page)).toMatchObject({ active: true, mainType: "mailingLabels" });
    // Every label but the first starts with Next Record.
    expect((await fields(page)).filter((f) => f.code === "NEXT")).toHaveLength(29);
    await chooseCsv(page, NAMES);
    await line(page, 0).click();
    await chooseMenu(page, "Mailings", "Insert merge field…");
    const insert = dialog(page, "Insert merge field");
    await insert.getByRole("option", { name: "First Name" }).dblclick();
    await insert.getByRole("button", { name: "Close" }).click();
    await chooseMenu(page, "Mailings", "Update labels");
    expect((await fields(page)).filter((f) => f.code === "MERGEFIELD")).toHaveLength(30);
    // Preview: the first four labels are the first four recipients, in reading order.
    await chooseMenu(page, "Mailings", "Preview results");
    await expect(strip(page)).toContainText("Records 1 to 4 of 4");
    const names = (await onFirstPage(page, ".flow table td")).map((b) => b.text).filter(Boolean);
    expect(names).toEqual(["Ada", "Grace", "Alan", "Edsger"]);
    await strip(page).getByRole("button", { name: "Done" }).click();
    // Finish and merge: one sheet, three rows of the list across it.
    await chooseMenu(page, "Mailings", "Finish and merge…");
    await dialog(page, "Finish and merge").getByRole("button", { name: "Merge" }).click();
    await expect.poll(() => fileName(page)).toMatch(/merged/);
    await expect.poll(() => pagesOf(page)).toBe(1);
    const merged = (await onFirstPage(page, ".flow table td")).map((b) => b.text).filter(Boolean);
    expect(merged).toEqual(["Ada", "Grace", "Alan", "Edsger"]);
    expect(await fields(page)).toEqual([]);
  });

  test("a longer list takes a second sheet", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [""] }] });
    await startMerge(page, "Labels…");
    const labels = dialog(page, "Labels");
    await labels.getByLabel("Product number").selectOption({ label: "5164 Shipping labels" });
    await labels.getByRole("button", { name: "OK" }).click();
    await expect.poll(async () => (await info(page)).mainType).toBe("mailingLabels");
    let csv = "First Name\n";
    for (let i = 1; i <= 8; i++) csv += `Person${i}\n`;
    await chooseCsv(page, csv);
    await line(page, 0).click();
    await chooseMenu(page, "Mailings", "Insert merge field…");
    const insert = dialog(page, "Insert merge field");
    await insert.getByRole("option", { name: "First Name" }).dblclick();
    await insert.getByRole("button", { name: "Close" }).click();
    await chooseMenu(page, "Mailings", "Update labels");
    await chooseMenu(page, "Mailings", "Finish and merge…");
    await dialog(page, "Finish and merge").getByRole("button", { name: "Merge" }).click();
    await expect.poll(() => fileName(page)).toMatch(/merged/);
    // Six labels to a sheet: eight people need two sheets.
    await expect.poll(() => pagesOf(page)).toBe(2);
    expect((await bodyText(page)).match(/Person\d/g)).toHaveLength(8);
  });

  test("Start mail merge, Envelopes: the envelope is the document, with an address block for each recipient", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [""] }] });
    await startMerge(page, "Envelopes…");
    const box = dialog(page, "Envelopes");
    await box.getByLabel("Return address", { exact: true }).fill("Grace Hopper");
    await box.getByLabel("Envelope size").selectOption("c6");
    await box.getByRole("button", { name: "OK" }).click();
    await expect.poll(async () => (await info(page)).mainType).toBe("envelopes");
    const s = await sections(page);
    expect(s).toHaveLength(1);
    expect([s[0].width, s[0].height]).toEqual([9184, 6463]);
    expect((await fields(page)).map((f) => f.code)).toEqual(["ADDRESSBLOCK"]);
    await chooseCsv(page, "First Name,Last Name,Street,City\nAda,Lovelace,12 Crumb Rd,London\nGrace,Hopper,1 Navy Way,Arlington\n");
    await chooseMenu(page, "Mailings", "Preview results");
    await expect(strip(page)).toContainText("Record 1 of 2");
    await expect(page.locator(".flow")).toContainText(/Ada Lovelace\s*12 Crumb Rd\s*London/);
    await strip(page).getByRole("button", { name: "Next record" }).click();
    await expect(page.locator(".flow")).toContainText(/Grace Hopper\s*1 Navy Way\s*Arlington/);
    await strip(page).getByRole("button", { name: "Done" }).click();
    await chooseMenu(page, "Mailings", "Finish and merge…");
    await dialog(page, "Finish and merge").getByRole("button", { name: "Merge" }).click();
    await expect.poll(() => fileName(page)).toMatch(/merged/);
    await expect.poll(() => pagesOf(page)).toBe(2);
    expect((await sections(page))).toHaveLength(2);
  });

  test("a merge letter keeps its merge fields apart from the sheet commands", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ raw: run("Hi ") + mergeField("First_Name") }] }] });
    await openMenu(page, "Mailings");
    await expect(menuItem(page, "Update labels")).toBeDisabled();
    await page.keyboard.press("Escape");
  });
});

test.describe("colours", () => {
  /** The contrast of the text in some elements against what is behind it, light and dark. */
  async function contrasts(page: Page, selectors: string[]) {
    return page.evaluate((sels) => {
      type Rgba = [number, number, number, number];
      const parse = (c: string): Rgba => {
        const m = /rgba?\(([^)]+)\)/.exec(c);
        const p = (m?.[1] ?? "0 0 0").split(/[\s,/]+/).filter(Boolean).map(Number);
        return [p[0], p[1], p[2], p[3] ?? 1];
      };
      const lum = ([r, g, b]: number[]) => {
        const f = (v: number) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
        return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
      };
      const backdrop = (el: Element): [number, number, number] => {
        const layers: Rgba[] = [];
        for (let e: Element | null = el; e; e = e.parentElement) {
          const bg = parse(getComputedStyle(e).backgroundColor);
          if (bg[3] > 0) layers.push(bg);
          if (bg[3] >= 1) break;
        }
        let out: [number, number, number] = [255, 255, 255];
        for (const l of layers.reverse()) out = [l[0] * l[3] + out[0] * (1 - l[3]), l[1] * l[3] + out[1] * (1 - l[3]), l[2] * l[3] + out[2] * (1 - l[3])];
        return out;
      };
      const results: { selector: string; ratio: number }[] = [];
      for (const sel of sels) {
        for (const el of Array.from(document.querySelectorAll(sel)).slice(0, 3)) {
          if (!(el.textContent ?? "").trim()) continue;
          const fg = parse(getComputedStyle(el).color);
          const bg = backdrop(el);
          const a = lum([fg[0] * fg[3] + bg[0] * (1 - fg[3]), fg[1] * fg[3] + bg[1] * (1 - fg[3]), fg[2] * fg[3] + bg[2] * (1 - fg[3])]);
          const b = lum(bg);
          results.push({ selector: sel, ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05) });
        }
      }
      return results;
    }, selectors);
  }

  for (const scheme of ["light", "dark"] as const) {
    test(`dialogs and the preview strip keep their text at 4.5:1 or better in the ${scheme} theme`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await openBuilt(page, { sections: [{ paragraphs: [{ raw: run("Dear ") + mergeField("First_Name") }, "Thanks"] }] });
      await chooseMenu(page, "Mailings", "Start mail merge", "Letters");
      await chooseCsv(page, NAMES);
      await chooseMenu(page, "Mailings", "Edit recipient list…");
      const list = dialog(page, "Recipients");
      await list.getByRole("button", { name: "Filter…" }).click();
      const all = [".mm-recipients h2", ".mm-recipients .sub", ".mm-recipients th", ".mm-recipients td", ".mm-recipients .btn", ".mm-recipients .mm-rule-join", ".mm-recipients .mm-pager"];
      for (const r of await contrasts(page, all)) expect(r.ratio, `${scheme}: ${r.selector}`).toBeGreaterThanOrEqual(4.5);
      await list.getByRole("button", { name: "Cancel" }).click();
      await caretAtEndOf(page, 1);
      await chooseMenu(page, "Mailings", "Address block…");
      const ab = dialog(page, "Insert address block");
      for (const r of await contrasts(page, [".mm-dialog h2", ".mm-dialog .sub", ".mm-dialog label", ".mm-dialog .mm-preview", ".mm-dialog .mm-nav-count", ".mm-dialog .btn"])) expect(r.ratio, `${scheme}: ${r.selector}`).toBeGreaterThanOrEqual(4.5);
      await ab.getByRole("button", { name: "Cancel" }).click();
      await chooseMenu(page, "Mailings", "Preview results");
      for (const r of await contrasts(page, [".mm-strip .mm-strip-where", ".mm-strip .tb"])) expect(r.ratio, `${scheme}: ${r.selector}`).toBeGreaterThanOrEqual(4.5);
      await strip(page).getByRole("button", { name: "Done" }).click();
      await chooseMenu(page, "Mailings", "Envelopes…");
      const env = dialog(page, "Envelopes");
      await env.getByLabel("Delivery address", { exact: true }).fill("Ada Lovelace");
      for (const r of await contrasts(page, [".mm-envelopes h2", ".mm-envelopes label", ".mm-env-text.to", ".mm-envelopes .btn"])) expect(r.ratio, `${scheme}: ${r.selector}`).toBeGreaterThanOrEqual(4.5);
    });
  }
});
