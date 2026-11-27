// Icons for sorting (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const LIST_ICONS = {
  // Lines getting longer beside an arrow that points down: ordered from the top.
  sort: wrap('<path d="M13 6h8M13 12h6M13 18h4"/><path d="M6 4v16M3 17l3 3 3-3"/>'),
};
