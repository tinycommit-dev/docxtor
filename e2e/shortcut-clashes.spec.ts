// No two built-in commands may share a keyboard shortcut. The host runs the first command registered for a key,
// so a second one with the same key silently never runs (Find previous and Change case both had Shift+F3).
// This reads the live host in the running app, so it covers every extension the editor and the app ship.
import { expect, openFixture, test } from "./helpers";

/**
 * Keys that two commands may share on purpose. A command's `when` is a function, so the test cannot tell whether two
 * contexts can overlap; a share has to be listed here, as "chord | command | command" (chord normalised as below,
 * commands sorted), with the reason. New clashes still fail.
 */
const INTENDED_SHARES: Record<string, string> = {
  // Escape leaves the Reading view, or Focus; each runs only while its own mode is on, and with both on the first Escape
  // leaves the Reading view and the second leaves Focus.
  "escape | view.exitFocusMode | view.exitView": "each only while its mode is on",
};

test("no two built-in commands share a keyboard shortcut", async ({ page }) => {
  await openFixture(page, "simple.docx");
  const { out: clashes, bound, ids } = await page.evaluate(() => {
    type Cmd = { id: string; keybinding?: string | string[]; when?: () => boolean };
    const host = (window as unknown as { docxtor: { fe: { extensions: { commands: { all(): { item: Cmd; extension: string }[] } } } } }).docxtor.fe.extensions;
    const ALIASES: Record<string, string> = { esc: "escape", return: "enter", space: " ", plus: "+", del: "delete", up: "arrowup", down: "arrowdown", left: "arrowleft", right: "arrowright" };
    // "Shift+Mod+G" and "Mod+Shift+G" are one key; Mod is Ctrl on Windows and Linux and Cmd on a Mac, so compare on both.
    const norm = (spec: string, mac: boolean): string => {
      const parts = spec.split(/\+(?!$)/).map((p) => p.trim().toLowerCase());
      const key = parts.pop()!;
      const mods = new Set<string>();
      for (const p of parts) {
        if (p === "mod") mods.add(mac ? "cmd" : "ctrl");
        else if (p === "cmd" || p === "meta" || p === "super") mods.add("cmd");
        else if (p === "ctrl" || p === "control") mods.add("ctrl");
        else if (p === "alt" || p === "option" || p === "opt") mods.add("alt");
        else if (p === "shift") mods.add("shift");
        else throw new Error(`Unknown modifier "${p}" in "${spec}"`);
      }
      return [...["ctrl", "alt", "shift", "cmd"].filter((m) => mods.has(m)), ALIASES[key] ?? key].join("+");
    };
    const bound = host.commands.all().filter(({ item }) => item.keybinding !== undefined);
    const ids = new Set(bound.map(({ item }) => item.id));
    const out: { chord: string; commands: string[]; platform: string; whens: number }[] = [];
    for (const mac of [false, true]) {
      const byChord = new Map<string, { id: string; when: boolean }[]>();
      for (const { item } of host.commands.all()) {
        const specs = item.keybinding === undefined ? [] : Array.isArray(item.keybinding) ? item.keybinding : [item.keybinding];
        // A command that lists the same key twice (Mod+Y and Ctrl+Y on Windows) is not a clash with itself.
        for (const k of new Set(specs.map((s) => norm(s, mac)))) byChord.set(k, [...(byChord.get(k) ?? []), { id: item.id, when: !!item.when }]);
      }
      for (const [chord, list] of byChord) if (list.length > 1) out.push({ chord, commands: list.map((c) => c.id).sort(), platform: mac ? "Mac" : "Windows/Linux", whens: list.filter((c) => c.when).length });
    }
    return { out, bound: bound.length, ids: ["find.next", "find.previous", "file.save"].filter((id) => ids.has(id)) };
  });
  // Guard against the test going blind: the editor's commands and the app's are both in the registry.
  expect(bound).toBeGreaterThan(60);
  expect(ids).toEqual(["find.next", "find.previous", "file.save"]);
  const unexpected = clashes.filter((c) => !(`${c.chord} | ${c.commands.join(" | ")}` in INTENDED_SHARES));
  expect(unexpected, "commands sharing a keyboard shortcut (the first registered wins, the rest never run); give one a different key").toEqual([]);
  // A listed share that no longer clashes is stale: remove it so the list stays honest.
  const live = new Set(clashes.map((c) => `${c.chord} | ${c.commands.join(" | ")}`));
  expect(Object.keys(INTENDED_SHARES).filter((k) => !live.has(k)), "entries in INTENDED_SHARES that no longer clash").toEqual([]);
});
