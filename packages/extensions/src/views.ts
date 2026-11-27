// View modes: the ways the document can be shown (print layout, pageless, reading, outline, draft). Pure helpers, no DOM
// work, tested in Node. The editor applies a mode; extensions add their own with `ctx.views.register`.

/** What a view's `enter` is given. */
export interface ViewHandle {
  /** The editor's page area, which carries the view's `className` while the view is on. */
  readonly root: HTMLElement;
}

/**
 * A way to show the document. Docxtor ships Print layout, Pageless, Reading, Outline and Draft; an extension can add more
 * (a two-column proofing layout, a typewriter view…). One view is on at a time. The commands and the menu line for a
 * view are made by the editor from this description, so a contributed view appears under View, Layout and in the
 * command palette without more work.
 */
export interface ViewMode {
  /** Unique and stable: `acme.typewriter`. Docxtor's are `print`, `pageless`, `reading`, `outline` and `draft`. */
  id: string;
  /** What the menu and the command palette say (English; it is translated like other text). */
  title: string;
  /** An icon id from the icon set (the icon of a command, or one the extension registers with `ctx.icons`). */
  icon?: string;
  /** One line for the command palette. */
  description?: string;
  /** Where it sits in the list, lowest first (Docxtor's are 10, 20, 30, 40, 50). */
  order?: number;
  /** A key for the view's command (`Mod+Alt+O`). */
  keybinding?: string;
  /**
   * `pages` (the default) draws sheets with page breaks, headers and footers. `continuous` is one unbroken sheet: no
   * page breaks, headers, footers or page borders, and sections run on without starting a new page.
   */
  layout?: "pages" | "continuous";
  /** For a continuous layout: `page` keeps the text as wide as the first section's text, `window` fits the text to the window. */
  width?: "page" | "window";
  /** For `window` width: the widest the text column may be, in pixels at 100% zoom (default 760). */
  maxWidth?: number;
  /** For a continuous layout: the side margins in pixels (the first section's when left out), and the left one on its own when it differs. */
  margin?: number;
  marginLeft?: number;
  /** For a continuous layout: the top and bottom margins in pixels. */
  marginTop?: number;
  /** The text can't be changed while the view is on. */
  readOnly?: boolean;
  /** A class put on the editor's root while the view is on, for the view's own CSS (every rule scoped under `.docxtor`). */
  className?: string;
  /** Runs when the view starts, after the layout has changed. Return a function that undoes what it did; it runs when the view ends. */
  enter?(view: ViewHandle): void | (() => void);
}

/** The views in the order they are offered: by `order`, then in the order they were registered. */
export function sortViews<T extends { order?: number }>(views: readonly T[]): T[] {
  return views.map((v, i) => ({ v, i })).sort((a, b) => (a.v.order ?? 100) - (b.v.order ?? 100) || a.i - b.i).map((x) => x.v);
}

/** The text column of a continuous layout, in pixels: what `width` and `maxWidth` ask for in a window `available` pixels wide. */
export function columnWidth(view: Pick<ViewMode, "width" | "maxWidth">, pageText: number, available: number): number {
  if (view.width !== "window") return pageText;
  const max = view.maxWidth && view.maxWidth > 0 ? view.maxWidth : 760;
  return Math.max(240, Math.min(max, Math.floor(available)));
}
