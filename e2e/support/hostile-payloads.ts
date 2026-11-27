// What a hostile document carries: the slots, the script every payload runs, and the addresses nothing may reach.

export const SLOTS = [
  "text",
  "links",
  "fields",
  "alt",
  "props",
  "notes",
  "styles",
  "numbering",
  "images",
  "svg",
  "altchunk",
  "customxml",
  "theme",
  "external",
  "shapes",
  "colours",
] as const;
export type Slot = (typeof SLOTS)[number];

/** The host every request payload asks for. Nothing may ask it for anything. */
export const EVIL_HOST = "evil.test";

export const W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
export const R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
export const REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
export const A = "http://schemas.openxmlformats.org/drawingml/2006/main";
export const XML = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>';

/** Escape for XML text and attribute values. */
export const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The script every payload runs. */
export const run = (id: string) => `__pwned.push('${id}')`;
/** HTML that runs script three ways if it is ever parsed as HTML. */
export const htmlPayload = (id: string) => `<img src=x onerror="${run(id)}"><script>${run(id)}</script><svg onload="${run(id)}"></svg>`;
/** A script address. */
export const jsUrl = (id: string) => `javascript:${run(id)}`;
/** An address on the host nothing may reach. */
export const evilUrl = (slot: string, where: string, scheme = "http") => `${scheme}://${EVIL_HOST}/${slot}/${where}`;

/** A font name that tries to get out of a CSS string and ask the network for something. */
export const cssFont = (id: string) => `Arial'; background-image: url(${evilUrl(id, "css-font")}); x: '`;
