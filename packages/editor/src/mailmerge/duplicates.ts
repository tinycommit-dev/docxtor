// Edit recipient list, Find duplicates: records that look like the same person. Two records are alike when all their values are the
// same, when the name and address are the same (ignoring case and spaces), or when the email address is the same. Records that are
// alike, directly or through a third one, form a group. Pure, so it runs in Node too.
import { validAddress } from "./email-fields.ts";

export type DuplicateReason = "identical" | "nameAddress" | "email";

export interface DuplicateGroup {
  /** Indexes into the list, in list order (two at least). */
  rows: number[];
  reasons: DuplicateReason[];
}

export interface DuplicateInput {
  columns: string[];
  rows: string[][];
  /** Word's address fields and the column that stands for each (`MergeData.matched`). */
  matched: [field: string, column: string | null][];
}

/** A value with case, accents' composition and every kind of space ignored. */
const squash = (v: string): string => v.normalize("NFKC").toLowerCase().replace(/\s+/g, "");

/** A value with case ignored and runs of spaces made one. */
const tidy = (v: string): string => v.normalize("NFKC").toLowerCase().replace(/\s+/g, " ").trim();

const NAME = ["First Name", "Last Name"];
const ADDRESS = ["Address 1", "Address 2", "Address 3", "City", "State", "Postal Code", "Country or Region"];

export function findDuplicates(input: DuplicateInput): DuplicateGroup[] {
  const { columns, rows } = input;
  const column = (field: string): number => {
    const name = input.matched.find(([f]) => f === field)?.[1];
    return name ? columns.indexOf(name) : -1;
  };
  const cols = (fields: string[]) => fields.map(column).filter((i) => i >= 0);
  let nameCols = cols(NAME);
  if (!nameCols.length) nameCols = cols(["Company"]);
  const addressCols = cols(ADDRESS);
  const emailCol = column("E-mail Address");

  const parent = rows.map((_, i) => i);
  const find = (i: number): number => (parent[i] === i ? i : (parent[i] = find(parent[i])));
  const reasonsOf = new Map<number, Set<DuplicateReason>>();
  // Link records that share a key; remember why the key linked them.
  const link = (keyOf: (row: string[]) => string | null, reason: DuplicateReason) => {
    const seen = new Map<string, number>();
    rows.forEach((row, i) => {
      const key = keyOf(row);
      if (key === null) return;
      const first = seen.get(key);
      if (first === undefined) {
        seen.set(key, i);
        return;
      }
      parent[find(i)] = find(first);
      for (const r of [i, first]) (reasonsOf.get(r) ?? reasonsOf.set(r, new Set()).get(r)!).add(reason);
    });
  };

  link((row) => (row.every((v) => !v.trim()) ? null : JSON.stringify(row.map(tidy))), "identical");
  if (nameCols.length && addressCols.length) {
    link((row) => {
      const name = nameCols.map((c) => squash(row[c] ?? "")).join("|");
      const address = addressCols.map((c) => squash(row[c] ?? "")).join("|");
      return /[^|]/.test(name) && /[^|]/.test(address) ? `${name}\n${address}` : null;
    }, "nameAddress");
  }
  // Only a real address: placeholders such as "n/a", "-" or "none" in many rows don't make those people one.
  if (emailCol >= 0) link((row) => (validAddress(row[emailCol] ?? "") ? squash(row[emailCol] ?? "") : null), "email");

  const groups = new Map<number, number[]>();
  rows.forEach((_, i) => {
    if (!reasonsOf.has(i)) return;
    const root = find(i);
    (groups.get(root) ?? groups.set(root, []).get(root)!).push(i);
  });
  const order: DuplicateReason[] = ["identical", "nameAddress", "email"];
  return [...groups.values()]
    .filter((g) => g.length > 1)
    .sort((a, b) => a[0] - b[0])
    .map((g) => ({ rows: g, reasons: order.filter((r) => g.some((i) => reasonsOf.get(i)?.has(r))) }));
}

/**
 * The ticks after "keep the first of every group": in each group the first record that is in the merge stays and the rest are
 * left out. Records that are in no group, and groups with nothing in the merge, keep their ticks.
 */
export function keepFirst(active: boolean[], groups: DuplicateGroup[]): boolean[] {
  const next = active.slice();
  for (const g of groups) {
    const keep = g.rows.find((r) => active[r]);
    for (const r of g.rows) if (r !== keep) next[r] = false;
  }
  return next;
}
