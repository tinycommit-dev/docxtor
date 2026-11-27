// Icons for the Review tools: the thesaurus, inspecting a document, proofing options, combining (24px grid, 1.75
// stroke, like the rest of Docxtor's). Registered by the extensions that use them.
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const REVIEW_ICONS = {
  /** Two arrows the opposite way: one word for another. */
  thesaurus: wrap('<path d="M4 8h12"/><path d="m13 4.5 3.5 3.5-3.5 3.5"/><path d="M20 16H8"/><path d="m11 12.5-3.5 3.5 3.5 3.5"/>'),
  /** A page under a magnifying glass. */
  inspect: wrap('<path d="M12 21H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h6l5 5v3"/><path d="M13 3v5h5"/><circle cx="16.5" cy="16.5" r="3.25"/><path d="m21 21-2.2-2.2"/>'),
  /** Lines with a check under them. */
  proofing: wrap('<path d="M4 6h12"/><path d="M4 11h7"/><path d="M4 16h4"/><path d="m11.5 17 3 3 6-7"/>'),
  /** Two pages, one over the other. */
  combine: wrap('<rect x="3.5" y="3" width="10" height="13" rx="2"/><rect x="10.5" y="8" width="10" height="13" rx="2"/>'),
};
