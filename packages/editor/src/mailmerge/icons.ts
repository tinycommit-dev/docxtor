// Icons for the Mailings menu (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const MAIL_ICONS = {
  mmEnvelope: wrap('<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="m3.5 7.5 8.5 6 8.5-6"/>'),
  mmLabels: wrap('<rect x="3" y="4" width="8" height="6" rx="1.5"/><rect x="13" y="4" width="8" height="6" rx="1.5"/><rect x="3" y="14" width="8" height="6" rx="1.5"/><rect x="13" y="14" width="8" height="6" rx="1.5"/>'),
  mmRecipients: wrap('<circle cx="9" cy="8" r="3"/><path d="M3.5 19a5.5 5.5 0 0 1 11 0"/><path d="M16.5 6.5H21M17.5 11H21M17.5 15.5H21"/>'),
  mmField: wrap('<path d="m8.5 7-5 5 5 5"/><path d="m15.5 7 5 5-5 5"/><path d="m13 5-2 14"/>'),
  mmAddress: wrap('<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="M7 10h5M7 14h9"/>'),
  mmGreeting: wrap('<path d="M4 7h11M4 12h16"/><path d="M4 17h5"/><path d="m12.5 17-1 2.5"/>'),
  mmRules: wrap('<circle cx="6" cy="6" r="2.2"/><circle cx="6" cy="18" r="2.2"/><circle cx="18" cy="12" r="2.2"/><path d="M6 8.2v7.6M6 12h9.8"/>'),
  mmPreview: wrap('<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="2.8"/>'),
  mmFirst: wrap('<path d="M6 5v14"/><path d="M18 6.5 10 12l8 5.5z"/>'),
  mmPrevious: wrap('<path d="M16 6.5 8 12l8 5.5z"/>'),
  mmNext: wrap('<path d="m8 6.5 8 5.5-8 5.5z"/>'),
  mmLast: wrap('<path d="M18 5v14"/><path d="m6 6.5 8 5.5-8 5.5z"/>'),
  mmFinish: wrap('<path d="M3 6h5l4 6"/><path d="M3 18h5l4-6"/><path d="M12 12h9"/><path d="m18 9 3 3-3 3"/>'),
  mmMatch: wrap('<rect x="3" y="4" width="7" height="6" rx="1.5"/><rect x="14" y="14" width="7" height="6" rx="1.5"/><path d="M10 7h2a2 2 0 0 1 2 2v5"/>'),
  mmSend: wrap('<path d="M21 12V7.5A2.5 2.5 0 0 0 18.5 5h-13A2.5 2.5 0 0 0 3 7.5v9A2.5 2.5 0 0 0 5.5 19H12"/><path d="m3.5 7.5 8.5 6 8.5-6"/><path d="M16 18.5h5.5M19 15.5l3 3-3 3"/>'),
  mmDuplicates: wrap('<rect x="3" y="3.5" width="12" height="12" rx="2.5"/><path d="M9 20.5h9.5a2.5 2.5 0 0 0 2.5-2.5V8.5"/><path d="M7 8.5h4M7 11.5h2.5"/>'),
  mmRefresh: wrap('<path d="M20 11.5A8 8 0 1 0 17.7 17"/><path d="M20 4v7.5h-7.5"/>'),
};

export type MailIconName = keyof typeof MAIL_ICONS;
