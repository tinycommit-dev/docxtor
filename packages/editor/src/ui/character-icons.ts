// Icons for the Font dialog, changing case and hidden text (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const CHARACTER_ICONS = {
  /** A large and a small A: the typeface and its size. */
  font: wrap('<path d="m3 18 5.5-13L14 18"/><path d="M5 13.5h7"/><path d="m15.5 18 2.7-6.2L20.5 18"/><path d="M16.6 16h3.2"/>'),
  /** A capital and a small letter with a line under them that runs both ways. */
  changeCase: wrap('<path d="m3 14 4-9 4 9"/><path d="M4.6 10.5h4.8"/><circle cx="17" cy="11.5" r="2.5"/><path d="M19.5 9v5"/><path d="M4 19h16"/><path d="m6.5 17-2.5 2 2.5 2M17.5 17l2.5 2-2.5 2"/>'),
  /** An eye with a line through it. */
  hiddenText: wrap('<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z"/><circle cx="12" cy="12" r="2.8"/><path d="M4 20 20 4"/>'),
};

export type CharacterIconName = keyof typeof CHARACTER_ICONS;
