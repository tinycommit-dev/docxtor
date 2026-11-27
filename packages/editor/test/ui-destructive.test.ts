// A button in a tool strip or a dialog (packages/editor/src/ui) that does what a command does must run the command, not
// send the op itself: the command checks that it is available, asks first when it must, and offers Undo (the host's
// `execute` is the only door). This fails when a file in ui/ sends one of
// the destructive ops directly. Where a ui/ class IS the body of a command (the command calls it), it is listed in
// `BODIES` with the command that calls it; that command must exist.
import { strict as assert } from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

const UI = new URL("../src/ui/", import.meta.url);
const BUILTINS = new URL("../src/builtins/", import.meta.url);
const WEB = new URL("../../../web/src/", import.meta.url);

/** What each pattern sends, so a failure says what was found. Patterns stay inside one object literal (no `}`). */
const DESTRUCTIVE: [string, RegExp][] = [
  ["a tab deleted", /op:\s*"tab"[^}]*action:\s*"delete"/],
  ["protection stopped", /op:\s*"protection"[^}]*action:\s*"stop"/],
  ["a table, row or column deleted", /op:\s*"table_edit"[^}]*action:\s*(?:a\b|"delete_)/],
  ["a table deleted from a button", /tableAction\(\s*"delete_/],
  ["a content control removed", /op:\s*"control"[^}]*action:\s*"remove"/],
  ["a watermark removed", /op:\s*"set_watermark"[^}]*text:\s*null/],
  ["a header or footer removed", /op:\s*"remove_header_footer"/],
  ["a cover page removed", /op:\s*"remove_cover_page"/],
  ["page numbers removed", /op:\s*"remove_page_numbers"/],
  ["a table of contents removed", /op:\s*"remove_toc"|\bremoveToc\(/],
  ["hidden data removed", /op:\s*"remove_hidden_data"/],
  ["a style deleted", /op:\s*"delete_style"/],
  ["a bookmark deleted", /op:\s*"delete_bookmark"|\.removeBookmark\(/],
  ["a source deleted", /op:\s*"delete_source"/],
  ["a citation removed", /op:\s*"remove_citation"|\.removeCitation\(/],
  ["blocks deleted", /op:\s*"delete_blocks"/],
];

/** ui/ files whose code is the body of a command: the file, what it sends, and the command that runs it. */
const BODIES: { file: string; what: string; command: string }[] = [
  { file: "controls.ts", what: "a content control removed", command: "control.remove" },
  // Convert to text on a smart chip's card: the chip's own edit API (a contribution's `exec`), the same op as Remove content control.
  { file: "chip-cards.ts", what: "a content control removed", command: "control.remove" },
];

const files = readdirSync(UI).filter((f) => f.endsWith(".ts"));

test("no file in ui/ sends a destructive op itself", () => {
  const found: string[] = [];
  for (const f of files) {
    const text = readFileSync(new URL(f, UI), "utf8").replace(/\/\/.*$/gm, "");
    for (const [what, re] of DESTRUCTIVE) {
      if (re.test(text) && !BODIES.some((b) => b.file === f && b.what === what)) found.push(`${f}: ${what}`);
    }
  }
  assert.deepEqual(found, [], "run the command through editor.command(id, ...) instead of sending the op");
});

test("a command that a ui/ file is the body of exists", () => {
  const sources = [BUILTINS, WEB].flatMap((dir) => readdirSync(dir).filter((f) => f.endsWith(".ts")).map((f) => readFileSync(new URL(f, dir), "utf8")));
  for (const b of BODIES) {
    const at = readFileSync(new URL(b.file, UI), "utf8");
    assert.ok(at.length > 0);
    const known = sources.some((s) => s.includes(`"${b.command}"`));
    assert.ok(known, `${b.command} is named in builtins/ or web/`);
  }
});
