// Icons for arranging objects: the stacking order, behind and in front of the text, groups, and drawing by hand
// (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const ARRANGE_ICONS = {
  bringToFront: wrap('<path d="M15 7V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/><rect x="9" y="9" width="12" height="12" rx="2"/>'),
  sendToBack: wrap('<path d="M21 13v6a2 2 0 0 1-2 2h-6a2 2 0 0 1-2-2v-2"/><rect x="3" y="3" width="12" height="12" rx="2"/>'),
  bringForward: wrap('<path d="M12 7V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v5a2 2 0 0 0 2 2h2"/><rect x="8" y="9" width="11" height="11" rx="2"/><path d="M19 2v4M17 4l2-2 2 2"/>'),
  sendBackward: wrap('<path d="M16 13v5a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2v-2"/><rect x="3" y="3" width="11" height="11" rx="2"/><path d="M19 2v4M17 4l2 2 2-2"/>'),
  behindText: wrap('<rect x="7.5" y="4.5" width="9" height="15" rx="2" stroke-dasharray="3 2.4"/><path d="M3 8.5h18M3 12h18M3 15.5h18"/>'),
  inFrontOfText: wrap('<rect x="7.5" y="4.5" width="9" height="15" rx="2"/><path d="M3 8.5h4.5M16.5 8.5H21M3 12h4.5M16.5 12H21M3 15.5h4.5M16.5 15.5H21"/>'),
  group: wrap('<rect x="3" y="3" width="18" height="18" rx="3" stroke-dasharray="3.4 2.6"/><rect x="7" y="7" width="5" height="5" rx="1.2"/><rect x="12" y="12" width="5" height="5" rx="1.2"/>'),
  ungroup: wrap('<rect x="3" y="3" width="8" height="8" rx="1.8"/><rect x="13" y="13" width="8" height="8" rx="1.8"/><path d="M14 6h3a3 3 0 0 1 3 3v1M10 18H7a3 3 0 0 1-3-3v-1"/>'),
  draw: wrap('<path d="M3 17c2.2-5 4.2 1.8 6.5-2.2"/><path d="m12.5 14.5 6-6 3 3-6 6-4 1z"/><path d="m16 11 3 3"/>'),
};

export type ArrangeIconName = keyof typeof ARRANGE_ICONS;
