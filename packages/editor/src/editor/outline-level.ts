// The outline level of a paragraph. Pure, so it is tested in Node.
import type { RPara } from "@docxtor/core";

/** The outline level of a paragraph as 1 to 9, or 0 for body text. */
export function outlineLevel(p: Pick<RPara, "style" | "fmt">): number {
  const l = p.fmt.outlineLvl;
  if (l !== undefined && l >= 0 && l < 9) return l + 1;
  if (l === 9) return 0;
  const n = /^Heading([1-9])$/.exec(p.style ?? "");
  return n ? Number(n[1]) : 0;
}
