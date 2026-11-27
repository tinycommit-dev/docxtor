// Why a document can't be changed right now. Read-only is not a flag that features set and put back: each feature holds
// a reason and lets go of it, and the document is read-only while any reason is held. Nobody saves or restores anybody
// else's value, so modes can end in any order and none leaves the text stuck.
//
// A reason has a scope. "document" reasons belong to the open document (the window lock, a document marked as final, a
// mail merge preview) and end when another document is loaded. "host" reasons belong to the page or the view (the host's
// `readOnly` option, the Reading view, Line focus, the page grid) and stay.

export type LockScope = "document" | "host";

export class LockSet {
  private held = new Map<symbol, { reason: string; scope: LockScope }>();

  constructor(private changed: () => void) {}

  /** Hold a reason; the returned function lets go of this hold only (calling it twice does nothing). */
  hold(reason: string, scope: LockScope = "host"): () => void {
    const key = Symbol(reason);
    this.held.set(key, { reason, scope });
    this.changed();
    return () => {
      if (this.held.delete(key)) this.changed();
    };
  }

  /** Let go of the reasons that belong to the document that was open. */
  dropDocument() {
    let any = false;
    for (const [key, v] of this.held) {
      if (v.scope === "document") {
        this.held.delete(key);
        any = true;
      }
    }
    if (any) this.changed();
  }

  get size(): number {
    return this.held.size;
  }

  has(reason: string): boolean {
    for (const v of this.held.values()) if (v.reason === reason) return true;
    return false;
  }

  reasons(): string[] {
    return [...new Set([...this.held.values()].map((v) => v.reason))];
  }
}
