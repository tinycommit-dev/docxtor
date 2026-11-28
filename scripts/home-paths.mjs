// Finds absolute home paths (/Users/<name>/, /home/<name>/) in built files, binary ones included. Rust writes the source path of
// every panic location into the WebAssembly, so a build on a developer's machine would carry their home folder into what ships;
// the wasm builds remap $HOME to ~ (RUSTFLAGS in packages/*/package.json), and these checks make sure nothing slipped through.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

/** Names that appear in sample paths in docs and test data, not anyone's home folder. */
export const MADE_UP = new Set(["me", "x", "ana", "name", "user", "username", "you"]);
export const HOME_PATH = /(?<![A-Za-z]:)(?<![A-Za-z0-9_.-])\/(?:Users|home)\/([A-Za-z0-9._-]+)\//g;

/** The home paths in one file, as "path" strings, de-duplicated. */
export function homePathsIn(file) {
  const text = readFileSync(file).toString("latin1");
  return [...new Set([...text.matchAll(HOME_PATH)].filter((m) => !MADE_UP.has(m[1])).map((m) => m[0]))];
}

/** Every file under `dir` that holds a home path, as "file: path, path" lines. */
export function homePathsUnder(dir) {
  const found = [];
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      const f = join(d, name);
      if (statSync(f).isDirectory()) walk(f);
      else {
        const hits = homePathsIn(f);
        if (hits.length) found.push(`${f}: ${hits.join(", ")}`);
      }
    }
  };
  walk(dir);
  return found;
}
