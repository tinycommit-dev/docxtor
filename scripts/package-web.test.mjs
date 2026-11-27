// The self-host archive: unpacked, it has the built files, the headers in three forms, the licences and a README that says what breaks
// without HTTPS. Made from a small stand-in for web/dist, so it runs in a second; `npm run package:web` makes the real one.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { makeArchive, readme, rootVersion } from "./package-web.mjs";

function fakeDist(dir) {
  mkdirSync(join(dir, "assets"), { recursive: true });
  writeFileSync(join(dir, "index.html"), "<!doctype html><title>Docxtor</title>");
  writeFileSync(join(dir, "sw.js"), "// sw");
  writeFileSync(join(dir, "licenses.txt"), "licences of everything");
  writeFileSync(join(dir, "_headers"), "copied by deploy.mjs for Cloudflare");
  writeFileSync(join(dir, "assets/engine-abc.wasm"), Buffer.from([0, 0x61, 0x73, 0x6d]));
  // The hosted build also has the documentation site; the archive must not.
  mkdirSync(join(dir, "docs"), { recursive: true });
  writeFileSync(join(dir, "docs/index.html"), "<!doctype html><title>Docs</title>");
}

test("the archive holds the site, the headers in three forms, the licences and the README", () => {
  const tmp = mkdtempSync(join(tmpdir(), "docxtor-archive-"));
  try {
    fakeDist(join(tmp, "web-dist"));
    const file = makeArchive({ dist: join(tmp, "web-dist"), out: join(tmp, "out"), version: "9.8.7" });
    assert.equal(file, join(tmp, "out/docxtor-web-9.8.7.tar.gz"));
    const names = execFileSync("tar", ["-tzf", file], { encoding: "utf8" }).split("\n").filter(Boolean).map((n) => n.replace(/\/$/, ""));
    assert.ok(names.every((n) => n.startsWith("docxtor-web-9.8.7")), "everything sits in one folder");
    assert.ok(!names.some((n) => /(^|\/)\._/.test(n)), "no Apple metadata files");
    const un = join(tmp, "un");
    mkdirSync(un);
    execFileSync("tar", ["-xzf", file, "-C", un]);
    const dir = join(un, "docxtor-web-9.8.7");
    for (const f of ["site/index.html", "site/sw.js", "site/assets/engine-abc.wasm", "site/licenses.txt", "licenses.txt", "_headers", "nginx.conf", "Caddyfile", "THIRD_PARTY_NOTICES.md", "LICENSE-APACHE", "LICENSE-MIT", "README.md"]) {
      assert.ok(existsSync(join(dir, f)), `${f} is in the archive`);
    }
    // The headers are the repository's, not the stand-in's, and sit beside the site, not inside it.
    assert.equal(readFileSync(join(dir, "_headers"), "utf8"), readFileSync("deploy/_headers", "utf8"));
    assert.equal(readFileSync(join(dir, "nginx.conf"), "utf8"), readFileSync("deploy/nginx.conf", "utf8"));
    assert.equal(readFileSync(join(dir, "Caddyfile"), "utf8"), readFileSync("deploy/Caddyfile", "utf8"));
    assert.ok(!existsSync(join(dir, "site/_headers")));
    assert.ok(!existsSync(join(dir, "site/docs")) && !names.some((n) => n.includes("/docs")), "the documentation site is not in the archive");
    // The README says how to serve it and what breaks without HTTPS.
    const text = readFileSync(join(dir, "README.md"), "utf8");
    assert.match(text, /Without HTTPS/);
    assert.match(text, /service worker/);
    assert.match(text, /docxtor-web:9\.8\.7/);
    assert.equal(text, readme("9.8.7"));
    assert.deepEqual(readdirSync(join(tmp, "out")), ["docxtor-web-9.8.7.tar.gz"], "no staging folder is left behind");
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

test("a folder with no built app is refused", () => {
  const tmp = mkdtempSync(join(tmpdir(), "docxtor-archive-"));
  try {
    assert.throws(() => makeArchive({ dist: tmp, out: join(tmp, "out"), version: "1.0.0" }), /No built app/);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

test("the version comes from the root package.json", () => {
  assert.match(rootVersion(), /^\d+\.\d+\.\d+$/);
});
