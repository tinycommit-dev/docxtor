// Tab stops drawn as Word lays them out: stops of every kind and leader, the default width, hanging indents,
// multi-line paragraphs, right-to-left paragraphs and absolute position tabs. Positions are checked against the
// stops' twips (15 twips to a pixel) measured from the page margin.
import { test, expect, type Page } from "./helpers";
import { openBuilt, type SectionSpec } from "./support/docx-builder";

const tabs = (...stops: [string, number, string?][]) => `<w:tabs>${stops.map(([v, pos, l]) => `<w:tab w:val="${v}"${l ? ` w:leader="${l}"` : ""} w:pos="${pos}"/>`).join("")}</w:tabs>`;

type P = SectionSpec["paragraphs"][number];

async function open(page: Page, paragraphs: P[], extra: { settings?: string; styles?: string } = {}) {
  await openBuilt(page, { sections: [{ paragraphs }], ...extra });
}

/** Where a word starts and ends, in px from the page's left margin (unzoomed). */
async function span(page: Page, id: number, word: string): Promise<{ left: number; right: number; top: number }> {
  return page.evaluate(
    ([pid, w]) => {
      const flow = document.querySelector<HTMLElement>(".flow")!;
      const fr = flow.getBoundingClientRect();
      const scale = fr.width / flow.offsetWidth;
      const origin = fr.left + parseFloat(getComputedStyle(flow).paddingLeft) * scale;
      const p = document.querySelector(`.flow [data-p="${pid}"]`)!;
      const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const i = (n as Text).data.indexOf(w);
        if (i < 0) continue;
        const r = document.createRange();
        r.setStart(n, i);
        r.setEnd(n, i + w.length);
        const b = r.getBoundingClientRect();
        return { left: (b.left - origin) / scale, right: (b.right - origin) / scale, top: b.top };
      }
      throw new Error(`"${w}" not found`);
    },
    [id, word] as const,
  );
}

const ids = (page: Page) => page.evaluate(() => Array.from(document.querySelectorAll<HTMLElement>(".flow [data-p]")).map((e) => Number(e.dataset.p)));

test.describe("tab stops", () => {
  test("a left stop puts the text at its position, measured from the margin and not from the indent", async ({ page }) => {
    await open(page, [
      { text: "Name{TAB}Value", ppr: tabs(["left", 2880]) },
      { text: "Indented{TAB}Value", ppr: `${tabs(["left", 2880])}<w:ind w:left="720"/>` },
    ]);
    const [a, b] = await ids(page);
    expect((await span(page, a, "Value")).left).toBeCloseTo(2880 / 15, 0);
    expect((await span(page, b, "Value")).left).toBeCloseTo(2880 / 15, 0);
  });

  test("right, centre and decimal stops align the text after the tab", async ({ page, browserName }) => {
    await open(page, [
      { text: "Right{TAB}end", ppr: tabs(["right", 6000]) },
      { text: "Centre{TAB}middle", ppr: tabs(["center", 4000]) },
      { text: "Price{TAB}12.50", ppr: tabs(["decimal", 4500]) },
      { text: "Cost{TAB}3.1", ppr: tabs(["decimal", 4500]) },
      { text: "Whole{TAB}250", ppr: tabs(["decimal", 4500]) },
    ]);
    const [r, c, d1, d2, d3] = await ids(page);
    expect((await span(page, r, "end")).right).toBeCloseTo(6000 / 15, 0);
    const m = await span(page, c, "middle");
    expect((m.left + m.right) / 2).toBeCloseTo(4000 / 15, 0);
    // The decimal points line up at the stop; a number with none ends there.
    const point = async (id: number, text: string) => {
      const s = await span(page, id, text.split(".")[0]);
      const dot = await span(page, id, ".");
      return { dot: dot.left, end: s.right };
    };
    // (The width of the text before the point comes from the browser's own measure, which Safari gives to a whole pixel: the point lands within one.)
    const tol = browserName === "webkit" ? 1.5 : 0.5;
    expect(Math.abs((await point(d1, "12.50")).dot - 4500 / 15)).toBeLessThan(tol);
    expect(Math.abs((await point(d2, "3.1")).dot - 4500 / 15)).toBeLessThan(tol);
    expect(Math.abs((await span(page, d3, "250")).right - 4500 / 15)).toBeLessThan(tol);
  });

  test("leaders fill the tab, on every kind of stop", async ({ page }) => {
    await open(page, [
      { text: "One{TAB}1", ppr: tabs(["right", 6000, "dot"]) },
      { text: "Two{TAB}2", ppr: tabs(["left", 3000, "hyphen"]) },
      { text: "Three{TAB}3", ppr: tabs(["center", 4000, "underscore"]) },
      { text: "Four{TAB}4", ppr: tabs(["decimal", 5000, "middleDot"]) },
      { text: "Five{TAB}5", ppr: tabs(["right", 6000, "heavy"]) },
    ]);
    for (const [i, leader] of ["dot", "hyphen", "underscore", "middleDot", "heavy"].entries()) {
      const tab = page.locator(".flow .fp").nth(i).locator(".ftab");
      await expect(tab).toHaveAttribute("data-leader", leader);
      await expect(tab).toHaveClass(/tab-fill/);
      expect(await tab.evaluate((e) => getComputedStyle(e).backgroundImage)).not.toBe("none");
    }
  });

  test("past the last stop tabs go to the default width, which the document sets", async ({ page }) => {
    await open(page, [{ text: "a{TAB}b{TAB}c{TAB}d", ppr: tabs(["left", 1000]) }, { text: "x{TAB}y" }], { settings: '<w:defaultTabStop w:val="1440"/>' });
    const [p1, p2] = await ids(page);
    // 1000 twips is the only custom stop; after it the grid is 1440 twips (96 px) from the margin.
    expect((await span(page, p1, "b")).left).toBeCloseTo(1000 / 15, 0);
    expect((await span(page, p1, "c")).left).toBeCloseTo(96 * 1, 0);
    expect((await span(page, p1, "d")).left).toBeCloseTo(96 * 2, 0);
    // With no custom stop the default grid starts at the margin.
    expect((await span(page, p2, "y")).left).toBeCloseTo(96, 0);
  });

  test("the default width is 0.5 inch when the file says nothing, and applies from the margin, not the indent", async ({ page }) => {
    await open(page, [{ text: "x{TAB}y", ppr: '<w:ind w:left="240"/>' }, { text: "a long line of words for the tab{TAB}y" }]);
    const [p1, p2] = await ids(page);
    expect((await span(page, p1, "y")).left).toBeCloseTo(48, 0);
    // Text that runs past a stop goes on to the next one: the first multiple of 48 px beyond the text.
    const end = (await span(page, p2, "tab")).right;
    expect((await span(page, p2, "y")).left).toBeCloseTo((Math.floor(end / 48) + 1) * 48, 0);
  });

  test("a hanging indent is a stop: a number then a tab puts the text at the indent", async ({ page }) => {
    await open(page, [{ text: "1.{TAB}First item", ppr: '<w:ind w:left="720" w:hanging="360"/>' }]);
    const [p] = await ids(page);
    expect((await span(page, p, "First")).left).toBeCloseTo(720 / 15, 0);
    expect((await span(page, p, "1.")).left).toBeCloseTo(360 / 15, 0);
  });

  test("each line of a paragraph with line breaks is laid out on its own", async ({ page }) => {
    await open(page, [{ text: "Name{TAB}Anna{LBR}Street{TAB}Main{LBR}City{TAB}Oslo", ppr: tabs(["left", 2400]) }]);
    const [p] = await ids(page);
    for (const w of ["Anna", "Main", "Oslo"]) expect((await span(page, p, w)).left).toBeCloseTo(160, 0);
    const tops = [(await span(page, p, "Anna")).top, (await span(page, p, "Main")).top, (await span(page, p, "Oslo")).top];
    expect(tops[1]).toBeGreaterThan(tops[0] + 5);
    expect(tops[2]).toBeGreaterThan(tops[1] + 5);
  });

  test("a stop cleared by the paragraph is gone, and a style's stops are inherited", async ({ page }) => {
    const styles = `<w:style w:type="paragraph" w:customStyle="1" w:styleId="Form"><w:name w:val="Form"/><w:basedOn w:val="Normal"/><w:pPr>${tabs(["left", 1440], ["left", 4320])}</w:pPr></w:style>`;
    await open(
      page,
      [
        { text: "a{TAB}b{TAB}c", ppr: '<w:pStyle w:val="Form"/>' },
        { text: "a{TAB}b{TAB}c", ppr: `<w:pStyle w:val="Form"/>${tabs(["clear", 1440])}` },
      ],
      { styles },
    );
    const [s1, s2] = await ids(page);
    expect((await span(page, s1, "b")).left).toBeCloseTo(96, 0);
    expect((await span(page, s1, "c")).left).toBeCloseTo(288, 0);
    // Without the 1440 stop the first tab goes to the 4320 one, and the second to the default grid after it.
    expect((await span(page, s2, "b")).left).toBeCloseTo(288, 0);
    expect((await span(page, s2, "c")).left).toBeGreaterThan(288 + 20);
  });

  test("a bar stop draws a line and a tab skips it", async ({ page }) => {
    await open(page, [{ text: "a{TAB}b", ppr: tabs(["bar", 2160], ["left", 3600]) }]);
    const [p] = await ids(page);
    expect((await span(page, p, "b")).left).toBeCloseTo(240, 0);
    const bg = await page.locator(".flow .fp").first().evaluate((e) => ({ image: getComputedStyle(e).backgroundImage, pos: getComputedStyle(e).backgroundPositionX }));
    expect(bg.image).toContain("linear-gradient");
    expect(bg.pos).toBe("144px");
  });

  test("absolute position tabs in a header put the text at the left, centre and right of the margins", async ({ page }) => {
    const ptab = (align: string, rel = "margin", leader = "none") => `<w:r><w:ptab w:relativeTo="${rel}" w:alignment="${align}" w:leader="${leader}"/></w:r>`;
    const run = (t: string) => `<w:r><w:t>${t}</w:t></w:r>`;
    await openBuilt(page, {
      sections: [{ paragraphs: ["Body"], headers: { default: "", raw: `${run("Left")}${ptab("center")}${run("Centre")}${ptab("right", "margin", "dot")}${run("Right")}` } }],
    });
    const hdr = page.locator(".hdr .fp").first();
    await expect(hdr).toContainText("Right");
    const m = await hdr.evaluate((e) => {
      const rect = (word: string) => {
        const walker = document.createTreeWalker(e, NodeFilter.SHOW_TEXT);
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          const i = (n as Text).data.indexOf(word);
          if (i < 0) continue;
          const r = document.createRange();
          r.setStart(n, i);
          r.setEnd(n, i + word.length);
          return r.getBoundingClientRect();
        }
        throw new Error(word);
      };
      const p = e.getBoundingClientRect();
      const c = rect("Centre");
      return { left: rect("Left").left - p.left, centre: (c.left + c.right) / 2 - p.left, right: p.right - rect("Right").right, width: p.width, leader: e.querySelector<HTMLElement>(".fptab[data-leader]")?.dataset.leader };
    });
    // The header is the width of the text area (6.5 inches = 624 px): left text at the start, centre at the middle, right at the end.
    expect(m.width).toBeCloseTo(624, 0);
    expect(m.left).toBeCloseTo(0, 0);
    expect(m.centre).toBeCloseTo(312, 0);
    expect(m.right).toBeCloseTo(0, 0);
    expect(m.leader).toBe("dot");
  });

  test("tabs work in a right-to-left paragraph, counted from the right margin", async ({ page }) => {
    // Hebrew words: with Latin letters in a right-to-left paragraph, Chrome puts the tab between two runs of the paragraph's direction (so the second word is
    // at the stop), and Firefox and Safari keep "ABC", the tab and "XYZ" as one left-to-right run lying against the right margin, which has no stop to speak of.
    await open(page, [{ text: "אבג{TAB}דהו", ppr: `<w:bidi/>${tabs(["left", 3000])}` }]);
    const [p] = await ids(page);
    const el = page.locator(`.flow [data-p="${p}"]`);
    const geo = await el.evaluate((e, w) => {
      const flow = document.querySelector<HTMLElement>(".flow")!;
      const fr = flow.getBoundingClientRect();
      const right = fr.right - parseFloat(getComputedStyle(flow).paddingRight);
      const walker = document.createTreeWalker(e, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const i = (n as Text).data.indexOf(w);
        if (i < 0) continue;
        const r = document.createRange();
        r.setStart(n, i);
        r.setEnd(n, i + w.length);
        return right - r.getBoundingClientRect().right;
      }
      return -1;
    }, "דהו");
    // The stop is 3000 twips from the right margin; the text after it starts there and runs towards the left.
    expect(geo).toBeCloseTo(200, 0);
  });
});
