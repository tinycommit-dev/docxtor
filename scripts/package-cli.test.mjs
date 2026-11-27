// A program's archive holds the program, the licence notices of that program (not the other one's) and a README line.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { PROGRAMS, packageCli } from "./package-cli.mjs";

for (const bin of Object.keys(PROGRAMS)) {
  test(`${bin}: the archive has the program, its own licence notices and a README`, () => {
    const tmp = mkdtempSync(join(tmpdir(), "docxtor-cli-"));
    try {
      const fake = join(tmp, bin);
      writeFileSync(fake, "#!/bin/sh\necho hi\n", { mode: 0o755 });
      const file = packageCli({ bin, target: "x86_64-unknown-linux-musl", version: "1.2.3", binary: fake, out: join(tmp, "out") });
      assert.equal(file, join(tmp, `out/${bin}-1.2.3-x86_64-unknown-linux-musl.tar.gz`));
      const un = join(tmp, "un");
      mkdirSync(un);
      execFileSync("tar", ["-xzf", file, "-C", un]);
      const dir = join(un, `${bin}-1.2.3-x86_64-unknown-linux-musl`);
      for (const f of [bin, "README.txt", "LICENSE-APACHE", "LICENSE-MIT", "THIRD_PARTY_LICENSES.txt"]) assert.ok(existsSync(join(dir, f)), f);
      assert.equal(readFileSync(join(dir, "THIRD_PARTY_LICENSES.txt"), "utf8"), readFileSync(join(PROGRAMS[bin].crate, "THIRD_PARTY_LICENSES.txt"), "utf8"));
      assert.match(readFileSync(join(dir, "README.txt"), "utf8"), new RegExp(`(Try|Use) it: .*${bin}`));
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
}

test("an unknown program or a missing binary is refused", () => {
  assert.throws(() => packageCli({ bin: "nope", target: "x", version: "1" }), /Unknown program/);
  assert.throws(() => packageCli({ bin: "docxtor", target: "x86_64-apple-darwin", version: "1", binary: "/nonexistent/docxtor" }), /No built program/);
});
