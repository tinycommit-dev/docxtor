// One save queue for one document: the single home of "is this document saved", for the open document and for the
// side-by-side pane. No DOM, so it runs in Node tests.
//
// - One write at a time. Asking to save while a write runs queues one more save; however many ask, at most one waits,
//   and it takes the document as it is when its turn comes (newest wins).
// - "Saved" means the revision whose bytes were written is the revision the document has now. An edit made while a
//   write runs leaves the document unsaved when the write ends, and nothing here forgets it.
// - The state (saving, saved, unsaved, failed) is reported through `onState`.

export type SaveStatus = "saved" | "unsaved" | "saving" | "failed";

/** What a save takes from the document, at one moment: its revision and whatever the write needs (the bytes, where to). */
export interface Snapshot {
  revision: number;
}

export interface SaveResult<T extends Snapshot> {
  /** The write finished (or, for a request that found nothing to save, there was nothing to save). */
  ok: boolean;
  /** What was written; absent when there was nothing to write. */
  snapshot?: T;
  error?: Error;
}

export interface SaveQueueOptions<T extends Snapshot> {
  /** The document's revision now (the engine's `revision`). Not called after `close()`. */
  revision(): number;
  /**
   * Take the document as it is now. The bytes and the revision belong together: the revision is the document's when taking
   * began (a take that goes on in steps, to keep the page free for typing, says the revision it began at, so an edit made
   * meanwhile leaves the document unsaved).
   */
  take(): T | Promise<T>;
  /** Write what was taken. Rejects when it can't. */
  write(snapshot: T): Promise<void>;
  /** The state changed. `error` comes with "failed". */
  onState?(status: SaveStatus, error?: Error): void;
}

interface Waiter<T extends Snapshot> {
  force: boolean;
  promise: Promise<SaveResult<T>>;
  resolve(r: SaveResult<T>): void;
}

export class SaveQueue<T extends Snapshot> {
  private saved: number;
  private running = false;
  private waiting: Waiter<T> | null = null;
  private lastError: Error | null = null;
  private closed = false;
  private idleWaiters: (() => void)[] = [];
  private opts: SaveQueueOptions<T>;

  /** `savedRevision`: the revision the document had when it was opened (it is saved as it stands) or -1 (it is not). */
  constructor(opts: SaveQueueOptions<T>, savedRevision: number) {
    this.opts = opts;
    this.saved = savedRevision;
  }

  /** The revision that was last written. */
  get savedRevision(): number {
    return this.saved;
  }

  /** The document has changes that no finished write holds. True while a write that holds the newest ones is still running. */
  get dirty(): boolean {
    if (this.closed) return false;
    try {
      return this.opts.revision() !== this.saved;
    } catch {
      return false; // a document that is gone has nothing to save
    }
  }

  /** A write is running. */
  get writing(): boolean {
    return this.running;
  }

  /** The last write failed and no write has worked since. */
  get failed(): boolean {
    return this.lastError !== null;
  }

  get status(): SaveStatus {
    if (this.running) return "saving";
    if (this.lastError) return "failed";
    return this.dirty ? "unsaved" : "saved";
  }

  /** The document changed: say "unsaved" unless a write is running (it says so itself when it ends). */
  touch() {
    if (this.closed || this.running) return;
    this.emit();
  }

  /** The document counts as unsaved whatever its revision (a new engine that has to be written, a file that was deleted). */
  markUnsaved() {
    this.saved = -1;
    this.touch();
  }

  /** The document was saved some other way (Save as): `revision` is what that wrote. */
  markSaved(revision: number) {
    this.saved = revision;
    this.lastError = null;
    if (!this.running) this.emit();
  }

  /**
   * Save the document. Resolves when a write that holds what the document has now has finished. Without `force`, a document
   * that is saved already is not written again (`ok` is true, with no snapshot). Never rejects.
   */
  save(force = false): Promise<SaveResult<T>> {
    if (this.closed) return Promise.resolve({ ok: false });
    if (this.running) {
      if (!this.waiting) {
        let resolve!: (r: SaveResult<T>) => void;
        const promise = new Promise<SaveResult<T>>((r) => (resolve = r));
        this.waiting = { force, promise, resolve };
      }
      this.waiting.force ||= force;
      return this.waiting.promise;
    }
    if (!force && !this.dirty) return Promise.resolve({ ok: true });
    let resolve!: (r: SaveResult<T>) => void;
    const promise = new Promise<SaveResult<T>>((r) => (resolve = r));
    this.running = true;
    void this.run({ force, promise, resolve });
    return promise;
  }

  /** Resolves when no write is running or waiting. */
  idle(): Promise<void> {
    return this.running ? new Promise((r) => this.idleWaiters.push(r)) : Promise.resolve();
  }

  /**
   * Stop for good: the document is being replaced or closed. A write that is running finishes, silently; one that waits
   * is dropped, so callers save what they need before closing.
   */
  close() {
    this.closed = true;
    this.waiting?.resolve({ ok: false });
    this.waiting = null;
  }

  private emit() {
    this.opts.onState?.(this.status, this.lastError ?? undefined);
  }

  private async run(first: Waiter<T>) {
    let current: Waiter<T> | null = first;
    while (current) {
      if (this.closed) {
        current.resolve({ ok: false });
        break;
      }
      let result: SaveResult<T>;
      let snap: T | undefined;
      try {
        const taken = this.opts.take();
        // A take that is not a promise is not waited for: the write begins in the same turn, as it always did.
        snap = taken instanceof Promise ? await taken : taken;
        this.opts.onState?.("saving");
        await this.opts.write(snap);
        this.saved = snap.revision;
        this.lastError = null;
        result = { ok: true, snapshot: snap };
      } catch (e) {
        this.lastError = e instanceof Error ? e : new Error(String(e));
        result = { ok: false, snapshot: snap, error: this.lastError };
      }
      let next: Waiter<T> | null = this.waiting;
      this.waiting = null;
      // What waited was asked for before this write ended; if this write holds everything and nobody insisted, it is done.
      if (next && result.ok && !next.force && !this.dirty) {
        next.resolve(result);
        next = null;
      }
      this.running = next !== null;
      // Between two writes nothing is said: the next one says "saving" itself.
      if (!next && !this.closed) this.emit();
      current.resolve(result);
      current = next;
    }
    this.running = false;
    const waiters = this.idleWaiters;
    this.idleWaiters = [];
    for (const w of waiters) w();
  }
}
