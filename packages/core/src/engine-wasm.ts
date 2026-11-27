// The WebAssembly binding: loading it once, and every loose function of it behind the crash guard (see crash.ts).
import init, { DocxtorCrypt, DocxtorDoc, packageKind as rawPackageKind, packageAsTemplate as rawPackageAsTemplate, packageAsDocument as rawPackageAsDocument, labelProducts as rawLabelProducts, pageGallery as rawPageGallery, envelopeSizes as rawEnvelopeSizes, mergeFieldInstr as rawMergeFieldInstr, mergeVocabulary as rawMergeVocabulary, readRecipients as rawReadRecipients, chartMerge as rawChartMerge, isEncrypted as rawIsEncrypted, sniffFormat as rawSniffFormat, fragmentText as rawFragmentText, spacingPresets as rawSpacingPresets, styleSetInfos as rawStyleSetInfos, themeLibrary as rawThemeLibrary, htmlImageSources as rawHtmlImageSources, htmlToFragment as rawHtmlToFragment, latexToMathml as rawLatexToMathml, parseBibtex as rawParseBibtex, parseCslJson as rawParseCslJson, readAutoMarkFile as rawReadAutoMarkFile, newAutoMarkFile as rawNewAutoMarkFile, rtfToFragment as rawRtfToFragment, setClipboardNonce as rawSetClipboardNonce, opsAreViewOnly as rawOpsAreViewOnly } from "../wasm/docxtor.js";
import { guardBinding, guarded } from "./crash.ts";
import { setEngineModule } from "./engine-module.ts";

// Every call into the binding is watched for a trap (see crash.ts): the classes once, the loose functions here.
guardBinding(DocxtorDoc);
guardBinding(DocxtorCrypt);
export const wasmPackageKind = guarded(rawPackageKind);
export const wasmPackageAsTemplate = guarded(rawPackageAsTemplate);
export const wasmPackageAsDocument = guarded(rawPackageAsDocument);
export const wasmLabelProducts = guarded(rawLabelProducts);
export const wasmPageGallery = guarded(rawPageGallery);
export const wasmEnvelopeSizes = guarded(rawEnvelopeSizes);
export const wasmMergeFieldInstr = guarded(rawMergeFieldInstr);
export const wasmMergeVocabulary = guarded(rawMergeVocabulary);
export const wasmReadRecipients = guarded(rawReadRecipients);
export const wasmChartMerge = guarded(rawChartMerge);
export const wasmIsEncrypted = guarded(rawIsEncrypted);
export const wasmSniffFormat = guarded(rawSniffFormat);
export const wasmFragmentText = guarded(rawFragmentText);
export const wasmSpacingPresets = guarded(rawSpacingPresets);
export const wasmStyleSetInfos = guarded(rawStyleSetInfos);
export const wasmThemeLibrary = guarded(rawThemeLibrary);
export const wasmHtmlImageSources = guarded(rawHtmlImageSources);
export const wasmHtmlToFragment = guarded(rawHtmlToFragment);
export const wasmLatexToMathml = guarded(rawLatexToMathml);
export const wasmParseBibtex = guarded(rawParseBibtex);
export const wasmParseCslJson = guarded(rawParseCslJson);
export const wasmReadAutoMarkFile = guarded(rawReadAutoMarkFile);
export const wasmNewAutoMarkFile = guarded(rawNewAutoMarkFile);
export const wasmRtfToFragment = guarded(rawRtfToFragment);
export const wasmSetClipboardNonce = guarded(rawSetClipboardNonce);
export const wasmOpsAreViewOnly = guarded(rawOpsAreViewOnly);


/** Where to load the engine's WebAssembly from: a URL, a fetch Response, or the bytes themselves. */
export type WasmSource = string | URL | Request | Response | BufferSource | WebAssembly.Module;

let ready: Promise<unknown> | null = null;

/**
 * In a page, the engine compiled in a worker that closes after (compile-worker.ts): compiling it here would leave the memory the
 * compiling used for the browser to give back later, about 27 MB. Undefined where there are no workers (Node, a worker itself) or the
 * worker fails: the engine is then compiled where it runs, as before.
 */
function compiledOffPage(): Promise<WebAssembly.Module | undefined> {
  if (typeof Worker === "undefined" || typeof document === "undefined") return Promise.resolve(undefined);
  return new Promise((resolve) => {
    let worker: Worker;
    try {
      worker = new Worker(new URL("./compile-worker.ts", import.meta.url), { type: "module" });
    } catch {
      resolve(undefined);
      return;
    }
    const done = (m: WebAssembly.Module | undefined) => {
      worker.terminate();
      resolve(m);
    };
    worker.onmessage = (e: MessageEvent<WebAssembly.Module | null>) => done(e.data instanceof WebAssembly.Module ? e.data : undefined);
    worker.onerror = () => done(undefined);
    worker.postMessage(new URL("../wasm/docxtor_bg.wasm", import.meta.url).href);
  });
}

/**
 * Load the engine once. In browsers with a bundler, call it with no argument: the `.wasm` file is
 * resolved next to this module. Elsewhere (Node, Deno, a CDN, a strict CSP host) pass its location
 * or its bytes, e.g. `loadEngine(await readFile("node_modules/@docxtor/core/wasm/docxtor_bg.wasm"))`.
 */
export function loadEngine(source?: WasmSource): Promise<unknown> {
  ready ??= (source === undefined ? compiledOffPage() : Promise.resolve(source)).then((src) => (setEngineModule(src), src)).then((src) => init(src === undefined ? undefined : { module_or_path: src })).then((r) => {
    // A secret for this page load: fragments Docxtor copies carry it, so a paste knows its own (see `Fragment.origin`).
    const bytes = crypto.getRandomValues(new Uint8Array(24));
    wasmSetClipboardNonce(Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join(""));
    return r;
  });
  return ready;
}
