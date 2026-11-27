// Icons for working on a picture: crop, turn, flip, format, position and reset (24px grid, 1.75 stroke, like the
// rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const PICTURE_ICONS = {
  crop: wrap('<path d="M6 2.5v13.5a2 2 0 0 0 2 2h13.5"/><path d="M2.5 6H16a2 2 0 0 1 2 2v13.5"/>'),
  cropShape: wrap('<path d="M6 2.5v13.5a2 2 0 0 0 2 2h13.5"/><circle cx="13.5" cy="10.5" r="4.5"/>'),
  cropAspect: wrap('<rect x="3" y="6" width="18" height="12" rx="2.5"/><path d="M7.5 14.5 11 11M7.5 11.5v3h3"/>'),
  rotateRight: wrap('<path d="M20 11.5A8 8 0 1 0 17.7 17"/><path d="M20 4v7.5h-7.5"/>'),
  rotateLeft: wrap('<path d="M4 11.5A8 8 0 1 1 6.3 17"/><path d="M4 4v7.5h7.5"/>'),
  flipHorizontal: wrap('<path d="M12 3v18" stroke-dasharray="2.6 2.6"/><path d="M9 7 3.5 12 9 17z"/><path d="m15 7 5.5 5-5.5 5z"/>'),
  flipVertical: wrap('<path d="M3 12h18" stroke-dasharray="2.6 2.6"/><path d="M7 9 12 3.5 17 9z"/><path d="m7 15 5 5.5 5-5.5z"/>'),
  pictureFormat: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M8 16v-3M12 16V9M16 16v-5"/>'),
  picturePosition: wrap('<rect x="3" y="3.5" width="18" height="17" rx="2.5"/><rect x="11.5" y="11.5" width="6.5" height="6" rx="1.4"/><path d="M6.5 8h6M6.5 11h2.5"/>'),
  pictureReset: wrap('<path d="M4 12a8 8 0 1 0 2.4-5.7"/><path d="M4 4v5h5"/>'),
  imageUrl: wrap('<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c3.2 3.4 3.2 14.6 0 18M12 3c-3.2 3.4-3.2 14.6 0 18"/>'),
  imageFile: wrap('<rect x="3" y="4" width="18" height="16" rx="2.5"/><circle cx="9" cy="10" r="1.6"/><path d="m21 16-4.5-4.5L7 20"/>'),
};

export type PictureIconName = keyof typeof PICTURE_ICONS;
