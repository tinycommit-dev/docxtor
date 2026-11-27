// A small PDF writer: numbered objects, Flate streams, the cross-reference table and trailer.
// It knows nothing about documents or fonts; the rest of src/pdf builds on it. No DOM: it runs in Node too.

const enc = new TextEncoder();

/** `12 0 R` for object number 12. */
export const ref = (n: number): string => `${n} 0 R`;

/** A number as PDF writes it: no exponent, at most three decimals, no trailing zeros. */
export function num(x: number): string {
  if (!Number.isFinite(x)) return "0";
  const s = x.toFixed(3);
  if (s.indexOf(".") < 0) return s;
  const t = s.replace(/0+$/, "").replace(/\.$/, "");
  return t === "-0" ? "0" : t;
}

/** A name object: `/Font`, with anything outside the printable ASCII range written as #xx. */
export function pdfName(name: string): string {
  let out = "/";
  for (const b of enc.encode(name)) {
    const c = String.fromCharCode(b);
    out += b < 33 || b > 126 || "#()<>[]{}/%".includes(c) ? `#${b.toString(16).padStart(2, "0")}` : c;
  }
  return out;
}

/** A text string: plain ASCII in parentheses, anything else as UTF-16 with a byte order mark. */
export function pdfString(s: string): string {
  if (/^[\x20-\x7e]*$/.test(s)) return `(${s.replace(/[\\()]/g, "\\$&")})`;
  let hex = "FEFF";
  for (let i = 0; i < s.length; i++) hex += s.charCodeAt(i).toString(16).padStart(4, "0");
  return `<${hex.toUpperCase()}>`;
}

/** Bytes as a hexadecimal string object. */
export function hexString(bytes: Uint8Array): string {
  let out = "<";
  for (const b of bytes) out += b.toString(16).padStart(2, "0");
  return `${out}>`;
}

/** A PDF date: `D:20261013120000Z`. */
export function pdfDate(d: Date): string {
  const p = (n: number, w = 2) => String(n).padStart(w, "0");
  return `(D:${p(d.getUTCFullYear(), 4)}${p(d.getUTCMonth() + 1)}${p(d.getUTCDate())}${p(d.getUTCHours())}${p(d.getUTCMinutes())}${p(d.getUTCSeconds())}Z)`;
}

/** Concatenate byte arrays. */
export function concat(parts: Uint8Array[]): Uint8Array {
  let n = 0;
  for (const p of parts) n += p.length;
  const out = new Uint8Array(n);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

async function pipe(data: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  const writer = stream.writable.getWriter();
  void writer.write(data as BufferSource).then(() => writer.close());
  const chunks: Uint8Array[] = [];
  const reader = stream.readable.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }
  return concat(chunks);
}

/** zlib-wrapped deflate, which is what a PDF's FlateDecode filter reads. */
export const deflate = (data: Uint8Array): Promise<Uint8Array> => pipe(data, new CompressionStream("deflate"));

/** Undo `deflate`. */
export const inflate = (data: Uint8Array): Promise<Uint8Array> => pipe(data, new DecompressionStream("deflate"));

/** Undo a bare deflate stream (no zlib header), as in a ZIP or a WOFF table. */
export const inflateRaw = (data: Uint8Array): Promise<Uint8Array> => pipe(data, new DecompressionStream("deflate-raw"));

/** The objects of a PDF file, written in the order they are added. */
export class PdfWriter {
  private bodies: (Uint8Array | null)[] = [null];
  /** Version of the PDF format the file says it is. */
  version = "1.7";

  /** Reserve an object number; fill it with `put` or `putStream` before `build`. */
  alloc(): number {
    this.bodies.push(null);
    return this.bodies.length - 1;
  }

  /** Set the body of object `n` (a dictionary, array, number…). */
  put(n: number, body: string): void {
    this.bodies[n] = enc.encode(`${n} 0 obj\n${body}\nendobj\n`);
  }

  /** Add an object in one step and return its number. */
  add(body: string): number {
    const n = this.alloc();
    this.put(n, body);
    return n;
  }

  /**
   * Set object `n` to a stream. `dict` holds the entries of the stream dictionary (without `<<` and `>>`
   * and without /Length). Data is Flate-compressed unless `compress` is false or it doesn't get smaller.
   */
  async putStream(n: number, dict: string, data: Uint8Array, compress = true): Promise<void> {
    let body = data;
    let filter = "";
    if (compress && data.length > 32) {
      const z = await deflate(data);
      if (z.length < data.length) {
        body = z;
        filter = "/Filter /FlateDecode ";
      }
    }
    const head = enc.encode(`${n} 0 obj\n<< ${dict} ${filter}/Length ${body.length} >>\nstream\n`);
    this.bodies[n] = concat([head, body, enc.encode("\nendstream\nendobj\n")]);
  }

  /** Add a stream object and return its number. */
  async addStream(dict: string, data: Uint8Array, compress = true): Promise<number> {
    const n = this.alloc();
    await this.putStream(n, dict, data, compress);
    return n;
  }

  /** The whole file: header, objects, cross-reference table and trailer. */
  build(root: number, info?: number, fileId?: Uint8Array): Uint8Array {
    const parts: Uint8Array[] = [enc.encode(`%PDF-${this.version}\n%`), new Uint8Array([0xe2, 0xe3, 0xcf, 0xd3, 10])];
    const offsets: number[] = [0];
    let at = parts[0].length + parts[1].length;
    for (let n = 1; n < this.bodies.length; n++) {
      const b = this.bodies[n];
      if (!b) throw new Error(`PDF object ${n} was reserved but never written`);
      offsets[n] = at;
      parts.push(b);
      at += b.length;
    }
    const size = this.bodies.length;
    let xref = `xref\n0 ${size}\n0000000000 65535 f \n`;
    for (let n = 1; n < size; n++) xref += `${String(offsets[n]).padStart(10, "0")} 00000 n \n`;
    const id = fileId ? hexString(fileId) : "<00000000000000000000000000000000>";
    xref += `trailer\n<< /Size ${size} /Root ${ref(root)}${info ? ` /Info ${ref(info)}` : ""} /ID [${id} ${id}] >>\nstartxref\n${at}\n%%EOF\n`;
    parts.push(enc.encode(xref));
    return concat(parts);
  }
}
