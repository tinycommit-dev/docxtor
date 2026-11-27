// Keyboard shortcuts written as "Mod+Shift+P" or "Alt+/": parsing, matching key events, and display.

/** The parts of a KeyboardEvent a shortcut looks at. */
export interface KeyLike {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export interface Chord {
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  /** Lower-case key: "p", "/", "enter", "f5"… */
  key: string;
}

const ALIASES: Record<string, string> = { esc: "escape", return: "enter", space: " ", plus: "+", del: "delete", up: "arrowup", down: "arrowdown", left: "arrowleft", right: "arrowright" };

/** Parse "Mod+Shift+P". `Mod` means ⌘ on macOS and Ctrl elsewhere. Throws on an unknown modifier. */
export function parseChord(spec: string, mac: boolean): Chord {
  const parts = spec.split(/\+(?!$)/).map((p) => p.trim());
  const c: Chord = { meta: false, ctrl: false, alt: false, shift: false, key: "" };
  for (const [i, raw] of parts.entries()) {
    const p = raw.toLowerCase();
    if (i === parts.length - 1) {
      c.key = ALIASES[p] ?? p;
      break;
    }
    if (p === "mod") c[mac ? "meta" : "ctrl"] = true;
    else if (p === "cmd" || p === "meta" || p === "super") c.meta = true;
    else if (p === "ctrl" || p === "control") c.ctrl = true;
    else if (p === "alt" || p === "option" || p === "opt") c.alt = true;
    else if (p === "shift") c.shift = true;
    else throw new Error(`Unknown modifier "${raw}" in shortcut "${spec}"`);
  }
  if (!c.key) throw new Error(`Shortcut "${spec}" has no key`);
  return c;
}

const CODE_KEYS: Record<string, string> = {
  Slash: "/", Backslash: "\\", Period: ".", Comma: ",", Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]",
  Minus: "-", Equal: "=", Backquote: "`", Space: " ",
};

/** The key a physical key stands for, ignoring layout changes made by Alt/Option and Shift. */
function codeKey(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit\d$/.test(code)) return code.slice(5);
  return CODE_KEYS[code] ?? null;
}

export function matches(chord: Chord, e: KeyLike): boolean {
  if (e.metaKey !== chord.meta || e.ctrlKey !== chord.ctrl || e.altKey !== chord.alt || e.shiftKey !== chord.shift) return false;
  const key = e.key.toLowerCase();
  return key === chord.key || codeKey(e.code) === chord.key;
}

/** How the shortcut is written on this platform: "⌘⇧P" on macOS, "Ctrl+Shift+P" elsewhere. */
export function formatChord(spec: string, mac: boolean): string {
  const c = parseChord(spec, mac);
  const names: Record<string, string> = { escape: "Esc", enter: mac ? "↩" : "Enter", " ": "Space", arrowup: "↑", arrowdown: "↓", arrowleft: "←", arrowright: "→" };
  const key = names[c.key] ?? (c.key.length === 1 ? c.key.toUpperCase() : c.key[0].toUpperCase() + c.key.slice(1));
  if (mac) return `${c.ctrl ? "⌃" : ""}${c.alt ? "⌥" : ""}${c.shift ? "⇧" : ""}${c.meta ? "⌘" : ""}${key}`;
  return [c.ctrl && "Ctrl", c.meta && "Win", c.alt && "Alt", c.shift && "Shift", key].filter(Boolean).join("+");
}

const MODIFIER_KEYS = new Set(["control", "shift", "alt", "meta", "altgraph", "os", "capslock", "dead"]);

/**
 * The shortcut a key press stands for, written as `Mod+Shift+K` (`Mod` is ⌘ on macOS and Ctrl elsewhere), for the dialog
 * where people choose their own. Null for a press of a modifier alone, and for a key with no modifier that would only type
 * (letters, digits, punctuation, Enter, Space...): function keys, Escape-less arrows with a modifier and the like are fine.
 */
export function specFromEvent(e: KeyLike, mac: boolean): string | null {
  const raw = e.key;
  if (MODIFIER_KEYS.has(raw.toLowerCase())) return null;
  const physical = codeKey(e.code);
  let key = physical ?? raw.toLowerCase();
  const names: Record<string, string> = { " ": "Space", arrowup: "Up", arrowdown: "Down", arrowleft: "Left", arrowright: "Right", escape: "Escape", enter: "Enter", tab: "Tab", backspace: "Backspace", delete: "Delete", "+": "Plus" };
  key = names[key] ?? (key.length === 1 ? key.toUpperCase() : /^f\d{1,2}$/.test(key) ? key.toUpperCase() : key[0].toUpperCase() + key.slice(1));
  const mod = mac ? e.metaKey : e.ctrlKey;
  const other = mac ? e.ctrlKey : e.metaKey;
  const functionKey = /^F\d{1,2}$/.test(key);
  if (!mod && !other && !e.altKey && !functionKey) return null;
  return [mod && "Mod", other && (mac ? "Ctrl" : "Meta"), e.altKey && "Alt", e.shiftKey && "Shift", key].filter(Boolean).join("+");
}
