// Field types: reading a field instruction into its parts and writing one from the values of a form. These are pure
// helpers (no DOM), used by the Insert, Field dialog and by extensions that evaluate their own fields; tested in Node.
import { msg } from "./l10n.ts";
import type { FieldOption, FieldType } from "./types.ts";

/** The groups of the Insert, Field dialog, in Word's order. `id` is what a field type's `category` says. */
export const FIELD_CATEGORIES: { id: string; label: string }[] = [
  { id: "Date and time", label: msg("Date and time") },
  { id: "Document information", label: msg("Document information") },
  { id: "Numbering", label: msg("Numbering") },
  { id: "Links and references", label: msg("Links and references") },
  { id: "Equations and formulas", label: msg("Equations and formulas") },
  { id: "User information", label: msg("User information") },
  { id: "Document automation", label: msg("Document automation") },
];

/** A field instruction taken apart. */
export interface ParsedField {
  /** The field name in capitals (`DATE`, `REF`, `=` for a formula). */
  code: string;
  /** The words after the name that are not switches or their values, unquoted. */
  args: string[];
  /** Switches other than `\*`, `\#` and `\@`: the letter(s) without the backslash, with the word that follows (or `true`). */
  switches: Record<string, string | true>;
  /** The words after `\*` switches: `MERGEFORMAT`, `Upper`, `roman`. */
  formats: string[];
  /** The `\@` date-time picture. */
  datePicture?: string;
  /** The `\#` numeric picture. */
  numberPicture?: string;
}

interface Word {
  text: string;
  quoted: boolean;
}

function words(s: string): Word[] {
  const out: Word[] = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) {
      i++;
    } else if (c === '"' || c === "\u201C") {
      i++;
      let text = "";
      while (i < s.length && s[i] !== '"' && s[i] !== "\u201D") {
        if (s[i] === "\\" && i + 1 < s.length) {
          text += s[i + 1];
          i += 2;
        } else text += s[i++];
      }
      i++;
      out.push({ text, quoted: true });
    } else {
      let text = "";
      while (i < s.length && !/\s/.test(s[i])) text += s[i++];
      out.push({ text, quoted: false });
    }
  }
  return out;
}

const isSwitch = (w: Word) => !w.quoted && w.text.length >= 2 && w.text.startsWith("\\");

/** Take a field instruction apart: ` DATE \@ "MMMM d, yyyy" \* Upper ` gives code `DATE`, a date picture and a format. */
export function parseInstruction(instr: string): ParsedField {
  const trimmed = instr.trim();
  // A formula can be written without a space: `=SUM(ABOVE)`.
  if (trimmed.startsWith("=")) {
    const rest = trimmed.slice(1);
    const cut = rest.search(/\s\\/);
    return { code: "=", args: [(cut < 0 ? rest : rest.slice(0, cut)).trim()], switches: {}, formats: [], ...picturesOf(words(cut < 0 ? "" : rest.slice(cut))) };
  }
  const all = words(instr);
  const code = (all.shift()?.text ?? "").toUpperCase();
  const args: string[] = [];
  const switches: Record<string, string | true> = {};
  const formats: string[] = [];
  for (let i = 0; i < all.length; i++) {
    const w = all[i];
    if (!isSwitch(w)) {
      args.push(w.text);
      continue;
    }
    const name = w.text.slice(1);
    const next = all[i + 1];
    if (name === "*") {
      if (next && !isSwitch(next)) formats.push(all[++i].text);
    } else if (name === "#" || name === "@") {
      if (next && !isSwitch(next)) i++;
    } else if (next && !isSwitch(next) && next.quoted) {
      switches[name] = all[++i].text;
    } else if (next && !isSwitch(next) && /^[0-9]+$/.test(next.text)) {
      switches[name] = all[++i].text;
    } else switches[name] = true;
  }
  return { code, args, switches, formats, ...picturesOf(all) };
}

function picturesOf(all: Word[]): { datePicture?: string; numberPicture?: string } {
  const out: { datePicture?: string; numberPicture?: string } = {};
  for (let i = 0; i < all.length - 1; i++) {
    if (!all[i].quoted && all[i].text === "\\@") out.datePicture = all[i + 1].text;
    if (!all[i].quoted && all[i].text === "\\#") out.numberPicture = all[i + 1].text;
  }
  return out;
}

/** A picture for `\@` or `\#`: Word always writes it in quotes. */
const quotePicture = (s: string) => `"${s.replace(/(["\\])/g, "\\$1")}"`;

/** A word for an instruction: quoted when it has a space, a quote or nothing in it. */
export function quoteWord(s: string): string {
  return s === "" || /[\s"\\]/.test(s) ? `"${s.replace(/(["\\])/g, "\\$1")}"` : s;
}

export type FieldValues = Record<string, string | boolean | undefined>;

/**
 * The instruction a form makes: the field name, then each option's value written the way its `write` says, in the order
 * the options are listed. Empty text and unchecked toggles write nothing. Switches that Word wants in front of the
 * rest (the argument) come after it, so ` REF Name \h ` reads as Word writes it.
 */
export function buildInstruction(type: Pick<FieldType, "name" | "options">, values: FieldValues): string {
  const parts: string[] = [type.name];
  const tail: string[] = [];
  for (const o of type.options ?? []) {
    const v = values[o.id] ?? o.default;
    if (v === undefined || v === false || v === "") continue;
    const text = v === true ? (o.value ?? "") : String(v);
    switch (o.write) {
      case "arg":
        parts.push(quoteWord(text));
        break;
      case "raw":
        parts.push(text);
        break;
      case "switch":
        tail.push(`\\${o.switch ?? o.id}`);
        break;
      case "switch-value":
        tail.push(text ? `\\${o.switch ?? o.id} ${quoteWord(text)}` : `\\${o.switch ?? o.id}`);
        break;
      case "format":
        tail.push(`\\* ${text}`);
        break;
      case "date":
        tail.push(`\\@ ${quotePicture(text)}`);
        break;
      case "number":
        tail.push(`\\# ${quotePicture(text)}`);
        break;
    }
  }
  return [...parts, ...tail].join(" ");
}

/** The values a field instruction gives to a type's options (the inverse of `buildInstruction`, for editing a field's code). */
export function readValues(type: Pick<FieldType, "options">, instr: string): FieldValues {
  const p = parseInstruction(instr);
  const out: FieldValues = {};
  let arg = 0;
  for (const o of type.options ?? []) {
    switch (o.write) {
      case "arg":
        out[o.id] = p.args[arg++];
        break;
      case "raw":
        out[o.id] = p.args.join(" ");
        break;
      case "switch":
        out[o.id] = !!p.switches[o.switch ?? o.id];
        break;
      case "switch-value": {
        const v = p.switches[o.switch ?? o.id];
        out[o.id] = typeof v === "string" ? v : undefined;
        break;
      }
      case "format":
        if (o.value) out[o.id] = p.formats.includes(o.value);
        else out[o.id] = p.formats.find((f) => (o.choices ? o.choices.some((c) => c.value === f) : true));
        break;
      case "date":
        out[o.id] = p.datePicture;
        break;
      case "number":
        out[o.id] = p.numberPicture;
        break;
    }
  }
  return out;
}

/** An option that only chooses between fixed values. */
export const isChoice = (o: FieldOption) => !!o.choices?.length;
