// What nothing else caught. A failure at start shows a plain page that says what happened instead of a blank one; a failure later
// says once a minute at most that something went wrong, and that the work is kept (the Library saves a moment after typing stops, and
// every document gets a recovery copy every few seconds, offered back next time). Docxtor sends no error reports anywhere.
import { onEngineCrash } from "@docxtor/core";
import { h, modal, t, toast } from "@docxtor/editor";
import { NewerDataError } from "./files/db";

/** Not failures: Chrome's notice about a resize loop, an error from another site's script, and requests the network did not answer. */
const HARMLESS =
  /ResizeObserver loop|^Script error\.?$|Failed to fetch|NetworkError|Load failed|^AbortError|^TimeoutError|The user aborted/;
const MIN_GAP = 60_000;
let shown = 0;

/** Errors that have a page of their own (the engine stopped, the data is from a newer Docxtor): a second notice on top would only confuse. */
const HAS_ITS_OWN_PAGE = /^(EngineCrashedError|NewerDataError)\b/;

function say(message: string) {
  if (HARMLESS.test(message) || HAS_ITS_OWN_PAGE.test(message)) return;
  const now = Date.now();
  if (now - shown < MIN_GAP) return;
  shown = now;
  toast(
    t(
      "Something went wrong in Docxtor. Your work is kept: Docxtor keeps a recovery copy of your changes every few seconds. If something looks wrong, reload the page.",
    ),
    15000,
    { label: t("Reload"), run: () => location.reload() },
  );
}

/** From now on, an error nothing caught is said in plain words (the console still has the details). */
export function watchForErrors() {
  window.addEventListener("error", (e) =>
    say(
      String(
        e.error?.name && e.error.name !== "Error"
          ? `${e.error.name}: ${e.message}`
          : (e.message ?? ""),
      ),
    ),
  );
  window.addEventListener("unhandledrejection", (e) => {
    const r = e.reason as { name?: string; message?: string } | undefined;
    say(
      r?.name && r.name !== "Error"
        ? `${r.name}: ${r.message ?? ""}`
        : String(r?.message ?? r ?? ""),
    );
  });
}

let saidInsecure = false;

/**
 * Said once per page load when the app is served from an address the browser does not call secure (plain http on a network address, say
 * a copy of Docxtor on a home server). `localhost` and https count as secure. The browser then turns off the clipboard, the offline copy
 * (the service worker), the request to keep storage, the file pickers and `crypto.subtle` (version history then
 * hashes without it, see files/versions.ts), and Docxtor can only say so: it still opens and edits.
 */
export function warnIfNotSecure() {
  if (saidInsecure || window.isSecureContext) return;
  saidInsecure = true;
  toast(
    t(
      "This page isn't served over HTTPS, so your browser turns some things off here: working offline, the clipboard buttons, opening files in place, and keeping your documents when the disk runs low. Docxtor still opens and edits documents. Serve it over HTTPS, or from localhost, to switch them on.",
    ),
    40000,
  );
}

/**
 * The engine stopped after an internal error (a Rust panic ends in a WebAssembly trap, see packages/core/src/crash.ts). From then on the
 * engine refuses every call, so nothing it could hand back is ever saved over a good copy: the Library's copy and the recovery copy stay as
 * they were. This says so in plain words, and offers the reload that goes on from the last saved copy.
 */
export function watchForEngineCrash() {
  // A timer or a key that finds the stopped engine throws its error with nothing to catch it: the notice below is the answer, so the
  // browser is told it was handled (and does not print a second, uglier one).
  const stopped = (e: unknown) => (e as { code?: string } | null)?.code === "engine_crashed";
  window.addEventListener("error", (e) => stopped(e.error) && e.preventDefault());
  window.addEventListener("unhandledrejection", (e) => stopped(e.reason) && e.preventDefault());
  let shown = false;
  onEngineCrash(() => {
    if (shown) return;
    shown = true;
    document.documentElement.dataset.engineCrashed = "true";
    showCrashNotice();
  });
}

/** The dialog that says the document hit a problem. Called again when the person closes it and tries to save. */
export function showCrashNotice() {
  if (document.querySelector(".crash-notice")) return; // one is enough, however many saves are refused
  const reload = h("button", { class: "btn primary", type: "button" }, t("Reload"));
  reload.addEventListener("click", () => location.reload());
  const dialog = h(
    "div",
    { class: "dialog crash-notice", role: "alertdialog", "aria-modal": "true", "aria-label": t("The document hit a problem"), style: "width:min(var(--dialog-s),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("The document hit a problem"))),
    h(
      "div",
      { class: "dialog-body" },
      h("p", {}, t("Docxtor stopped working on this document, so it can't save any more from this page. The last saved copy is safe and was not changed.")),
      h("p", {}, t("Reload the page to keep working. Changes made since the last save may need to be redone.")),
    ),
    h("div", { class: "dialog-actions" }, reload),
  );
  modal(dialog);
  reload.focus();
}

/** The browser's data is from a newer Docxtor than this page: a plain page that says so and offers the reload that brings it. */
export function dataFromNewerVersion(root: HTMLElement) {
  console.warn("Docxtor: the data in this browser was saved by a newer version; this page leaves it alone.");
  root.replaceChildren(
    h(
      "div",
      { class: "docxtor start-failed-page" },
      h(
        "div",
        { class: "start-failed newer-data", role: "alert" },
        h("h1", {}, t("Your documents were saved by a newer Docxtor")),
        h("p", {}, t("This page is an older version of Docxtor and can't read them yet. Nothing was changed or deleted: your documents are still here.")),
        h("p", {}, t("Reload the page to get the newer version. If this page comes back, the site may be serving the older version again: your documents wait here until a newer version opens them.")),
        h("button", { class: "btn primary", type: "button", onclick: () => location.reload() }, t("Reload")),
      ),
    ),
  );
}

/** Docxtor could not start: a page that says so, with what the browser said and what to try. */
export function startFailed(root: HTMLElement, err: unknown) {
  if (err instanceof NewerDataError) return dataFromNewerVersion(root);
  console.error("Docxtor could not start:", err);
  const reason = String((err as Error)?.message ?? err);
  root.replaceChildren(
    h(
      "div",
      { class: "docxtor start-failed-page" },
      h(
        "div",
        { class: "start-failed", role: "alert" },
        h("h1", {}, t("Docxtor couldn't start")),
        h(
          "p",
          {},
          t(
            "Your documents are safe: they stay in this browser until Docxtor opens again.",
          ),
        ),
        h(
          "p",
          {},
          t(
            "Reload the page. If it happens again, check that this is a recent Chrome or Edge, and that the site's data isn't blocked in the browser's settings.",
          ),
        ),
        h(
          "p",
          { class: "start-failed-reason" },
          t("What the browser said: {reason}", { reason }),
        ),
        h(
          "button",
          {
            class: "btn primary",
            type: "button",
            onclick: () => location.reload(),
          },
          t("Reload"),
        ),
      ),
    ),
  );
}

/**
 * Docxtor is built and tested for Chrome and Edge. Another browser, or a Chrome too old for what Docxtor uses, gets a word once a session; it
 * can carry on. Without WebAssembly or browser storage Docxtor cannot run at all, and says so.
 */
export function checkBrowser(): string | null {
  if (typeof WebAssembly !== "object" || typeof indexedDB === "undefined")
    return t(
      "This browser can't run Docxtor: it needs WebAssembly and browser storage. Open Docxtor in a recent Chrome or Edge.",
    );
  const brands =
    (
      navigator as Navigator & {
        userAgentData?: { brands?: { brand: string; version: string }[] };
      }
    ).userAgentData?.brands ?? [];
  const chromium = brands.find((b) => b.brand === "Chromium");
  const tooOld = chromium && Number(chromium.version) < 120;
  if (chromium && !tooOld) return null;
  try {
    if (sessionStorage.getItem("docxtor-browser-warned")) return null;
    sessionStorage.setItem("docxtor-browser-warned", "1");
  } catch {
    // Without session storage the word comes on every visit.
  }
  toast(
    tooOld
      ? t(
          "This version of Chrome is old for Docxtor: some things may not work. Update Chrome to use everything.",
        )
      : t(
          "Docxtor is made for Chrome and Edge for now. Some things may not work in this browser.",
        ),
    12000,
  );
  return null;
}
