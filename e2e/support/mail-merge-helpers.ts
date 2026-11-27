// Shared by the mail merge specs: a letter with merge fields as Word writes them, a list of recipients, and ways to start a
// merge from the Mailings menu and to read the engine's view of it.
import { openBuilt, type DocSpec } from "./docx-builder";
import { readParts } from "./docx-parts";
import { chooseMenu, expect, LINE_END, type Page } from "../helpers";

export const CSV = "First Name,Last Name,State,Balance\nAda,Lovelace,CA,1234.5\nGrace,Hopper,NY,7\nAlan,Turing,CA,\nEdsger,Dijkstra,NL,99\n";

/** A list with the columns an address block looks for. */
export const CSV2 = "Title,First Name,Last Name,Street,City,State,ZIP,Country\nMs.,Ada,Lovelace,12 Crumb Rd,London,,N1 9GU,United Kingdom\nRear Adm.,Grace,Hopper,1 Navy Way,Arlington,VA,22202,United States\n,,,5 Main St,Springfield,IL,62701,\n";

export const run = (t: string) => `<w:r><w:t xml:space="preserve">${t}</w:t></w:r>`;
export const mergeField = (name: string, extra = "") =>
  `<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> MERGEFIELD ${name}${extra} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t>«${name}»</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>`;

/** A letter with merge fields, as Word writes them. */
export const LETTER: DocSpec = {
  sections: [
    {
      paragraphs: [
        { raw: run("Dear ") + mergeField("First_Name") + run(" ") + mergeField("Last_Name") + run(",") },
        { raw: run("Your balance is ") + mergeField("Balance", ' \\# "$#,##0.00"') + run(".") },
        "Yours sincerely",
      ],
    },
  ],
};

export interface MergeApi {
  fileName: string;
  mergeInfo(): { active: boolean; mainType: string; sourceName: string | null; hasRecipients: boolean; columns: string[]; records: number; included: number; previewing: number | null; mapping: Record<string, string> };
  mergeData(): { rows: string[][]; active: boolean[]; order: number[]; filter: unknown[]; sort: unknown[]; matched: [string, string | null][] } | null;
  listFields(): { p: number; o: number; instr: string; result: string; code: string }[];
  save(): Uint8Array;
  text(): string;
  pageSetup(): { sections: { width: number; height: number; margin: number[] }[] };
}

/** Run something against the open document's engine, in the page. */
export const eng = <R, A = undefined>(page: Page, fn: (e: MergeApi, a: A) => R | Promise<R>, arg?: A): Promise<R> =>
  page.evaluate(([src, a]) => new Function("e", "a", `return (${src})(e, a)`)((window as unknown as { docxtor: { fe: { editor: { engine: unknown } } } }).docxtor.fe.editor.engine, a), [fn.toString(), arg] as const) as Promise<R>;

export const info = (page: Page) => eng(page, (e) => e.mergeInfo());
export const fields = (page: Page) => eng(page, (e) => e.listFields());
export const bodyText = (page: Page) => eng(page, (e) => e.text());
export const fileName = (page: Page) => eng(page, (e) => e.fileName);

/** The parts of the document as the engine would save it now. */
export const parts = (page: Page) => eng(page, (e) => Array.from(e.save())).then((b) => readParts(b));

/** Mailings, Select recipients, From a CSV file…, with the file given. */
export async function chooseCsv(page: Page, text = CSV, name = "people.csv") {
  const [chooser] = await Promise.all([page.waitForEvent("filechooser"), chooseMenu(page, "Mailings", "Select recipients", "From a CSV file…")]);
  await chooser.setFiles({ name, mimeType: "text/csv", buffer: Buffer.from(text) });
  await expect.poll(async () => (await info(page)).hasRecipients).toBe(true);
}

/** A letter with its recipients chosen. */
export async function mergeLetter(page: Page, spec: DocSpec = LETTER, csv = CSV) {
  await openBuilt(page, spec);
  await chooseMenu(page, "Mailings", "Start mail merge", "Letters");
  await chooseCsv(page, csv);
}

/** How many pages the document on screen has. */
export const pagesOf = (page: Page) => page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { pages: number } } } }).docxtor.fe.editor.pages);

export const toast = (page: Page) => page.locator(".toast");
export const line = (page: Page, n: number) => page.locator(".flow .fp").nth(n);
export const strip = (page: Page) => page.locator(".mm-strip");

/** The caret at the end of a line of the document. */
export async function caretAtEndOf(page: Page, n: number) {
  await line(page, n).click();
  await page.keyboard.press(LINE_END);
}
