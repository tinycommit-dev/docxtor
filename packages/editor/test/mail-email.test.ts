// Email merge: the .eml file (MIME structure, encodings, attachments), the .zip it goes into, the fields of the dialog and the duplicate finder.
// None of these need a browser.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { findDuplicates, keepFirst } from "../src/mailmerge/duplicates.ts";
import { fillSubject, guessEmailColumn, validAddress } from "../src/mailmerge/email-fields.ts";
import { buildEml, encodeHeader, quotedPrintable, safeFileName } from "../src/mailmerge/eml.ts";
import { mailtoLink } from "../src/mailmerge/mailto.ts";
import { crc32, makeZip, readStoredZip } from "../src/mailmerge/zip.ts";

/** Options with a fixed date and boundaries B1, B2… so the output can be compared. */
const opts = () => ({ date: new Date(Date.UTC(2026, 9, 14, 12, 30, 0)), boundary: (() => { let n = 0; return () => `B${++n}`; })() });

/** Undo quoted-printable the way a mail app does. */
function unQp(s: string): string {
  const joined = s.replace(/=\r\n/g, "");
  const bytes: number[] = [];
  const raw = new TextEncoder().encode(joined);
  for (let i = 0; i < raw.length; i++) {
    if (raw[i] === 0x3d) {
      bytes.push(parseInt(String.fromCharCode(raw[i + 1], raw[i + 2]), 16));
      i += 2;
    } else bytes.push(raw[i]);
  }
  return new TextDecoder().decode(Uint8Array.from(bytes));
}

test("a plain text message is one text part with the headers a mail app needs", () => {
  const eml = buildEml({ to: "ada@example.com", subject: "Hello", text: "Dear Ada,\nYour balance is 12.", record: 0 }, opts());
  const [head, body] = eml.split("\r\n\r\n");
  assert.match(head, /^X-Unsent: 1\r\nTo: ada@example\.com\r\nSubject: Hello\r\nDate: Wed, 14 Oct 2026 12:30:00 \+0000\r\nMIME-Version: 1\.0\r\nContent-Type: text\/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable$/);
  assert.equal(body, "Dear Ada,\r\nYour balance is 12.\r\n");
  // Every line ends in CRLF and nothing but ASCII is in the file.
  assert.ok(!/[^\x00-\x7f]/.test(eml));
  assert.ok(!/(^|[^\r])\n/.test(eml));
});

test("HTML with a text version is multipart/alternative, the text first", () => {
  const eml = buildEml({ to: "a@b.co", subject: "S", text: "plain", html: "<p>rich</p>", record: 0 }, opts());
  assert.match(eml, /Content-Type: multipart\/alternative; boundary="B1"/);
  const parts = eml.split("--B1");
  assert.equal(parts.length, 4);
  assert.match(parts[1], /Content-Type: text\/plain; charset=utf-8/);
  assert.match(parts[2], /Content-Type: text\/html; charset=utf-8/);
  assert.ok(parts[3].startsWith("--"));
  assert.ok(parts[1].indexOf("plain") < parts[2].indexOf("<p>rich</p>"));
});

test("HTML alone is a single text/html part", () => {
  const eml = buildEml({ to: "a@b.co", subject: "S", html: "<b>x</b>", record: 0 }, opts());
  assert.match(eml, /Content-Type: text\/html; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n<b>x<\/b>/);
  assert.ok(!eml.includes("multipart"));
});

test("a subject with other letters is an encoded word that decodes to the same text, in pieces that stay short", () => {
  assert.equal(encodeHeader("Plain subject"), "Plain subject");
  const subject = "Grüße für Zoë Łukasiewicz – 你好，世界 – ご案内のお知らせ and a good deal more text to force folding";
  const enc = encodeHeader(subject);
  const words = enc.split("\r\n ");
  assert.ok(words.length > 1);
  for (const w of words) {
    assert.match(w, /^=\?UTF-8\?B\?[A-Za-z0-9+/=]+\?=$/);
    assert.ok(w.length <= 75);
  }
  const back = words.map((w) => Buffer.from(w.slice(10, -2), "base64").toString("utf8")).join("");
  assert.equal(back, subject);
  // The subject in a message is folded the same way and has no raw non-ASCII letters.
  const eml = buildEml({ to: "a@b.co", subject, text: "x", record: 0 }, opts());
  assert.ok(!/[^\x00-\x7f]/.test(eml));
  assert.match(eml, /Subject: =\?UTF-8\?B\?/);
});

test("line breaks in a subject or an address cannot start another header", () => {
  const eml = buildEml({ to: "a@b.co\r\nBcc: spy@example.com", subject: "Hi\r\nBcc: spy@example.com", text: "x", record: 0 }, opts());
  const head = eml.split("\r\n\r\n")[0];
  assert.ok(!/^Bcc:/m.test(head));
  assert.match(head, /^To: a@b\.coBcc: spy@example\.com$/m);
  assert.match(head, /^Subject: Hi Bcc: spy@example\.com$/m);
});

test("quoted-printable keeps non-ASCII letters, equals signs and trailing spaces, and wraps at 76", () => {
  const text = "Café = naïve  \nsecond line";
  const qp = quotedPrintable(text);
  assert.ok(!/[^\x00-\x7f]/.test(qp));
  assert.equal(unQp(qp), "Café = naïve  \r\nsecond line");
  assert.match(qp, /naïve =20|na=C3=AFve =20/);
  const long = quotedPrintable("é".repeat(200) + " " + "word ".repeat(60));
  for (const line of long.split("\r\n")) assert.ok(line.length <= 76, `line of ${line.length}`);
  assert.equal(unQp(long), "é".repeat(200) + " " + "word ".repeat(60));
});

test("attachments are base64 in multipart/mixed, with names that survive other alphabets", () => {
  const bytes = Uint8Array.from({ length: 300 }, (_, i) => i % 256);
  const eml = buildEml(
    {
      to: "a@b.co",
      subject: "Letter",
      text: "See the file.",
      html: "<p>See the file.</p>",
      attachments: [
        { name: "Letter.docx", type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", bytes },
        { name: "Für Zoë.pdf", type: "application/pdf", bytes: new TextEncoder().encode("%PDF-1.7 hello") },
      ],
      record: 0,
    },
    opts(),
  );
  assert.match(eml, /Content-Type: multipart\/mixed; boundary="B2"/);
  assert.match(eml, /Content-Type: multipart\/alternative; boundary="B1"/);
  assert.match(eml, /Content-Disposition: attachment; filename="Letter\.docx"/);
  assert.match(eml, /Content-Type: application\/pdf; name="F_r Zo_\.pdf"; name\*=UTF-8''F%C3%BCr%20Zo%C3%AB\.pdf/);
  assert.match(eml, /Content-Disposition: attachment; filename="F_r Zo_\.pdf"; filename\*=UTF-8''F%C3%BCr%20Zo%C3%AB\.pdf/);
  // The first attachment decodes to the same bytes, in lines of at most 76 characters.
  const at = eml.indexOf('filename="Letter.docx"');
  const block = eml.slice(eml.indexOf("\r\n\r\n", at) + 4, eml.indexOf("\r\n--B2", at));
  for (const l of block.split("\r\n")) assert.ok(l.length <= 76);
  assert.deepEqual(Array.from(Buffer.from(block.replace(/\r\n/g, ""), "base64")), Array.from(bytes));
  assert.ok(eml.trimEnd().endsWith("--B2--"));
});

test("an attachment-only message still has a readable (empty) body part", () => {
  const eml = buildEml({ to: "a@b.co", subject: "Letter", attachments: [{ name: "a.docx", type: "application/octet-stream", bytes: Uint8Array.of(1, 2, 3) }], record: 0 }, opts());
  assert.match(eml, /multipart\/mixed/);
  assert.match(eml, /Content-Type: text\/plain; charset=utf-8/);
  assert.match(eml, /Content-Transfer-Encoding: base64\r\nContent-Disposition: attachment; filename="a\.docx"\r\n\r\nAQID/);
});

test("file names lose the characters systems reserve", () => {
  assert.equal(safeFileName('Re: a/b\\c*"d"?<e>|f'), "Re a b c d e f");
  assert.equal(safeFileName("   "), "message");
  assert.equal(safeFileName("..hidden"), "hidden");
  assert.ok([...safeFileName("x".repeat(300))].length <= 80);
});

test("a zip holds each file whole, with the right checksums, and reads back", () => {
  assert.equal(crc32(new TextEncoder().encode("123456789")), 0xcbf43926);
  const a = new TextEncoder().encode("first message");
  const b = Uint8Array.from({ length: 5000 }, (_, i) => (i * 7) % 256);
  const zip = makeZip([
    { name: "1 ada@example.com.eml", bytes: a },
    { name: "2 grüße.eml", bytes: b },
  ]);
  assert.equal(new DataView(zip.buffer).getUint32(0, true), 0x04034b50);
  const back = readStoredZip(zip);
  assert.deepEqual(back.map((e) => e.name), ["1 ada@example.com.eml", "2 grüße.eml"]);
  assert.deepEqual(back[0].bytes, a);
  assert.deepEqual(back[1].bytes, b);
  // The end record says how many files there are.
  const end = new DataView(zip.buffer, zip.byteLength - 22);
  assert.equal(end.getUint32(0, true), 0x06054b50);
  assert.equal(end.getUint16(10, true), 2);
});

test("the address column is the one matched to E-mail Address, or named like it, or full of addresses", () => {
  const rows = [["Ada", "ada@example.com", "x"]];
  assert.equal(guessEmailColumn({ columns: ["Name", "Contact", "Note"], rows, matched: [["E-mail Address", "Contact"]] }), "Contact");
  assert.equal(guessEmailColumn({ columns: ["Name", "E-mail", "Note"], rows, matched: [] }), "E-mail");
  assert.equal(guessEmailColumn({ columns: ["Name", "Email_Address", "Note"], rows, matched: [] }), "Email_Address");
  assert.equal(guessEmailColumn({ columns: ["Name", "Contact", "Note"], rows, matched: [] }), "Contact");
  assert.equal(guessEmailColumn({ columns: ["Name", "City"], rows: [["Ada", "London"]], matched: [] }), "Name");
});

test("the subject takes «Column» from the record, whatever the case or spacing of the name", () => {
  const cols = ["First Name", "Last Name"];
  assert.equal(fillSubject("Hello «First Name» «last_name»!", cols, ["Ada", "Lovelace"]), "Hello Ada Lovelace!");
  assert.equal(fillSubject("About «Nothing»", cols, ["Ada", "Lovelace"]), "About ");
  assert.equal(fillSubject("No fields", cols, ["Ada", "Lovelace"]), "No fields");
  assert.ok(validAddress("ada@example.com") && validAddress(" a.b+c@d.org "));
  assert.ok(!validAddress("ada@") && !validAddress("ada example@x.com") && !validAddress("") && !validAddress("a@b"));
});

const matched: [string, string | null][] = [
  ["First Name", "First"],
  ["Last Name", "Last"],
  ["Address 1", "Street"],
  ["City", "City"],
  ["E-mail Address", "Email"],
];
const columns = ["First", "Last", "Street", "City", "Email"];

test("duplicates: identical rows, the same name and address, and the same email form groups", () => {
  const rows = [
    ["Ada", "Lovelace", "12 Crumb Rd", "London", "ada@x.org"], // 0
    ["Grace", "Hopper", "1 Navy Way", "Arlington", "grace@x.org"], // 1
    ["ada", "LOVELACE", "12  Crumb Rd", "london", "other@x.org"], // 2: same name and address as 0 (case, spaces)
    ["Alan", "Turing", "3 Bletchley", "Milton Keynes", "ada@x.org"], // 3: same email as 0
    ["Grace", "Hopper", "1 Navy Way", "Arlington", "grace@x.org"], // 4: identical to 1
    ["Edsger", "Dijkstra", "5 Main St", "Nuenen", ""], // 5: alone
  ];
  const groups = findDuplicates({ columns, rows, matched });
  assert.deepEqual(groups.map((g) => g.rows), [[0, 2, 3], [1, 4]]);
  assert.deepEqual(groups[0].reasons, ["nameAddress", "email"]);
  assert.deepEqual(groups[1].reasons, ["identical", "nameAddress", "email"]);
});

test("duplicates: different people at one address are not duplicates, and empty rows are ignored", () => {
  const rows = [
    ["Ada", "Lovelace", "12 Crumb Rd", "London", ""],
    ["Byron", "Lovelace", "12 Crumb Rd", "London", ""],
    ["", "", "", "", ""],
    ["", "", "", "", ""],
    ["", "", "5 Main St", "Town", ""],
    ["", "", "5 Main St", "Town", ""],
  ];
  const groups = findDuplicates({ columns, rows, matched });
  // The two blank rows are not alike; the two with an address only are alike in everything, so they are identical.
  assert.deepEqual(groups.map((g) => g.rows), [[4, 5]]);
  assert.deepEqual(groups[0].reasons, ["identical"]);
});

test("duplicates: placeholder values in the email column are not an address, so they don't group people", () => {
  const rows = [
    ["Ada", "Lovelace", "12 Crumb Rd", "London", "n/a"],
    ["Alan", "Turing", "3 Bletchley", "Milton Keynes", "n/a"],
    ["Grace", "Hopper", "1 Navy Way", "Arlington", "-"],
    ["Edsger", "Dijkstra", "5 Main St", "Nuenen", "-"],
  ];
  assert.deepEqual(findDuplicates({ columns, rows, matched }), []);
});

test("an address whose domain has letters outside ASCII is written in its ASCII form", () => {
  const eml = buildEml({ to: "ada@bücher.example", subject: "S", text: "x", record: 0 }, opts());
  assert.match(eml, /^To: ada@xn--bcher-kva\.example\r$/m);
});

test("duplicates: without name or address columns only identical rows count", () => {
  const groups = findDuplicates({ columns: ["A", "B"], rows: [["1", "2"], ["x", "y"], ["1", " 2"]], matched: [] });
  assert.deepEqual(groups.map((g) => g.rows), [[0, 2]]);
});

test("keeping the first of each group leaves out the rest, and leaves groups with nothing in the merge alone", () => {
  const groups = [{ rows: [0, 2], reasons: ["identical" as const] }, { rows: [3, 4], reasons: ["identical" as const] }];
  assert.deepEqual(keepFirst([true, true, true, false, true], groups), [true, true, false, false, true]);
  assert.deepEqual(keepFirst([false, true, true, true, true], groups), [false, true, true, true, false]);
  assert.deepEqual(keepFirst([false, true, false, false, false], groups), [false, true, false, false, false]);
});

test("a mailto link carries the address, subject and text, and cuts a long text to fit", () => {
  const short = mailtoLink({ to: "ada@example.com", subject: "Grüße & more", text: "Dear Ada,\nBye" });
  assert.equal(short.cut, false);
  assert.equal(short.url, "mailto:ada@example.com?subject=Gr%C3%BC%C3%9Fe%20%26%20more&body=Dear%20Ada%2C%0D%0ABye");
  const long = mailtoLink({ to: "ada@example.com", subject: "S", text: "word ".repeat(2000) }, 500);
  assert.equal(long.cut, true);
  assert.ok(long.url.length <= 500);
  assert.ok(decodeURIComponent(long.url).endsWith("…"));
  // A message of emoji is cut between them, never through one (half an emoji made encodeURIComponent throw).
  for (let limit = 300; limit < 340; limit++) {
    const emoji = mailtoLink({ to: "ada@example.com", subject: "S", text: "😀".repeat(500) }, limit);
    assert.equal(emoji.cut, true);
    assert.ok(emoji.url.length <= limit);
    assert.match(decodeURIComponent(emoji.url), /^mailto:ada@example\.com\?subject=S&body=(😀)+…$/);
  }
  // A lone half in the document's text is replaced, not thrown on.
  assert.ok(decodeURIComponent(mailtoLink({ to: "a@b.c", subject: "x\uD83D", text: "y\uDE00" }).url).includes("\uFFFD"));
});
