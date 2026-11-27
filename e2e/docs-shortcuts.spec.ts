// The shortcut table of the user guide (docs/use/shortcuts.md) is made from the commands the app registers, so it cannot drift.
// The test builds the table from the running app and fails when the page differs; to rewrite the page after a change:
//   DOCXTOR_DOCS_WRITE=1 npx playwright test e2e/docs-shortcuts.spec.ts --project=chromium
import { readFileSync, writeFileSync } from "node:fs";
import { formatChord } from "../packages/extensions/src/keys";
import { expect, openFixture, test, type Page } from "./helpers";

const FILE = "docs/use/shortcuts.md";
const START = "<!-- shortcuts:start -->";
const END = "<!-- shortcuts:end -->";

interface Row {
  category: string;
  title: string;
  /** The command's keys on Windows and Linux, and on a Mac: some commands have more keys on one of them. */
  win: string[];
  mac: string[];
}

/** The table, written as Markdown: one section a category, a row a command. */
function table(rows: Row[]): string {
  const out: string[] = [];
  const order = [...new Set(rows.map((r) => r.category))].sort((a, b) => a.localeCompare(b));
  for (const category of order) {
    out.push(`### ${category}`, "", "| Command | Windows and Linux | Mac |", "|---|---|---|");
    for (const r of rows.filter((x) => x.category === category).sort((a, b) => a.title.localeCompare(b.title))) {
      const win = r.win.map((s) => `\`${formatChord(s, false)}\``).join(" or ");
      const mac = r.mac.map((s) => `\`${formatChord(s, true)}\``).join(" or ");
      out.push(`| ${r.title} | ${win} | ${mac} |`);
    }
    out.push("");
  }
  return out.join("\n");
}

/** Each command's default keys as the app registers them on one platform (`navigator.platform`, which the extension host reads). */
async function shortcutsOn(page: Page, platform: string) {
  await page.addInitScript((p) => Object.defineProperty(Navigator.prototype, "platform", { get: () => p, configurable: true }), platform);
  await openFixture(page, "simple.docx");
  return page.evaluate(() => {
    const ext = (window as unknown as { docxtor: { fe: { extensions: any } } }).docxtor.fe.extensions;
    const found: Record<string, { category: string; title: string; specs: string[] }> = {};
    for (const c of ext.commands.list()) {
      const specs: string[] = (ext.defaultShortcutSpecs(c.id) as string[]).length ? ext.defaultShortcutSpecs(c.id) : c.shortcutHint ? [c.shortcutHint] : [];
      // Escape only closes what is open; it is not worth a row.
      const kept = specs.filter((s) => s.toLowerCase() !== "escape");
      if (kept.length) found[c.id] = { category: c.category || "General", title: c.title, specs: kept };
    }
    return found;
  });
}

test("the shortcut table of the user guide lists exactly the shortcuts the commands have", async ({ page }) => {
  // The page is the same whichever computer makes it: each column comes from the app as it starts on that platform.
  const onMac = await shortcutsOn(page, "MacIntel");
  const onWin = await shortcutsOn(await page.context().newPage(), "Win32");
  const rows: Row[] = [...new Set([...Object.keys(onWin), ...Object.keys(onMac)])].map((id) => {
    const c = onWin[id] ?? onMac[id];
    return { category: c.category, title: c.title, win: onWin[id]?.specs ?? [], mac: onMac[id]?.specs ?? [] };
  });
  expect(rows.length).toBeGreaterThan(80);
  const text = readFileSync(FILE, "utf8");
  const a = text.indexOf(START);
  const b = text.indexOf(END);
  expect(a, `${FILE} has the ${START} and ${END} markers`).toBeGreaterThan(-1);
  const next = `${text.slice(0, a + START.length)}\n\n${table(rows)}\n${text.slice(b)}`;
  if (process.env.DOCXTOR_DOCS_WRITE) writeFileSync(FILE, next);
  else expect(text, `${FILE} is out of date: run with DOCXTOR_DOCS_WRITE=1 to rewrite it`).toBe(next);
});
