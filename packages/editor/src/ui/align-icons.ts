// Icons for aligning and distributing objects, and for the Selection pane (24px grid, 1.75 stroke, like the rest of
// Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const ALIGN_ICONS = {
  alignObjectsLeft: wrap('<path d="M4 3v18"/><rect x="7" y="6" width="13" height="4" rx="1.2"/><rect x="7" y="14" width="8" height="4" rx="1.2"/>'),
  alignObjectsCenter: wrap('<path d="M12 3v18"/><rect x="5" y="6" width="14" height="4" rx="1.2"/><rect x="8" y="14" width="8" height="4" rx="1.2"/>'),
  alignObjectsRight: wrap('<path d="M20 3v18"/><rect x="4" y="6" width="13" height="4" rx="1.2"/><rect x="9" y="14" width="8" height="4" rx="1.2"/>'),
  alignObjectsTop: wrap('<path d="M3 4h18"/><rect x="6" y="7" width="4" height="13" rx="1.2"/><rect x="14" y="7" width="4" height="8" rx="1.2"/>'),
  alignObjectsMiddle: wrap('<path d="M3 12h18"/><rect x="6" y="5" width="4" height="14" rx="1.2"/><rect x="14" y="8" width="4" height="8" rx="1.2"/>'),
  alignObjectsBottom: wrap('<path d="M3 20h18"/><rect x="6" y="4" width="4" height="13" rx="1.2"/><rect x="14" y="9" width="4" height="8" rx="1.2"/>'),
  distributeObjectsH: wrap('<path d="M4 3v18M20 3v18"/><rect x="9.5" y="7" width="5" height="10" rx="1.2"/>'),
  distributeObjectsV: wrap('<path d="M3 4h18M3 20h18"/><rect x="7" y="9.5" width="10" height="5" rx="1.2"/>'),
  alignObjects: wrap('<path d="M4 3v18"/><rect x="7" y="5" width="11" height="4" rx="1.2"/><rect x="7" y="11" width="6" height="4" rx="1.2"/><rect x="7" y="17" width="9" height="3" rx="1.2"/>'),
  selectionPane: wrap('<rect x="3" y="3" width="18" height="18" rx="3"/><path d="M14 3v18"/><path d="M6 8h4M6 12h4M6 16h4"/>'),
  eye: wrap('<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="3"/>'),
  eyeOff: wrap('<path d="M10.2 5.7A9.6 9.6 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a16 16 0 0 1-2.6 3.4M6.5 7.2A16 16 0 0 0 2.5 12S6 18.5 12 18.5a9.4 9.4 0 0 0 4-.9"/><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2M3 3l18 18"/>'),
  kindShape: wrap('<rect x="3.5" y="5" width="11" height="10" rx="2"/><circle cx="15.5" cy="14.5" r="5"/>'),
  kindTextBox: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M8 9h8M12 9v7"/>'),
  kindPicture: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><circle cx="8.5" cy="9.5" r="1.6"/><path d="m21 16-5-5-8 8"/>'),
  kindChart: wrap('<path d="M4 20V4"/><path d="M4 20h16"/><rect x="7.5" y="12" width="3" height="5" rx=".6"/><rect x="12.5" y="8" width="3" height="9" rx=".6"/><rect x="17" y="14" width="2.4" height="3" rx=".6"/>'),
  kindGroup: wrap('<rect x="3" y="3" width="18" height="18" rx="3" stroke-dasharray="3.4 2.6"/><rect x="7" y="7" width="5" height="5" rx="1.2"/><rect x="12" y="12" width="5" height="5" rx="1.2"/>'),
  dragHandle: wrap('<circle cx="9" cy="6" r=".6"/><circle cx="15" cy="6" r=".6"/><circle cx="9" cy="12" r=".6"/><circle cx="15" cy="12" r=".6"/><circle cx="9" cy="18" r=".6"/><circle cx="15" cy="18" r=".6"/>'),
  pen: wrap('<path d="M4 20l1-4L16 5a2.1 2.1 0 0 1 3 3L8 19z"/><path d="m14 7 3 3"/>'),
  pencil: wrap('<path d="M4 20l1-4L16 5a2.1 2.1 0 0 1 3 3L8 19z"/><path d="m14 7 3 3M4 20l4-1"/>'),
  marker: wrap('<path d="M9 4h6l2 5v4H7V9z"/><path d="M9 13v5l3 3 3-3v-5"/>'),
  highlighter: wrap('<path d="m10 4 10 10-5 5-3-1-2 2H5l-1-2 2-2 4-2-1-3z"/><path d="M4 21h8"/>'),
  eraser: wrap('<path d="m7 21-4-4a2 2 0 0 1 0-2.8L14.2 3a2 2 0 0 1 2.8 0l4 4a2 2 0 0 1 0 2.8L11 20h10"/><path d="m8 9 7 7"/>'),
  lasso: wrap('<ellipse cx="12" cy="9" rx="9" ry="6" stroke-dasharray="3.2 2.4"/><path d="M8 14.5c-.4 2.4 1.4 3.5 3 3.5s2.6 1.4 1 3"/>'),
};

export type AlignIconName = keyof typeof ALIGN_ICONS;
