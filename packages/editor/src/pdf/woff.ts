// WOFF (version 1) is a TrueType font whose tables are compressed one by one with zlib. Unwrapping it
// gives the plain font file back. (WOFF2, which needs Brotli, is in woff2.ts.)
import { buildSfnt } from "./truetype.ts";
import { inflate } from "./writer.ts";

export const isWoff = (b: Uint8Array): boolean => b.length > 44 && b[0] === 0x77 && b[1] === 0x4f && b[2] === 0x46 && b[3] === 0x46;

/** A WOFF file as a plain TrueType or OpenType file. */
export async function unwrapWoff(bytes: Uint8Array): Promise<Uint8Array> {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const count = v.getUint16(12);
  const flavor = v.getUint32(4);
  const tables: [string, Uint8Array][] = [];
  for (let i = 0; i < count; i++) {
    const r = 44 + i * 20;
    const tag = String.fromCharCode(bytes[r], bytes[r + 1], bytes[r + 2], bytes[r + 3]);
    const offset = v.getUint32(r + 4);
    const comp = v.getUint32(r + 8);
    const orig = v.getUint32(r + 12);
    if (offset + comp > bytes.length) throw new Error("The WOFF file is cut short");
    const data = bytes.subarray(offset, offset + comp);
    tables.push([tag, comp < orig ? await inflate(data) : data.slice()]);
  }
  const sfnt = buildSfnt(tables);
  // buildSfnt writes a TrueType header; an OpenType font with CFF outlines keeps its own.
  if (flavor === 0x4f54544f) new DataView(sfnt.buffer).setUint32(0, flavor);
  return sfnt;
}
