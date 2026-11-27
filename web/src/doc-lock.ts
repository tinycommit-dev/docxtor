// One document open in two browser windows (View, New window, or the same file opened twice): the window that opened it
// first edits it, and the later one is read-only, so neither overwrites the other's changes without a word. Windows ask
// each other over a BroadcastChannel; a browser without one lets every window edit, as before.

const channel = typeof BroadcastChannel !== "undefined" ? new BroadcastChannel("docxtor-open-documents") : null;
/** The document this window edits (it answers for it), if any. */
let editing: string | null = null;

channel?.addEventListener("message", (e: MessageEvent<{ type: string; key: string }>) => {
  if (e.data?.type === "who" && editing && e.data.key === editing) channel.postMessage({ type: "here", key: editing });
});

/**
 * Say this window opened a document. Resolves true when no other window edits it (this one does now), false when one
 * does (this one should be read-only).
 */
export function claim(key: string, wait = 200): Promise<boolean> {
  editing = null;
  if (!channel) {
    editing = key;
    return Promise.resolve(true);
  }
  return new Promise((resolve) => {
    let taken = false;
    const hear = (e: MessageEvent<{ type: string; key: string }>) => {
      if (e.data?.type === "here" && e.data.key === key) taken = true;
    };
    channel.addEventListener("message", hear);
    channel.postMessage({ type: "who", key });
    window.setTimeout(() => {
      channel.removeEventListener("message", hear);
      if (!taken) editing = key;
      resolve(!taken);
    }, wait);
  });
}

/** This window no longer has the document open. */
export function release() {
  editing = null;
}
