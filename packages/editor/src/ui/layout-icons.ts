// Icons for the page layout commands: page borders and colour, drop caps, hyphens, odd and even pages and the two
// column presets (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const LAYOUT_ICONS = {
  /** A page with a second outline inside it. */
  pageBorders: wrap('<rect x="4" y="3" width="16" height="18" rx="2"/><rect x="7.5" y="6.5" width="9" height="11" rx="1" stroke-dasharray="2 2"/>'),
  /** A page struck through with diagonal shading. */
  pageColor: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="m5 13 8-8M5 19 17 7M11 21l8-8"/>'),
  /** A tall first letter beside lines of text. */
  dropCap: wrap('<rect x="3" y="4" width="8" height="9" rx="1.5"/><path d="M14 5h7M14 9h7M14 13h7M3 17h18M3 21h12"/>'),
  /** A word with a dotted break where it may split. */
  optionalHyphen: wrap('<path d="M3 14h5M16 14h5"/><path d="M12 5v14" stroke-dasharray="2 2.4"/>'),
  /** Two halves of a word held together by a hyphen in a box. */
  nonBreakingHyphen: wrap('<path d="M3 12h4M17 12h4"/><rect x="8.5" y="8" width="7" height="8" rx="1.5"/><path d="M10.5 12h3"/>'),
  /** Two pages side by side, each with a mark at its outer top corner. */
  oddEven: wrap('<rect x="3" y="4" width="8" height="16" rx="1.5"/><rect x="13" y="4" width="8" height="16" rx="1.5"/><path d="M5.5 8h3M15.5 8h3M5.5 12h3M15.5 12h3"/>'),
  /** A narrow column and a wide one. */
  columnsLeft: wrap('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M6 8h2M6 12h2M6 16h2M12 8h6M12 12h6M12 16h6"/>'),
  /** A wide column and a narrow one. */
  columnsRight: wrap('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M6 8h6M6 12h6M6 16h6M16 8h2M16 12h2M16 16h2"/>'),
  /** A page with a number in its top corner. */
  pageNumber: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M13.5 6.5h3M13.5 8.5h3"/><path d="M8 12h8M8 15.5h8"/>'),
  /** A page with a number on its side. */
  pageNumberMargin: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M2.5 10.5v3"/><path d="M9 8h6M9 12h6M9 16h6"/>'),
  /** A page with the number at the foot. */
  pageNumberBottom: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 7h6M9 10.5h6"/><path d="M10.5 17h3"/>'),
  /** A page with the number at the head. */
  pageNumberTop: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M10.5 7h3"/><path d="M9 11h6M9 14.5h6"/>'),
  /** A page with a number inside the text. */
  pageNumberHere: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 16h6"/><path d="M10.5 12h3"/>'),
  /** A page with a crossed-out number. */
  pageNumberRemove: wrap('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 9l6 6M15 9l-6 6"/>'),
};
