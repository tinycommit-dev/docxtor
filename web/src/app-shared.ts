// What the app and its modules share: storage keys, recovery timing, document references and the save helpers.
import { navigator } from "./builtins";
import { listVersions } from "./files/versions";
import type { Snapshot } from "./save-queue";
import type { Backend } from "./files/fs";

export const THEME_KEY = "docxtor.theme";
export const ICON_PACK_KEY = "docxtor.iconPack";
export const EXT_KEY = "docxtor.extensions";
export const LANG_KEY = "docxtor.language";
/**
 * When a recovery copy is kept: after typing has paused for `idle`, and at least every `max` while it never does. A copy is the
 * whole document, so it is not made on every pause of a second or two. (The tests shorten these.)
 */
export const RECOVERY_DELAYS = { idle: 4000, max: 30000 };

export interface DocRef {
  backend: Backend;
  path: string;
}

export type SaveState = "saved" | "dirty" | "saving" | "error" | "unsaved-file";

/** A key or a click is waiting for the page (Chrome's `isInputPending`): a save that is about to begin lets it through first. */
export function inputPending(): boolean {
  try {
    return !!(navigator as unknown as { scheduling?: { isInputPending?: () => boolean } }).scheduling?.isInputPending?.();
  } catch {
    return false;
  }
}

/** Between the steps of a save: let the keys that arrived run (a timer task goes behind input), and wait while more are pending. */
export async function yieldToInput(): Promise<void> {
  for (let i = 0; i < 10; i++) {
    await new Promise<void>((r) => window.setTimeout(r, i ? 16 : 0));
    if (!inputPending()) return;
  }
}

/** What one save takes from the open document: the revision, its bytes, and where they go (the document may be renamed or saved elsewhere before the write ends). */
export interface Taken extends Snapshot {
  bytes: Uint8Array;
  ref: DocRef;
}

/** Move a document's saved versions to its new name. */
export async function moveHistory(from: string, to: string) {
  const { put } = await import("./files/db");
  for (const v of await listVersions(from)) await put("versions", { ...v, doc: to });
}
