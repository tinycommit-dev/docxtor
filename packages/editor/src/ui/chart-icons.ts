// Icons for the chart types (24px grid, 1.75 stroke, like the rest of Docxtor's).
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const CHART_ICONS = {
  chart: wrap('<path d="M4 4v16h16"/><path d="M8.5 16v-4M12.5 16V8M16.5 16v-6"/>'),
  chartColumn: wrap('<path d="M4 20h16"/><rect x="5.5" y="10" width="3.5" height="10" rx="0.8"/><rect x="10.25" y="5" width="3.5" height="15" rx="0.8"/><rect x="15" y="13" width="3.5" height="7" rx="0.8"/>'),
  chartBar: wrap('<path d="M4 4v16"/><rect x="4" y="5.5" width="10" height="3.5" rx="0.8"/><rect x="4" y="10.25" width="15" height="3.5" rx="0.8"/><rect x="4" y="15" width="7" height="3.5" rx="0.8"/>'),
  chartLine: wrap('<path d="M4 4v16h16"/><path d="m7 15 3.5-4 3 2.5L19 7"/><circle cx="7" cy="15" r="0.9" fill="currentColor"/><circle cx="10.5" cy="11" r="0.9" fill="currentColor"/><circle cx="13.5" cy="13.5" r="0.9" fill="currentColor"/><circle cx="19" cy="7" r="0.9" fill="currentColor"/>'),
  chartPie: wrap('<path d="M12 3.5a8.5 8.5 0 1 0 8.5 8.5H12z"/><path d="M14.5 2.7A8.5 8.5 0 0 1 21.3 9.5h-6.8z"/>'),
  chartDoughnut: wrap('<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="3.5"/><path d="M12 3.5v5M20.5 12h-5"/>'),
  chartArea: wrap('<path d="M4 4v16h16"/><path d="m5 17 4-5 3.5 2.5L19 8v12H5z"/>'),
  chartScatter: wrap('<path d="M4 4v16h16"/><circle cx="8.5" cy="14.5" r="1.1"/><circle cx="12" cy="10.5" r="1.1"/><circle cx="15.5" cy="14" r="1.1"/><circle cx="18" cy="7.5" r="1.1"/>'),
  chartBubble: wrap('<path d="M4 4v16h16"/><circle cx="9" cy="14.5" r="2.6"/><circle cx="15.5" cy="9.5" r="3.6"/><circle cx="17.2" cy="16.8" r="1.4"/>'),
  chartStock: wrap('<path d="M4 4v16h16"/><path d="M9 6.5v3M9 14.5v3M15.5 5.5v2.5M15.5 12.5v3"/><rect x="7.5" y="9.5" width="3" height="5" rx="0.6"/><rect x="14" y="8" width="3" height="4.5" rx="0.6"/>'),
  chartSurface: wrap('<path d="M3.5 12c2.5-4 4.5 1 7-2.5s4.5-2 7-5.5"/><path d="M3.5 18c2.5-4 4.5 1 7-2.5s4.5-2 7-5.5"/><path d="M3.5 12v6M10.5 9.5v6M17.5 4v6"/>'),
  chartCombo: wrap('<path d="M4 20h16"/><rect x="5.5" y="13" width="3.2" height="7" rx="0.8"/><rect x="10.4" y="10" width="3.2" height="10" rx="0.8"/><rect x="15.3" y="14" width="3.2" height="6" rx="0.8"/><path d="m5 8 5.5-3 4.5 2.5L19.5 4"/>'),
  chartRadar: wrap('<path d="m12 3.5 8 5.9-3 9.6H7l-3-9.6z"/><path d="m12 8.5 3.8 2.8-1.4 4.4H9.6l-1.4-4.4z"/>'),
};

export type ChartIconName = keyof typeof CHART_ICONS;
