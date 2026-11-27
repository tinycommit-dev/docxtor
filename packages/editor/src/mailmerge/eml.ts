// One merged message as an .eml file (RFC 5322 with MIME): plain text, a web page, or both as alternatives, with the merged document
// attached when that is how it is sent. Everything in the file is ASCII: text is quoted-printable UTF-8, files are base64, and a
// subject with other letters is an encoded word (RFC 2047). The `X-Unsent` header makes mail apps open it as a message to send.
// Pure, so it runs in Node too.
import type { EmailMessage } from "@docxtor/extensions";

export interface EmlOptions {
  /** The Date header. Default now. */
  date?: Date;
  /** Makes the strings that separate the parts. Default random. */
  boundary?: () => string;
  /** The From header, when the message has a sender. Default none: the mail app fills it in. */
  from?: string;
}

const CRLF = "\r\n";
const enc = new TextEncoder();

const toBase64 = (bytes: Uint8Array): string => {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
};

/** Base64 in lines of at most 76 characters. */
const wrap64 = (b64: string): string => (b64.match(/.{1,76}/g) ?? []).join(CRLF);

/** Text as quoted-printable UTF-8 with lines of at most 76 characters (RFC 2045). Line breaks become CRLF. */
export function quotedPrintable(text: string): string {
  const out: string[] = [];
  const lines = text.replace(/\r\n|\r/g, "\n").split("\n");
  for (const line of lines) {
    const bytes = enc.encode(line);
    let row = "";
    for (let i = 0; i < bytes.length; i++) {
      const b = bytes[i];
      // A space or tab at the end of a line would be lost in transit, so it is written out.
      const last = i === bytes.length - 1;
      const plain = (b >= 33 && b <= 126 && b !== 61) || ((b === 32 || b === 9) && !last);
      const piece = plain ? String.fromCharCode(b) : `=${b.toString(16).toUpperCase().padStart(2, "0")}`;
      if (row.length + piece.length > 75) {
        out.push(`${row}=`);
        row = "";
      }
      row += piece;
    }
    out.push(row);
  }
  return out.join(CRLF);
}

/** A header value with letters outside ASCII as encoded words (RFC 2047), folded into lines that stay short. */
export function encodeHeader(value: string): string {
  const clean = value.replace(/[\r\n]+/g, " ").trim();
  if (/^[\x20-\x7e]*$/.test(clean)) return clean;
  const words: string[] = [];
  let chunk: number[] = [];
  const flush = () => {
    if (chunk.length) words.push(`=?UTF-8?B?${toBase64(Uint8Array.from(chunk))}?=`);
    chunk = [];
  };
  // Whole characters only, so no word ends in the middle of a letter. 36 bytes make a word of 60 characters.
  for (const ch of clean) {
    const bytes = enc.encode(ch);
    if (chunk.length + bytes.length > 36) flush();
    chunk.push(...bytes);
  }
  flush();
  return words.join(`${CRLF} `);
}

/** A file name as the Content-Disposition header gives it: plain when it is ASCII, with the RFC 2231 form beside it when it is not. */
function fileNameParam(name: string, param: "filename" | "name"): string {
  const safe = name.replace(/[\r\n"\\]/g, "_");
  if (/^[\x20-\x7e]*$/.test(safe)) return `${param}="${safe}"`;
  const ascii = safe.replace(/[^\x20-\x7e]/g, "_");
  const pct = encodeURIComponent(safe).replace(/['()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
  return `${param}="${ascii}"; ${param}*=UTF-8''${pct}`;
}

function randomBoundary(): string {
  const bytes = new Uint8Array(12);
  globalThis.crypto.getRandomValues(bytes);
  return `=_docxtor_${Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")}`;
}

/**
 * The address as one line with nothing that could start another header or another address. A domain with letters
 * outside ASCII (`bücher.example`) is written in its ASCII form (punycode), as every mail app reads it. A name before
 * the @ with such letters can't be written in ASCII; it stays as it is, which mail apps with UTF-8 headers accept.
 */
const address = (to: string): string => {
  const a = to.replace(/[\x00-\x1f\x7f<>,;"]/g, "").trim();
  const at = a.lastIndexOf("@");
  if (at < 0 || /^[\x00-\x7f]*$/.test(a.slice(at + 1))) return a;
  try {
    return `${a.slice(0, at)}@${new URL(`http://${a.slice(at + 1)}`).hostname}`;
  } catch {
    return a;
  }
};

const part = (headers: string[], body: string): string => `${headers.join(CRLF)}${CRLF}${CRLF}${body}`;

const rfcDate = (d: Date): string => d.toUTCString().replace("GMT", "+0000");

/** The .eml text of a message. */
export function buildEml(msg: EmailMessage, opts: EmlOptions = {}): string {
  const boundary = opts.boundary ?? randomBoundary;
  const text = msg.text;
  const html = msg.html;
  const atts = msg.attachments ?? [];
  const textPart = (body: string) => part(["Content-Type: text/plain; charset=utf-8", "Content-Transfer-Encoding: quoted-printable"], quotedPrintable(body));
  const htmlPart = (body: string) => part(["Content-Type: text/html; charset=utf-8", "Content-Transfer-Encoding: quoted-printable"], quotedPrintable(body));
  const multipart = (kind: "alternative" | "mixed", parts: string[]): { type: string; body: string } => {
    const b = boundary();
    return { type: `multipart/${kind}; boundary="${b}"`, body: [...parts.map((p) => `--${b}${CRLF}${p}`), `--${b}--`].join(CRLF) + CRLF };
  };

  // The readable body: both alternatives (the plainest first, as the standard asks), or whichever there is, or an empty text.
  let inner: { type: string; body: string };
  if (html !== undefined && text !== undefined) inner = multipart("alternative", [textPart(text), htmlPart(html)]);
  else if (html !== undefined) inner = { type: "text/html; charset=utf-8", body: quotedPrintable(html) };
  else inner = { type: "text/plain; charset=utf-8", body: quotedPrintable(text ?? "") };
  const innerIsLeaf = !inner.type.startsWith("multipart/");

  let top: { type: string; body: string; encoding?: string };
  if (atts.length) {
    const lead = innerIsLeaf ? part([`Content-Type: ${inner.type}`, "Content-Transfer-Encoding: quoted-printable"], inner.body) : part([`Content-Type: ${inner.type}`], inner.body);
    const files = atts.map((a) =>
      part([`Content-Type: ${a.type || "application/octet-stream"}; ${fileNameParam(a.name, "name")}`, "Content-Transfer-Encoding: base64", `Content-Disposition: attachment; ${fileNameParam(a.name, "filename")}`], wrap64(toBase64(a.bytes))),
    );
    top = multipart("mixed", [lead, ...files]);
  } else top = innerIsLeaf ? { ...inner, encoding: "quoted-printable" } : inner;

  const head = ["X-Unsent: 1", ...(opts.from ? [`From: ${address(opts.from)}`] : []), `To: ${address(msg.to)}`, `Subject: ${encodeHeader(msg.subject)}`, `Date: ${rfcDate(opts.date ?? new Date())}`, "MIME-Version: 1.0", `Content-Type: ${top.type}`];
  if (top.encoding) head.push(`Content-Transfer-Encoding: ${top.encoding}`);
  return `${head.join(CRLF)}${CRLF}${CRLF}${top.body}${top.body.endsWith(CRLF) ? "" : CRLF}`;
}

/** The bytes of the .eml file (the text is all ASCII). */
export const emlBytes = (msg: EmailMessage, opts?: EmlOptions): Uint8Array => enc.encode(buildEml(msg, opts));

/** A file name that any system takes: no slashes, colons or other characters it reserves, and not too long. */
export function safeFileName(name: string, fallback = "message"): string {
  const cleaned = name.replace(/[\x00-\x1f\x7f<>:"/\\|?*]+/g, " ").replace(/\s+/g, " ").trim().replace(/^\.+/, "");
  const cut = [...cleaned].slice(0, 80).join("").trim();
  return cut || fallback;
}
