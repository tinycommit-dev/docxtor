// The small pure parts of an email merge: which column holds the address, the subject line with its fields filled in, and whether an
// address can be used. Pure, so they run in Node too.
import type { MergeData } from "@docxtor/core";

/** The column names compared as the engine does: case and spaces don't matter, an underscore is a space. */
const same = (a: string, b: string) => a.replace(/_/g, " ").trim().toLowerCase() === b.replace(/_/g, " ").trim().toLowerCase();

/** The column most likely to hold the address: the one matched to Word's E-mail Address field, or one named like it, or one full of addresses. */
export function guessEmailColumn(data: Pick<MergeData, "columns" | "rows" | "matched">): string {
  const matched = data.matched.find(([f]) => f === "E-mail Address")?.[1];
  if (matched) return matched;
  const named = data.columns.find((c) => /^e-?mail(\s*address)?$/i.test(c.trim().replace(/_/g, " "))) ?? data.columns.find((c) => /e-?mail/i.test(c));
  if (named) return named;
  const looks = (v: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v.trim());
  return data.columns.find((_, i) => data.rows.length > 0 && data.rows.every((r) => !r[i] || looks(r[i])) && data.rows.some((r) => looks(r[i] ?? ""))) ?? data.columns[0] ?? "";
}

/** A subject line with «Column» filled in from the record. A name that is no column gives nothing. */
export function fillSubject(subject: string, columns: string[], row: string[]): string {
  return subject.replace(/«([^»]*)»/g, (_, name: string) => {
    const at = columns.findIndex((c) => same(c, name));
    return at >= 0 ? (row[at] ?? "") : "";
  });
}

/** Whether a string can be an address a mail service takes. */
export const validAddress = (a: string) => /^[^\s@<>,;"]+@[^\s@<>,;"]+\.[^\s@<>,;"]+$/.test(a.trim());
