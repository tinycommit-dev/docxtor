// Icons for the View commands: the layouts, zoom to a page or several, folding and the outline strip's buttons (24px
// grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const VIEW_ICONS = {
  /** A sheet with lines of text. */
  printLayout: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M8.5 8h7M8.5 12h7M8.5 16h4"/>'),
  /** Text with open ends: one sheet that carries on. */
  pageless: wrap('<path d="M5 8V5M19 8V5M5 19v-3M19 19v-3"/><path d="M5 11v2M19 11v2"/><path d="M8.5 7h7M8.5 12h7M8.5 17h7"/>'),
  /** An open book. */
  reading: wrap('<path d="M12 7C10 5.6 7.5 5 4 5v13c3.5 0 6 .6 8 2 2-1.4 4.5-2 8-2V5c-3.5 0-6 .6-8 2Z"/><path d="M12 7v13"/>'),
  /** Lines at different depths. */
  outlineView: wrap('<path d="M4 6h16M8 12h12M12 18h8"/>'),
  /** One whole page with its corners marked. */
  zoomPage: wrap('<rect x="7" y="3.5" width="10" height="17" rx="1.5"/><path d="M3 8V4.5A1.5 1.5 0 0 1 4.5 3M21 8V4.5A1.5 1.5 0 0 0 19.5 3M3 16v3.5A1.5 1.5 0 0 0 4.5 21M21 16v3.5a1.5 1.5 0 0 1-1.5 1.5"/>'),
  /** Pages side by side. */
  zoomPages: wrap('<rect x="3" y="5" width="8" height="14" rx="1.5"/><rect x="13" y="5" width="8" height="14" rx="1.5"/><path d="M5.5 9h3M15.5 9h3M5.5 12h3M15.5 12h3"/>'),
  /** The menu bar folded away: a window with its top row small. */
  compact: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M3 9h18"/><path d="M6.5 6.5h.01"/>'),
  /** A pane with headings beside the page. */
  navigation: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M9 4v16"/><path d="M5.2 9h1.6M5.2 13h1.6M5.2 17h1.6"/>'),
  /** Arrow out to the left. */
  promote: wrap('<path d="M19 12H5M11 6l-6 6 6 6"/>'),
  /** Arrow in to the right. */
  demote: wrap('<path d="M5 12h14M13 6l6 6-6 6"/>'),
  /** An arrow into a body line. */
  demoteBody: wrap('<path d="M4 12h9M9 7l5 5-5 5"/><path d="M18 6v12"/>'),
  moveUp: wrap('<path d="M12 19V5M6 11l6-6 6 6"/>'),
  moveDown: wrap('<path d="M12 5v14M6 13l6 6 6-6"/>'),
  /** A box with a plus. */
  expand: wrap('<rect x="4" y="4" width="16" height="16" rx="2.5"/><path d="M12 8.5v7M8.5 12h7"/>'),
  /** A box with a minus. */
  collapse: wrap('<rect x="4" y="4" width="16" height="16" rx="2.5"/><path d="M8.5 12h7"/>'),
  /** One whole line, then short dashes. */
  firstLine: wrap('<path d="M4 7h16"/><path d="M4 12h2.5M9.5 12H12M4 17h2.5M9.5 17H12"/>'),
  /** A heading with an arrow beside it. */
  foldHeadings: wrap('<path d="M4 6h10M4 12h16M4 18h10"/><path d="m17 5 3 2-3 2"/>'),
};
