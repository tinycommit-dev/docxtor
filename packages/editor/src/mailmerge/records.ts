// The "which records" part shared by the dialogs that finish a merge (Finish and merge, Print merged documents, Send email messages):
// all of them, the one shown, or a range.
import type { MergeInfo, MergeRange } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { FormField, FormValues } from "./form";

/** The three fields: which records, and where a range starts and ends (counted from 1). */
export function recordFields(info: MergeInfo): FormField[] {
  const shown = info.previewing;
  return [
    {
      id: "range",
      label: t("Records"),
      kind: "select",
      value: "all",
      options: [
        { value: "all", label: t("All") },
        ...(shown !== null ? [{ value: "current", label: t("The record shown") }] : []),
        { value: "range", label: t("From … to …") },
      ],
    },
    { id: "from", label: t("From record"), kind: "text", value: "1" },
    { id: "to", label: t("To record"), kind: "text", value: String(info.included) },
  ];
}

/** The range fields show only for a range. */
export const recordsVisible = (id: string, values: FormValues): boolean => (id === "from" || id === "to" ? values.range === "range" : true);

/** A message when the range isn't usable, or null. */
export function recordsProblem(info: MergeInfo, values: FormValues): string | null {
  if (values.range !== "range") return null;
  const a = Number(values.from);
  const b = Number(values.to);
  return Number.isInteger(a) && Number.isInteger(b) && a >= 1 && b >= a && b <= info.included ? null : t("Give two record numbers between 1 and {n}, the first not after the second.", { n: info.included });
}

/** The engine's range for the dialog's values. */
export function recordsRange(info: MergeInfo, values: FormValues): MergeRange {
  const shown = info.previewing;
  if (values.range === "current" && shown !== null) return { from: shown, to: shown + 1 };
  if (values.range === "range") return { from: Number(values.from) - 1, to: Number(values.to) };
  return {};
}

/** How many records the dialog's values pick. */
export function recordsCount(info: MergeInfo, values: FormValues): number {
  const r = recordsRange(info, values);
  return Math.max(0, (r.to ?? info.included) - (r.from ?? 0));
}
