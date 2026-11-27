// What the page layout puts in fields whose value depends on where they are: page numbers, the number of pages, the
// section, the nearest heading (STYLEREF) and the parts of today's date that Word keeps as elements. Pure helpers; the
// paginator (layout.ts) knows the pages and calls these.
import { parseInstruction } from "@docxtor/extensions";

/** A paragraph of the text, for STYLEREF: where it is and what it says. */
export interface RefPara {
  /** The page it starts on (from 0). */
  page: number;
  /** The id of its paragraph style. */
  style: string;
  /** Its text without the list number. */
  text: string;
  /** Its list number as shown (`2.1.`), if it has one. */
  number: string;
}

/** A style name or id for comparing: `Heading 1` and `Heading1` are the same style. */
export const styleKey = (s: string) => s.toLowerCase().replace(/\s+/g, "");

/** Word's case formats, for text a field shows. */
export function caseOf(text: string, instr: string | undefined): string {
  if (!instr) return text;
  for (const f of parseInstruction(instr).formats) {
    switch (f.toLowerCase()) {
      case "upper":
        return text.toUpperCase();
      case "lower":
        return text.toLowerCase();
      case "firstcap":
        return text.charAt(0).toUpperCase() + text.slice(1).toLowerCase();
      case "caps":
        return text.toLowerCase().replace(/(^|[\s\-(])(\p{L})/gu, (_, a: string, b: string) => a + b.toUpperCase());
    }
  }
  return text;
}

/** The text of a date element (`w:dayLong` and the like): `dayShort`, `dayLong`, `monthShort`, `monthLong`, `yearShort`, `yearLong`. */
export function dateElementText(kind: string, lang: string | undefined, now = new Date()): string | null {
  const l = lang || undefined;
  try {
    switch (kind) {
      case "dayShort":
        return String(now.getDate());
      case "dayLong":
        return new Intl.DateTimeFormat(l, { weekday: "long" }).format(now);
      case "monthShort":
        return new Intl.DateTimeFormat(l, { month: "short" }).format(now);
      case "monthLong":
        return new Intl.DateTimeFormat(l, { month: "long" }).format(now);
      case "yearShort":
        return String(now.getFullYear()).slice(-2);
      case "yearLong":
        return String(now.getFullYear());
    }
  } catch {
    // An unknown language tag: the browser's own.
    return dateElementText(kind, undefined, now);
  }
  return null;
}

/**
 * The text a STYLEREF field shows on page `k`: the first (or, with `\l`, last) paragraph of the style on that page; if
 * there is none, the nearest one before the page, else the nearest after it. `\n`, `\r` and `\w` give the paragraph's
 * number instead of its text; `\p` says whether it is above or below the page. `styleKeys` are the `styleKey`s the field's
 * style name stands for. Returns "" when no paragraph has the style.
 */
export function styleRefText(instr: string, k: number, paras: RefPara[], styleKeys: Set<string>): string {
  const p = parseInstruction(instr);
  const mine = paras.filter((x) => styleKeys.has(styleKey(x.style)));
  const here = mine.filter((x) => x.page === k);
  let found: RefPara | undefined;
  if (here.length) found = p.switches.l ? here[here.length - 1] : here[0];
  else found = [...mine].reverse().find((x) => x.page < k) ?? mine.find((x) => x.page > k);
  if (!found) return "";
  if (p.switches.p) return found.page > k ? "below" : "above";
  if (p.switches.n || p.switches.r || p.switches.w) return found.number.replace(/\.$/, "");
  return caseOf(found.text, instr);
}
