// Password-protection errors, and the package helpers that need no document (template or document, protected or not).
import { wasmIsEncrypted, wasmPackageAsDocument, wasmPackageAsTemplate, wasmPackageKind } from "./engine-wasm.ts";

/** Why a protected file could not be opened. `code` tells the cases apart. */
export type CryptCode = "encrypted" | "wrong_password" | "integrity" | "corrupt" | "unsupported";

/** A password-protected file could not be opened. `EncryptedError` and `WrongPasswordError` are the two a host asks the person about. */
export class CryptError extends Error {
  readonly code: CryptCode;
  constructor(code: CryptCode, message: string) {
    super(message);
    this.name = "CryptError";
    this.code = code;
  }
}

/** The file is protected with a password and none was given. */
export class EncryptedError extends CryptError {
  constructor(message = "This document is protected with a password") {
    super("encrypted", message);
    this.name = "EncryptedError";
  }
}

/** A password was given and it is not the file's. */
export class WrongPasswordError extends CryptError {
  constructor(message = "That password isn't right") {
    super("wrong_password", message);
    this.name = "WrongPasswordError";
  }
}

/** Turn what the engine threw while decrypting into a typed error. */
export function cryptError(e: unknown): unknown {
  const m = /^docxtor-crypt:(\w+): ([\s\S]*)$/.exec(e instanceof Error ? e.message : String(e));
  if (!m) return e;
  if (m[1] === "wrong_password") return new WrongPasswordError(m[2]);
  return new CryptError(m[1] as CryptCode, m[2]);
}

/** Whether the bytes are a password-protected Office file (not a .docx zip yet). Needs `loadEngine()` first. */
export function isEncrypted(bytes: Uint8Array): boolean {
  return wasmIsEncrypted(bytes);
}

/** Whether .docx bytes are a Word template (`.dotx`, `.dotm` with macros) or a document, and whether they have macros. Needs `loadEngine()` first. */
export function packageKind(bytes: Uint8Array): { template: boolean; macros: boolean } {
  return JSON.parse(wasmPackageKind(bytes));
}

/** The same package as a template: only the content type of the main part changes, so everything else stays byte for byte. */
export function packageAsTemplate(bytes: Uint8Array): Uint8Array {
  return wasmPackageAsTemplate(bytes);
}

/**
 * The same package as a document. With `fresh` it is also a new document, as Word makes one from a template: made
 * now, with no record of who changed it last.
 */
export function packageAsDocument(bytes: Uint8Array, fresh?: Date): Uint8Array {
  return wasmPackageAsDocument(bytes, fresh ? fresh.toISOString().replace(/\.\d+Z$/, "Z") : undefined);
}
