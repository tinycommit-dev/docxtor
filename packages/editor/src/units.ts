// The measuring unit dialogs show lengths in: inches or centimetres. The files always hold twips; this
// only decides what a person types and reads. One helper, so a preference can switch every dialog and the rulers at once.
import { t } from "@docxtor/extensions";

export type UnitSystem = "in" | "cm";

/** The unit a place uses: inches in the United States, Liberia and Myanmar, centimetres everywhere else. */
export function localeUnitSystem(language = typeof navigator !== "undefined" ? navigator.language : "en-US"): UnitSystem {
  return /^en-(US|LR|MM)/.test(language || "en-US") ? "in" : "cm";
}

let system: UnitSystem = localeUnitSystem();
const listeners = new Set<() => void>();

/** The unit lengths are shown in. The place's own until a preference says otherwise. */
export function unitSystem(): UnitSystem {
  return system;
}

export function setUnitSystem(u: UnitSystem) {
  if (u === system) return;
  system = u;
  for (const fn of listeners) fn();
}

/** Called when the unit changes (the rulers redraw their ticks). Returns a function that stops listening. */
export function onUnitChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Twips in one unit. */
export function twipsPerUnit(): number {
  return system === "cm" ? 1440 / 2.54 : 1440;
}

/** A short name for the unit, as the interface shows it. */
export function unitName(): string {
  return system === "cm" ? t("cm") : t("in");
}

/** The unit spelled out for a label: "inches" or "cm". */
export function unitWord(): string {
  return system === "cm" ? t("cm") : t("inches");
}

/** The step of a number field: 0.05 inch, or 0.1 cm. */
export function unitStep(): number {
  return system === "cm" ? 0.1 : 0.05;
}

/** Twips as a number of the current unit, rounded to two places. */
export function toUnits(twips: number): number {
  return Math.round((twips / twipsPerUnit()) * 100) / 100;
}

/** A number of the current unit as whole twips. */
export function fromUnits(value: number): number {
  return Math.round(value * twipsPerUnit());
}

/** Twips as text for a field: no trailing zeros ("0.5", "1", "2.54"). */
export function formatUnits(twips: number): string {
  return String(toUnits(twips));
}

/**
 * Read what someone typed into a length field as twips. A bare number is in the current unit; "cm", "mm", "in",
 * a double quote, "pt" and "pc" after the number say otherwise, as Word accepts them. Returns null for anything else.
 */
export function parseLength(text: string): number | null {
  const m = /^\s*(-?\d+(?:[.,]\d+)?)\s*(cm|mm|in|"|pt|pc)?\s*$/i.exec(text);
  if (!m) return null;
  const n = Number(m[1].replace(",", "."));
  const per: Record<string, number> = { cm: 1440 / 2.54, mm: 144 / 2.54, in: 1440, '"': 1440, pt: 20, pc: 240 };
  const unit = m[2]?.toLowerCase();
  return Math.round(n * (unit ? per[unit] : twipsPerUnit()));
}
