// Icons for content controls, form fields and document protection (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const CONTROL_ICONS = {
  formControls: wrap('<rect x="3" y="3.5" width="18" height="17" rx="2.5"/><rect x="6.5" y="7" width="4" height="4" rx="1"/><path d="m7.3 9 .9.9 1.4-1.6M13.5 8.5H18M6.5 15.5H18"/>'),
  controlCheckbox: wrap('<rect x="4" y="4" width="16" height="16" rx="3"/><path d="m8 12.2 2.8 2.8L16.2 9"/>'),
  controlDropDown: wrap('<rect x="3" y="6" width="18" height="12" rx="2.5"/><path d="m14.5 11 2 2 2-2"/><path d="M6.5 12H12"/>'),
  controlCombo: wrap('<rect x="3" y="6" width="18" height="12" rx="2.5"/><path d="M15 6v12"/><path d="m16.5 11 1.5 1.8 1.5-1.8"/><path d="M6.5 12H11"/>'),
  controlDate: wrap('<rect x="3.5" y="5" width="17" height="15.5" rx="2.5"/><path d="M3.5 10h17M8 3v4M16 3v4"/><path d="M8 14h2M14 14h2M8 17.2h2"/>'),
  controlText: wrap('<rect x="3" y="6" width="18" height="12" rx="2.5"/><path d="M8 10.2h8M12 10.2V15"/>'),
  controlRichText: wrap('<rect x="3" y="6" width="18" height="12" rx="2.5"/><path d="m7.5 15 2.5-6 2.5 6M8.3 13.2h3.4"/><path d="M15 15h2.5"/>'),
  controlPicture: wrap('<rect x="3" y="5" width="18" height="14" rx="2.5"/><circle cx="8.5" cy="10" r="1.5"/><path d="m21 16-4-4-7 7"/>'),
  controlRepeating: wrap('<rect x="5" y="3.5" width="14" height="7" rx="2"/><rect x="5" y="13.5" width="14" height="7" rx="2"/><path d="M21 10.5v3M19.5 12h3"/>'),
  controlProperties: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M7 9h10M7 13h6M7 17h3"/><circle cx="17" cy="16" r="2.4"/>'),
  restrictEditing: wrap('<path d="M12 3.5 5 6v5.5c0 4.2 2.9 7.3 7 9 4.1-1.7 7-4.8 7-9V6z"/><path d="m9.2 12 2.2 2.2 3.6-4"/>'),
  markFinal: wrap('<path d="M6 21V4M6 5h11.5l-2.5 3.5 2.5 3.5H6"/>'),
  formField: wrap('<rect x="3" y="7" width="18" height="10" rx="1.5" stroke-dasharray="3 2.4"/><path d="M7 12h5"/>'),
} as const;
