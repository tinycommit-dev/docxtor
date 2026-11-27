// PRIVACY.md says the same thing as e2e/data/outside-hosts.json, the list e2e/privacy.spec.ts holds the app to: every address in the
// list is on the page, and no other address is. Run by `npm test`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const list = JSON.parse(readFileSync(join(root, "e2e/data/outside-hosts.json"), "utf8"));
const page = readFileSync(join(root, "PRIVACY.md"), "utf8");

/** Every address written on the page: a name with a dot and one of the endings below, or localhost with a port. */
function hostsOnPage(text) {
  const found = new Set();
  for (const m of text.matchAll(/\b(?:[a-z0-9][a-z0-9-]*\.)+(?:com|org|net|dev|io|ai|app|test)(?![\w-]|\.[a-z0-9])(?::\d+)?|\blocalhost:\d+/gi)) found.add(m[0].toLowerCase());
  return found;
}

test("every address in the list is on PRIVACY.md", () => {
  assert.ok(list.hosts.length >= 10);
  const on = hostsOnPage(page);
  for (const h of list.hosts) assert.ok(on.has(h.host), `${h.host} is in e2e/data/outside-hosts.json and not on PRIVACY.md`);
});

test("no other address is on PRIVACY.md", () => {
  const known = new Set([...list.hosts.map((h) => h.host), ...list.own]);
  for (const host of hostsOnPage(page)) assert.ok(known.has(host), `${host} is on PRIVACY.md and not in e2e/data/outside-hosts.json`);
});

test("each entry of the list says what it needs", () => {
  const WHEN = ["automatic", "when you ask", "after you set it up", "after you turn it on"];
  for (const h of list.hosts) {
    assert.ok(WHEN.includes(h.when), `${h.host}: when`);
    for (const k of ["what", "calledBy", "sends"]) assert.ok(typeof h[k] === "string" && h[k].length > 10, `${h.host}: ${k}`);
    assert.equal(typeof h.onByDefault, "boolean");
    assert.equal(typeof h.documentText, "boolean");
    assert.ok(h.seenIn.length > 0, `${h.host}: the phases that show it`);
    // Text of the document leaves only after a person turned something on or set it up.
    if (h.documentText) assert.ok(h.when === "after you turn it on" || h.when === "after you set it up", `${h.host} gets the text of the document`);
  }
});

test("PRIVACY.md says which calls happen without asking, and is linked from the README and SECURITY.md", () => {
  for (const h of list.hosts.filter((x) => x.when === "automatic")) assert.ok(page.includes(h.host));
  assert.match(page, /Without asking, and on by default/);
  for (const f of ["README.md", "SECURITY.md"]) assert.ok(readFileSync(join(root, f), "utf8").includes("PRIVACY.md"), `${f} links PRIVACY.md`);
});
