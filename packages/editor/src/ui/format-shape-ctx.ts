// What the sections of the Format shape panel share: how they change the selected shape, and the shape of a section.
import type { Engine, RShape } from "@docxtor/core";
import type { PanelSection } from "./panel-controls";

export interface SectionCtx {
  engine: Engine;
  /** Change the selected shape (or the member of a group that is selected) with a `set_shape` patch. One undo step, or merged into the one before. */
  patch(patch: Record<string, unknown>, opts?: { merge?: boolean }): void;
  /** The selected shape as the engine reads it now. */
  shape(): RShape | null;
  /** Where the selected shape is on the page, in points counted the way its position is stored; null if it can't be measured. */
  measured(): { x: number; y: number } | null;
  /** Mirror the selected shape, or turn it a quarter turn (90 to the right, -90 to the left). */
  flip(axis: "h" | "v"): void;
  rotate(deg: number): void;
}

export interface ShapeSection {
  sec: PanelSection;
  /** Whether the section has anything to show for this shape. */
  applies(s: RShape): boolean;
  /** Show the shape's values in the controls. */
  update(s: RShape): void;
}

/** A transparency in percent from an opacity of 0 to 1 (omitted when opaque). */
export const toTransparency = (alpha?: number): number => Math.round((1 - (alpha ?? 1)) * 100);

/** An opacity of 0 to 1 from a transparency in percent; undefined when opaque, so the file carries no alpha. */
export const toAlpha = (percent: number): number | undefined => (percent <= 0 ? undefined : Math.round((1 - percent / 100) * 1000) / 1000);

/** A line or connector: it has an outline but no inside to fill. */
export function isLineShape(s: RShape): boolean {
  return s.geometry === "line" || /Connector\d*$/.test(s.geometry);
}

/** Shapes whose outline is open, so arrowheads make sense. */
export function isOpenShape(s: RShape): boolean {
  if (isLineShape(s) || /^(arc|leftBracket|rightBracket|leftBrace|rightBrace)$/.test(s.geometry)) return true;
  return s.geometry === "custom" && !!s.custom?.paths.some((p) => !/[Zz]\s*$/.test(p.d.trim()));
}
