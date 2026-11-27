// The zoom numbers: which factor a mode asks for. Pure, so they are tested in Node (see zoom.ts for the controller).

export type ZoomMode = "auto" | "width" | "page" | "pages" | "custom";

export const ZOOM_MIN = 0.1;
export const ZOOM_MAX = 5;
/** The presets the zoom menu and the status bar offer. */
export const ZOOM_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2];
/** How many pages "Several pages" shows side by side. */
export const PAGES_ACROSS = [2, 3, 4];

export const clampZoom = (z: number): number => (Number.isFinite(z) ? Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(z * 100) / 100)) : 1);

/** One step in or out: 10 points, snapped to a multiple of 5 so repeated steps stay tidy. */
export function stepZoom(z: number, dir: 1 | -1): number {
  return clampZoom(Math.round((z + dir * 0.1) * 20) / 20);
}

/** The factor at which a page `pageW` wide fills `available` pixels, rounded down to a whole percent. `cap` limits it (auto stops at 1). */
export function fitWidthZoom(available: number, pageW: number, cap = ZOOM_MAX): number {
  if (!(pageW > 0) || !(available > 0)) return 1;
  return clampZoom(Math.min(cap, Math.floor((available / pageW) * 100) / 100));
}

/** The factor at which a whole page fits a window: the smaller of the width and height fits. `extra` is height that scales with the page and isn't the page (the ruler). */
export function fitPageZoom(availW: number, availH: number, pageW: number, pageH: number, extra = 0): number {
  if (!(pageW > 0) || !(pageH > 0) || !(availW > 0) || !(availH > 0)) return 1;
  return clampZoom(Math.floor(Math.min(availW / pageW, availH / (pageH + extra)) * 100) / 100);
}

/** The factor at which `across` pages of width `pageW`, `gap` pixels apart, fill `available` pixels. */
export function pagesZoom(available: number, pageW: number, across: number, gap: number): number {
  if (!(pageW > 0) || !(available > 0) || across < 1) return 1;
  return clampZoom(Math.floor(((available - gap * (across - 1)) / (across * pageW)) * 100) / 100);
}

/**
 * The factor after one wheel event with Ctrl held. A trackpad pinch arrives as small deltas (ctrlKey set by the
 * browser) and zooms smoothly; a mouse wheel arrives in notches of 100 and steps by ten points.
 */
export function wheelZoom(z: number, deltaY: number, deltaMode = 0): number {
  const dy = deltaMode === 1 ? deltaY * 16 : deltaY;
  if (dy === 0) return z;
  if (Math.abs(dy) >= 40) return stepZoom(z, dy < 0 ? 1 : -1);
  return clampZoom(z * Math.exp(-dy * 0.02));
}

/** The factor while two fingers are down: the start factor, scaled by how far the fingers moved apart. */
export function pinchZoom(start: number, startDistance: number, distance: number): number {
  if (!(startDistance > 0) || !(distance > 0)) return start;
  return clampZoom(start * (distance / startDistance));
}

/** What the zoom is called in the interface: whole percent. */
export const zoomLabel = (z: number): string => `${Math.round(z * 100)}%`;
