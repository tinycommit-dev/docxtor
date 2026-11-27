// Recovery copies: while a document has changes that are not saved, a copy of it is kept in the browser every few
// seconds, and dropped when the changes are saved. If the page closes any other way (a crash, a killed tab, a power
// cut), the copy is still there next time, and Docxtor offers it back. Each page holds a Web Lock while it is open, so a
// copy that belongs to a page that is still open is not offered as lost.
import { del, getAll, put } from "./db";

export interface RecoveryCopy {
  /** The document's key (`library:Reports/Plan.docx`), one copy for each. */
  key: string;
  name: string;
  /** Where it lives, in words ("your Library", "a file on disk"). */
  place: string;
  /** The bytes as `Engine.save()` gives them (a protected document stays protected). */
  bytes: Uint8Array;
  words: number;
  at: number;
  /** The page that kept it. */
  session: string;
}

/** Pages hold a lock with this prefix and their session's name while they are open. */
const LOCK = "docxtor-page-";

/** This page's session name. */
export const session: string = (typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : String(Math.random()).slice(2)).slice(0, 18);

let holding = false;

/** Hold the page's lock for as long as it lives (called once at start). */
export function holdSession(): void {
  if (holding || !navigator.locks) return;
  holding = true;
  void navigator.locks.request(`${LOCK}${session}`, () => new Promise<void>(() => undefined)).catch(() => undefined);
}

/** The sessions of pages that are open right now (this one included). Without Web Locks every copy counts as lost. */
async function liveSessions(): Promise<Set<string>> {
  const live = new Set<string>([session]);
  try {
    const held = await navigator.locks?.query();
    for (const l of held?.held ?? []) if (l.name?.startsWith(LOCK)) live.add(l.name.slice(LOCK.length));
  } catch {
    /* no Web Locks */
  }
  return live;
}

export async function keepCopy(copy: Omit<RecoveryCopy, "session">): Promise<void> {
  await put("recovery", { ...copy, session }).catch(() => undefined);
}

export async function dropCopy(key: string): Promise<void> {
  await del("recovery", key).catch(() => undefined);
}

/** The copies left by pages that are no longer open, newest first. */
export async function lostCopies(): Promise<RecoveryCopy[]> {
  const live = await liveSessions();
  const all = await getAll<RecoveryCopy>("recovery").catch(() => []);
  return all.filter((c) => !live.has(c.session)).sort((a, b) => b.at - a.at);
}
