// Read the parts of a .docx (or any zip) in a test: the file as the engine saved it, to check what is written into it.
import { inflateRawSync } from "node:zlib";

/** The text of each part of a zip, by name. Handles stored and deflated parts. */
export function readParts(bytes: Uint8Array | number[]): Map<string, string> {
  const b = Buffer.from(bytes);
  // The end of central directory record is the last 22 or more bytes.
  let end = b.length - 22;
  while (end >= 0 && b.readUInt32LE(end) !== 0x06054b50) end--;
  if (end < 0) throw new Error("not a zip file");
  const count = b.readUInt16LE(end + 10);
  let at = b.readUInt32LE(end + 16);
  const out = new Map<string, string>();
  for (let i = 0; i < count; i++) {
    if (b.readUInt32LE(at) !== 0x02014b50) throw new Error("bad central directory");
    const method = b.readUInt16LE(at + 10);
    const size = b.readUInt32LE(at + 20);
    const nameLen = b.readUInt16LE(at + 28);
    const extraLen = b.readUInt16LE(at + 30);
    const commentLen = b.readUInt16LE(at + 32);
    const local = b.readUInt32LE(at + 42);
    const name = b.toString("utf8", at + 46, at + 46 + nameLen);
    const dataStart = local + 30 + b.readUInt16LE(local + 26) + b.readUInt16LE(local + 28);
    const raw = b.subarray(dataStart, dataStart + size);
    out.set(name, (method === 0 ? raw : inflateRawSync(raw)).toString("utf8"));
    at += 46 + nameLen + extraLen + commentLen;
  }
  return out;
}
