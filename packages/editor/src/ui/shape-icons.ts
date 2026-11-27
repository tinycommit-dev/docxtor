// Icons for shaping an object: flipping and turning it, and the Format shape panel (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const SHAPE_ICONS = {
  flipHorizontal: wrap('<path d="M12 3v18" stroke-dasharray="2.6 2.4"/><path d="M9 7.5 3.5 18H9z"/><path d="M15 7.5 20.5 18H15z"/>'),
  flipVertical: wrap('<path d="M3 12h18" stroke-dasharray="2.6 2.4"/><path d="M7.5 9 18 3.5V9z"/><path d="M7.5 15 18 20.5V15z"/>'),
  rotateRight: wrap('<rect x="3.5" y="12" width="9" height="8.5" rx="1.8"/><path d="M8 7.5V7a3 3 0 0 1 3-3h8"/><path d="m16.5 1.5 2.5 2.5-2.5 2.5"/>'),
  rotateLeft: wrap('<rect x="11.5" y="12" width="9" height="8.5" rx="1.8"/><path d="M16 7.5V7a3 3 0 0 0-3-3H5"/><path d="M7.5 1.5 5 4l2.5 2.5"/>'),
  formatShape: wrap('<path d="M4 7h9M19 7h1M4 17h1M11 17h9"/><circle cx="16" cy="7" r="2.2"/><circle cx="8" cy="17" r="2.2"/>'),
};

export type ShapeIconName = keyof typeof SHAPE_ICONS;
