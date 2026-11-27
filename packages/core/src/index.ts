// @docxtor/core — Docxtor's lossless .docx engine.
//
//   import { loadEngine, Engine } from "@docxtor/core";
//   await loadEngine();
//   const doc = Engine.open(bytes, "report.docx");
//   doc.apply({ op: "replace_text", find: "2025", replace: "2026", all: true });
//   const out = doc.save();
//
// Every edit is a serializable op (see docs/ai/index.md and crates/docxtor-core/src/ops.rs).
import { diffDocuments as rawDiff, fidelityReport as rawFidelity, DocxtorDoc, parseBibtex as rawParseBibtex, parseCslJson as rawParseCslJson } from "../wasm/docxtor.js";
import { guarded } from "./crash.ts";
import type { DiffSummary, FidelityReport, Source, ToolDefinition } from "./types.ts";

const wasmFidelity = guarded(rawFidelity);
const wasmDiff = guarded(rawDiff);
const wasmParseBibtex = guarded(rawParseBibtex);
const wasmParseCslJson = guarded(rawParseCslJson);

export { assertEngineSound, engineCrash, EngineCrashedError, onEngineCrash, type EngineCrashedCode } from "./crash.ts";
export { CryptError, EncryptedError, Engine, type PerfMeter, isEncrypted, loadEngine, packageAsDocument, packageAsTemplate, packageKind, WrongPasswordError, type CryptCode, type SaveInfo, type WasmSource } from "./engine.ts";
export * from "./types.ts";
export { OpenError, type OpenFormat, type OpenNote, type OpenReport, type SniffedFormat } from "./open-types.ts";
export * from "./find-types.ts";
export * from "./outline-types.ts";
export * from "./shapes.ts";
export * from "./display-types.ts";
export * from "./chart-types.ts";
export * from "./chartex-types.ts";
export * from "./list-types.ts";
export * from "./objects.ts";export * from "./table-types.ts";
export * from "./design-types.ts";
export * from "./east-asian-types.ts";
export * from "./mail-types.ts";

/** Compare two .docx packages part by part (how much of the original survives). */
export function fidelityReport(original: Uint8Array, saved: Uint8Array): FidelityReport {
  return JSON.parse(wasmFidelity(original, saved));
}

/** Paragraph-level changes between two versions of a document. */
export function diffDocuments(older: Uint8Array, newer: Uint8Array): DiffSummary {
  return JSON.parse(wasmDiff(older, newer));
}

/** The AI tool set (Anthropic tool-use shape), shared with the MCP server and CLI. */
export function toolDefinitions(): ToolDefinition[] {
  return JSON.parse(DocxtorDoc.toolDefinitions());
}

/** Guidance for models editing through the tools, for use in a system prompt. */
export function systemGuide(): string {
  return DocxtorDoc.systemGuide();
}

/** Read BibTeX text into sources (without tags); throws a plain message when there is nothing to read. */
export function parseBibtex(text: string): Source[] {
  return JSON.parse(wasmParseBibtex(text));
}

/** Read CSL-JSON (Zotero, Mendeley, doi.org) into sources (without tags). */
export function parseCslJson(text: string): Source[] {
  return JSON.parse(wasmParseCslJson(text));
}
