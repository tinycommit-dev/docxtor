// The nginx and Caddy files are the same rules as deploy/_headers. Caddy is not installed here, so the Caddyfile is read back by a small
// parser of its own (braces, quoting, matchers) and both files are asked what they send for real paths; the answer must be Cloudflare's.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { OUTPUTS, effectiveHeaders, generate, mergeHeaders, parseHeaders, patternKind, patternRegExp, toCaddy, toNginx } from "./headers.mjs";

const source = readFileSync(new URL("../deploy/_headers", import.meta.url), "utf8");
const rules = parseHeaders(source);
const PATHS = ["/", "/index.html", "/sw.js", "/manifest.webmanifest", "/assets/index-abc.js", "/assets/sub/x.wasm", "/favicon.svg", "/icons/a.png", "/licenses.txt", "/docs/", "/docs/embedding", "/docs/hashmap.json", "/docs/assets/app.abc.js", "/docs/assets/chunks/theme.x.js"];

/** What Cloudflare sends: every rule whose pattern matches the path adds its headers, in file order; the same name twice is joined with ", "; `! Name` removes what came before. */
function cloudflare(path) {
  return Object.fromEntries(mergeHeaders(rules.filter((r) => patternRegExp(r.pattern).test(path)).flatMap((r) => r.headers)));
}

/** Reads nginx's locations back and answers like nginx does: an exact match wins, then the longest `^~` prefix, then `location /`. */
function parseNginx(text) {
  const blocks = [];
  const re = /^location (?:(=|\^~) )?(\S+) \{\n([\s\S]*?)\n\}/gm;
  for (let m; (m = re.exec(text)); ) {
    const headers = {};
    for (const line of m[3].split("\n")) {
      const h = /^ {4}add_header (\S+) "([^"]*)" always;$/.exec(line);
      assert.ok(h, `an add_header line nginx reads: ${line}`);
      headers[h[1]] = h[2];
    }
    blocks.push({ mod: m[1] ?? "", path: m[2], headers });
  }
  return (path) => {
    const exact = blocks.find((b) => b.mod === "=" && b.path === path);
    if (exact) return exact.headers;
    const prefixes = blocks.filter((b) => b.mod === "^~" && path.startsWith(b.path)).sort((a, b) => b.path.length - a.path.length);
    return (prefixes[0] ?? blocks.find((b) => b.mod === "" && b.path === "/")).headers;
  };
}

/** Reads the Caddy snippet back: balanced braces, one directive per line, quoted values, named path matchers. */
function parseCaddy(text) {
  const lines = text.split("\n").filter((l) => l.trim() && !l.trim().startsWith("#"));
  assert.equal(lines[0], "(docxtor-headers) {", "the snippet is named");
  const matchers = new Map();
  const blocks = [];
  let depth = 1;
  let open = null;
  for (const line of lines.slice(1)) {
    const t = line.trim();
    let m;
    if ((m = /^@(\w+) path (\S+)$/.exec(t))) {
      assert.equal(depth, 1);
      matchers.set(m[1], m[2]);
    } else if ((m = /^header(?: @(\w+))? \{$/.exec(t))) {
      assert.equal(depth, 1);
      assert.ok(!m[1] || matchers.has(m[1]), `matcher ${m[1]} is defined before it is used`);
      open = { matcher: m[1] ? matchers.get(m[1]) : null, headers: {} };
      depth++;
    } else if (t === "}") {
      depth--;
      if (open) blocks.push(open);
      open = null;
    } else {
      assert.ok(open, `a directive outside a header block: ${t}`);
      m = /^([A-Za-z][\w-]*) "([^"]*)"$/.exec(t);
      assert.ok(m, `a header line Caddy reads (name, then a quoted value): ${t}`);
      open.headers[m[1]] = m[2];
    }
  }
  assert.equal(depth, 0, "braces are balanced");
  const matches = (matcher, path) => (matcher === null ? true : matcher.endsWith("*") ? path.startsWith(matcher.slice(0, -1)) : matcher === path);
  return (path) => Object.assign({}, ...blocks.filter((b) => matches(b.matcher, path)).map((b) => b.headers));
}

test("every pattern in _headers is one the generator supports", () => {
  for (const r of rules) assert.ok(["all", "prefix", "exact"].includes(patternKind(r.pattern).kind));
  assert.ok(rules.length >= 5 && rules.some((r) => r.headers.some(([k]) => k === "Content-Security-Policy")));
});

test("nginx sends what Cloudflare sends, for every path", () => {
  const answer = parseNginx(toNginx(rules));
  for (const p of PATHS) assert.deepEqual(answer(p), cloudflare(p), p);
});

test("Caddy sends what Cloudflare sends, for every path", () => {
  const answer = parseCaddy(toCaddy(rules));
  for (const p of PATHS) assert.deepEqual(answer(p), cloudflare(p), p);
});

test("effective headers: a specific rule replaces the general one's value only by detaching it first", () => {
  const r = parseHeaders("/*\n  A: 1\n  B: 2\n/x/*\n  ! B\n  B: 3\n/x/y\n  C: 4\n");
  assert.deepEqual(effectiveHeaders(r, "/x/y"), [["A", "1"], ["B", "3"], ["C", "4"]]);
  assert.deepEqual(effectiveHeaders(r, "/x/*"), [["A", "1"], ["B", "3"]]);
});

test("effective headers: the same name from two rules is joined, as Cloudflare does", () => {
  const r = parseHeaders("/*\n  B: 2\n/x/*\n  B: 3\n");
  assert.deepEqual(effectiveHeaders(r, "/x/*"), [["B", "2, 3"]]);
  assert.deepEqual(effectiveHeaders(parseHeaders("/*\n  B: 2\n/x/*\n  ! B\n"), "/x/*"), []);
});

test("the docs get their own policy and caching, and nothing of the app's", () => {
  const csp = (p) => cloudflare(p)["Content-Security-Policy"];
  assert.ok(!csp("/docs/embedding").includes("wasm-unsafe-eval") && !csp("/docs/embedding").includes("http"), "no engine and no outside host");
  assert.ok(!/script-src[^;]*unsafe-inline/.test(csp("/docs/embedding")), "scripts are never 'unsafe-inline'");
  assert.equal(cloudflare("/docs/embedding")["Cache-Control"], "no-cache");
  assert.equal(cloudflare("/docs/assets/app.x.js")["Cache-Control"], "public, max-age=31536000, immutable");
  assert.ok(csp("/").includes("wasm-unsafe-eval"), "the app keeps its own policy");
  assert.ok(!csp("/assets/x.js").includes("sha256-"));
});

test("a value that cannot be quoted safely is refused, not written wrong", () => {
  assert.throws(() => toNginx(parseHeaders('/*\n  A: say "hi"\n')));
  assert.throws(() => toCaddy(parseHeaders("/*\n  A: back\\slash\n")));
  assert.throws(() => toNginx(parseHeaders("/a/*/b\n  A: 1\n")), /not supported/);
});

test("the committed files are what the generator makes from _headers now (npm run headers rewrites them)", () => {
  for (const { file, text } of generate()) assert.equal(readFileSync(new URL(`../${file}`, import.meta.url), "utf8"), text, `${file} is stale: node scripts/headers.mjs`);
  assert.deepEqual(OUTPUTS.map((o) => o.file), ["deploy/nginx.conf", "deploy/Caddyfile"]);
});

test("changing _headers changes both files (so a stale copy is caught)", () => {
  const changed = parseHeaders(`${source}\n/extra\n  X-Test: 1\n`);
  assert.notEqual(toNginx(changed), toNginx(rules));
  assert.notEqual(toCaddy(changed), toCaddy(rules));
});
