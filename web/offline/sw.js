// Docxtor's service worker. It keeps the app (its pages, scripts, spelling dictionaries and fonts) in the browser, so
// Docxtor opens and works without a connection. The build fills in the version and the list of files below (see vite.config.ts).
// Other servers (the assistant's provider, Wiktionary, DOI lookups, LanguageTool) are never touched or kept.

const VERSION = "__VERSION__";
const PRECACHE = __PRECACHE__;
const CACHE = `docxtor-app-${VERSION}`;
// What is kept as it is used belongs to the version too, so an update never leaves an old copy answering for ever.
const RUNTIME = `docxtor-runtime-${VERSION}`;
// The page is kept under the directory's own address: some servers (Cloudflare Pages) send /index.html on to / with a redirect.
const SHELL = new URL("./", self.registration.scope).href;
// Docxtor owns the directory it is served from, but only its own page is the app: other addresses in the scope are not ours to answer.
const HOME = new URL("./", self.registration.scope).pathname;
const isApp = (url) => url.pathname === HOME || url.pathname === `${HOME}index.html`;
// The documentation site is served from /docs/ on the same address. It is not the app: its requests are never answered or kept from here.
const DOCS = `${HOME}docs`;
const isDocs = (url) => url.pathname === DOCS || url.pathname.startsWith(`${DOCS}/`);

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE);
      // All or nothing: a half-kept app is worse than the one that was there before.
      await cache.addAll(PRECACHE.map((path) => new Request(new URL(path, self.registration.scope).href, { cache: "reload" })));
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      // Pages that are still open on the last version may ask for one more of its files: keep the version before this one.
      // Versions sort by their name (the time comes first). The runtime copies go with the version they were made for.
      const keys = await caches.keys();
      for (const prefix of ["docxtor-app-", "docxtor-runtime-"]) {
        const mine = keys.filter((name) => name.startsWith(prefix)).sort();
        for (const name of mine.slice(0, -2)) await caches.delete(name);
      }
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin || url.pathname.endsWith("/sw.js") || isDocs(url)) return;
  event.respondWith(answer(event, request).catch((error) => new Response(`Docxtor's service worker could not answer: ${error}`, { status: 500, headers: { "content-type": "text/plain; charset=utf-8" } })));
});

/** A page load cannot take a copy that came through a redirect (the browser fails it): the same body and headers, without the redirect. */
function direct(response) {
  return response.redirected ? new Response(response.body, { status: response.status, statusText: response.statusText, headers: response.headers }) : response;
}

/** The kept copy of a request: this version's first, then any other version's (an open page may still ask for it). */
async function kept(request, options) {
  const own = await caches.open(CACHE);
  // The files are the same whatever headers the asking page sent (a script asks with Origin, and the server says Vary: Origin).
  const how = { ...options, ignoreVary: true };
  return (await own.match(request, how)) ?? (await caches.match(request, how));
}

async function answer(event, request) {
  // The app is one page: every address of it (a document is a #hash) opens the same file, from the kept copy when there is one.
  // Any other address on this server (a privacy page, a file, a 404) gets what the server says, or its kept file when it has one.
  if (request.mode === "navigate" && isApp(new URL(request.url))) {
    const shell = await kept(SHELL);
    if (shell) return direct(shell);
    try {
      return await fetch(request);
    } catch {
      return new Response("Docxtor isn't kept on this device yet. Open it once while you are online.", { status: 503, headers: { "content-type": "text/plain; charset=utf-8" } });
    }
  }
  const copy = await kept(request, { ignoreSearch: true });
  if (copy) return request.mode === "navigate" ? direct(copy) : copy;
  try {
    const response = await fetch(request);
    // What the build did not list (a font for another script) is kept as it is used. Other pages are not Docxtor's to keep.
    if (response.ok && response.type === "basic" && request.mode !== "navigate") {
      const clone = response.clone();
      event.waitUntil(caches.open(RUNTIME).then((cache) => cache.put(request, clone)));
    }
    return response;
  } catch {
    return Response.error();
  }
}
