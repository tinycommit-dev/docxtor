// The built app carries its licence file, and the file credits what the build bundles. Run after `npm run build`
// (`npm run check:notices` does both this and the freshness check in notices.mjs).
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "web/dist");

test("web/dist has licenses.txt, the same as the committed file", () => {
  assert.ok(existsSync(join(dist, "licenses.txt")), "web/dist/licenses.txt is missing: run `npm run build` (Vite copies web/public/licenses.txt)");
  assert.equal(readFileSync(join(dist, "licenses.txt"), "utf8"), readFileSync(join(root, "web/public/licenses.txt"), "utf8"));
});

test("licenses.txt credits every npm package in the build, the fonts, the dictionaries and the Rust crates", () => {
  const text = readFileSync(join(dist, "licenses.txt"), "utf8");
  const bundled = JSON.parse(readFileSync(join(root, "node_modules/.cache/docxtor-notices/bundled-packages.json"), "utf8"));
  assert.ok(bundled.length > 10, "the build report lists the bundled packages");
  for (const { name, version } of bundled) assert.ok(text.includes(`${name} ${version}`), `${name} ${version} is credited`);
  for (const must of ["@fontsource/arimo", "@fontsource-variable/inter", "dictionary-de", "dictionary-es", "SIL OPEN FONT LICENSE", "spellbook", "wasm-bindgen", "zlib-rs", "Citation Style Language", "Brotli"]) {
    assert.ok(text.includes(must), `${must} is credited`);
  }
});

test("the offline file list names licenses.txt, so the licence is there offline too", () => {
  assert.ok(readFileSync(join(dist, "sw.js"), "utf8").includes("licenses.txt"));
});

test("the packages that ship a WebAssembly program list their notices file", () => {
  for (const pkg of ["core", "editor"]) {
    const info = JSON.parse(readFileSync(join(root, `packages/${pkg}/package.json`), "utf8"));
    assert.ok(info.files.includes("THIRD_PARTY_LICENSES.txt"), `@docxtor/${pkg} ships THIRD_PARTY_LICENSES.txt`);
    assert.ok(existsSync(join(root, `packages/${pkg}/THIRD_PARTY_LICENSES.txt`)));
  }
});

// One licence allow-list in four places: ALLOWED in notices.mjs (npm packages), `accepted` in about/*.toml (the crates in
// the notices files) and `allow` in deny.toml (cargo deny in CI). They must be the same list.
test("the licence allow-list is the same in notices.mjs, about/*.toml and deny.toml", () => {
  const quoted = (text) => [...text.matchAll(/"([^"]+)"/g)].map((m) => m[1]).sort();
  const js = readFileSync(join(root, "scripts/notices.mjs"), "utf8").match(/const ALLOWED = new Set\(\[([^\]]*)\]\)/);
  assert.ok(js, "ALLOWED is found in notices.mjs");
  const want = quoted(js[1]);
  assert.ok(want.length >= 10);
  for (const file of ["about/native.toml", "about/wasm.toml"]) {
    const m = readFileSync(join(root, file), "utf8").match(/^accepted\s*=\s*\[([^\]]*)\]/m);
    assert.ok(m, `${file} has accepted`);
    assert.deepEqual(quoted(m[1]), want, `${file}: accepted differs from ALLOWED`);
  }
  const deny = readFileSync(join(root, "deny.toml"), "utf8").match(/^\[licenses\][\s\S]*?^allow\s*=\s*\[([^\]]*)\]/m);
  assert.ok(deny, "deny.toml has [licenses] allow");
  assert.deepEqual(quoted(deny[1]), want, "deny.toml: allow differs from ALLOWED");
});
