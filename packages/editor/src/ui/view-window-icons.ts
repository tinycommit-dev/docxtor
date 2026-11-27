// Icons for the Draft view, Immersive, the Show items and the Window group of the View menu (24px grid, 1.75 stroke,
// like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const VIEW_WINDOW_ICONS = {
  /** Plain lines of text with a dotted break under them: no page around them. */
  draftView: wrap('<path d="M5 6h14M5 10h14M5 14h9"/><path d="M5 18.5h2M10 18.5h2M15 18.5h2"/>'),
  /** Lines of text with the middle one in a frame. */
  lineFocus: wrap('<path d="M5 5.5h14M5 18.5h14"/><rect x="4" y="9" width="16" height="6" rx="1.75"/>'),
  /** A mark with its corners drawn in: attention on one thing. */
  focusMode: wrap('<circle cx="12" cy="12" r="2.75"/><path d="M4 9V6.5A2.5 2.5 0 0 1 6.5 4H9M20 9V6.5A2.5 2.5 0 0 0 17.5 4H15M4 15v2.5A2.5 2.5 0 0 0 6.5 20H9M20 15v2.5a2.5 2.5 0 0 1-2.5 2.5H15"/>'),
  /** A square cut into cells. */
  gridlines: wrap('<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M4 9.33h16M4 14.67h16M9.33 4v16M14.67 4v16"/>'),
  /** The text area of a page, drawn with a dotted line. */
  textBoundaries: wrap('<rect x="5" y="4" width="14" height="16" rx="1.5" stroke-dasharray="2.2 2.6"/><path d="M8.5 9h7M8.5 12.5h7M8.5 16h4"/>'),
  /** A window with a line across it. */
  splitWindow: wrap('<rect x="3.5" y="4" width="17" height="16" rx="2.5"/><path d="M3.5 12h17"/>'),
  /** The same window with the line taken out. */
  removeSplit: wrap('<rect x="3.5" y="4" width="17" height="16" rx="2.5"/><path d="M8 12h1.5M11.25 12h1.5M14.5 12H16"/>'),
  /** A window that opens another one. */
  newWindow: wrap('<rect x="3.5" y="8" width="12.5" height="12" rx="2"/><path d="M8 4h9.5A2.5 2.5 0 0 1 20 6.5V16"/>'),
  /** Two pages next to each other, with lines on them. */
  sideBySide: wrap('<rect x="3" y="4" width="8" height="16" rx="1.5"/><rect x="13" y="4" width="8" height="16" rx="1.5"/><path d="M5.5 9h3M15.5 9h3M5.5 13h3M15.5 13h3"/>'),
  /** Two arrows that go up and down together. */
  syncScroll: wrap('<path d="M7 5v14M4 8l3-3 3 3M4 16l3 3 3-3"/><path d="M17 5v14M14 8l3-3 3 3M14 16l3 3 3-3"/>'),
  /** An arrow that goes round to where it began. */
  resetWindow: wrap('<path d="M4.5 12a7.5 7.5 0 1 0 2.3-5.4"/><path d="M4.5 4.5v4h4"/>'),
};
