// Icons for smart chips, the @ menu and building blocks (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const CHIP_ICONS = {
  smartChips: wrap('<rect x="2.5" y="7" width="19" height="10" rx="5"/><circle cx="8" cy="12" r="1.6"/><path d="M12 12h6"/>'),
  chipPerson: wrap('<circle cx="12" cy="8.5" r="3.5"/><path d="M5 20c.6-3.6 3.4-5.5 7-5.5s6.4 1.9 7 5.5"/>'),
  chipFile: wrap('<path d="M6.5 3h7l4.5 4.5V19a2 2 0 0 1-2 2h-9.5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M13.5 3v4.5H18"/><path d="M9 12.5h6M9 16h4"/>'),
  chipPlace: wrap('<path d="M12 21s-6.5-5.7-6.5-11a6.5 6.5 0 0 1 13 0c0 5.3-6.5 11-6.5 11z"/><circle cx="12" cy="10" r="2.3"/>'),
  chipDropdown: wrap('<rect x="2.5" y="7" width="19" height="10" rx="5"/><path d="m14 11.2 2 2 2-2"/><path d="M6.5 12H11"/>'),
  atSign: wrap('<circle cx="12" cy="12" r="3.6"/><path d="M15.6 8.5v5.2c0 1.4 1 2.3 2.2 2.3 1.4 0 2.7-1.7 2.7-4A8.5 8.5 0 1 0 16 19.6"/>'),
  buildingBlocks: wrap('<rect x="3.5" y="3.5" width="7.5" height="7.5" rx="1.8"/><rect x="13" y="3.5" width="7.5" height="7.5" rx="1.8"/><rect x="3.5" y="13" width="7.5" height="7.5" rx="1.8"/><path d="M16.75 13v7.5M13 16.75h7.5"/>'),
  mail: wrap('<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="m3.5 7.5 8.5 6 8.5-6"/>'),
  externalLink: wrap('<path d="M14 4h6v6"/><path d="M20 4 11 13"/><path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4"/>'),
  unlink: wrap('<path d="M10 14a4.5 4.5 0 0 0 6.4 0l2.8-2.8a4.5 4.5 0 0 0-6.4-6.4l-1 1"/><path d="M14 10a4.5 4.5 0 0 0-6.4 0l-2.8 2.8a4.5 4.5 0 0 0 6.4 6.4l1-1"/><path d="m4 4 16 16"/>'),
} as const;
