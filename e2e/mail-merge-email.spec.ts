// Mail merge, finishing: Send email messages (the dialog, the .zip of .eml files, the mail app step by step, a sender an extension adds),
// Print merged documents (every record in one print job, a page each), and Find duplicates in the recipient list.
import { readFileSync } from "node:fs";
import { readStoredZip } from "../packages/editor/src/mailmerge/zip";
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { readParts } from "./support/docx-parts";
import { chooseMenu, dialog, expect, inPage, menuItem, openMenu, test, type Page } from "./helpers";
import { chooseCsv, eng, fileName, info, LETTER, mergeLetter, toast } from "./support/mail-merge-helpers";
import { readPdf } from "./support/pdf-reader";

const PEOPLE = "First Name,Last Name,Email,Balance\nAda,Lovelace,ada@example.com,1234.5\nGrace,Hopper,grace@example.com,7\nÉmile,Zoë-Müller,emile@example.com,\nAlan,Turing,,99\n";

/** The headers and parts of an .eml file, with the encodings undone. */
function parseEml(raw: string) {
  const unQp = (s: string) => {
    const bytes: number[] = [];
    const joined = s.replace(/=\r\n/g, "");
    for (let i = 0; i < joined.length; i++) {
      if (joined[i] === "=") {
        bytes.push(parseInt(joined.slice(i + 1, i + 3), 16));
        i += 2;
      } else bytes.push(...Buffer.from(joined[i]));
    }
    return Buffer.from(bytes).toString("utf8");
  };
  const split = (text: string) => {
    const at = text.indexOf("\r\n\r\n");
    const head = text.slice(0, at);
    const headers: Record<string, string> = {};
    for (const line of head.replace(/\r\n[ \t]+/g, " ").split("\r\n")) {
      const c = line.indexOf(":");
      headers[line.slice(0, c).toLowerCase()] = line.slice(c + 1).trim();
    }
    return { headers, body: text.slice(at + 4) };
  };
  const walk = (text: string): { type: string; headers: Record<string, string>; text?: string; bytes?: Buffer }[] => {
    const { headers, body } = split(text);
    const type = headers["content-type"] ?? "";
    const boundary = /boundary="([^"]+)"/.exec(type)?.[1];
    if (boundary) {
      const pieces = body.split(`--${boundary}`).slice(1, -1);
      return pieces.flatMap((p) => walk(p.replace(/^\r\n/, "")));
    }
    if (headers["content-transfer-encoding"] === "base64") return [{ type, headers, bytes: Buffer.from(body.replace(/\r\n/g, ""), "base64") }];
    return [{ type, headers, text: unQp(body) }];
  };
  const top = split(raw);
  const subject = top.headers["subject"].split(/=\?UTF-8\?B\?|\?=\s*/).filter(Boolean);
  return {
    headers: top.headers,
    subject: top.headers["subject"].startsWith("=?") ? subject.map((w) => Buffer.from(w, "base64").toString("utf8")).join("") : top.headers["subject"],
    parts: walk(raw),
  };
}

/** Start Send email messages, fill the dialog, and click Send. Resolves with the download when the sender saves a file. */
async function sendDialog(page: Page, v: { subject?: string; format?: string; sender?: string; to?: string; range?: string; from?: string; upTo?: string } = {}) {
  await chooseMenu(page, "Mailings", "Send email messages…");
  const box = dialog(page, "Send email messages");
  if (v.sender) await box.getByLabel("Send with").selectOption({ label: v.sender });
  if (v.to) await box.getByLabel("To", { exact: true }).selectOption(v.to);
  if (v.subject !== undefined) await box.getByLabel("Subject line").fill(v.subject);
  if (v.format) await box.getByLabel("Mail format").selectOption(v.format);
  if (v.range) await box.getByLabel("Records").selectOption(v.range);
  if (v.from) await box.getByLabel("From record").fill(v.from);
  if (v.upTo) await box.getByLabel("To record").fill(v.upTo);
  return box;
}

const saveZip = async (page: Page, box: ReturnType<typeof dialog>) => {
  const [download] = await Promise.all([page.waitForEvent("download"), box.getByRole("button", { name: "Send", exact: true }).click()]);
  const entries = readStoredZip(new Uint8Array(readFileSync((await download.path())!)));
  return { name: download.suggestedFilename(), entries, emls: entries.map((e) => ({ name: e.name, ...parseEml(Buffer.from(e.bytes).toString("latin1")) })) };
};

test.describe("send email messages", () => {
  test("the menu has the two finishing commands, the dialog finds the address column and wants a subject", async ({ page }) => {
    await openBuilt(page, LETTER);
    await openMenu(page, "Mailings");
    await expect(menuItem(page, "Print merged documents…")).toBeDisabled();
    await expect(menuItem(page, "Send email messages…")).toBeDisabled();
    await page.keyboard.press("Escape");
    await mergeLetter(page, LETTER, PEOPLE);
    const box = await sendDialog(page);
    await expect(box).toContainText("4 recipients are in the merge.");
    await expect(box.getByLabel("To", { exact: true })).toHaveValue("Email");
    await expect(box.getByLabel("Send with").locator("option")).toHaveText(["Save as email files", "Open in my mail app"]);
    await expect(box.getByLabel("Mail format").locator("option")).toHaveText(["HTML", "Plain text", "Attachment as a Word document (.docx)", "Attachment as a PDF document (.pdf)"]);
    await expect(box).toContainText("Nothing leaves your computer.");
    await box.getByRole("button", { name: "Send", exact: true }).click();
    await expect(box).toContainText("Type a subject line.");
  });

  test("a .zip with an .eml for each recipient: HTML and text bodies with each person's letter, a subject with the fields filled in", async ({ page }) => {
    await mergeLetter(page, LETTER, PEOPLE);
    const box = await sendDialog(page, { subject: "Hello «First Name»" });
    const zip = await saveZip(page, box);
    expect(zip.name).toMatch(/emails\.zip$/);
    // Alan has no address, so three files.
    expect(zip.emls.map((e) => e.name)).toEqual(["1 ada@example.com.eml", "2 grace@example.com.eml", "3 emile@example.com.eml"]);
    const [ada, grace, emile] = zip.emls;
    expect(ada.headers["to"]).toBe("ada@example.com");
    expect(ada.headers["x-unsent"]).toBe("1");
    expect(ada.headers["mime-version"]).toBe("1.0");
    expect(ada.headers["content-type"]).toMatch(/^multipart\/alternative; boundary=/);
    expect(ada.subject).toBe("Hello Ada");
    expect(grace.subject).toBe("Hello Grace");
    expect(emile.subject).toBe("Hello Émile");
    // The subject of the third is an encoded word and the whole file is ASCII.
    expect(emile.headers["subject"]).toMatch(/^=\?UTF-8\?B\?/);
    for (const e of zip.entries) expect(Buffer.from(e.bytes).every((b) => b < 128)).toBe(true);
    const text = (e: (typeof zip.emls)[number]) => e.parts.find((p) => p.type.startsWith("text/plain"))!.text!;
    const html = (e: (typeof zip.emls)[number]) => e.parts.find((p) => p.type.startsWith("text/html"))!.text!;
    expect(text(ada)).toContain("Dear Ada Lovelace,");
    expect(text(ada)).toContain("Your balance is $1,234.50.");
    expect(text(ada)).not.toContain("Grace");
    expect(html(ada)).toMatch(/<html/i);
    expect(html(ada)).toContain("Dear Ada Lovelace,");
    expect(text(grace)).toContain("Dear Grace Hopper,");
    expect(text(emile)).toContain("Dear Émile Zoë-Müller,");
    expect(html(emile)).toContain("Dear Émile Zoë-Müller,");
    // The record without an address is said to be left out.
    const report = dialog(page, "Some records were left out");
    await expect(report).toContainText("3 messages went through.");
    await expect(report).toContainText("Record 4");
    await expect(report).toContainText("There is no email address.");
    await report.getByRole("button", { name: "OK" }).click();
    // The template is untouched.
    expect((await info(page)).active).toBe(true);
  });

  test("plain text is one text part and nothing else", async ({ page }) => {
    await mergeLetter(page, LETTER, PEOPLE);
    const box = await sendDialog(page, { subject: "Your balance", format: "text", range: "range", from: "1", upTo: "2" });
    const zip = await saveZip(page, box);
    expect(zip.emls).toHaveLength(2);
    const ada = zip.emls[0];
    expect(ada.headers["content-type"]).toBe("text/plain; charset=utf-8");
    expect(ada.parts).toHaveLength(1);
    expect(ada.parts[0].text).toContain("Dear Ada Lovelace,");
    expect(zip.emls[1].parts[0].text).toContain("Dear Grace Hopper,");
    // Everyone had an address, so the answer is a short notice rather than a list.
    await expect(toast(page)).toContainText("Saved 2 email files in a .zip.");
  });

  test("the address can come from another column, and a bad address is left out with the reason", async ({ page }) => {
    const csv = "Name,Work,Home\nAda,not an address,ada@home.example\nGrace,grace@work.example,\n";
    const spec: DocSpec = { sections: [{ paragraphs: ["Hello"] }] };
    await openBuilt(page, spec);
    await chooseMenu(page, "Mailings", "Start mail merge", "Email messages");
    expect((await info(page)).mainType).toBe("email");
    await chooseCsv(page, csv, "contacts.csv");
    const box = await sendDialog(page, { subject: "Hi", to: "Work", format: "text" });
    const zip = await saveZip(page, box);
    expect(zip.emls.map((e) => e.headers["to"])).toEqual(["grace@work.example"]);
    const report = dialog(page, "Some records were left out");
    await expect(report).toContainText("Record 1 (not an address): This is not an email address.");
    await report.getByRole("button", { name: "OK" }).click();
  });

  test("the merged document as an attachment: a .docx or a PDF with that person's letter, named for the subject", async ({ page }) => {
    await mergeLetter(page, LETTER, PEOPLE);
    let box = await sendDialog(page, { subject: "Letter for «Last Name»", format: "docx", range: "range", from: "1", upTo: "2" });
    let zip = await saveZip(page, box);
    expect(zip.emls).toHaveLength(2);
    const att = zip.emls[1].parts.find((p) => p.bytes)!;
    expect(att.headers["content-disposition"]).toBe('attachment; filename="Letter for Hopper.docx"');
    expect(att.type).toMatch(/^application\/vnd\.openxmlformats-officedocument\.wordprocessingml\.document; name="Letter for Hopper\.docx"/);
    const docx = readParts(att.bytes!);
    // Merged values are runs of their own: the text, without the markup.
    expect(docx.get("word/document.xml")!.replace(/<[^>]+>/g, "")).toContain("Dear Grace Hopper,");
    expect(docx.get("word/document.xml")).not.toContain("Lovelace");
    // The body of an attachment message has no letter in it.
    expect((zip.emls[1].parts.find((p) => p.type.startsWith("text/plain"))?.text ?? "").trim()).toBe("");

    box = await sendDialog(page, { subject: "PDF for «First Name»", format: "pdf", range: "range", from: "1", upTo: "2" });
    zip = await saveZip(page, box);
    expect(zip.emls).toHaveLength(2);
    const pdfPart = zip.emls[0].parts.find((p) => p.bytes)!;
    expect(pdfPart.type).toMatch(/^application\/pdf; name="PDF for Ada\.pdf"/);
    expect(pdfPart.bytes!.subarray(0, 5).toString()).toBe("%PDF-");
    const pdf = readPdf(pdfPart.bytes!);
    expect(pdf.pages).toHaveLength(1);
    expect(pdf.pageText[0]).toContain("Dear Ada Lovelace,");
  });

  test("Open in my mail app: plain text, one message each time Next email is chosen", async ({ page }) => {
    await mergeLetter(page, LETTER, PEOPLE);
    await page.evaluate(() => {
      const w = window as unknown as { __mail: string[]; __targets: string[] };
      w.__mail = [];
      w.__targets = [];
      document.addEventListener(
        "click",
        (e) => {
          const a = (e.target as HTMLElement).closest?.("a[href^='mailto:']") as HTMLAnchorElement | null;
          if (a) {
            e.preventDefault();
            w.__mail.push(a.href);
            w.__targets.push(a.target);
          }
        },
        true,
      );
    });
    const box = await sendDialog(page, { subject: "Hi «First Name»", sender: "Open in my mail app" });
    // The format is plain text whatever was asked for, and the dialog says so.
    await expect(box).toContainText("so the messages go as: Plain text.");
    await box.getByRole("button", { name: "Send", exact: true }).click();
    const step = dialog(page, "Open in my mail app");
    await expect(step.getByRole("button", { name: "Next email (1 of 3)" })).toBeVisible();
    await step.getByRole("button", { name: "Next email (1 of 3)" }).click();
    await expect(step.getByRole("button", { name: "Next email (2 of 3)" })).toBeVisible();
    await expect(step).toContainText("Opened 1 message.");
    const mail = () => page.evaluate(() => (window as unknown as { __mail: string[] }).__mail);
    const first = new URL((await mail())[0].replace(/^mailto:/, "mailto://"));
    expect(decodeURIComponent((await mail())[0].slice(7).split("?")[0])).toBe("ada@example.com");
    const q = new URLSearchParams((await mail())[0].split("?")[1]);
    expect(q.get("subject")).toBe("Hi Ada");
    expect(q.get("body")).toContain("Dear Ada Lovelace,");
    expect(first.protocol).toBe("mailto:");
    // A mail app on the web opens in a tab of its own: this one keeps the document.
    expect(await page.evaluate(() => (window as unknown as { __targets: string[] }).__targets)).toEqual(["_blank"]);
    await step.getByRole("button", { name: "Next email (2 of 3)" }).click();
    await step.getByRole("button", { name: "Next email (3 of 3)" }).click();
    await expect(step).toBeHidden();
    const urls = await mail();
    expect(urls).toHaveLength(3);
    expect(new URLSearchParams(urls[2].split("?")[1]).get("subject")).toBe("Hi Émile");
    expect(new URLSearchParams(urls[2].split("?")[1]).get("body")).toContain("Dear Émile Zoë-Müller,");
    // The one without an address is reported.
    await dialog(page, "Some records were left out").getByRole("button", { name: "OK" }).click();
  });

  test("stopping the mail app steps leaves the rest as not done, and a long list gets a warning first", async ({ page }) => {
    const rows = Array.from({ length: 12 }, (_, i) => `Person${i + 1},Last${i + 1},p${i + 1}@example.com,${i}`).join("\n");
    await mergeLetter(page, LETTER, `First Name,Last Name,Email,Balance\n${rows}\n`);
    await page.evaluate(() => {
      document.addEventListener("click", (e) => ((e.target as HTMLElement).closest?.("a[href^='mailto:']") ? e.preventDefault() : undefined), true);
    });
    const box = await sendDialog(page, { subject: "Hi", sender: "Open in my mail app" });
    await expect(box).toContainText("That is a lot of windows to open by hand.");
    // With a range of three records the warning goes away.
    await box.getByLabel("Records").selectOption("range");
    await box.getByLabel("From record").fill("1");
    await box.getByLabel("To record").fill("3");
    await expect(box).not.toContainText("That is a lot of windows");
    await box.getByLabel("To record").fill("12");
    await expect(box).toContainText("That is a lot of windows");
    await box.getByRole("button", { name: "Send", exact: true }).click();
    const step = dialog(page, "Open in my mail app");
    await step.getByRole("button", { name: "Next email (1 of 12)" }).click();
    await step.getByRole("button", { name: "Next email (2 of 12)" }).click();
    await step.getByRole("button", { name: "Stop" }).click();
    const report = dialog(page, "Some messages did not go");
    await expect(report).toContainText("2 messages went through.");
    await expect(report).toContainText("10 messages did not go");
    await expect(report).toContainText("Record 3 (p3@example.com): Not opened.");
  });

  test("a sender an extension adds is in the list and gets the messages with the format asked for", async ({ page }) => {
    await mergeLetter(page, LETTER, PEOPLE);
    await inPage(page, async (w) => {
      const sent: unknown[] = [];
      (window as unknown as { __sent: unknown[] }).__sent = sent;
      await w.fe.extensions.add({
        id: "acme.mail",
        name: "Acme mail",
        activate: (ctx) =>
          void ctx.mailMerge.mailSenders.register({
            id: "acme.smtp",
            label: "Send with Acme mail",
            note: "Sends each message through mail.acme.example.",
            formats: ["html", "text"],
            summary: (n) => `${n} handed to Acme`,
            send: (messages, c) => {
              messages.forEach((m, i) => {
                sent.push({ to: m.to, subject: m.subject, hasHtml: m.html !== undefined, text: m.text, record: m.record, name: c.name });
                c.progress(i, m.to.startsWith("grace") ? { ok: false, error: "Acme refused it" } : { ok: true });
              });
            },
          }),
      });
    });
    const box = await sendDialog(page, { subject: "Acme test", sender: "Send with Acme mail", format: "docx" });
    // A format the sender cannot send is changed to its first.
    await expect(box).toContainText("Sends each message through mail.acme.example.");
    await expect(box).toContainText("can't take that format, so the messages go as: HTML.");
    await box.getByRole("button", { name: "Send", exact: true }).click();
    await expect.poll(() => page.evaluate(() => (window as unknown as { __sent: unknown[] }).__sent.length)).toBe(3);
    const sent = await page.evaluate(() => (window as unknown as { __sent: { to: string; subject: string; hasHtml: boolean; text: string; record: number; name: string }[] }).__sent);
    expect(sent.map((m) => [m.to, m.subject, m.hasHtml, m.record])).toEqual([
      ["ada@example.com", "Acme test", true, 0],
      ["grace@example.com", "Acme test", true, 1],
      ["emile@example.com", "Acme test", true, 2],
    ]);
    expect(sent[0].text).toContain("Dear Ada Lovelace,");
    expect(sent[0].name).toBe(await fileName(page).then((n) => n.replace(/\.docx$/, "")));
    // The one it refused is listed, with its reason, beside the record that had no address.
    const report = dialog(page, "Some messages did not go");
    await expect(report).toContainText("2 messages went through.");
    await expect(report).toContainText("Record 2 (grace@example.com): Acme refused it");
  });
});

test.describe("print merged documents", () => {
  const LANDSCAPE: DocSpec = {
    sections: [
      {
        size: [15840, 12240],
        paragraphs: [{ raw: `<w:r><w:t xml:space="preserve">Dear </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> MERGEFIELD First_Name </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>«First_Name»</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>` }, "Yours sincerely"],
      },
    ],
  };
  // Each printed page holds a copy of the whole document and shows its own part of it: what a page shows is
  // measured with the print styles on, while the pages are there (until "afterprint").
  const stubPrint = (page: Page) =>
    page.evaluate(() => {
      const w = window as unknown as { __printed: { count: number; sizes: string[]; css: string }[] };
      w.__printed = [];
      window.print = () => {
        const pages = [...document.querySelectorAll(".docxtor-print .print-page")] as HTMLElement[];
        w.__printed.push({
          count: pages.length,
          sizes: pages.map((p) => `${p.style.width} ${p.style.height}`),
          css: [...document.head.querySelectorAll("style")].map((s) => s.textContent ?? "").filter((c) => c.includes("@page")).join("\n"),
        });
      };
    });
  const printed = (page: Page) => page.evaluate(() => (window as unknown as { __printed: { count: number; sizes: string[]; css: string }[] }).__printed);
  /** The text of the paragraphs each printed page shows. */
  const pageTexts = async (page: Page): Promise<string[]> => {
    await page.emulateMedia({ media: "print" });
    const texts = await page.evaluate(() => {
      document.documentElement.classList.add("docxtor-printing");
      const out = [...document.querySelectorAll(".docxtor-print .print-page")].map((p) => {
        const r = p.getBoundingClientRect();
        return [...p.querySelectorAll("[data-p]")]
          .filter((e) => {
            const q = e.getBoundingClientRect();
            return q.height > 0 && q.top >= r.top - 1 && q.bottom <= r.bottom + 1;
          })
          .map((e) => e.textContent ?? "")
          .join("\n");
      });
      document.documentElement.classList.remove("docxtor-printing");
      return out;
    });
    await page.emulateMedia({ media: "screen" });
    return texts;
  };

  test("every record is printed in one job, each on a page of its own with the template's page setup", async ({ page }) => {
    await mergeLetter(page, LANDSCAPE, PEOPLE);
    await stubPrint(page);
    const name = await fileName(page);
    await chooseMenu(page, "Mailings", "Print merged documents…");
    const box = dialog(page, "Print merged documents");
    await expect(box).toContainText("4 recipients are in the merge.");
    await expect(box).toContainText("Each record starts on a new page");
    await box.getByRole("button", { name: "Print", exact: true }).click();
    await expect.poll(async () => (await printed(page)).length).toBe(1);
    const job = (await printed(page))[0];
    expect(job.count).toBe(4);
    const texts = await pageTexts(page);
    expect(texts[0]).toContain("Dear Ada");
    expect(texts[1]).toContain("Dear Grace");
    expect(texts[2]).toContain("Dear Émile");
    expect(texts[3]).toContain("Dear Alan");
    for (const p of texts) expect(p.match(/Dear /g)).toHaveLength(1);
    // Landscape letter: 11 by 8.5 inches.
    expect(new Set(job.sizes)).toEqual(new Set(["1056px 816px"]));
    expect(job.css).toContain("size: 11in 8.5in");
    // Nothing was opened: the template is still the document, still a merge.
    expect(await fileName(page)).toBe(name);
    expect((await info(page)).active).toBe(true);
    // When the print dialog is done the pages and the hidden editor go away.
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
    await expect(page.locator(".docxtor-print")).toHaveCount(0);
    await expect(page.locator("[data-mm-offscreen]")).toHaveCount(0);
  });

  test("only the record on show, or a range", async ({ page }) => {
    await mergeLetter(page, LETTER, PEOPLE);
    await stubPrint(page);
    await chooseMenu(page, "Mailings", "Next record");
    await chooseMenu(page, "Mailings", "Next record");
    await chooseMenu(page, "Mailings", "Print merged documents…");
    let box = dialog(page, "Print merged documents");
    await box.getByLabel("Records").selectOption("current");
    await box.getByRole("button", { name: "Print", exact: true }).click();
    await expect.poll(async () => (await printed(page)).length).toBe(1);
    expect((await printed(page))[0].count).toBe(1);
    expect((await pageTexts(page))[0]).toContain("Dear Grace Hopper,");
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
    await chooseMenu(page, "Mailings", "Print merged documents…");
    box = dialog(page, "Print merged documents");
    await box.getByLabel("Records").selectOption("range");
    await box.getByLabel("From record").fill("2");
    await box.getByLabel("To record").fill("3");
    await box.getByRole("button", { name: "Print", exact: true }).click();
    await expect.poll(async () => (await printed(page)).length).toBe(2);
    expect((await pageTexts(page)).map((p) => /Dear (\p{L}+)/u.exec(p)?.[1])).toEqual(["Grace", "Émile"]);
  });

  test("a long record keeps its pages together and the next record still starts on a new page", async ({ page }) => {
    const long: DocSpec = { sections: [{ paragraphs: [{ raw: `<w:r><w:t>Dear </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> MERGEFIELD First_Name </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>«First_Name»</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>` }, ...Array.from({ length: 70 }, (_, i) => `Line ${i + 1} of the letter`)] }] };
    await mergeLetter(page, long, PEOPLE);
    await stubPrint(page);
    await chooseMenu(page, "Mailings", "Print merged documents…");
    await dialog(page, "Print merged documents").getByRole("button", { name: "Print", exact: true }).click();
    await expect.poll(async () => (await printed(page)).length).toBe(1);
    const pages = await pageTexts(page);
    expect(pages.length).toBeGreaterThanOrEqual(8);
    // A page that starts a record has "Dear" on it, and no page has two records.
    const starts = pages.map((p, i) => (p.includes("Dear ") ? i : -1)).filter((i) => i >= 0);
    expect(starts).toHaveLength(4);
    for (const p of pages) expect(p.match(/Dear /g)?.length ?? 0).toBeLessThanOrEqual(1);
  });
});

test.describe("find duplicates", () => {
  const DUPES =
    "First Name,Last Name,Street,City,Email\n" +
    "Ada,Lovelace,12 Crumb Rd,London,ada@x.org\n" + // 1
    "Grace,Hopper,1 Navy Way,Arlington,grace@x.org\n" + // 2
    "ada,LOVELACE,12  crumb rd,LONDON,ada.l@y.org\n" + // 3 same name and address as 1
    "Alan,Turing,3 Bletchley,Milton Keynes,alan@x.org\n" + // 4
    "Grace,Hopper,1 Navy Way,Arlington,grace@x.org\n" + // 5 identical to 2
    "Edsger,Dijkstra,5 Main St,Nuenen,\n"; // 6

  const active = (page: Page) => eng(page, (e) => e.mergeData()!.active);

  test("the groups are listed with the reason, the first of each is kept, and OK leaves the others out as one undo step", async ({ page }) => {
    await mergeLetter(page, LETTER, DUPES);
    await chooseMenu(page, "Mailings", "Find duplicates…");
    const box = dialog(page, "Find duplicates");
    await expect(box).toContainText("2 groups of records look alike: 4 records, 2 of them in the merge.");
    const groups = box.getByRole("group");
    await expect(groups).toHaveCount(2);
    await expect(groups.nth(0)).toContainText("Group 1: same name and address");
    await expect(groups.nth(0).getByRole("row")).toHaveCount(3); // the heading and two records
    await expect(groups.nth(0).getByRole("cell", { name: "London" })).toHaveCount(2);
    await expect(groups.nth(1)).toContainText("Group 2: all the details are the same");
    await expect(groups.nth(0).getByRole("checkbox")).toHaveCount(2);
    await expect(groups.nth(0).getByRole("checkbox").nth(0)).toBeChecked();
    await expect(groups.nth(0).getByRole("checkbox").nth(1)).not.toBeChecked();
    // Keep the second Grace as well.
    await groups.nth(1).getByRole("checkbox").nth(1).check();
    await expect(box).toContainText("3 of them in the merge");
    await box.getByRole("button", { name: "OK" }).click();
    expect(await active(page)).toEqual([true, true, false, true, true, true]);
    expect((await info(page)).included).toBe(5);
    await page.keyboard.press("ControlOrMeta+z");
    expect(await active(page)).toEqual([true, true, true, true, true, true]);
  });

  test("Keep the first of each group and Keep every record, and Cancel changes nothing", async ({ page }) => {
    await mergeLetter(page, LETTER, DUPES);
    await chooseMenu(page, "Mailings", "Find duplicates…");
    let box = dialog(page, "Find duplicates");
    await box.getByRole("button", { name: "Keep every record" }).click();
    await expect(box).toContainText("4 of them in the merge");
    await box.getByRole("button", { name: "Keep the first of each group" }).click();
    await expect(box).toContainText("2 of them in the merge");
    await box.getByRole("button", { name: "Cancel" }).click();
    expect(await active(page)).toEqual([true, true, true, true, true, true]);
    // Records that are already left out stay out, and a group with nothing in the merge is not touched.
    await chooseMenu(page, "Mailings", "Edit recipient list…");
    const list = dialog(page, "Recipients");
    await list.getByLabel("Include recipient 1").uncheck();
    await list.getByLabel("Include recipient 3").uncheck();
    await list.getByRole("button", { name: "OK" }).click();
    await chooseMenu(page, "Mailings", "Find duplicates…");
    box = dialog(page, "Find duplicates");
    await expect(box.getByRole("group").nth(0).getByRole("checkbox").nth(0)).not.toBeChecked();
    await expect(box.getByRole("group").nth(0).getByRole("checkbox").nth(1)).not.toBeChecked();
    await box.getByRole("button", { name: "OK" }).click();
    expect((await active(page)).slice(0, 3)).toEqual([false, true, false]);
  });

  test("the recipient list has the button too, and its ticks wait for OK", async ({ page }) => {
    await mergeLetter(page, LETTER, DUPES);
    await chooseMenu(page, "Mailings", "Edit recipient list…");
    const list = dialog(page, "Recipients");
    await list.getByRole("button", { name: "Find duplicates…" }).click();
    const box = dialog(page, "Find duplicates");
    await box.getByRole("button", { name: "OK" }).click();
    await expect(list.getByLabel("Include recipient 3")).not.toBeChecked();
    await expect(list.getByLabel("Include recipient 5")).not.toBeChecked();
    await expect(list.getByLabel("Include recipient 1")).toBeChecked();
    expect(await active(page)).toEqual([true, true, true, true, true, true]);
    await list.getByRole("button", { name: "OK" }).click();
    expect(await active(page)).toEqual([true, true, false, true, false, true]);
  });

  test("a list with no look-alikes says so", async ({ page }) => {
    await mergeLetter(page, LETTER, "First Name,Last Name,Email\nAda,Lovelace,a@x.org\nGrace,Hopper,g@x.org\n");
    await chooseMenu(page, "Mailings", "Find duplicates…");
    await expect(toast(page)).toContainText("No two records look alike.");
  });

  test("two records with the same email address are a group", async ({ page }) => {
    await mergeLetter(page, LETTER, "First Name,Last Name,Email\nAda,Lovelace,same@x.org\nAda B,Lovelace,SAME@x.org\n");
    await chooseMenu(page, "Mailings", "Find duplicates…");
    await expect(dialog(page, "Find duplicates").getByRole("group").first()).toContainText("same email address");
  });
});
