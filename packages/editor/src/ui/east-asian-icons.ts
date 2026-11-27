// Icons for the Asian layout commands (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const EAST_ASIAN_ICONS = {
  /** Small marks over a larger character. */
  phoneticGuide: wrap('<path d="M7 5.5h2M12 5.5h2M17 5.5h2"/><path d="M5 11h14M12 11v9M8.5 14.5h7"/>'),
  /** A character in a circle. */
  encloseCharacters: wrap('<circle cx="12" cy="12" r="8.5"/><path d="M8 10.5h8M12 8v8M9.5 14l5 0"/>'),
  /** Two short lines inside a pair of brackets. */
  twoLinesInOne: wrap('<path d="M7 4.5H5v15h2M17 4.5h2v15h-2"/><path d="M9.5 9.5h5M9.5 14.5h5"/>'),
  /** A vertical line of text with a short horizontal run in it. */
  horizontalInVertical: wrap('<path d="M12 3.5v5M12 15.5v5"/><rect x="7.5" y="9.5" width="9" height="5" rx="1"/>'),
  /** A line of text with a bar where it may not break. */
  lineBreakRules: wrap('<path d="M3 7h12M3 12h8M3 17h5"/><path d="M17 11v6M14 14h6"/>'),
};
