// Download size: compressed bytes, exact and the same on every machine.
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { readFileSync } from "node:fs";
import type { Metrics } from "./gate.mts";

export const gz = (b: Uint8Array) => gzipSync(b, { level: 9 }).byteLength;
export const br = (b: Uint8Array) => brotliCompressSync(b, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_SIZE_HINT]: b.byteLength } }).byteLength;

export function wasmSizes(path: string, prefix = "size.engine"): Metrics {
  const b = readFileSync(path);
  return { [`${prefix}.raw`]: b.byteLength, [`${prefix}.gzip`]: gz(b), [`${prefix}.brotli`]: br(b) };
}
