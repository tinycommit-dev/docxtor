// Word's markup for content controls, legacy form fields and protection, for the browser tests, and a way to read a part
// of the document the editor would save.
import type { Page } from "../helpers";

const NS = 'xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"';

/** A paragraph holding some runs and controls. */
export const p = (inner: string) => `<w:p>${inner}</w:p>`;
export const run = (text: string) => `<w:r><w:t xml:space="preserve">${text}</w:t></w:r>`;

const placeholder = (name: string, text: string) => `<w:r><w:rPr><w:rStyle w:val="PlaceholderText"/></w:rPr><w:t>${text}</w:t></w:r>`.replace("PLACEHOLDER", name);

export const checkbox = (alias = "Agree", extra = "") =>
  `<w:sdt><w:sdtPr ${NS}><w:alias w:val="${alias}"/><w:id w:val="101"/>${extra}<w14:checkbox><w14:checked w14:val="0"/><w14:checkedState w14:val="2612" w14:font="MS Gothic"/><w14:uncheckedState w14:val="2610" w14:font="MS Gothic"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:rPr><w:rFonts w:ascii="MS Gothic" w:eastAsia="MS Gothic" w:hAnsi="MS Gothic" w:hint="eastAsia"/></w:rPr><w:t>☐</w:t></w:r></w:sdtContent></w:sdt>`;

export const dropdown = (extra = "") =>
  `<w:sdt><w:sdtPr ${NS}><w:alias w:val="Colour"/><w:id w:val="102"/>${extra}<w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013438"/></w:placeholder><w:showingPlcHdr/><w:dropDownList><w:listItem w:value="Choose an item."/><w:listItem w:displayText="Red" w:value="r"/><w:listItem w:displayText="Blue" w:value="b"/></w:dropDownList></w:sdtPr><w:sdtContent>${placeholder("", "Choose an item.")}</w:sdtContent></w:sdt>`;

export const combo = () =>
  `<w:sdt><w:sdtPr><w:alias w:val="Fruit"/><w:id w:val="103"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013438"/></w:placeholder><w:showingPlcHdr/><w:comboBox><w:listItem w:value="Choose an item."/><w:listItem w:displayText="Apple" w:value="a"/><w:listItem w:displayText="Pear" w:value="p"/></w:comboBox></w:sdtPr><w:sdtContent>${placeholder("", "Choose an item.")}</w:sdtContent></w:sdt>`;

export const date = (format = "dddd, MMMM d, yyyy") =>
  `<w:sdt><w:sdtPr><w:alias w:val="Due"/><w:id w:val="104"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013437"/></w:placeholder><w:showingPlcHdr/><w:date><w:dateFormat w:val="${format}"/><w:lid w:val="en-US"/><w:storeMappedDataAs w:val="dateTime"/><w:calendar w:val="gregorian"/></w:date></w:sdtPr><w:sdtContent>${placeholder("", "Click or tap to enter a date.")}</w:sdtContent></w:sdt>`;

export const text = (lock = "", id = 105) =>
  `<w:sdt><w:sdtPr><w:alias w:val="Name"/><w:id w:val="${id}"/>${lock ? `<w:lock w:val="${lock}"/>` : ""}<w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013440"/></w:placeholder><w:showingPlcHdr/><w:text/></w:sdtPr><w:sdtContent>${placeholder("", "Click or tap here to enter text.")}</w:sdtContent></w:sdt>`;

export const filled = (lock: string, content = "locked words", id = 106) =>
  `<w:sdt><w:sdtPr><w:alias w:val="Fixed"/><w:id w:val="${id}"/><w:lock w:val="${lock}"/><w:richText/></w:sdtPr><w:sdtContent><w:r><w:t>${content}</w:t></w:r></w:sdtContent></w:sdt>`;

export const picture = () =>
  `<w:sdt><w:sdtPr><w:alias w:val="Logo"/><w:id w:val="107"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013440"/></w:placeholder><w:showingPlcHdr/><w:picture/></w:sdtPr><w:sdtContent>${placeholder("", "Click or tap to add a picture.")}</w:sdtContent></w:sdt>`;

/** A repeating section of paragraphs (block level), each item one paragraph holding a text control. */
export const repeating = (items = 1) => {
  const item = (n: number) =>
    `<w:sdt><w:sdtPr ${NS}><w:id w:val="${210 + n}"/><w15:repeatingSectionItem/></w:sdtPr><w:sdtContent><w:p><w:r><w:t xml:space="preserve">Line ${n + 1}: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="${230 + n}"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013440"/></w:placeholder><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>item ${n + 1}</w:t></w:r></w:sdtContent></w:sdt></w:p></w:sdtContent></w:sdt>`;
  return `<w:sdt><w:sdtPr ${NS}><w:alias w:val="Lines"/><w:id w:val="200"/><w15:repeatingSection><w15:sectionTitle w15:val="Line"/></w15:repeatingSection></w:sdtPr><w:sdtContent>${Array.from({ length: items }, (_, i) => item(i)).join("")}</w:sdtContent></w:sdt>`;
};

const fld = (kind: string) => `<w:r><w:fldChar w:fldCharType="${kind}"/></w:r>`;

/** Word's legacy text field with its blank of five en spaces. */
export const formText = (extra = "") =>
  `<w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:name w:val="Text1"/><w:enabled/><w:calcOnExit w:val="0"/><w:textInput>${extra}</w:textInput></w:ffData></w:fldChar></w:r><w:r><w:instrText xml:space="preserve"> FORMTEXT </w:instrText></w:r>${fld("separate")}<w:r><w:t>     </w:t></w:r>${fld("end")}`;

export const formCheck = () =>
  `<w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:name w:val="Check1"/><w:enabled/><w:calcOnExit w:val="0"/><w:checkBox><w:sizeAuto/><w:default w:val="0"/></w:checkBox></w:ffData></w:fldChar></w:r><w:r><w:instrText xml:space="preserve"> FORMCHECKBOX </w:instrText></w:r>${fld("separate")}${fld("end")}`;

export const formDrop = () =>
  `<w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:name w:val="Drop1"/><w:enabled/><w:calcOnExit w:val="0"/><w:ddList><w:result w:val="0"/><w:listEntry w:val="Tea"/><w:listEntry w:val="Coffee"/><w:listEntry w:val="Water"/></w:ddList></w:ffData></w:fldChar></w:r><w:r><w:instrText xml:space="preserve"> FORMDROPDOWN </w:instrText></w:r>${fld("separate")}${fld("end")}`;

/** `w:documentProtection` for the settings part. */
export const protect = (edit: string, extra = "") => `<w:documentProtection w:edit="${edit}" w:enforcement="1"${extra}/>`;

/** A part of the document the editor would save now, as text (or "" when it has none). */
export async function savedPart(page: Page, name = "word/document.xml"): Promise<string> {
  return page.evaluate(async (want) => {
    const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } } } } }).docxtor;
    const bytes = w.fe.editor.engine.save();
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let end = bytes.length - 22;
    while (end > 0 && dv.getUint32(end, true) !== 0x06054b50) end--;
    let p = dv.getUint32(end + 16, true);
    const n = dv.getUint16(end + 10, true);
    for (let i = 0; i < n; i++) {
      const method = dv.getUint16(p + 10, true);
      const csize = dv.getUint32(p + 20, true);
      const nlen = dv.getUint16(p + 28, true);
      const xlen = dv.getUint16(p + 30, true);
      const clen = dv.getUint16(p + 32, true);
      const off = dv.getUint32(p + 42, true);
      const name = new TextDecoder().decode(bytes.subarray(p + 46, p + 46 + nlen));
      p += 46 + nlen + xlen + clen;
      if (name !== want) continue;
      const start = off + 30 + dv.getUint16(off + 26, true) + dv.getUint16(off + 28, true);
      const data = bytes.subarray(start, start + csize);
      if (method === 0) return new TextDecoder().decode(data);
      return await new Response(new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"))).text();
    }
    return "";
  }, name);
}

/** Move the caret to the end or the start of its line (the End and Home keys scroll the page on a Mac). */
export const endOfLine = (page: Page) => page.keyboard.press(process.platform === "darwin" ? "Meta+ArrowRight" : "End");
export const startOfLine = (page: Page) => page.keyboard.press(process.platform === "darwin" ? "Meta+ArrowLeft" : "Home");
