# Performance

Docxtor measures its speed and memory with an automatic harness, and CI fails a change that makes a gate worse, as it fails a test. The cost of a key is meant to stay flat as a document grows, and the harness checks that by exact counts rather than by timing.

*Sketch, not run by a test: each command takes minutes to build and measure.*

```bash
npm run perf:quick    # engine counts, about a minute when warm
npm run perf          # adds the browser (production build) and a War and Peace sized novel
npm run perf:full     # every size, a Proust sized novel, the corpus file, longer runs
npm run perf:accept   # run the standard tier and make its numbers the new baseline (see below).
```

Each run writes `perf/report.md` (what was measured, in plain words) and adds a line to `perf/history.jsonl`. A failed gate exits non-zero.

## What is measured, and why it is split in two

**Gated: exact counts.** A count does not change with how busy the machine is, so a gate on a count never fails by accident and never lets a regression through because the machine was quiet. These are checked against `perf/baseline.json`:

| Count | Where it comes from |
|---|---|
| Allocations, bytes requested and peak extra bytes per open, keystroke, split, undo and save, at 100, 1,000 and 5,000 paragraphs (20,000 in full) | A counting allocator in `crates/docxtor-core/tests/perf_counts.rs` (the one in `tests/common/proc.rs`) |
| Paragraphs visited by whole-document walks (`walk_paragraphs` and its variants) per keystroke: flat, whatever the length of the document | `perf::paragraphs_walked` in docxtor-core (per thread), read by `perf_counts` and as `paragraphsWalked` through `perfCounters` in the browser build; the debug comparisons of `document/facts.rs` are left out of it (`perf::unmetered`) |
| Whole-document copies per keystroke, and top-level blocks copied for undo | `perf::body_clones` and `perf::block_clones` in docxtor-core (per thread), one add where the document or a block is copied; an undo snapshot copies only the blocks an edit touched |
| JSON characters crossing the engine boundary per keystroke, and calls per keystroke | `PerfMeter` in `packages/core/src/engine.ts`, on when `globalThis.__DOCXTOR_PERF__` is set before an Engine is made. In the browser this is the editor's real call sequence |
| The engine's linear memory high-water | The size of the WebAssembly memory after the run |
| Layouts, style recalculations, nodes added and listeners added per keystroke | Chrome DevTools Protocol `Performance.getMetrics` deltas over N keystrokes |
| Compressed download: JavaScript, engine, everything a first visit fetches | gzip and brotli of the files the production build served on a first visit |

**Reported: times and memory.** Milliseconds and megabytes move with the machine, so they are never a gate. They are reported as medians of repeats, and compared against another build only when both were run **interleaved in the same session** (`perf/ab.mts`). Every run records the load average and, on macOS, the memory pressure. When the machine is busy (more than half the cores), the report says so in bold and the figures are not to be used for a decision.

| Reported | How |
|---|---|
| Physical footprint of the tab: at rest, peak while opening, typing and scrolling, settled, and after a forced collection | macOS `footprint` on the Chromium renderer, sampled every 0.4 s during the action |
| The same by Chrome's own memory dump (`Tracing`, `disabled-by-default-memory-infra`), with V8, Blink, malloc and PartitionAlloc | `requestMemoryDump`; its `private_footprint_bytes` works on every platform |
| Own work per key | CDP `ScriptDuration + LayoutDuration + RecalcStyleDuration` over N keys, divided by N |
| Input delay and processing per key | Capture-phase listeners in the page (delay is the time from the event's timestamp to the first listener; processing is until a listener on the way back up) and the Event Timing API |
| Time to next paint per key | The frame after the key. Quantised to 16.7 ms, so it is secondary: up to one frame of it is waiting for the screen |
| Time to first editable | From navigation until a blank document, and until a document opened from bytes, accepts a keystroke |
| Open, save, undo, scrolling (frame times, frames over 25 ms, long tasks) | In the page, on the production build |

### Documents

All built the same way every time (`perf/lib/docs.mts`, no downloads): 100, 1,000, 5,000 and 20,000 paragraphs of ordinary prose (the **series**, for the per-key slope: typing cost should stay flat as documents grow); **War and Peace** (about 580,000 words, 9,000 paragraphs, 1,400 pages) and **Proust** (about 1,300,000 words, 20,000 paragraphs, 3,000 pages), novels with paragraphs of about 65 words, short lines of dialogue, italics and bold, a chapter heading about every 4,000 words with a page break before it, and a few footnotes; and `poi/bug65649.docx` from the corpus when it has been fetched (`npm run corpus:fetch`; untrusted data, read from `~/.cache/docxtor-corpus`, never copied into the repo).

### The browser runs

- The harness builds the **production** app itself (`vite build` into `~/.cache/docxtor-perf/work`) and serves it with a small static server. It never uses the dev server, which inflates memory and can serve stale code.
- The offline service worker is blocked so a visit is the same every time. The window is 1280 by 900. The Chromium binary and version are in every report.
- One key at a time, each given its own frame, so counts do not depend on how keys batch into frames.
- The engine is built without `wasm-opt`, as the app's own build does.

## How to read `perf/report.md`

Top to bottom: **passed or failed** and the gates that failed (baseline, now, the allowed margin); **the machine** (load, and whether times and memory can be trusted); one section per tier with the tables; **goals** (the aspirational numbers in `perf/goals.json`, reported as met or not, never gated); notes.

The tables for sizes are a series: read across to see whether the cost per key is flat. The slope is printed under the table.

## How a gate's margin is set

`perf/noise.json` holds, for every metric, the smallest and largest value the **same build** gave against itself over repeated runs (`node --experimental-strip-types perf/run.mts quick --aa 5`, and the same with `standard`). A gate allows twice that spread, and never less than 0.5% of the value. Run it again after adding a metric.

Some counts move between runs of one build, all of them for keys typed near the end of a document (`at95`): the layouts and style recalculations a key, the engine calls and the JSON. What lands among those keys depends on timing: a grammar answer, the spelling pass of a line that wraps, and the autosave that the keys typed in the middle started a second earlier (its saved file would count as JSON out of the engine). So near the end those counts are reported, not gated; in the middle (`at50`), where nothing else is pending, they are exact and gated, and the structural counts (blocks laid out, blocks left out, far-page visits) are gated at both. The engine's meter counts a file that crosses as bytes (a save, an open) apart from JSON (`outBytes`, `inBytes`).

### Planted regressions

`perf/plants.json` records that the harness catches deliberate regressions (a build with the `perf-plant` feature: `crates/docxtor-core/src/perf.rs`): megabytes allocated on open, a second copy of the body on every edit (both caught by counts), and a busy loop on every edit (caught by time, interleaved against a clean build, with the smallest loop caught). Re-run with `node --experimental-strip-types perf/plants.mts`.

## How to add a metric

1. Produce a number that is exact (a count) or a time. Put it into the `metrics` object of the tier (`collectQuick` in `perf/run.mts`, or `summarize` in `perf/lib/browser-run.mts`) under a name like `native.1000.key.allocs`; times and memory go into `reported`.
2. Add a line to the tables in the same place so the report shows it.
3. Run the tier five times with `--aa 5` so `perf/noise.json` learns its spread, then `npm run perf:accept`.
4. If the number has a target, add it to `perf/goals.json` (metric name, the most it may be, a unit).

## How to accept a new baseline

After a deliberate trade (for example a feature that costs one more allocation per key), `npm run perf:accept` runs the standard tier and writes its counts into `perf/baseline.json` (with the commit and the machine load). The file is committed with the change that caused it, and the commit message says why.

## Hard limits: `perf/budgets.json`

A baseline says "no worse than last time", and when a count improves it stays slack until someone accepts the new one. `perf/budgets.json` holds a most for some exact counts (metric name, `max`, `why`); a run that goes over one fails like a gate, whatever the baseline says. It keeps an improvement from creeping back: the browser's layouts and style recalculations per key at 1,000 and 5,000 paragraphs and in War and Peace, which the editor holds to about two (the browser's own layout for what the key changed, and one more for the underlines drawn after it). A budget changes in the same commit as the change that earns it.

## What keeps a key cheap in a long document

The gates above exist because of a few design choices in the editor and the engine. A change that undoes one of them shows up as a worse count.

**Pagination starts at the changed block.** `Paginator.layout({ incremental: true })` (asked for after `exec`, `refresh` and the typing pause) starts at the block before the first one drawn and stops at the first block it reaches in the state the last pass reached it in: same page, same place on it, same footnote room. After one key it measures 4 blocks at every document length (`pause.measured`), and it makes no whole-document pass (`pause.fullPasses`). Blocks are measured in exact layout units, not the browser's whole-pixel `offsetTop` and `offsetHeight`, because a pass that reuses what lies after the edit can equal a full pass only if a block's result does not depend on how far the text above it is shifted. Set `__DOCXTOR_PAGINATION_CHECKS__` before the app starts and every incremental layout is compared with a full one made in place; the end-to-end tests do.

**Blocks far from the view are out of the browser's layout.** Blocks on pages far from the view are `display: none` (the app's far-pages rule; `__DOCXTOR_FAR_PAGES__ = n` or `DOCXTOR_FAR=n` sets the number of pages kept either side in tests). That makes the number of blocks the browser lays out independent of the document's length: `browser.<doc>.at50.layoutBlocks` is gated at 200 in `perf/budgets.json`. A hidden block returns zeros for `getBoundingClientRect()`, `getClientRects()` and `offsetTop`, so work that runs after a key must not read the rectangles of blocks far from the view. The harness counts those reads in `pause.rectReads`, `pause.maxRectReads` and `pause.rectBlocks` (reported, not gated, because the window is bounded by timers). Word count, Go to and line numbers read the page map's per-block line counts (`Paginator.blockLines`) and run on demand, not after a key.

**A zoom is a picture, not a layout.** The pages are laid out at 100% inside `.zoom-view` and drawn at the zoom with `transform: scale(z)` (origin top left, no transform at 100%); `.zoom-fit` around it has the scaled size, so the scroll extent and the centring follow. A zoom takes no pass, the page map is the same at every zoom, and `Paginator.scale()` is the editor's own zoom, exact. A zoom still redoes the work that depends on the view: far pages near the view, the marks, spelling, other selections and fold marks. So `.zoom-view` has no `will-change` (the text would be drawn once and go blurry at another scale) and nothing inside it is `position: sticky` or `fixed` (a transform is their containing block: the ruler has a box of its own outside it, and popups live in the overlay layer), and `setZoom` does no layout work.

**Footnote room is given as the pages fill.** A full layout measures every note once at the text's width (`notesForPass`). As the pass fills a page, each line that holds a reference gives the page its note's room, counted as `noteNeeds` counts it, while the line still fits above it (`roomOnPage`). When lines move to the next page after their notes were given room (a cut, widow and orphan control, a paragraph that moves whole), the room goes with them (`takeBack`). Rounds of `layoutBody` after the pass settle what it left: a reference that the room for its own note pushes to the next page, and room that chases itself (room for a note pushes its reference on, which leaves the page needing none, and with none the reference comes back). A cycle like that is settled by a floor under the room of each page in the cycle, and the rounds go on from it, up to twelve times without a new floored page. The reference stays on the next page with its note, as in Word, and the page it left is filled up to the line. A page that holds room it does not use is why `reserve` can be larger than `noteNeeds()`; `notesSettled` is true when the room equals the larger of the two. Keeping the floor between layouts is not done: the answer would then depend on where the rounds started, and random edit sequences fail the check when it is kept. `Paginator.notesInPass = false` turns the first pass's room off, for the tests of the rounds. Where the pass looks ahead before it knows a page's room (a heading kept with the next block, endnotes under a section's last block, a floating table, columns, a break inside a paragraph), it records how much it had to spare, and a page that ends under 1 px has its layout made again by the rounds' first pass. `ROOM_EPS` (1/128 px) is the difference that makes a page count as changed in the passes; the rounds call two rooms the same under a pixel. Far down a long text the browser reports positions to a float's precision (1/32 px from 262,144 px down), coarser than its layout unit, so when a retry puts its first block back where it was, the margin is searched between the two sides until the block stands where it did.

**Counting and saving are cached per block.** The status bar's word count is `Document::word_count`, kept per top-level block (`WordFacts` in `document/facts.rs`) and counted again only for the blocks an op marked; in tests and debug builds every answer is compared with a walk. `Document::stamp` takes the statistics of the main text from the same cache (`Document::main_counts`). `Document::save_packed` writes `document.xml` through `BodyXml` (`document/body_xml.rs`): it remembers, for each top-level block, where its text is in the `document.xml` the last save wrote, writes the blocks `Changed::write` names again and copies the others byte for byte. Any place that replaces the body whole drops the cache (a mail merge preview). Gated: `node.<doc>.<where>.wordCountWalked`, `stampWalked` and `blocksWritten`, each 1 after a key typed in the middle or near the end. `stampMs` and `savePackedMs` are printed with the times, not gated.

**The Library autosaves in steps.** The autosave runs a second after the last edit. It goes in steps (`Engine.saveInSteps`: `stamp`, a pause that lets keys through and waits while input is pending, `savePacked`), compresses in a worker (`packages/core/src/zip-worker.ts`, which runs the same engine file; `zipPacked` writes byte for byte what `save()` writes), and renders only the first blocks for the recent-files preview (`docscriptHead`). `Session.autosaveStall` (browser tier, `autosave: true` in a scenario) types a key, types again while the autosave runs, and reports the longest stall of the main thread and the wait of the second key; both are budgeted in `perf/budgets.json`. `stamp` and `savePacked` are whole-document walks, so they still grow with the book.

**Sheets and the pause.** What runs 120 ms after the last key is small: a pass over the changed blocks, `noteNeeds`, `setFar`, the spelling layer, the sheets and the fold marks. Sheets are made again only when what they are made of differs (`Paginator.sheetKey`): an incremental layout reuses a sheet whose box, headers, footers, notes, rules and page labels are the same; a layout that is not incremental, one with a header being edited and one with a `STYLEREF` field make them all. The stall after a pause in a long document is reported (`pause.longestMs`), not gated. While the harness types the measured keys it holds the editor's typing pause (`holdPause` in `perf/lib/browser.mts`), so a stall in the harness's own round trips cannot start a layout among the keys.

## Memory of a tab

`perf/lib/scenarios.mts` with `memory: true` measures how much memory a browser tab holds by document size: plans `mem-thesis-10`, `mem-thesis-100`, `mem-thesis-1000` and `mem-war-and-peace` in `npm run perf`, and `mem-thesis-3000` in `perf:full`. They use the production build, the far-pages rule, 300 keys in the middle, a save, and a scroll down and back, with macOS `footprint` sampled every 0.15 to 0.4 s and Chrome's memory dump after the open, the typing, the scroll and a forced collection. The thesis comes from `thesis()` in `perf/lib/docs.mts` (chapters, three heading levels, lists, quotes, a table every second section, a picture and caption every third, a footnote in every eighth paragraph; `thesis-N-nopic`, `-notab` and `-plain` leave out pictures and tables to find what costs what). Targets in `perf/goals.json`: 100 pages under 200 MB while opening, typing and scrolling, and War and Peace under 600 MB scrolling. Limits in `perf/budgets.json` fail the run if memory grows; they hold the current line, they are not the targets.

Where the memory goes:

- **A blank document is 110 to 120 MB** (the start screen 97): malloc 62, V8 17, Blink 15, tiles 9. That leaves under 90 MB of a 200 MB target for the document and the browser's garbage.
- **Garbage, not what the document keeps.** After a forced collection War and Peace is about 250 to 350 MB, not 500 to 900; the rest is memory the browser frees on its next collection, 10 to 30 idle seconds later. It comes from laying every block out once at open (about 31 KB of layout and text shaping per paragraph) and again, in part, for each page that scrolling shows.
- **The spelling layer allocates little.** It does not read the whole document when the browser is idle (the check panel reads what it needs when opened). While a fling goes by it waits until the view has stayed put for 150 ms and two frames before it finds and draws marks (`settleFling` in `spell-layer.ts`; scrolling a little at a time still marks as it goes). It reads a paragraph with `getAttribute` and `firstChild`/`nextSibling`, not `style`, `dataset` and `childNodes`, which leave an object per element for as long as the element lives.
- **The save worker does not compile a second engine.** It is given the page's compiled engine (`engine-module.ts`, posted as its first message) and is let go after 10 idle seconds. It starts only when parts that need compressing add up to 1 MB or more (`toDeflate` in `zip-client.ts`): pictures are stored in the zip as they are, so they do not count.
- **Footnote-heavy long documents lay out again and again.** A 979-page thesis with 3,400 notes opens with 30 whole-document passes and 30 part-way ones, and a key followed by a pause takes about 30 part-way passes (about 20 seconds). The rounds of `layoutBody` that settle each page's note room chase each other across the document in waves before they agree; a 100-page thesis takes 4 part-way passes and about 20 ms.
- **Copies of the file bytes.** For a 4.5 MB file, five 4.3 MB array buffers and about 30 MB more stay after a collection (the page's copies, the worker's, the blob behind each picture).

## The rest of the performance pages

- [Profiling and comparing](profiling.md): Where a key's time goes, comparing builds, checking a change to the paginator and comparing with Google Docs.
