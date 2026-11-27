// Icons for paragraph direction, distributed text and tab stops (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const PARAGRAPH_ICONS = {
  /** A pilcrow with an arrow that runs left to right under it. */
  textLtr: wrap('<path d="M12 3.5H9.5a3.5 3.5 0 0 0 0 7H12"/><path d="M12 3.5v11M16 3.5v11"/><path d="M4 19.5h16M17 16.5l3 3-3 3"/>'),
  /** The same pilcrow facing the other way, with the arrow running right to left. */
  textRtl: wrap('<path d="M12 3.5h2.5a3.5 3.5 0 0 1 0 7H12"/><path d="M12 3.5v11M8 3.5v11"/><path d="M20 19.5H4M7 16.5l-3 3 3 3"/>'),
  /** Lines that fill the width, with a stop at each end. */
  alignDistribute: wrap('<path d="M4 4v16M20 4v16"/><path d="M8 8h8M8 12h8M8 16h8"/>'),
  /** A ruler line with a tab stop on it and the arrow of a tab. */
  tabStops: wrap('<path d="M3 18h18"/><path d="M8 18v-3M13 18v-3M18 18v-3"/><path d="M4 8h12"/><path d="m13 5 3 3-3 3"/><path d="M19 5v6"/>'),
};

export type ParagraphIconName = keyof typeof PARAGRAPH_ICONS;
