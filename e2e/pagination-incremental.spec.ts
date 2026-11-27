// Pagination from the page that changed. After an edit the paginator starts at the block before the first one drawn and stops
// where a block is reached in the state the last pass reached it in; everything after that stays as it was. The result must
// be what a pass over the whole text gives, and the tests run with the paginator's own check on (e2e/helpers.ts sets
// `__DOCXTOR_PAGINATION_CHECKS__`): after every incremental layout a full one is made in place and compared, page by page,
// spacer by spacer and margin by margin, and a difference throws. So these tests make random edits and let the check judge,
// and then count what an edit costs.
import { readFileSync } from "node:fs";
import { openBuilt, type SectionSpec } from "./support/docx-builder";
import { endOfLine } from "./support/controls-markup";
import { expect, inPage, openFixture, showPara, test, type Page } from "./helpers";

/** A small seeded random generator, so a failure can be run again. */
function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

interface Counts {
  fullPasses: number;
  incPasses: number;
  synced: number;
  measured: number;
  lastMeasured: number;
  lastIncremental: boolean;
  fallbacks: Record<string, number>;
}
const stats = (page: Page): Promise<Counts> => inPage(page, (w) => JSON.parse(JSON.stringify((w.fe.editor as unknown as { paginator: { stats: unknown } }).paginator.stats)));

const WORDS = "the quick brown fox jumps over the lazy dog while seven bright lanterns swing above a quiet harbour at dusk".split(" ");

type Kind = "type" | "enter" | "join" | "delete" | "paste" | "break" | "footnote" | "table" | "undo" | "redo" | "backspace";

/** One random edit, chosen from `kinds`, through the editor's own `exec` (it lays out at once for a document this size). */
async function edit(page: Page, kinds: Kind[], next: () => number, log: string[]) {
  const kind = kinds[Math.floor(next() * kinds.length)];
  log.push(kind);
  try {
  await inPage(
    page,
    (w, a) => {
      const ed = w.fe.editor as unknown as { exec(ops: unknown): unknown; undo(): void; redo(): void; engine: { renderAll(): { order: number[]; blocks: { t: string; id: number; len?: number }[] } } };
      const all = ed.engine.renderAll();
      const paras = all.blocks.filter((b) => b.t === "p");
      if (!paras.length) return;
      const at = paras[Math.floor(a.x * paras.length)];
      const o = Math.floor(a.y * ((at.len ?? 0) + 1));
      const words = a.words as string[];
      const text = Array.from({ length: 1 + Math.floor(a.z * a.n) }, (_, i) => words[(Math.floor(a.x * 997) + i) % words.length]).join(" ");
      switch (a.kind as Kind) {
        case "type":
          ed.exec({ op: "insert_text", at: { p: at.id, o }, text: text + " " });
          break;
        case "enter":
          ed.exec({ op: "split_paragraph", at: { p: at.id, o } });
          break;
        case "join": {
          const i = all.order.indexOf(at.id);
          const nextBlock = all.blocks.find((b) => b.id === all.order[i + 1]);
          if (nextBlock?.t === "p") ed.exec({ op: "delete", from: { p: at.id, o: at.len ?? 0 }, to: { p: nextBlock.id, o: 0 } });
          break;
        }
        case "backspace":
          if (o > 1) ed.exec({ op: "delete", from: { p: at.id, o: o - 1 }, to: { p: at.id, o } });
          break;
        case "delete":
          if (paras.length > 6) ed.exec({ op: "delete_blocks", ids: [at.id] });
          break;
        case "paste":
          ed.exec({ op: "insert_paragraph", after: at.id, text: Array.from({ length: 6 }, () => text).join(" ") });
          break;
        case "break":
          ed.exec({ op: "insert_page_break", at: { p: at.id, o } });
          break;
        case "footnote":
          ed.exec({ op: "insert_footnote", at: { p: at.id, o }, text: `A note ${text}` });
          break;
        case "table":
          ed.exec({ op: "insert_table", after: at.id, rows: 2 + Math.floor(a.z * 5), cols: 2 });
          break;
        case "undo":
          ed.undo();
          break;
        case "redo":
          ed.redo();
          break;
      }
    },
    { kind, x: next(), y: next(), z: next(), n: 40, words: WORDS },
  );
  } catch (e) {
    throw new Error(`after ${log.length} edits (${log.join(", ")}): ${(e as Error).message}`);
  }
}

const MIX: Kind[] = ["type", "type", "type", "type", "enter", "enter", "join", "backspace", "delete", "paste", "undo", "undo", "redo"];
const MIX_PLUS: Kind[] = [...MIX, "break", "footnote", "table"];

async function randomEdits(page: Page, seed: number, steps: number, kinds: Kind[]) {
  const next = rng(seed);
  const log: string[] = [];
  for (let i = 0; i < steps; i++) await edit(page, kinds, next, log);
  // The last one is in, and the layout it started is done: the editor has nothing left to lay out.
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done(null)))));
}

/**
 * What the page map answers (the page of each paragraph, the first paragraph on each page) against what the rectangles say, which is
 * how the editor worked these out before the paginator kept a map. They must be the same.
 */
async function agree(page: Page) {
  const r = await inPage(page, (w) => {
    const ed = w.fe.editor as unknown as {
      flow: HTMLElement;
      stage: HTMLElement;
      zoom: number;
      pages: number;
      paginator: { page(k: number): { top: number; num: number }; pageAt(y: number): number };
      pageMap(): Record<string, number>;
      firstParagraphOnPage(k: number): number | null;
      withDrawn<T>(fn: () => T): T;
    };
    // The rectangles of every paragraph are read on purpose: with far pages left out they are zeros, so the pages are drawn first.
    return ed.withDrawn(() => {
    const stage = ed.stage.getBoundingClientRect().top;
    const all = Array.from(ed.flow.querySelectorAll<HTMLElement>("[data-p]"));
    const rects: Record<string, number> = {};
    for (const el of all) rects[el.dataset.p!] = ed.paginator.page(ed.paginator.pageAt((el.getBoundingClientRect().top - stage) / ed.zoom)).num;
    const firsts: (number | null)[] = [];
    for (let k = 0; k < ed.pages; k++) {
      const top = ed.paginator.page(k).top;
      let found: number | null = null;
      for (const el of all) {
        if (el.closest("[data-fold]")) continue;
        if ((el.getBoundingClientRect().top - stage) / ed.zoom >= top - 1) {
          found = Number(el.dataset.p);
          break;
        }
      }
      firsts.push(found);
    }
    return { map: ed.pageMap(), rects, firstsMap: Array.from({ length: ed.pages }, (_, k) => ed.firstParagraphOnPage(k)), firsts };
    });
  });
  expect(r.map).toEqual(r.rects);
  expect(r.firstsMap).toEqual(r.firsts);
}

const paragraphs = (n: number, prefix = "Paragraph") => Array.from({ length: n }, (_, i) => `${prefix} ${i}: ${WORDS.map((_, k) => WORDS[(k + i) % WORDS.length]).join(" ")}. ${WORDS.slice(i % 5).join(" ")}.`);

const note = (id: number, text: string) => `<w:footnote w:id="${id}"><w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> ${text}</w:t></w:r></w:p></w:footnote>`;
const ref = (id: number) => `<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="${id}"/></w:r>`;
const NOTE_STYLES = '<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:pPr><w:spacing w:after="0"/></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:qFormat/><w:pPr><w:keepNext/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>';

/** A document with footnotes spread over its pages, headings that keep with the next paragraph, and a table. */
function notedDocument(count: number): Parameters<typeof openBuilt>[1] {
  const body = paragraphs(count).map((text, i): SectionSpec["paragraphs"][number] => {
    if (i % 17 === 0) return { text: `Heading ${i}`, ppr: '<w:pStyle w:val="Heading1"/>' };
    if (i % 11 === 3) return { text, xml: ref(1 + (i % 9)) };
    return text;
  });
  return { sections: [{ paragraphs: body }], footnotes: Array.from({ length: 9 }, (_, i) => note(1 + i, `The note ${i + 1} says something that takes a line or two of the page it stands on.`)).join(""), styles: NOTE_STYLES };
}

test.describe("pagination from the page that changed", () => {
  // More seeds and longer runs: DOCXTOR_PAGINATION_SEEDS=20 DOCXTOR_PAGINATION_STEPS=120.
  const SEEDS = Array.from({ length: Number(process.env.DOCXTOR_PAGINATION_SEEDS ?? 2) }, (_, i) => 11 + i * 7);
  const STEPS = Number(process.env.DOCXTOR_PAGINATION_STEPS ?? 40);
  for (const name of ["rich.docx", "long.docx", "complex.docx"]) {
    for (const seed of SEEDS) {
      test(`random edits in ${name}, seed ${seed}, give what a full pass gives`, async ({ page }) => {
        await openFixture(page, name);
        await randomEdits(page, seed, STEPS, MIX_PLUS);
        await agree(page);
        const s = await stats(page);
        // The check ran after every incremental layout (it throws when they differ), and most of them were incremental.
        expect(s.incPasses).toBeGreaterThan(STEPS / 4);
      });
    }
  }

  for (const seed of SEEDS) {
    test(`random edits in a document with footnotes, headings and tables, seed ${seed}`, async ({ page }) => {
      await openBuilt(page, notedDocument(160));
      await randomEdits(page, seed, STEPS, MIX_PLUS);
      await agree(page);
      const s = await stats(page);
      expect(s.incPasses).toBeGreaterThan(STEPS / 4);
    });
  }

  test("random edits in a long built document, typed with the keyboard and by ops", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: paragraphs(500) }] });
    await randomEdits(page, 3, 30, MIX);
    // Typing with the keyboard lays the pages out after a pause in a document this long.
    await showPara(page, 250);
    await page.locator(".flow [data-p]").nth(250).click();
    await endOfLine(page);
    await page.keyboard.type("A line of typing that runs on for a while, long enough to need another line of the paragraph. ");
    await page.waitForTimeout(400);
    await randomEdits(page, 4, 20, MIX);
    await agree(page);
    const s = await stats(page);
    expect(s.incPasses).toBeGreaterThan(10);
    expect(s.fullPasses).toBeLessThan(10);
  });

  test("a key in the middle of a long document measures a few blocks, whatever the length", async ({ page }) => {
    const measured: number[] = [];
    for (const n of [300, 1200]) {
      await openBuilt(page, { sections: [{ paragraphs: paragraphs(n) }] });
      await showPara(page, Math.floor(n / 2));
      await page.locator(".flow [data-p]").nth(Math.floor(n / 2)).click();
      await endOfLine(page);
      await page.waitForTimeout(300);
      const before = await stats(page);
      await page.keyboard.type("x");
      await page.waitForTimeout(400);
      const after = await stats(page);
      expect(after.fullPasses).toBe(before.fullPasses);
      expect(after.incPasses).toBe(before.incPasses + 1);
      expect(after.lastIncremental).toBe(true);
      measured.push(after.lastMeasured);
    }
    expect(measured[0]).toBeLessThan(20);
    expect(measured[1]).toBe(measured[0]);
  });
});

/**
 * Notes that stand near the bottom of their pages, so that keeping room for one pushes the next one on, and that one the next:
 * the footnote room is not settled by the first pass (a note on page 53 moves every later one a page or two on), it takes three more. The notes start late in the text, as in a book whose notes are in its last chapters: a pass that starts where the room changed then measures a fraction of the text.
 * (The document settles. Room that swings between two answers has its own tests below.)
 */
function cascadeDocument(pages: number, every: number, noteFrom: number): Parameters<typeof openBuilt>[1] {
  const body = Array.from({ length: pages * every }, (_, i): SectionSpec["paragraphs"][number] => {
    const text = `Line ${i}`;
    return i % every === every - 1 && i >= noteFrom * every ? { text, xml: ref(1 + (((i / every) | 0) % 2)) } : text;
  });
  const notes = [note(1, "A short note."), note(2, "A longer note that says something which takes more than one line of the page it stands on, and then some more words after that.")];
  return { sections: [{ paragraphs: body }], footnotes: notes.join(""), styles: NOTE_STYLES };
}

test.describe("footnote room in a full layout", () => {
  test("a full layout is one full pass, then passes from where the room changed, and gives what passes from the top give", async ({ page }) => {
    await openBuilt(page, cascadeDocument(200, 44, 150));
    const r = await inPage(page, (w) => {
      type Pg = { invalidate(): void; stats: Record<string, number>; reserve: number[]; pages: number; layoutBody(inc: null, from: number[] | null, top: boolean): number; verify: boolean; noteNeeds(): number[]; notesInPass: boolean };
      const ed = w.fe.editor as unknown as { layoutNow(): void; paginator: Pg; pageMap(): Record<string, number> };
      const pg = ed.paginator;
      const counts = () => ({ full: pg.stats.fullPasses, inc: pg.stats.incPasses, measured: pg.stats.measured });
      const since = (a: ReturnType<typeof counts>) => ({ full: pg.stats.fullPasses - a.full, inc: pg.stats.incPasses - a.inc, measured: pg.stats.measured - a.measured });
      const result = () => ({ reserve: pg.reserve.slice(), pages: pg.pages, map: ed.pageMap(), settled: pg.noteNeeds().every((v, k) => Math.abs((v ?? 0) - (pg.reserve[k] ?? 0)) < 1) });
      // The check is off here: this test makes the comparison itself.
      pg.verify = false;
      // From nothing, the way every layout of a document did.
      let a = counts();
      pg.invalidate();
      pg.layoutBody(null, null, true);
      const scratch = { ...since(a), ...result() };
      // The same, with the passes after the first starting where the room changed (the first pass leaves the notes' room to them).
      pg.notesInPass = false;
      a = counts();
      pg.invalidate();
      pg.reserve = [];
      ed.layoutNow();
      const fromNothing = { ...since(a), ...result() };
      // With the room the last layout found, as a zoom does.
      a = counts();
      pg.invalidate();
      ed.layoutNow();
      const carried = { ...since(a), ...result() };
      // From nothing, with the first pass giving each page its notes' room as it fills it (what a layout does).
      pg.notesInPass = true;
      a = counts();
      pg.invalidate();
      pg.reserve = [];
      ed.layoutNow();
      const inPass = { ...since(a), ...result() };
      return { scratch, fromNothing, carried, inPass };
    });
    // Reserving room moves later notes: the room is not settled by the first pass, and from the top every pass measures everything.
    expect(r.scratch.settled).toBe(true);
    expect(r.scratch.full).toBe(4);
    expect(r.scratch.inc).toBe(0);
    for (const l of [r.fromNothing, r.carried, r.inPass]) {
      expect(l.reserve).toEqual(r.scratch.reserve);
      expect(l.pages).toBe(r.scratch.pages);
      expect(l.map).toEqual(r.scratch.map);
      expect(l.settled).toBe(true);
      expect(l.full).toBe(1);
    }
    // From nothing: the same rounds, but each after the first measures only from where the room changed.
    expect(r.fromNothing.inc).toBe(3);
    expect(r.fromNothing.measured).toBeLessThan(r.scratch.measured * 0.6);
    // From the room the last layout found: nothing changes, so one pass.
    expect(r.carried.inc).toBe(0);
    expect(r.carried.measured).toBeLessThan(r.scratch.measured / 3);
    // Room given as the pages fill: one pass, and nothing after it.
    expect(r.inPass.inc).toBe(0);
    expect(r.inPass.measured).toBe(r.scratch.measured / 4);
  });

  test("a full layout in a document with notes agrees with one from scratch while the check is on", async ({ page }) => {
    // The check (on in every spec) lays out again from nothing after each full layout that started from some room, and throws if they differ.
    await openBuilt(page, cascadeDocument(200, 44, 150));
    await inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { layoutNow(): void; paginator: { invalidate(): void } };
      for (let i = 0; i < 2; i++) {
        ed.paginator.invalidate();
        ed.layoutNow();
      }
    });
  });
});

/**
 * A note whose reference is the last line of a page, with a note too tall for what is left under it: room for the note pushes the
 * line to the next page, which leaves the first page needing no room, so the line comes back, and so on. The paginator keeps the
 * room that moves just the line as a floor (see `layoutBody`): the line stays on the next page with its note, and the page it left is filled up to it.
 */
type Pg = {
  invalidate(): void;
  reserve: number[];
  pages: number;
  verify: boolean;
  notesSettled: boolean;
  notesByPage: { id: string }[][];
  stats: { fullPasses: number; incPasses: number; measured: number; fallbacks: Record<string, number> };
  layoutBody(inc: null, from: number[] | null, top: boolean): number;
};
type Ed = {
  layoutNow(): void;
  paginator: Pg;
  pageMap(): Record<string, number>;
  exec(ops: unknown): unknown;
  engine: { renderAll(): { order: number[] } };
};

test.describe("footnote room that swings between two answers", () => {
  const LONG = "A note much too long to leave a line of the page under its reference, so that room for it pushes the line on, because it takes a good three or four lines of the page it stands on before it is done.";

  test("a reference pushed on by the room for its own note ends with its note on the next page, from anywhere, and stays there", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: Array.from({ length: 140 }, (_, i) => `Line ${i}`) }], footnotes: note(1, "A first note."), styles: NOTE_STYLES });
    const r = await inPage(page, (w, long) => {
      const ed = w.fe.editor as unknown as Ed;
      const pg = ed.paginator;
      const map = ed.pageMap();
      const order = ed.engine.renderAll().order;
      // The last paragraph that starts on page 2 carries the reference: it is the last line of that page.
      const onSecond = order.filter((id) => map[id] === 2);
      const last = onSecond[onSecond.length - 1];
      // Through the editor's own exec: the layout it starts is checked against a full one (the check is on in every spec).
      ed.exec({ op: "insert_footnote", at: { p: last, o: 6 }, text: long });
      const result = () => ({
        reserve: Array.from({ length: pg.pages }, (_, k) => pg.reserve[k] ?? 0),
        pages: pg.pages,
        map: ed.pageMap(),
        notes: pg.notesByPage.map((l) => l?.map((n) => n.id).join(",") ?? ""),
        settled: pg.notesSettled,
      });
      const edit = result();
      // The check is off from here: this test makes the comparisons itself.
      pg.verify = false;
      pg.invalidate();
      pg.layoutBody(null, null, true);
      const scratch = result();
      pg.invalidate();
      pg.reserve = [];
      ed.layoutNow();
      const fromNothing = result();
      // From the room the layout ended with (a zoom does this), and again from that.
      pg.invalidate();
      ed.layoutNow();
      const carried = result();
      pg.invalidate();
      ed.layoutNow();
      const again = result();
      // From each of the two answers the loop swings between, and from the larger of them.
      const [a, b] = [pg.reserve.findIndex((v) => v > 0), 0];
      void b;
      const room = scratch.reserve;
      const starts = [room.map((v, k) => (k === a ? 0 : v)), room.map((v, k) => (k === a + 1 ? 0 : v)), room.map((v, k) => (k === a || k === a + 1 ? v || room[a] : v))];
      const others = starts.map((s) => {
        pg.invalidate();
        pg.reserve = s;
        ed.layoutNow();
        return result();
      });
      const refParagraph = last;
      const beforeRef = order[order.indexOf(last) - 1];
      return { edit, scratch, fromNothing, carried, again, others, page: a, refParagraph, beforeRef };
    }, LONG);
    const s = r.scratch;
    // The reference is on the third page of the lines (index 2 here counts from 0): its page keeps room, but shows no note; the next one shows it.
    const first = r.scratch.reserve.findIndex((v) => v > 0);
    expect(first).toBeGreaterThanOrEqual(0);
    expect(s.notes[first] ?? "").toBe("");
    expect(s.notes[first + 1]).not.toBe("");
    expect(s.reserve[first + 1]).toBeGreaterThan(0);
    expect(s.settled).toBe(true);
    // The page the reference left is filled up to the reference's line, as in Word (only that line goes on): the paragraph before the
    // reference's paragraph is still on the page (the map counts pages from 1, the reserve from 0), and the room it keeps is less than the room its note takes on the next page.
    const refParagraph = r.refParagraph;
    expect(s.map[r.beforeRef]).toBe(first + 1);
    expect(s.map[refParagraph]).toBe(first + 2);
    expect(s.reserve[first]).toBeGreaterThan(0);
    expect(s.reserve[first]).toBeLessThan(s.reserve[first + 1]);
    for (const l of [r.edit, r.fromNothing, r.carried, r.again, ...r.others]) {
      expect(l.reserve).toEqual(s.reserve);
      expect(l.pages).toBe(s.pages);
      expect(l.map).toEqual(s.map);
      expect(l.notes).toEqual(s.notes);
      expect(l.settled).toBe(true);
    }
  });

  test("the same with the note in the middle of a cascade of notes, and after typing before it", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: Array.from({ length: 300 }, (_, i) => `Line ${i}`) }], footnotes: note(1, "A first note."), styles: NOTE_STYLES });
    const r = await inPage(page, (w, long) => {
      const ed = w.fe.editor as unknown as Ed;
      const pg = ed.paginator;
      const order = () => ed.engine.renderAll().order;
      const lastOn = (n: number) => {
        const map = ed.pageMap();
        const on = order().filter((id) => map[id] === n);
        return on[on.length - 1];
      };
      // Notes on the last lines of three pages: one pushed on shifts the next pages by a line.
      const refs: number[] = [];
      for (const n of [2, 3, 4]) {
        const p = lastOn(n);
        refs.push(p);
        ed.exec({ op: "insert_footnote", at: { p, o: 6 }, text: long });
      }
      const before = refs.map((p) => order()[order().indexOf(p) - 1]);
      const result = () => ({ reserve: Array.from({ length: pg.pages }, (_, k) => pg.reserve[k] ?? 0), map: ed.pageMap(), notes: pg.notesByPage.map((l) => l?.map((x) => x.id).join(",") ?? ""), settled: pg.notesSettled });
      // Typing in the first page (an incremental layout, checked against a full one).
      const first = order()[0];
      for (let i = 0; i < 3; i++) ed.exec({ op: "insert_text", at: { p: first, o: 0 }, text: "ab" });
      const edited = result();
      pg.verify = false;
      pg.invalidate();
      pg.layoutBody(null, null, true);
      const scratch = result();
      pg.invalidate();
      ed.layoutNow();
      const carried = result();
      return { edited, scratch, carried, refs, before };
    }, LONG);
    expect(r.scratch.settled).toBe(true);
    // Only the line with the reference goes on with its note: it is the first line of its page, and the line before it ends the page before.
    r.refs.forEach((p, i) => expect(r.scratch.map[p]).toBe(r.scratch.map[r.before[i]] + 1));
    expect(r.edited).toEqual(r.scratch);
    expect(r.carried).toEqual(r.scratch);
  });
});

test.describe("many pages whose reference the room for its own note pushes on", () => {
  test("the first pass gives the rounds' answer, measures no more, and a key typed after it changes nothing", async ({ page }) => {
    const LONG = "A note much too long to leave a line of the page under its reference, so that room for it pushes the line on, because it takes a good three or four lines of the page it stands on before it is done.";
    await openBuilt(page, { sections: [{ paragraphs: Array.from({ length: 2400 }, (_, i) => `Line ${i}`) }], footnotes: note(1, "A first note."), styles: NOTE_STYLES });
    const r = await inPage(page, (w, long) => {
      const ed = w.fe.editor as unknown as Ed;
      const pg = ed.paginator as Pg & { notesInPass: boolean };
      const order = () => ed.engine.renderAll().order;
      // A long note on the last line of every other page from the second on: each is pushed on by its own note's room, and moves the
      // pages after it, so the chases meet.
      const map = ed.pageMap();
      const last = new Map<number, string>();
      for (const id of order()) last.set(map[id], id as unknown as string);
      const at = [...last.keys()].filter((k) => k >= 2 && k % 2 === 0).slice(0, 20).map((k) => last.get(k)!);
      ed.exec(at.map((p) => ({ op: "insert_footnote", at: { p, o: 6 }, text: long })));
      const result = () => ({ reserve: Array.from({ length: pg.pages }, (_, k) => pg.reserve[k] ?? 0), pages: pg.pages, map: ed.pageMap(), notes: pg.notesByPage.map((l) => l?.map((x) => x.id).join(",") ?? ""), settled: pg.notesSettled });
      const counts = () => ({ full: pg.stats.fullPasses, inc: pg.stats.incPasses, measured: pg.stats.measured });
      const since = (a: ReturnType<typeof counts>) => ({ full: pg.stats.fullPasses - a.full, inc: pg.stats.incPasses - a.inc, measured: pg.stats.measured - a.measured });
      // Each layout below is checked against the rounds from no room as it is made (the check is on in every spec); counted without it.
      const run = (on: boolean) => {
        pg.notesInPass = on;
        pg.verify = false;
        let a = counts();
        pg.invalidate();
        pg.reserve = [];
        ed.layoutNow();
        const n = since(a);
        pg.verify = true;
        pg.invalidate();
        pg.reserve = [];
        ed.layoutNow();
        return { ...n, ...result() };
      };
      const off = run(false);
      const on = run(true);
      pg.verify = false;
      pg.invalidate();
      pg.layoutBody(null, null, true);
      const scratch = result();
      // The pass's answer, then a key typed on the first page (an incremental layout, checked).
      pg.verify = true;
      pg.invalidate();
      pg.reserve = [];
      ed.layoutNow();
      const before = result();
      ed.exec({ op: "insert_text", at: { p: order()[0], o: 0 }, text: "a" });
      const typed = result();
      const ids = order() as unknown as string[];
      const lines = at.map((p) => ({ p, before: ids[ids.indexOf(p) - 1] }));
      return { off, on, scratch, before, typed, refs: at.length, lines };
    }, LONG);
    expect(r.refs).toBe(20);
    expect(r.scratch.settled).toBe(true);
    // Only the line with the first reference goes on with its note: it is the first line of its page, and the line before it ends the page before.
    // (Only the first is a chase: it moves every page after it by a line, so the other references are no longer on the last line of their pages.)
    expect(r.scratch.map[r.lines[0].p]).toBe(r.scratch.map[r.lines[0].before] + 1);
    for (const l of [r.off, r.on, r.before]) {
      expect(l.reserve).toEqual(r.scratch.reserve);
      expect(l.pages).toBe(r.scratch.pages);
      expect(l.map).toEqual(r.scratch.map);
      expect(l.notes).toEqual(r.scratch.notes);
      expect(l.settled).toBe(true);
    }
    expect(r.typed.reserve).toEqual(r.before.reserve);
    expect(r.typed.notes).toEqual(r.before.notes);
    expect(r.on.measured).toBeLessThanOrEqual(r.off.measured);
    console.log("chases", JSON.stringify({ off: [r.off.full, r.off.inc, r.off.measured], on: [r.on.full, r.on.inc, r.on.measured] }));
  });
});

test.describe("where chases meet", () => {
  // A long note on the last line of a page pushes that line on (room for the note), and the text after it moves by a line. The next
  // reference is placed on the last line of the page after the one the last reference landed on, after the chases before it have
  // moved the text, and so on: the chases meet. A page keeps room only for its own notes or, where its reference's line went on to
  // the next page, only the room that moves that line (less than the room of the note): it is filled up to the line, as in Word.
  // (One cycle of the rounds settles one more chase: the rounds go on while a cycle still floors a page that had none.)
  for (const count of [4, 12]) test(`${count} chases meet: a page keeps no room it does not need, from anywhere, and typing changes nothing`, async ({ page }) => {
    test.setTimeout(120_000);
    const LONG = "A note much too long to leave a line of the page under its reference, so that room for it pushes the line on, because it takes a good three or four lines of the page it stands on before it is done.";
    await openBuilt(page, { sections: [{ paragraphs: Array.from({ length: 2400 }, (_, i) => `Line ${i}`) }], footnotes: note(1, "A first note."), styles: NOTE_STYLES });
    const r = await inPage(page, (w, [long, count]) => {
      const ed = w.fe.editor as unknown as Ed;
      const pg = ed.paginator as Pg & { noteNeeds(): number[] };
      const order = () => ed.engine.renderAll().order as unknown as string[];
      const state = () => {
        const n = pg.pages;
        return {
          reserve: Array.from({ length: n }, (_, k) => Math.round((pg.reserve[k] ?? 0) * 100) / 100),
          need: Array.from({ length: n }, (_, k) => Math.round((pg.noteNeeds()[k] ?? 0) * 100) / 100),
          notes: Array.from({ length: n }, (_, k) => pg.notesByPage[k]?.length ?? 0),
          settled: pg.notesSettled,
          pages: n,
          map: ed.pageMap(),
        };
      };
      const placed: string[] = [];
      let from = 2;
      for (let i = 0; i < (count as number); i++) {
        const map = ed.pageMap();
        const last = new Map<number, string>();
        for (const id of order()) last.set(map[id], id);
        const k = [...last.keys()].filter((k) => k >= from).sort((a, b) => a - b)[0];
        const p = last.get(k)!;
        ed.exec({ op: "insert_footnote", at: { p, o: 6 }, text: long });
        placed.push(p);
        from = ed.pageMap()[p] + 1;
      }
      const lines = placed.map((p) => { const o = order(); return { p, before: o[o.indexOf(p) - 1] }; });
      const made = state();
      // The same answer from nothing, from the last room, from scratch, and after a key typed.
      pg.invalidate();
      pg.reserve = [];
      ed.layoutNow();
      const nothing = state();
      pg.invalidate();
      ed.layoutNow();
      const again = state();
      pg.verify = false;
      pg.invalidate();
      pg.layoutBody(null, null, true);
      const scratch = state();
      pg.verify = true;
      ed.exec({ op: "insert_text", at: { p: order()[0], o: 0 }, text: "a" });
      const typed = state();
      const counts = { full: pg.stats.fullPasses, inc: pg.stats.incPasses, measured: pg.stats.measured };
      return { made, nothing, again, scratch, typed, lines, counts };
    }, [LONG, count] as [string, number]);
    console.log(`chases ${count}`, JSON.stringify(r.counts), "pages", r.made.pages);
    const m = r.made;
    expect(m.settled).toBe(true);
    for (const l of [r.nothing, r.again, r.scratch, r.typed]) {
      expect(l.reserve).toEqual(m.reserve);
      expect(l.map).toEqual(m.map);
      expect(l.settled).toBe(true);
    }
    // Room beyond what a page's notes need is only the room that moves a line: the next page starts with a reference whose note is
    // taller, and the page is not left holding the room of that whole note.
    let kept = 0;
    m.reserve.forEach((room, k) => {
      if (room - m.need[k] < 1) return;
      kept++;
      expect(m.notes[k + 1], `page ${k + 1} keeps ${room} for no note`).toBeGreaterThan(0);
      expect(room, `page ${k + 1} keeps the room of the whole note`).toBeLessThan(m.need[k + 1] - 1);
    });
    expect(kept).toBeGreaterThan(0);
    expect(r.lines.length).toBe(count);
    // Every reference that went on to the next page left room to move it on the page it left, and the paragraph before it stays there.
    for (const l of r.lines) {
      const page = m.map[l.p];
      if (page !== m.map[l.before] + 1) continue;
      expect(m.reserve[page - 2]).toBeGreaterThan(1);
    }
  });
});

test.describe("a retry from where the room changed, at zooms that cut the layout unit", () => {
  // The paginator puts the block it starts at back where it was by setting its margin. The browser lays a margin out in whole units
  // of a sixty-fourth of a zoomed pixel, and reports a position far down a long text to a float's precision (coarser), so at a zoom
  // the margin cannot always be set by a difference in one go, and the layout started over from the top (counted in `stats.fallbacks`).
  // Which zooms do this depends on the document: 87%, 110%, 120% and 175% did here, the novel's 133% did not in this one.
  test("at 87%, 110%, 120%, 133%, 150% and 175% the passes after the first start where the room changed, without starting over", async ({ page }) => {
    // Six whole layouts of a long document: alone, 14 s in Chromium and Firefox and 26 s in WebKit, which crossed the
    // 30 s default when the whole suite ran beside it.
    test.setTimeout(60_000);
    await openBuilt(page, cascadeDocument(420, 44, 350));
    const r = await inPage(page, async (w, zooms) => {
      const ed = w.fe.editor as unknown as Ed & { zoomer: { set(z: number): void } };
      const pg = ed.paginator as Pg & { notesInPass: boolean };
      // The rounds after a first pass that leaves the notes' room to them.
      pg.notesInPass = false;
      const out: { z: number; full: number; inc: number; fallbacks: Record<string, number>; settled: boolean }[] = [];
      for (const z of zooms) {
        ed.zoomer.set(z);
        await new Promise((done) => setTimeout(done, 800));
        pg.verify = false;
        pg.stats.fallbacks = {};
        const a = { full: pg.stats.fullPasses, inc: pg.stats.incPasses };
        // From no room at all: three rounds, each from where the room changed.
        pg.invalidate();
        pg.reserve = [];
        ed.layoutNow();
        out.push({ z, full: pg.stats.fullPasses - a.full, inc: pg.stats.incPasses - a.inc, fallbacks: { ...pg.stats.fallbacks }, settled: pg.notesSettled });
      }
      return out;
    }, [0.87, 1.1, 1.2, 1.33, 1.5, 1.75]);
    for (const l of r) {
      expect(l.fallbacks, `zoom ${l.z}`).toEqual({});
      expect(l.full, `zoom ${l.z}`).toBe(1);
      expect(l.inc, `zoom ${l.z}`).toBeGreaterThanOrEqual(3);
      expect(l.settled, `zoom ${l.z}`).toBe(true);
    }
  });
});

// A font that arrives changes the text it is used in before its
// `loadingdone` event asks for a layout, so a key typed in between laid out part-way down against records of the text above as it
// was. A layout now starts from the top when a face has loaded since the last whole layout.
test.describe("a font that arrives between two layouts", () => {
  test("the next layout starts from the top, and agrees with one from scratch", async ({ page }) => {
    // A monospaced face, and words of narrow letters (i, l): much wider than in any sans-serif the text falls back to (Linux's DejaVu
    // Sans is nearly as wide as a monospaced face for ordinary words), so the first paragraph wraps onto more lines when it comes.
    const fontBytes = Array.from(readFileSync("node_modules/@fontsource/cousine/files/cousine-latin-400-normal.woff2"));
    const late = (text: string) => ({ raw: `<w:r><w:rPr><w:rFonts w:ascii="Late Test Face" w:hAnsi="Late Test Face"/><w:sz w:val="40"/></w:rPr><w:t>${text}</w:t></w:r>` });
    await openBuilt(page, { sections: [{ paragraphs: [late("Illicit lilies fill little lit hills in Illinois, and nimble lily lilts fill tiny villa sills till nil is still."), "Second", "Third", "Fourth"] }] });
    const r = await inPage(page, async (w, bytes) => {
      const ed = w.fe.editor as unknown as Ed & { paginator: Pg; exec(op: unknown): unknown };
      const pg = ed.paginator;
      const before = (document.querySelector(".flow .fp span") as HTMLElement).getBoundingClientRect().height;
      const face = new FontFace("Late Test Face", new Uint8Array(bytes).buffer);
      document.fonts.add(face);
      await face.load();
      const after = (document.querySelector(".flow .fp span") as HTMLElement).getBoundingClientRect().height;
      pg.stats.fallbacks = {};
      const last = Array.from(document.querySelectorAll<HTMLElement>(".flow [data-p]")).at(-1)!;
      ed.exec({ op: "insert_text", at: { p: Number(last.dataset.p), o: 0 }, text: "z" });
      let error = "";
      try {
        ed.layoutNow({ incremental: true });
      } catch (e) {
        error = (e as Error).message.split("\n")[0];
      }
      return { moved: Math.abs(after - before), error, fallbacks: { ...pg.stats.fallbacks } };
    }, fontBytes);
    // (Looked for from a third of a pixel, since Safari lays a line out in 1/64 px; the face adds whole lines in every browser.)
    expect(r.moved, "the face changes the first paragraph's height").toBeGreaterThan(0.3);
    expect(r.error).toBe("");
    expect(r.fallbacks["a font arrived"]).toBeGreaterThanOrEqual(1);
  });
});

// Word puts the line's box at the top margin on every page: the letters sit below the margin by the leading above them, whether the page
// starts with a whole paragraph or carries one on. A page that carried a paragraph on put its letters at the margin, a pixel or more high.
test.describe("the first line of a page", () => {
  test("sits as low on a page that carries a paragraph on as on one that starts with a whole paragraph", async ({ page }) => {
    const words = "words that fill a line of the page and wrap ".repeat(40);
    const paragraphs = Array.from({ length: 8 }, (_, i) => ({ text: `Paragraph ${i} ${words}`, ppr: `${i === 3 ? "<w:pageBreakBefore/>" : ""}<w:spacing w:line="360" w:lineRule="auto"/>` }));
    await openBuilt(page, { sections: [{ paragraphs }] });
    const tops = await inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { zoom: number; pages: number; pageBox(k: number): { top: number } };
      const stage = document.querySelector(".stage")!.getBoundingClientRect();
      const out: { page: number; below: number; carried: boolean }[] = [];
      for (let k = 1; k < ed.pages; k++) {
        const box = ed.pageBox(k);
        let best = Infinity;
        let carried = false;
        for (const p of document.querySelectorAll<HTMLElement>(".flow .fp")) {
          const r = document.createRange();
          r.selectNodeContents(p);
          for (const rc of Array.from(r.getClientRects())) {
            const y = (rc.top - stage.top) / ed.zoom;
            if (y >= box.top && y < best) {
              best = y;
              carried = (p.getBoundingClientRect().top - stage.top) / ed.zoom < box.top;
            }
          }
        }
        out.push({ page: k + 1, below: Math.round((best - box.top - 96) * 100) / 100, carried });
      }
      return out;
    });
    const whole = tops.filter((x) => !x.carried);
    const carried = tops.filter((x) => x.carried);
    expect(whole.length).toBeGreaterThan(0);
    expect(carried.length).toBeGreaterThan(1);
    for (const c of carried) expect(Math.abs(c.below - whole[0].below), `page ${c.page}`).toBeLessThanOrEqual(0.25);
    expect(whole[0].below).toBeGreaterThan(1);
  });
});
