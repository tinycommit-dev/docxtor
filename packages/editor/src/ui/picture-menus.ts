// The two menus that crop a picture: to a shape and to an aspect ratio.
import { msg, t } from "@docxtor/extensions";
import { PICTURE_SHAPES, shapePath } from "../editor/picture";
import { h, popover, svg } from "./dom";

/** Aspect ratios the crop menu offers: width over height, with the label (a ratio is the same in any language). */
export const ASPECTS: [string, number, number][] = [
  ["1:1", 1, 1],
  ["4:3", 4, 3],
  ["3:4", 3, 4],
  ["3:2", 3, 2],
  ["2:3", 2, 3],
  ["16:9", 16, 9],
  ["9:16", 9, 16],
];

export const ASPECT_NAMES: Record<string, string> = {
  "1:1": msg("Square, 1:1"),
  "4:3": msg("Landscape, 4:3"),
  "3:4": msg("Portrait, 3:4"),
  "3:2": msg("Landscape, 3:2"),
  "2:3": msg("Portrait, 2:3"),
  "16:9": msg("Widescreen, 16:9"),
  "9:16": msg("Tall, 9:16"),
};

/** A small drawing of a picture shape for the menu. */
export function shapeIcon(geom: string): SVGElement {
  const path = shapePath(geom, 18, 18, undefined);
  return svg(`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linejoin="round" aria-hidden="true"><path transform="translate(3 3)" d="${path}"/></svg>`);
}

/** The shapes to crop to, as a grid below `anchor`. */
export function openShapeMenu(anchor: HTMLElement, current: string | undefined, pick: (geom: string) => void): () => void {
  const grid = h("div", { class: "shape-grid", role: "group", "aria-label": t("Crop to shape") });
  let close = () => {};
  for (const [id, label] of PICTURE_SHAPES) {
    const b = h("button", { class: `shape-cell${id === (current ?? "rect") ? " on" : ""}`, type: "button", title: t(label), "aria-label": t(label), "aria-pressed": String(id === (current ?? "rect")) }, shapeIcon(id));
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => {
      close();
      pick(id);
    });
    grid.append(b);
  }
  close = popover(anchor, grid);
  return close;
}

/** The ratios to crop to, as a list below `anchor`. */
export function openAspectMenu(anchor: HTMLElement, pick: (w: number, h: number) => void): () => void {
  const list = h("div", { class: "arrange-menu" });
  let close = () => {};
  for (const [id, w, hh] of ASPECTS) {
    const b = h("button", { class: "menu-item", type: "button", role: "menuitem" }, h("span", { class: "menu-label" }, t(ASPECT_NAMES[id])));
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => {
      close();
      pick(w, hh);
    });
    list.append(b);
  }
  close = popover(anchor, list);
  return close;
}
