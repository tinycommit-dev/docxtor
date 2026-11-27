// Icons for the table commands beyond rows and columns (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const TABLE_ICONS = {
  /** A table with a dial in one cell. */
  tableProperties: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M3 9.5h18M9 9.5V20"/><circle cx="15" cy="14.75" r="1.6"/>'),
  /** A cell cut in two. */
  splitCells: wrap('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M12 5v14"/><path d="m8.5 10-2 2 2 2M15.5 10l2 2-2 2"/>'),
  /** Two tables with a gap between them. */
  splitTable: wrap('<rect x="3" y="3.5" width="18" height="6" rx="1.5"/><rect x="3" y="14.5" width="18" height="6" rx="1.5"/><path d="M3 12h2M9 12h2M13 12h2M19 12h2"/>'),
  /** A block of cells with one added. */
  insertCells: wrap('<rect x="3" y="4" width="12" height="12" rx="2"/><path d="M3 10h12M9 4v12"/><path d="M19 13v6M16 16h6"/>'),
  /** A block of cells with one taken out. */
  deleteCells: wrap('<rect x="3" y="4" width="12" height="12" rx="2"/><path d="M3 10h12M9 4v12"/><path d="M16 16h6"/>'),
  /** Rows of one height. */
  distributeRows: wrap('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9.3h18M3 14.7h18"/>'),
  /** A table with an arrow down beside it. */
  sortTable: wrap('<rect x="3" y="4" width="13" height="16" rx="2"/><path d="M3 9.5h13M3 14.5h13"/><path d="M20 7v10M18 15l2 2 2-2"/>'),
  /** A sigma. */
  formula: wrap('<path d="M18 5H7l6 7-6 7h11"/>'),
  /** Lines of text, then a table. */
  convertToTable: wrap('<path d="M3 7h5M3 12h5M3 17h5"/><rect x="11" y="4" width="10" height="16" rx="1.5"/><path d="M11 9.5h10M11 14.5h10M16 4v16"/>'),
  /** A table, then lines of text. */
  convertToText: wrap('<rect x="3" y="4" width="10" height="16" rx="1.5"/><path d="M3 9.5h10M3 14.5h10M8 4v16"/><path d="M16 7h5M16 12h5M16 17h5"/>'),
  /** A letter with a line of text running down from it. */
  textDirection: wrap('<path d="M9 4h6M12 4v10"/><path d="m8.5 17.5 3.5 3.5 3.5-3.5"/>'),
  /** A table with a smaller one in a cell. */
  nestedTable: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><rect x="9" y="10" width="9" height="7" rx="1"/><path d="M9 13.5h9M13.5 10v7"/>'),
};

export type TableIconName = keyof typeof TABLE_ICONS;
