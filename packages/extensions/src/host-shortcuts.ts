// The person's own shortcuts: overrides, conflicts, keys given back, and the key handler.
import type { ExtensionHost, TakenKey } from "./host.ts";
import type { Chord, KeyLike } from "./keys.ts";
import { matches, parseChord } from "./keys.ts";
import { t } from "./l10n.ts";
import type { Command } from "./types.ts";

/** Keys that, with ⌘/Ctrl, edit the text of an input: undo, redo, select all, clipboard, bold… */
const FIELD_KEYS = /^([zyacvxbiu]|backspace|delete|arrow(left|right|up|down)|home|end)$/;

/** A chord as one string, to compare shortcuts that were written differently ("Mod+Z" and "Cmd+z"). */
function chordKey(c: Chord): string {
  return `${c.meta ? "m" : ""}${c.ctrl ? "c" : ""}${c.alt ? "a" : ""}${c.shift ? "s" : ""}:${c.key}`;
}

/** Work out which default keys are free now. */
export function releaseFreed(host: ExtensionHost) {
  const now = new Set<string>();
  for (const cmd of host.commands.list()) for (const k of shortcutSpecs(host, cmd.id)) now.add(keyOf(host, k));
  const freed = new Map<string, Chord>();
  for (const id of Object.keys(host.shortcutOverrides)) {
    for (const k of defaultShortcutSpecs(host, id)) {
      try {
        const c = parseChord(k, host.mac);
        if (!now.has(chordKey(c))) freed.set(chordKey(c), c);
      } catch {
        /* ignore */
      }
    }
  }
  host.released = [...freed.values()];
}

/** The shortcuts a command has now, as written in `keybinding` (`Mod+Shift+K`): the person's own, else the command's. */
export function shortcutSpecs(host: ExtensionHost, id: string): string[] {
  const own = host.shortcutOverrides[id];
  return own ? [...own] : defaultShortcutSpecs(host, id);
}

/** The shortcuts the command came with. A command whose key the browser handles itself (Cut, Copy) reports it as a hint here. */
export function defaultShortcutSpecs(host: ExtensionHost, id: string): string[] {
  const cmd = host.command(id);
  if (!cmd) return [];
  return cmd.keybinding === undefined ? [] : Array.isArray(cmd.keybinding) ? [...cmd.keybinding] : [cmd.keybinding];
}

/**
 * Give a command shortcuts of the person's own, replacing its defaults (an empty list takes them all away), or
 * `null` to go back to the defaults. A shortcut another command uses is taken from that command, so each key does
 * one thing. Throws for a shortcut that cannot be parsed.
 */
export function setShortcuts(host: ExtensionHost, id: string, specs: string[] | null) {
  if (!host.command(id)) throw new Error(`No command ${id}`);
  if (specs === null) {
    delete host.shortcutOverrides[id];
    host.overrideChords.delete(id);
  } else {
    const chords = specs.map((k) => parseChord(k, host.mac));
    const taken = new Set(chords.map(chordKey));
    // Take each chord away from every other command that has it.
    for (const other of host.commands.list()) {
      if (other.id === id) continue;
      const have = shortcutSpecs(host, other.id);
      const keep = have.filter((k) => !taken.has(keyOf(host, k)));
      if (keep.length !== have.length) {
        for (const k of have) {
          if (!taken.has(keyOf(host, k))) continue;
          // Whatever `other` took the key from is now at the mercy of this command instead.
          for (const r of host.taken) if (r.by === other.id && r.chord === keyOf(host, k)) r.by = id;
          host.taken.push({ chord: keyOf(host, k), spec: k, from: other.id, by: id });
        }
        host.shortcutOverrides[other.id] = keep;
        host.overrideChords.set(other.id, keep.map((k) => parseChord(k, host.mac)));
      }
    }
    host.shortcutOverrides[id] = [...specs];
    host.overrideChords.set(id, chords);
  }
  giveBackTaken(host, id);
  releaseFreed(host);
  for (const fn of host.shortcutListeners) fn();
}

/**
 * A command that no longer uses a key it took from another command gives it back (resetting it, or choosing other keys),
 * so the key is not left with nothing, which would make it do nothing at all. A key the person cleared on purpose stays free.
 */
export function giveBackTaken(host: ExtensionHost, by: string) {
  const now = new Set(shortcutSpecs(host, by).map((k) => keyOf(host, k)));
  const given = new Set<string>();
  host.taken = host.taken.filter((t) => {
    if (t.by !== by || now.has(t.chord)) return true;
    // The first command that lost the key gets it, and only if nobody else has it by now.
    if (!given.has(t.chord) && !host.commands.list().some((c) => shortcutSpecs(host, c.id).some((k) => keyOf(host, k) === t.chord))) {
      given.add(t.chord);
      const have = shortcutSpecs(host, t.from);
      const next = [...have, t.spec];
      const defaults = defaultShortcutSpecs(host, t.from);
      const same = next.length === defaults.length && next.every((k) => defaults.some((d) => keyOf(host, d) === keyOf(host, k)));
      if (same) {
        delete host.shortcutOverrides[t.from];
        host.overrideChords.delete(t.from);
      } else {
        host.shortcutOverrides[t.from] = next;
        host.overrideChords.set(t.from, next.map((k) => parseChord(k, host.mac)));
      }
    }
    return false;
  });
}

/**
 * Replace every shortcut the person chose (what was saved earlier), and the record of which command took which key from
 * which (`shortcutTaken`). Unreadable shortcuts are skipped, and so is a record the saved shortcuts no longer match.
 */
export function loadShortcutOverrides(host: ExtensionHost, saved: Record<string, string[]> | null | undefined, taken?: TakenKey[] | null) {
  host.shortcutOverrides = {};
  host.overrideChords.clear();
  host.taken = [];
  for (const [id, specs] of Object.entries(saved ?? {})) {
    if (!Array.isArray(specs)) continue;
    try {
      const chords = specs.map((k) => parseChord(String(k), host.mac));
      host.shortcutOverrides[id] = specs.map(String);
      host.overrideChords.set(id, chords);
    } catch {
      /* a shortcut that no longer parses is dropped */
    }
  }
  if (Array.isArray(taken)) {
    for (const t of taken) {
      if (!t || typeof t.chord !== "string" || typeof t.spec !== "string" || typeof t.from !== "string" || typeof t.by !== "string") continue;
      if (host.shortcutOverrides[t.by]?.some((k) => keyOf(host, k) === t.chord)) host.taken.push({ chord: t.chord, spec: t.spec, from: t.from, by: t.by });
    }
  }
  releaseFreed(host);
  for (const fn of host.shortcutListeners) fn();
}

/** The commands that use this shortcut now (the person's own or default), so a dialog can warn before taking it. */
export function shortcutUsers(host: ExtensionHost, spec: string, except?: string): Command[] {
  const want = keyOf(host, spec);
  return host.commands.list().filter((c) => c.id !== except && shortcutSpecs(host, c.id).some((k) => keyOf(host, k) === want));
}

export function keyOf(host: ExtensionHost, spec: string): string {
  try {
    return chordKey(parseChord(spec, host.mac));
  } catch {
    return spec;
  }
}

/**
 * Run the command bound to this key event, if any. Skips events something else already handled
 * (the editor's own shortcuts). In a text field (`textField: true`), keys without ⌘/Ctrl/Alt and
 * the field's own editing shortcuts (⌘Z, ⌘A, ⌘C…) stay with the field. Returns true when a command ran.
 */
export function handleKey(host: ExtensionHost, e: KeyLike & { defaultPrevented?: boolean; isComposing?: boolean; preventDefault(): void }, opts: { textField?: boolean } = {}): boolean {
  if (e.defaultPrevented || e.isComposing) return false;
  if (opts.textField) {
    const mod = e.metaKey || e.ctrlKey;
    if (!mod && !e.altKey) return false;
    if (mod && !e.altKey && FIELD_KEYS.test(e.key.toLowerCase())) return false;
  }
  // A shortcut the person chose wins over every default.
  for (const cmd of host.commands.list()) {
    const own = host.overrideChords.get(cmd.id);
    if (!own?.some((c) => matches(c, e)) || !host.enabled(cmd)) continue;
    e.preventDefault();
    void host.execute(cmd.id).catch(() => undefined);
    return true;
  }
  // The first registration of a shortcut wins: built-ins register first, so extensions can't take ⌘B.
  for (const cmd of host.commands.list()) {
    if (host.overrideChords.has(cmd.id)) continue;
    const chords = host.chords.get(cmd);
    if (!chords?.some((c) => matches(c, e)) || !host.enabled(cmd)) continue;
    e.preventDefault();
    void host.execute(cmd.id).catch(() => undefined);
    return true;
  }
  // A key the person took away from a command and gave to nothing does nothing.
  if (host.released.some((c) => matches(c, e))) {
    e.preventDefault();
    return true;
  }
  return false;
}
