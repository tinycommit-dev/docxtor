// Icons for styles and the Design menu (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const DESIGN_ICONS = {
  /** Lines of text and a capital A: the list of styles. */
  styles: wrap('<path d="M4 6.5h6M4 12h4M4 17.5h6"/><path d="m13.5 18 3.5-11 3.5 11"/><path d="M14.8 14.5h4.4"/>'),
  /** A capital A with a plus: a new style. */
  styleAdd: wrap('<path d="m3.5 19 4.5-12 4.5 12"/><path d="M5.2 14.5h5.6"/><path d="M18 5v6M15 8h6"/>'),
  /** A capital A with a pencil: change a style. */
  styleEdit: wrap('<path d="m3.5 19 4.5-12 4.5 12"/><path d="M5.2 14.5h5.6"/><path d="m14 18 .6-2.6 5-5a1.5 1.5 0 0 1 2.1 2.1l-5 5z"/>'),
  /** A capital A with a circle of arrows: bring a style up to date. */
  styleRefresh: wrap('<path d="m3.5 19 4.5-12 4.5 12"/><path d="M5.2 14.5h5.6"/><path d="M20 9.5A4.5 4.5 0 0 0 12.6 7"/><path d="m20.5 5.5-.5 4-4-.5"/>'),
  /** A magnifier over lines of text: what the text has and where it comes from. */
  reveal: wrap('<path d="M4 6h10M4 11h6M4 16h7"/><circle cx="16.5" cy="15.5" r="3.5"/><path d="m19 18 2.5 2.5"/>'),
  /** A window with three colour dots: a theme. */
  designThemes: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><circle cx="8.5" cy="9.5" r="1.2"/><circle cx="12.5" cy="9.5" r="1.2"/><circle cx="16.5" cy="9.5" r="1.2"/><path d="M7 15h10"/>'),
  /** A palette. */
  designColors: wrap('<path d="M12 3.5c-5 0-8.5 3.5-8.5 8 0 4.2 3 7 6.5 7 1.2 0 1.6-.8 1.2-1.6-.5-1 .1-2.3 1.4-2.3H15c3 0 5.5-1.2 5.5-4 0-4-3.6-7.1-8.5-7.1z"/><circle cx="8" cy="11" r="1" fill="currentColor"/><circle cx="12" cy="7.8" r="1" fill="currentColor"/><circle cx="16" cy="10.8" r="1" fill="currentColor"/>'),
  /** A capital A and a lower-case a: the heading and body fonts. */
  designFonts: wrap('<path d="m3 18 4.5-12L12 18"/><path d="M4.8 13.5h5.4"/><circle cx="17" cy="14.6" r="2.6"/><path d="M19.6 12v6"/>'),
  /** A page with a heading and lines: a style set. */
  styleSet: wrap('<rect x="4" y="3.5" width="16" height="17" rx="2.5"/><path d="M8 8h5M8 12h8M8 16h6"/>'),
  /** Lines with arrows above and below: space between paragraphs. */
  paragraphSpacing: wrap('<path d="M10 6.5h10M10 12h10M10 17.5h10"/><path d="M5 5v14"/><path d="m3 7 2-2 2 2M3 17l2 2 2-2"/>'),
  /** A box with a tick: keep this design for new documents. */
  setDefault: wrap('<rect x="4" y="4" width="16" height="16" rx="3"/><path d="m8.5 12.3 2.4 2.4 4.6-5"/>'),
};

export type DesignIconName = keyof typeof DESIGN_ICONS;
