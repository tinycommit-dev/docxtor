// What happens when the engine traps.
//
// The engine is Rust compiled to WebAssembly. If Rust panics, the instance traps (`WebAssembly.RuntimeError: unreachable`) and
// stops where it was: its memory may be half changed, and every later call into it is unsafe. Nothing it returns after that can
// be trusted, least of all bytes to save over a good copy.
//
// So the first trap marks the engine as crashed, for good. There is one WebAssembly instance for the whole page, so the mark is for
// the page: every Engine (open documents, the side-by-side pane) refuses every later call with an `EngineCrashedError`
// (`code: "engine_crashed"`). A host that saves must not write what a crashed engine gives, and it can't: the call that makes the
// bytes throws first.
//
// One place does the catching: `guardBinding` wraps the methods of the binding's classes once, when this package loads, and
// `guarded` wraps the loose functions it exports. A call that works costs a flag check and a try block, which the engine runs in
// no time; there is no try/catch around any call site.

/** The code of an `EngineCrashedError`, for hosts that tell errors apart without `instanceof`. */
export type EngineCrashedCode = "engine_crashed";

/**
 * The engine hit a fault (a Rust panic ends in a WebAssembly trap) and can no longer be trusted. Reload the page to go on; what was saved
 * before is as it was. `cause` is the error the browser threw (the console has Rust's own message, written by the panic hook).
 */
export class EngineCrashedError extends Error {
  readonly code: EngineCrashedCode = "engine_crashed";
  constructor(cause?: unknown) {
    super("The document engine stopped after an internal error, so nothing it returns can be trusted. Reload the page to go on. Your last saved copy is safe.", cause === undefined ? undefined : { cause });
    this.name = "EngineCrashedError";
  }
}

let crash: EngineCrashedError | null = null;
const listeners = new Set<(e: EngineCrashedError) => void>();

/** The crash that ended the engine, or null while it is sound. */
export function engineCrash(): EngineCrashedError | null {
  return crash;
}

/** Throws the `EngineCrashedError` when the engine has crashed. Hosts call it before they write anything an engine made. */
export function assertEngineSound(): void {
  if (crash) throw crash;
}

/** Be told when the engine crashes (at once, if it already has). Returns the function that stops the telling. */
export function onEngineCrash(fn: (e: EngineCrashedError) => void): () => void {
  listeners.add(fn);
  if (crash) fn(crash);
  return () => void listeners.delete(fn);
}

/** Whether what a call into the binding threw means the engine's memory can no longer be trusted. */
export function isTrap(e: unknown): boolean {
  if (e instanceof EngineCrashedError) return false;
  if (typeof WebAssembly !== "undefined" && e instanceof WebAssembly.RuntimeError) return true;
  // Running out of stack or memory inside the engine abandons Rust halfway, just as a trap does. (A call on a document that was freed is
  // not one: wasm-bindgen refuses it before any Rust runs, and `Engine` answers a closed document itself.)
  if (e instanceof RangeError) return /Maximum call stack size exceeded|WebAssembly\.Memory|Out of memory/i.test(e.message);
  return false;
}

/** Record the crash (once) and tell the listeners. Returns the error to throw. */
function crashed(cause: unknown): EngineCrashedError {
  if (crash) return crash;
  crash = new EngineCrashedError(cause);
  console.error("Docxtor engine crashed; saving is switched off until the page is reloaded.", cause);
  for (const fn of [...listeners]) {
    try {
      fn(crash);
    } catch {
      // A listener that fails must not hide the crash from the others.
    }
  }
  return crash;
}

/** `fn` for a call into the binding: refuses once the engine has crashed, and is the one place a trap is noticed. */
export function guarded<F extends (...args: never[]) => unknown>(fn: F): F {
  return function (this: unknown, ...args: unknown[]) {
    if (crash) throw crash;
    try {
      return (fn as unknown as (...a: unknown[]) => unknown).apply(this, args);
    } catch (e) {
      throw isTrap(e) ? crashed(e) : e;
    }
  } as unknown as F;
}

/** Run `make` (a constructor call into the binding, which has no method to wrap) with the same watch. */
export function guardedNew<T>(make: () => T): T {
  if (crash) throw crash;
  try {
    return make();
  } catch (e) {
    throw isTrap(e) ? crashed(e) : e;
  }
}

/** Wrap every method and static function of a binding class (not `free`, which lets go and calls nothing of ours). Once per class. */
export function guardBinding(cls: { prototype: object }): void {
  const done = cls as { __guarded?: boolean };
  if (done.__guarded) return;
  done.__guarded = true;
  for (const target of [cls.prototype, cls as object]) {
    for (const name of Object.getOwnPropertyNames(target)) {
      if (name === "constructor" || name === "free" || name === "prototype" || name === "length" || name === "name" || name === "__guarded") continue;
      const d = Object.getOwnPropertyDescriptor(target, name)!;
      if (typeof d.value !== "function" || !d.writable) continue;
      Object.defineProperty(target, name, { ...d, value: guarded(d.value) });
    }
  }
}
