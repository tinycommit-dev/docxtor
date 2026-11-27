// A time limit for every request to another server (a dictionary, a thesaurus, a DOI, Zotero, a picture at a web address, a font):
// a network that stalls ends in the request's own message ("Couldn't reach …"), never in a wait that does not end. One place, so
// every request has one and the limits can be read together.

/**
 * A signal for `fetch` that aborts after `ms` milliseconds (a `TimeoutError`) or when `signal` does (an `AbortError`, as when a newer
 * request replaces this one). Code that tells the two apart keeps working: a time-out is not an `AbortError`.
 */
export function timeLimit(ms: number, signal?: AbortSignal | null): AbortSignal {
  const timeout = AbortSignal.timeout(ms);
  if (!signal) return timeout;
  // AbortSignal.any is new (Firefox 124, Safari 17.4): older browsers join the two by hand.
  if (typeof AbortSignal.any === "function") return AbortSignal.any([signal, timeout]);
  const both = new AbortController();
  const stop = (from: AbortSignal) => () => both.abort(from.reason);
  if (signal.aborted) both.abort(signal.reason);
  else signal.addEventListener("abort", stop(signal), { once: true });
  timeout.addEventListener("abort", stop(timeout), { once: true });
  return both.signal;
}
