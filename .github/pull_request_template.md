## What this changes

<!-- One or two sentences: what and why. Link the issue if there is one. -->

## How it was checked

- [ ] `cargo test` and `cargo clippy --all-targets -- -D warnings`
- [ ] `npm run build` and `npm test`
- [ ] A test for the change: an engine test in `crates/docxtor-core/tests/all/`, or a browser test in `e2e/` for editor and app changes
- [ ] `npm run perf:quick` for a change to the engine or the editor
- [ ] `npm run notices` if a dependency, font or dictionary changed (and the regenerated licence files are committed)
- [ ] New interface text goes through `t("…")` and `npm run messages` was run

## Notes for the reviewer

<!-- Anything you were unsure about, a file Word opens differently, screenshots for a visible change. -->

By contributing you agree that your contribution is dual-licensed under MIT and Apache-2.0.
