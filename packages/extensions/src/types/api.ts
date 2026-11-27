// The document, UI and state APIs, cautions and commands
import type { Disposable } from "./extension.ts";
import type { Engine, FormatState, MPos, Op, OpResult } from "@docxtor/core";

export interface SelectionInfo {
  /** Start and end in document order. Equal when the selection is just a caret. */
  from: MPos;
  to: MPos;
  /** The selected text (empty for a caret). */
  text: string;
}

export interface DocumentApi {
  /** The open document's engine, for anything the API doesn't cover. Null when nothing is open. */
  readonly engine: Engine | null;
  /** The file name, e.g. `Report.docx`. */
  readonly name: string | null;
  /** Apply edit ops as one undoable step (the same ops the AI tools and the CLI use). */
  exec(ops: Op | Op[]): OpResult[] | null;
  /** Replace the selection with Markdown (or insert it at the caret), as one undoable step. */
  insertMarkdown(markdown: string): void;
  /** Run one of the built-in document tools (`search`, `replace_text`, …) and return its output. */
  tool(name: string, input: Record<string, unknown>): string;
  /** The real selection: the one the toolbar shows. With several ranges selected (Edit, Select all matching text) the others are not in it: see `selections()`. */
  selection(): SelectionInfo | null;
  /** Every selected range in document order, the real selection included. One entry unless several ranges are selected. */
  selections(): SelectionInfo[];
  /** Select a range (or place the caret when `to` is omitted) and scroll it into view. This ends any multiple selection. */
  select(from: MPos, to?: MPos): void;
  /**
   * Select several ranges at once. The first becomes the real selection (what `selection()` returns); the others are drawn
   * the same way and follow it. Ranges that overlap an earlier one are dropped. Formatting, deleting, typing, copying and
   * pasting then apply to all of them as one undoable step. Any caret move or click by the person ends it.
   */
  selectRanges(ranges: { from: MPos; to?: MPos }[]): void;
  /** Formatting under the caret. */
  format(): FormatState | null;
  /** The body as Markdown. */
  markdown(): string;
  /** The body as plain text. */
  text(): string;
  /** Scroll a paragraph into view. */
  reveal(paragraph: number): void;
  focus(): void;
  onDidChange(fn: () => void): Disposable;
  onDidChangeSelection(fn: (format: FormatState | null) => void): Disposable;
  /** A document was opened (or a new one created). */
  onDidOpen(fn: () => void): Disposable;
}

export interface UiApi {
  /** A short notice that disappears by itself, with a button (Undo) when `action` is given. */
  notify(message: string, opts?: { action?: { label: string; run: () => void } }): void;
  confirm(message: string, opts?: { title?: string; confirm?: string; danger?: boolean }): Promise<boolean>;
  prompt(title: string, opts?: { value?: string; placeholder?: string; confirm?: string }): Promise<string | null>;
  /** Let the user choose from a filterable list. Resolves to the chosen item, or null. */
  pick<T extends PickItem>(items: T[], opts?: { placeholder?: string; label?: string }): Promise<T | null>;
}

export interface PickItem {
  label: string;
  /** Shown after the label, quieter. */
  detail?: string;
  /** Shown at the right edge, e.g. a shortcut. */
  hint?: string;
}

export interface StateApi {
  get<T>(key: string, fallback: T): T;
  set(key: string, value: unknown): void;
}

// ---------------------------------------------------------------------------
// Commands and where they appear
// ---------------------------------------------------------------------------

/** Text, or a function of the command's arguments for names and counts ("Remove “{name}”?"). */
export type CautionText = string | ((...args: unknown[]) => string);

/**
 * A warning for a risky action (DESIGN.md, Risky actions). `ask` asks first and says what changes: for what can't be
 * undone, or changes the whole document (a header on every page). `undo` does it, then offers Undo: for a local
 * change the person sees happen (a table deleted). Small edits get neither.
 */
export interface Caution {
  /** The command it is for, or the name a dialog's button asks with (`host.ask(id)`). */
  id: string;
  kind: "ask" | "undo";
  /** `ask`: the question ("Remove the header?"). `undo`: what was done ("Table deleted."). */
  title: CautionText;
  /** `ask`: what changes, in plain words ("It goes from every page of this section."). */
  impact?: CautionText;
  /** `ask`: the button, naming the action ("Remove header"), never "OK". */
  confirm?: CautionText;
  /** It can't be undone: the question says so. */
  permanent?: boolean;
  /** `ask`: draw the button as dangerous (the default); false for a big change that keeps a way back (restoring a version). */
  danger?: boolean;
}

export interface CommandOptions {
  /** Sentence case, as shown in the command palette and menus: "Insert table of contents". */
  title: string;
  /** Groups commands in the palette: "Edit", "Insert", "Format", "File", "View"… */
  category?: string;
  /**
   * Keyboard shortcut(s): `Mod` is ⌘ on macOS and Ctrl elsewhere, e.g. `Mod+Shift+L`, `Alt+/`.
   * When two commands claim the same shortcut, the one registered first wins, so the editor's
   * own shortcuts (⌘B, ⌘Z…) can't be taken over.
   */
  keybinding?: string | string[];
  /** When it returns false the command is hidden from the palette and its shortcut does nothing. */
  when?: () => boolean;
  /** List in the command palette (default true). */
  palette?: boolean;
  /**
   * Let the AI assistant run it (default false). Only for commands that need no further input:
   * nothing that opens a dialog or a picker.
   */
  ai?: boolean;
  /**
   * A shortcut the browser handles itself (`Mod+C` for copy): shown beside the command in menus and
   * the palette, but not bound, so the native behaviour stays.
   */
  shortcutHint?: string;
  /** An icon id (see `icons`) for menus, or SVG markup. */
  icon?: string;
  /** For settings that are on or off ("Show ruler"): menus show a check mark while it returns true. */
  toggled?: () => boolean;
}

/**
 * What `execute` answers: the command ran (`value` is what it returned), the person was asked and said no, or the command
 * is not available right now (its `when` says no, for example in a read-only document). Nothing is thrown for the last
 * two; a command that does not exist, or one that fails, still rejects.
 */
export type CommandResult<T = unknown> = { status: "done"; value: T } | { status: "declined" } | { status: "unavailable" };

export interface CommandsApi {
  register(id: string, run: (...args: unknown[]) => unknown, opts: CommandOptions): Disposable;
  /** The only door to a command: checks it is available, asks if it must, runs it. See `CommandResult`. */
  execute<T = unknown>(id: string, ...args: unknown[]): Promise<CommandResult<T>>;
  /** Commands available right now (their `when` allows them). */
  list(): { id: string; title: string; category?: string; ai: boolean }[];
}

export interface Command extends CommandOptions {
  id: string;
  run: (...args: unknown[]) => unknown;
  /** The extension that registered it. */
  extension: string;
}

