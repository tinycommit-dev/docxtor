# Contributing to Docxtor

Thanks for helping build a free, open document editor. The documentation is a website at https://docxtor.tinycommit.dev/docs/, built from the Markdown in `docs/` and from this file, `PRIVACY.md` and `SECURITY.md`.

## From a fresh clone to a running app and a passing test

### What you need first

- **Git** and a **C toolchain** (Xcode command line tools on a Mac, `build-essential` on Debian and Ubuntu) for Rust to link with.
- **Rust**, through [rustup](https://rustup.rs). The repository names its Rust release in `rust-toolchain.toml` (1.92.0, the one CI uses), and rustup installs it with the WebAssembly target the first time you build.
- **wasm-pack**, which builds the engine for the browser. CI installs 0.15.0 exactly.
- **Node 22.18 or later** (CI uses 24). The tests run TypeScript directly, with Node's built-in type stripping.
- Optional, for the browser tests: Playwright's Chromium (`npx playwright install chromium`, after `npm install`).
- Optional, for the Word compatibility gates (`npm run corpus:validate`, `npm run corpus:lo`): the .NET SDK (8 in CI), LibreOffice (`soffice`) and poppler (`pdftotext`). CI runs them nightly; you only need them when you change what the engine writes.
- Optional, to make the fixtures again with `python3 fixtures/make_fixtures.py`: Python with `python-docx` and `pillow`. The fixtures are in git, so most people never do.

*Sketch, not run by a test: installs tools on your machine; the steps are run and timed below.*
```bash
cargo install wasm-pack --locked --version 0.15.0
```

### The steps, with how long each takes

Timings on an Apple M1 Max (10 cores, 32 GB) with nothing built, Rust, wasm-pack and Node installed and the package caches full (so no download time), and an empty Rust target folder. A slower machine takes longer, mostly in the two Rust builds.

| Step | Command | Time |
| --- | --- | --- |
| 1. The JavaScript packages | `npm install` (timed as `npm ci --offline`) | 1 s |
| 2. The engine as WebAssembly (and the spelling checker) | `npm run wasm` | 77 s from nothing; 72 s again after a change to the engine |
| 3. The app | `npm run dev` | the server answers in 1 s; a page with a document open shows about 1 s after that |
| 4. The engine's tests | `cargo test` | 285 s from nothing (1,979 tests); 230 s when nothing needs building |
| 5. The JavaScript tests | `npm test` | 69 s from nothing (it type-checks every example and builds the command line program); 19 s when that is built |

*Sketch, not run by a test: setup and a dev server that keeps running; each step is run and timed in the table above.*
```bash
npm install
npm run wasm
npm run dev       # the app at http://localhost:5173
```

Then open http://localhost:5173. The start screen has a sample document. To open one of the test documents from `fixtures/` by address (`?open=rich.docx`), the dev server needs to know where they are: put `VITE_FIXTURES=/absolute/path/to/your/clone/fixtures` in `web/.env.development.local` (git leaves that file out).

`cargo test` and `npm test` are independent, and `npm test` needs step 2 (it loads the engine). `npm test` runs the packages' Node tests, the checks on the interface text and the checks on these documents and their examples (`npm run test:docs`, which builds the CLI and the MCP server and type-checks every TypeScript example, so it also needs the Rust target folder from step 4 or builds one first).

To run one thing, not everything:

<!-- example: contributing-one-test -->
```bash
cargo test -p docxtor-core --test all roundtrip::
```

The engine's integration tests are one program (`crates/docxtor-core/tests/all/`), so a new test is a module there and not a new file in `tests/`.

### The browser tests

`npm run test:e2e` runs every browser test (more than 2,300, about 20 to 25 minutes with 4 workers) on Chromium; Playwright starts its own server on port 5180 and needs step 2 done. Run one file while you work: `npx playwright test e2e/find.spec.ts`. Firefox and WebKit run the core set only (`npm run test:e2e:core`, see [docs/browsers.md](docs/browsers.md)).

### Before you send a pull request

`cargo test`, `cargo clippy --all-targets -- -D warnings`, `npm run build`, `npm test`, and the browser tests that touch what you changed (CI runs the whole suite).

## Where help counts most

1. Documents that don't round-trip. Run `cargo run -p docxtor-cli -- corpus path/to/folder`, and if a file scores below 100% or won't open, open an issue with the file (remove anything private first) or add it to `fixtures/` in a PR.
2. Rendering differences. Send the file along with a screenshot of it in Word and in Docxtor.
3. Translations. Copy `web/src/languages/es.json`, translate the values (the keys are the English text, listed in `locales/source.json`), and check it with `node scripts/messages.mjs --pack your.json`.
4. Features from [the roadmap](docs/roadmap.md). For anything large, please open an issue first to agree on the approach.

## Where a change goes

| You want to change | Look in | And add a test in |
|---|---|---|
| How a .docx is read, edited or saved | `crates/docxtor-core/src` (edits are ops: `ops.rs`) | `crates/docxtor-core/tests/all/` (a module per area) |
| What the AI tools, the CLI or the MCP server offer | `crates/docxtor-core/src/ai*.rs`, `crates/docxtor-cli`, `crates/docxtor-mcp` | the crate's `tests/` |
| The engine's TypeScript API | `packages/core/src` | `packages/core/test` |
| What extensions can contribute | `packages/extensions/src` (`types.ts` is the API) | `packages/extensions/test` |
| The editing surface: pages, selection, tools, menus | `packages/editor/src` (a feature is one file in `src/builtins/`) | `e2e/`, the spec for that area |
| The app: Library, history, start screen, the assistant | `web/src` | `e2e/` |
| Interface text | the `t("…")` call, then `npm run messages` and `web/src/languages/es.json` | `npm test` checks both |
| Colours, spacing, type | the tokens in `DESIGN.md` and on `.docxtor` in `packages/editor/src/styles/tokens.css` | `npm run test:a11y` for contrast |
| Speed | anywhere; measure with `npm run perf:quick` | `perf/` (a gate per metric, see the performance docs) |
| The docs site | `docs/` (a page is a file, a section is a folder) | `npm run docs:build`, `npm test` |
| How the app is served | `deploy/` (`_headers` is the one source of headers) | `e2e/production.spec.ts` |

`npm test` also checks the layers (`scripts/check-layers.mjs`: core, then extensions, then the editor, then the app; each imports only the ones before it), that no hand-written file grows past about 15,000 tokens (`scripts/check-size.mjs`), and that the repository speaks to readers (`scripts/voice.test.mjs`).

## Ground rules

- The engine must never drop content it doesn't understand. New features extend the model and leave the path that preserves unknown content in place.
- Every edit goes through an op (`crates/docxtor-core/src/ops.rs`). The UI, the AI tools, the CLI and the MCP server all share the ops, which is what keeps them in sync.
- Engine changes need a test in `crates/docxtor-core/tests/all/roundtrip.rs` (or a module beside it). Editor and app changes need a browser test in `e2e/` (one spec per area; `e2e/helpers.ts` opens fixtures and finds menus).
- Documents and their examples: a code block in `README.md`, `CONTRIBUTING.md` or `docs/` has a marker on the line before it, `<!-- example: name -->` for a program that a test runs, a visible line `*Sketch, not run by a test: why.*` for one that is not run, `<!-- text -->` for output and diagrams. `npm run test:docs` (Node and the command line) and `e2e/docs-examples.spec.ts` (the browser) fail when an example stops working or a block has no marker.
- When a dependency changes (`Cargo.lock`, `package-lock.json`, a font or a dictionary), run `npm run build` and then `npm run notices`, and commit the regenerated licence files; `npm run check:notices` (CI runs it) fails when they are out of date or when a shipped dependency has a licence outside the allow-list. `npm run check:paths` fails on an absolute home path in a tracked file.
- Keep the layers apart. Lowest first: `@docxtor/core` (the engine), `@docxtor/extensions` (the extension API), `@docxtor/editor` (the component), `web/` (the app); each uses only the ones before it. New features should be extensions where they can be; see [docs/extensions/index.md](docs/extensions/index.md). The editor never touches storage, addresses or UI that belongs to the app. [docs/roadmap.md](docs/roadmap.md) explains why.
- The documentation site: `npm run docs:dev` shows it with live reload, `npm run docs:build` builds it and fails on a dead link (CI runs it). Once built, `npm run dev` serves it at /docs/ too, and the app's Documentation link opens it there. Write plain Markdown that GitHub shows too: relative links to other `.md` files, no front matter, no `{{ }}` or `<tag>` outside backticks. Every Markdown file in `docs/` is a page; a new one is added to the sidebar in `docs/.vitepress/config.mts`, and `scripts/docs-site.test.mjs` fails when a page is missing from the sidebar or the sidebar names a file that does not exist, and when the hashes of the page scripts in the docs' Content-Security-Policy (`deploy/_headers`) are not those of the built pages.
- Keep the web UI free of frameworks and accessible, with every control reachable from the keyboard and labelled.

By contributing, you agree that your contributions are dual-licensed under MIT and Apache-2.0, like the rest of the project.
