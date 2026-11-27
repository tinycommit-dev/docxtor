// The one place a colour from a document becomes CSS or SVG. A colour in a .docx is a string the file chooses (`red;background:url(...)`
// is as good a `w:color` as `FF0000`), and the engine already hands out only six hex digits, `auto` or a name from a closed list; this
// is the second check, in the editor, for every string that is built from it (a `style` attribute, `cssText`, an SVG attribute like
// `fill`, `stroke`, `stop-color` or `flood-color`). The guard in packages/editor/test/colour-sinks.test.ts fails a `#${...}` written into one of
// those without going through here. A leaf module: it imports nothing, so the drawing code can use it without a cycle.

const HEX6 = /^[0-9a-f]{6}$/i;

/** `#rrggbb` for six hex digits; `auto`, and anything else (a value that is not a colour must never become CSS), is none. */
export const hex = (c?: string | null): string | undefined => (c && HEX6.test(c) ? `#${c}` : undefined);

/** `hex`, with the colour to use when the value is not one. */
export const hexOr = (c: string | null | undefined, fallback = "#000000"): string => hex(c) ?? fallback;

/** Whether a string is six hex digits. */
export const isHex6 = (c?: string | null): c is string => !!c && HEX6.test(c);
