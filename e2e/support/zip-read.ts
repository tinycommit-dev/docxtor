// Reads one part of a zip (a .docx saved by the page) in a test: enough of the format to find a part by name.
import { inflateRawSync } from "node:zlib";

/** The text of a part, or null when the zip has none by that name. */
export function partOf(bytes: ArrayLike<number>, name: string): string | null {
  const buf = Buffer.from(Array.from(bytes));
  let end = buf.length - 22;
  while (end > 0 && buf.readUInt32LE(end) !== 0x06054b50) end--;
  let p = buf.readUInt32LE(end + 16);
  const count = buf.readUInt16LE(end + 10);
  for (let i = 0; i < count; i++) {
    const method = buf.readUInt16LE(p + 10);
    const csize = buf.readUInt32LE(p + 20);
    const nlen = buf.readUInt16LE(p + 28);
    const xlen = buf.readUInt16LE(p + 30);
    const clen = buf.readUInt16LE(p + 32);
    const off = buf.readUInt32LE(p + 42);
    const n = buf.toString("utf8", p + 46, p + 46 + nlen);
    p += 46 + nlen + xlen + clen;
    if (n !== name) continue;
    const start = off + 30 + buf.readUInt16LE(off + 26) + buf.readUInt16LE(off + 28);
    const data = buf.subarray(start, start + csize);
    return (method === 0 ? data : inflateRawSync(data)).toString("utf8");
  }
  return null;
}

/** The names of every part, in the order the zip lists them. */
export function partNames(bytes: ArrayLike<number>): string[] {
  const buf = Buffer.from(Array.from(bytes));
  let end = buf.length - 22;
  while (end > 0 && buf.readUInt32LE(end) !== 0x06054b50) end--;
  let p = buf.readUInt32LE(end + 16);
  const count = buf.readUInt16LE(end + 10);
  const names: string[] = [];
  for (let i = 0; i < count; i++) {
    const nlen = buf.readUInt16LE(p + 28);
    const xlen = buf.readUInt16LE(p + 30);
    const clen = buf.readUInt16LE(p + 32);
    names.push(buf.toString("utf8", p + 46, p + 46 + nlen));
    p += 46 + nlen + xlen + clen;
  }
  return names;
}
