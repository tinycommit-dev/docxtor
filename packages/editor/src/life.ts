// One editor's lifetime: every global listener, observer and overlay it creates is released by dispose(),
// so an editor can be mounted and destroyed repeatedly, or several can share a page.
export class Lifetime {
  private ac = new AbortController();
  readonly signal = this.ac.signal;

  /** addEventListener that is removed when the lifetime ends. */
  on(target: EventTarget, type: string, fn: EventListenerOrEventListenerObject, opts: AddEventListenerOptions = {}) {
    target.addEventListener(type, fn, { ...opts, signal: this.signal });
  }

  /** Remove `el` from the page when the lifetime ends. */
  own<T extends Element>(el: T): T {
    this.signal.addEventListener("abort", () => el.remove(), { once: true });
    return el;
  }

  /** Run `fn` when the lifetime ends. Returns a function that takes it back, for work that can end sooner (a panel that closes). */
  defer(fn: () => void): () => void {
    this.signal.addEventListener("abort", fn, { once: true });
    return () => this.signal.removeEventListener("abort", fn);
  }

  get disposed() {
    return this.signal.aborted;
  }

  dispose() {
    this.ac.abort();
  }
}
