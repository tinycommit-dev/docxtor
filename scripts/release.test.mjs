// The helpers the release workflow uses: the version in every place, the tag check, and the changelog section that becomes the release notes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { changelogSection, checkTag, releaseVersion, versions } from "./release.mjs";

const SAMPLE = `# Changelog

## [Unreleased]

Next.

## [0.2.0] - 2026-12-01

### Added

- Comments.

## [0.1.10] - 2026-11-30

- Ten.

## [0.1.0] - 2026-11-25

- First.
`;

test("the notes are the section of that version, up to the next heading", () => {
  assert.equal(changelogSection(SAMPLE, "0.2.0"), "### Added\n\n- Comments.\n");
  assert.equal(changelogSection(SAMPLE, "0.1.0"), "- First.\n");
});

test("a version is not mistaken for another that starts the same", () => {
  assert.equal(changelogSection(SAMPLE, "0.1.10"), "- Ten.\n");
  assert.throws(() => changelogSection(SAMPLE, "0.1.1"), /no section/);
  assert.throws(() => changelogSection(SAMPLE, "0.1"), /no section/);
});

test("a missing or empty section stops the release", () => {
  assert.throws(() => changelogSection(SAMPLE, "9.9.9"), /Write it before tagging/);
  assert.throws(() => changelogSection("## [1.0.0]\n\n## [0.9.0]\n- x\n", "1.0.0"), /empty/);
  assert.equal(changelogSection("## [1.0.0] - x\n- only\n", "1.0.0"), "- only\n");
});

test("the tag must be v plus the version", () => {
  checkTag("v1.2.3", "1.2.3");
  assert.throws(() => checkTag("1.2.3", "1.2.3"), /must be v1\.2\.3/);
  assert.throws(() => checkTag("v1.2.4", "1.2.3"), /must be v1\.2\.3/);
});

test("the repository's versions agree (package.json, web/package.json, Cargo.toml)", () => {
  const all = versions();
  assert.equal(new Set(Object.values(all)).size, 1, JSON.stringify(all));
  assert.equal(releaseVersion(), all["package.json"]);
  assert.match(readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8"), /^## \[Unreleased\]/m);
});
