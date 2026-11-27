// One IndexedDB database for everything Docxtor keeps in the browser:
// the Library (an in-browser file system), remembered folder/file handles,
// recent documents, and version history (records + bytes stored separately), plus what was deleted (the trash),
// what a crash could have lost (recovery copies) and the person's own templates.

import { t } from "@docxtor/editor";

const NAME = "docxtor";
const VERSION = 3;

let conn: Promise<IDBDatabase> | null = null;
/** Set when a newer Docxtor asked this page to let go of the database: this page's build can't open it again. */
let stale = false;

/** What the page says when the database can't be opened right away (the app sets these; this file has no interface of its own). */
export const dbNotices: {
  /** A page of an older Docxtor still holds the database, so opening waits for it. */
  waiting?: () => void;
  /** This page let go of the database because a newer Docxtor needs it. */
  outdated?: () => void;
  /** The database was saved by a newer Docxtor than this page: nothing can be opened or kept from here. */
  newer?: () => void;
} = {};

/**
 * The database in this browser was written by a newer Docxtor than this page's build (a rollback, or a tab still on an old build
 * after an update). The browser refuses to open it (a `VersionError`), and so does this page: it never deletes it, never opens it at
 * its own version and never writes to it, so what the newer Docxtor kept stays as it is. Reloading brings the newer build.
 */
export class NewerDataError extends Error {
  readonly code = "newer_data";
  constructor() {
    super(t("The documents kept in this browser were saved by a newer version of Docxtor, which this page can't read yet. Nothing was changed. Reload the page to get the newer version."));
    this.name = "NewerDataError";
  }
}

/** Set when the browser refused to open the database because it is newer: for the rest of this page's life, every use of it says so. */
let newer: NewerDataError | null = null;

/** Whether this page let go of the database for a newer Docxtor: nothing can be saved to the browser from here any more. */
export const isStale = (): boolean => stale;

export function db(): Promise<IDBDatabase> {
  // Asked once, answered the same way for ever: no retry, so there is no loop of failed opens and notices.
  if (newer) return Promise.reject(newer);
  if (stale) return Promise.reject(new Error(t("Docxtor was updated in another tab, so this page can't keep anything in the browser. Download a copy of your document if it has changes, then reload this page.")));
  conn ??= new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(NAME, VERSION);
    // An older page that never lets go of its connection holds the upgrade up: say so, and carry on once it is closed.
    req.onblocked = () => dbNotices.waiting?.();
    req.onupgradeneeded = () => {
      const d = req.result;
      const has = (n: string) => d.objectStoreNames.contains(n);
      if (!has("library")) d.createObjectStore("library", { keyPath: "path" });
      if (!has("handles")) d.createObjectStore("handles");
      if (!has("recent")) d.createObjectStore("recent", { keyPath: "key" });
      if (!has("versions")) {
        const v = d.createObjectStore("versions", { keyPath: "id", autoIncrement: true });
        v.createIndex("doc", "doc");
      }
      if (!has("blobs")) d.createObjectStore("blobs");
      if (!has("trash")) d.createObjectStore("trash", { keyPath: "id", autoIncrement: true });
      if (!has("recovery")) d.createObjectStore("recovery", { keyPath: "key" });
      if (!has("templates")) d.createObjectStore("templates", { keyPath: "id", autoIncrement: true });
    };
    req.onsuccess = () => {
      const d = req.result;
      // A newer page wants to upgrade: let go so it can, and ask this page to be reloaded.
      d.onversionchange = () => {
        d.close();
        conn = null;
        stale = true;
        dbNotices.outdated?.();
      };
      resolve(d);
    };
    req.onerror = (event) => {
      // Opening at an older version than the stored one is refused. Stop the error here: it is not a failure of the page.
      if (req.error?.name === "VersionError") {
        event.preventDefault();
        reject((newer = new NewerDataError()));
        dbNotices.newer?.();
        return;
      }
      reject(req.error);
    };
  }).catch((e) => {
    // A failed open is tried again by the next call, except one that is final (a newer database).
    if (e !== newer) conn = null;
    throw e;
  });
  return conn;
}

function wrap<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function get<T>(store: string, key: IDBValidKey): Promise<T | undefined> {
  const d = await db();
  return wrap(d.transaction(store, "readonly").objectStore(store).get(key)) as Promise<T | undefined>;
}

export async function getAll<T>(store: string): Promise<T[]> {
  const d = await db();
  return wrap(d.transaction(store, "readonly").objectStore(store).getAll()) as Promise<T[]>;
}

export async function getAllByIndex<T>(store: string, index: string, key: IDBValidKey): Promise<T[]> {
  const d = await db();
  return wrap(d.transaction(store, "readonly").objectStore(store).index(index).getAll(key)) as Promise<T[]>;
}

/**
 * A write is done when its transaction is: a full disk (`QuotaExceededError`) shows when the browser commits, after the request itself
 * succeeded, so a save that waited only for the request could say "saved" for a document that was never kept.
 */
function committed<T>(tx: IDBTransaction, req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve(req.result);
    tx.onabort = () => reject(tx.error ?? req.error ?? new DOMException("The write was not saved", "AbortError"));
    tx.onerror = () => reject(tx.error ?? req.error);
  });
}

export async function put(store: string, value: unknown, key?: IDBValidKey): Promise<IDBValidKey> {
  const d = await db();
  const tx = d.transaction(store, "readwrite");
  return committed(tx, tx.objectStore(store).put(value, key));
}

export async function del(store: string, key: IDBValidKey): Promise<void> {
  const d = await db();
  const tx = d.transaction(store, "readwrite");
  await committed(tx, tx.objectStore(store).delete(key));
}

/** The browser said there is no room left for this site's storage. */
export function isStorageFull(e: unknown): boolean {
  const name = (e as { name?: string } | null)?.name;
  return name === "QuotaExceededError" || name === "NS_ERROR_DOM_QUOTA_REACHED" || /quota/i.test(String((e as Error | null)?.message ?? ""));
}

let askedToKeep = false;

/**
 * Ask the browser to keep this site's storage when the disk runs low, so the Library is not cleared to make room (Chrome decides by itself,
 * without asking the person, from how the site is used). Once a session, after the first document is kept in the Library.
 */
export async function keepStorage(): Promise<void> {
  if (askedToKeep || typeof navigator === "undefined" || !navigator.storage?.persist) return;
  askedToKeep = true;
  try {
    if (!(await navigator.storage.persisted())) await navigator.storage.persist();
  } catch {
    // A browser that refuses keeps the storage as it was.
  }
}
