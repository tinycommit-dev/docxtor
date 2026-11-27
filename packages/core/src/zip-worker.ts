// Compresses a saved document in a worker, so the page's main thread does not: a long book spends more than 100 ms in the
// compressing alone. It holds its own copy of the engine (the same .wasm file, which the browser already has) and uses
// the functions that need no document: `zipPacked`, and `fidelityReport` for the status bar's fidelity readout (with `original`: the file as
// it was opened, compared part by part with the zip; the zip itself is not sent back).
import init, { fidelityReport, zipPacked } from "../wasm/docxtor.js";

/** The first message: the engine's compiled module, so the worker does not compile it again. */
export interface ZipInit {
  module: WebAssembly.Module;
}
export interface ZipRequest {
  id: number;
  packed: ArrayBuffer;
  original?: ArrayBuffer;
}
export type ZipResponse = { id: number; bytes: ArrayBuffer } | { id: number; report: string } | { id: number; error: string };

const scope = self as unknown as { postMessage(message: ZipResponse, transfer: Transferable[]): void; onmessage: ((e: MessageEvent<ZipRequest | ZipInit>) => void) | null };
let ready: Promise<unknown> | null = null;
/** The engine, from the page's compiled module when it sent one, else compiled here. */
const start = (module?: WebAssembly.Module) => (ready ??= module ? init({ module_or_path: module }) : init());

scope.onmessage = async (e) => {
  if ("module" in e.data) {
    start(e.data.module);
    return;
  }
  const { id, packed, original } = e.data;
  try {
    await start();
    const bytes = zipPacked(new Uint8Array(packed));
    if (original) {
      scope.postMessage({ id, report: fidelityReport(new Uint8Array(original), bytes) }, []);
      return;
    }
    // The result owns its buffer (it was copied out of the engine's memory), so it moves without a copy.
    scope.postMessage({ id, bytes: bytes.buffer as ArrayBuffer }, [bytes.buffer as ArrayBuffer]);
  } catch (err) {
    scope.postMessage({ id, error: err instanceof Error ? err.message : String(err) }, []);
  }
};
