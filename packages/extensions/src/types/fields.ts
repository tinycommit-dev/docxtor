// Field types
import type { Engine } from "@docxtor/core";

/** One choice in a field's form: a word after the name, a switch, or a format. */
export interface FieldOption {
  id: string;
  label: string;
  /** Text to type, a list to choose from (`choices`), or a check box. */
  kind: "text" | "choice" | "toggle";
  /**
   * How the value goes into the instruction: `arg` (a word after the name), `switch` (`\x`, for a check box),
   * `switch-value` (`\x "value"`), `format` (`\* value`), `date` (`\@ "picture"`) or `number` (`\# "picture"`).
   */
  write: "arg" | "raw" | "switch" | "switch-value" | "format" | "date" | "number";
  /** For a check box that writes a fixed word (`format` or `switch-value`): the word, such as `MERGEFORMAT`. */
  value?: string;
  /** The switch letter(s) without the backslash, for `switch` and `switch-value` (default: the option's id). */
  switch?: string;
  choices?: { value: string; label: string }[];
  placeholder?: string;
  help?: string;
  default?: string | boolean;
}

/** What an extension's field evaluator is given. */
export interface FieldCall {
  /** The instruction as written, such as ` WEATHER London \* Upper `. */
  instr: string;
  /** The field name in capitals. */
  code: string;
  /** The words after the name that aren't switches. */
  args: string[];
  switches: Record<string, string | true>;
  /** The words after `\*` switches. */
  formats: string[];
  datePicture?: string;
  numberPicture?: string;
  /** The open document's engine. */
  engine: Engine;
  /** The time now. */
  now: Date;
}

/**
 * A kind of field. Docxtor's own fields (`DATE`, `PAGE`, `REF`…) are registered this way and the engine works out their
 * results. An extension adds a field type with an `evaluate` function: the field is updated with Update fields (F9),
 * on printing and on download, like any other.
 */
export interface FieldType {
  /** The field name in capitals, as it is written in the instruction: `DATE`, `WEATHER`. */
  name: string;
  /** A short name for the list, such as "Date". Default: the field name. */
  title?: string;
  /** One of the groups of Insert, Field (`Date and time`, `Document information`, `Numbering`, `Links and references`, `Equations and formulas`, `User information`, `Document automation`), or a name of your own. */
  category: string;
  /** A sentence about what it shows. */
  description?: string;
  /** The form the dialog shows; the instruction is made from its values. */
  options?: FieldOption[];
  /** The instruction the dialog starts with, when the form alone can't say it. */
  example?: string;
  /**
   * Work out the field's result as plain text. Return null (or nothing) to leave the result as it is. Without this the
   * engine works the field out if it knows it, and otherwise the result stays what the file has.
   */
  evaluate?(call: FieldCall): string | null | undefined | Promise<string | null | undefined>;
}

