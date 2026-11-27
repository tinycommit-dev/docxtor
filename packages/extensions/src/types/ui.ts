// Toolbar items, menus and panels
import type { Command } from "./api.ts";
import type { FormatState } from "@docxtor/core";

export interface ToolbarItem {
  id: string;
  /** The command to run on click. */
  command: string;
  args?: unknown[];
  /** Tooltip and accessible name (defaults to the command's title). */
  title?: string;
  /** An SVG icon (24×24 viewBox, `currentColor` strokes), or a short text label. */
  icon?: string;
  label?: string;
  /** Which toolbar group: `text`, `paragraph`, `insert`, or `extensions` (the default, at the end). */
  group?: "text" | "paragraph" | "insert" | "extensions";
  /** Show the button as pressed for the formatting under the caret. */
  isActive?: (format: FormatState | null) => boolean;
}

/**
 * A menu: one of the menu bar's (`file`, `edit`, `view`, `insert`, `format`, `tools`, `extensions`,
 * `help`), `context` (the right-click menu on the page), or the id of a submenu.
 */
export type MenuId = "file" | "edit" | "view" | "insert" | "format" | "tools" | "extensions" | "help" | "context" | (string & {});

export interface MenuItem {
  menu: MenuId;
  /**
   * Items with the same group sit together between separators. Groups are sorted by name, so
   * Docxtor's own use `1_…`, `2_…`; items without a group come last.
   */
  group?: string;
  /** Position within the group, lowest first (default: the order they were added). */
  order?: number;
  /** The command to run. Leave it out for an entry that opens a `submenu`. */
  command?: string;
  args?: unknown[];
  /** Open the submenu with this id: other items name it as their `menu`. */
  submenu?: string;
  /** Defaults to the command's title. Required for a submenu entry. */
  title?: string;
  /** An icon id or SVG markup; defaults to the command's icon. */
  icon?: string;
}

/** One line of a menu, resolved: what to show and whether it can be used right now. */
export interface MenuEntry {
  item: MenuItem;
  title: string;
  /** The shortcut, written for this platform (⌘B), or "". */
  shortcut: string;
  enabled: boolean;
  checked: boolean;
  /** The icon's SVG markup, if it has one. */
  icon?: string;
  command?: Command;
}

/** A view in a side panel (the app's navigator shows panels as tabs). */
export interface Panel {
  id: string;
  title: string;
  /** Lower comes first. Built-ins use 10, 20, 30. */
  order?: number;
  /**
   * A panel for a tool (the accessibility check, the dictionary) rather than a view of the document.
   * It has no tab until something shows it (`show(id)`, usually from a command); then it gets a tab
   * with a close button.
   */
  tool?: boolean;
  /**
   * Fill `el` (called once, when the panel is first shown). Return a function to clean up when the
   * panel is removed.
   */
  render(el: HTMLElement): void | (() => void);
  /** Called each time the panel becomes visible. */
  onShow?(): void;
}

// ---------------------------------------------------------------------------
// Formats
// ---------------------------------------------------------------------------

