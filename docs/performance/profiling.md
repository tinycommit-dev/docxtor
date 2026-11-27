# Profiling and comparing

Finding where time goes, comparing two builds and checking a change to the paginator. Part of the [performance](index.md) pages.

## Where a key's time goes: `perf/profile-keys.mts`

*Sketch, not run by a test: it needs a production build and the cached benchmark documents.*

```bash
node --experimental-strip-types -e 'import("./perf/lib/build.mts").then(m => m.viteBuild("/tmp/docxtor-build"))'   # after npm run wasm
BUILD=/tmp/docxtor-build node --experimental-strip-types perf/profile-keys.mts [doc] [keys] [--layouts]
CARET=0.95 BUILD=...  node --experimental-strip-types perf/profile-keys.mts      # caret at 95% of the document (default 50%)
```

Opens a document from `~/.cache/docxtor-perf/docs` (War and Peace by default; the file names are the ones the harness made), puts the caret in the middle, types `keys` letters (30 by default) one frame each and prints Chrome's counters per key (script, layout and style time, layouts and style recalculations), the hottest functions by self and inclusive time, and **who calls the browser's own expensive functions** (`querySelectorAll`, `getBoundingClientRect`, `setBaseAndExtent`...), which is what finds a lookup that walks the page. For readable names build without minifying (`vite build --minify false` in `web/`). The engine's own functions have names only in an engine built with `DOCXTOR_WASM_NAMES=1 npm run wasm`: the build removes the WebAssembly name section, 1.4 MB of the engine that only stack traces and profiles read (`scripts/strip-wasm-names.mjs`).

With `--layouts` it records a trace as well and lists every layout and style recalculation in the key's frame with the JavaScript stack that **forced** it (the stack is where the browser was asked for a measurement while the page was dirty), and every write that **invalidated** style or layout, by stack. In that mode the editor's 120 ms pagination timer is held back until the keys are done (tracing makes each key slow enough for the timer to fire between keys, which paginates and redraws everything, and that is not what a person typing at speed gets). The rule it checks: do the key's DOM writes, then one layout (setting the caret needs it), then all the reads; a read after a write that came after the layout is a second layout.

What the War and Peace profile shows (30 keys in the middle, production build, one frame each; the harness's own numbers are in the table of `perf/report.md`): own work of about 24 ms per key with the profiler on (23 ms in the harness), 1.9 layouts and 1.9 style recalculations per key. The two layouts are the browser's own: one forced by setting the caret after the text changed (6.5 ms, because the flow is one very long block) and one that lays out the underlines drawn after it. What is left in the key is Chrome's, not the editor's: about 16 ms per key inside `setBaseAndExtent` (that forced layout, and about 10 ms in which the browser tells its input method about the new selection, `Editor::SyncSelection`, which it appears to do only while an input event is being handled and which grows with the size of the one editable flow), and about 3.5 ms of the editor's and the engine's own.

Where a key's time goes wrong:

- **A write between two reads is a second layout.** Measuring a paragraph, writing its underlines, then measuring the next paragraph costs one layout per paragraph. The spelling layer measures everything first, then writes.
- **A search of the page is a cost that grows with the document.** `querySelector('[data-p="…"]')`, `querySelectorAll('.cell-sel')`, `[...json.values()]` to find one paragraph's view, `Array.from(flow.children)`: each is cheap alone and costs a lot when it runs a dozen times a key. Paragraph elements are found by id from a map (`paraIn` in `editor/selection.ts`, told about each block the editor draws), the view of any block from `Editor.views`, and what a tool marked is taken back by what it remembers.
- **Live Ranges make every removal slow.** The browser walks every Range that has not been garbage collected each time a node leaves the page. The pagination made one `document.createRange()` per paragraph (9,000 in a pass), and taking 77 underline boxes off the page cost 17 ms until the collector ran (0.2 ms after it). `layout.ts` keeps one scratch Range per place that measures, not one per paragraph or per word.
- **A long task between keys starts the pagination.** The 120 ms pause that waits for the last key is wall clock: a stall of 120 ms between two keys (a harness that waits for a frame between keys, a long garbage collection) paginates the novel on the spot (2.8 s). In the harness it shows as a spike in own work and in layouts per key.
- **Setting the caret in the next frame**, not in the key's own task, would take the 10 ms of `SyncSelection` out of the key (own work 12 ms, total main-thread time per key 38 ms instead of 47 in one test), but the page would then have a stale selection for a frame, and every reader of the selection (events, tools, tests that read it straight after a key) would have to flush it first. The editor sets the caret in the key's own task.

## Comparing builds: A/B

`perf/ab.mts` builds two variants and runs them interleaved (A, B, A, B...) with the median and range of each. `node --experimental-strip-types perf/ab.mts wasm-opt` compares the engine with and without `wasm-opt`; `... coalesce` compares the menu bar and toolbar drawing once at start against every change. Results go to `perf/ab/`.

## Checking a change to the paginator

A faster paginator must put the page breaks where the old one did. `perf/pagination-dump.mts` opens documents on a production build and records the page count, every spacer the paginator inserted (the paragraph, the text before it, its height), every block's top margin and the layouts Chrome ran:

*Sketch, not run by a test: it runs the browser on a built app and on documents you choose.*

```bash
node --experimental-strip-types perf/pagination-dump.mts <build before> before.json --corpus 100 fixtures/*.docx --novel
node --experimental-strip-types perf/pagination-dump.mts <build after> after.json --corpus 100 fixtures/*.docx --novel
```

Compare the page counts and the spacer positions. Heights can differ by Chrome's layout unit (1/64 px); a break that moves needs explaining. Where the two disagree, measure which is right: on a page that starts with body text, the first line should sit at the top margin.

## Comparing with Google Docs

There are no published typing numbers for Google Docs, so the harness ships a script you can paste into the DevTools console of any editor tab, **Docxtor or Google Docs**, which measures the keys you type the same way on both: `perf/console-typing.js`.

1. Open the document, open DevTools (Cmd+Option+J on a Mac), paste the whole file into the Console, press Enter.
2. Click into the document and type about 50 characters at your normal pace (the same text in both editors, in a document of the same length).
3. Run `typingReport()`. It prints a table per frame: input delay, processing per key, and the Event Timing figures. `typingStop()` removes it.

**Google Docs takes typing in a hidden iframe** (`docs-texteventtarget-iframe`), and an event fires in the frame that receives it, so a listener on the top window never sees a key. The script installs itself in the page and in every same-origin iframe (now, and as new ones appear, checked every second), each with that frame's own `PerformanceObserver` and its own listeners. A cross-origin frame cannot be reached and is reported as skipped. It prints which frames it watched and how many events each saw: if the iframe shows no keys, the numbers are not comparable. `perf/check-console-script.mts` tests it on Docxtor; it has not been run against Google Docs, which needs a Google account. If Docs changes how it takes input, check the "Frames watched" line first.

What the numbers mean: **processing** is how long the page's own code held the main thread for one key (keydown, beforeinput and input together); **input delay** is how long the key waited before any of it ran. Neither includes the time to draw the character, so a low number says the editor is light, not that the screen updates sooner. Event Timing alone only reports events over 16 ms (and rounds to 8 ms), so on a fast editor its table is nearly empty; that is why the exact columns are there too.
