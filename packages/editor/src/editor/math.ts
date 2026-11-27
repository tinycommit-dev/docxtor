// Equations on the page: the engine hands over MathML Core text, and this turns it into a `<math>` element
// that holds nothing but MathML. The engine is trusted, but the markup also comes from files other people made
// and from an extension's input, so it is rebuilt from an allow-list: no scripts, links, styles, event handlers
// or foreign elements ever reach the page.

const MATHML = "http://www.w3.org/1998/Math/MathML";

const ELEMENTS = new Set([
  "math",
  "mrow",
  "mi",
  "mn",
  "mo",
  "mtext",
  "mspace",
  "mfrac",
  "msqrt",
  "mroot",
  "msub",
  "msup",
  "msubsup",
  "munder",
  "mover",
  "munderover",
  "mmultiscripts",
  "mprescripts",
  "none",
  "mtable",
  "mtr",
  "mtd",
  "mphantom",
  "mpadded",
  "merror",
]);

const ATTRIBUTES = new Set([
  "display",
  "displaystyle",
  "scriptlevel",
  "mathvariant",
  "stretchy",
  "fence",
  "separator",
  "symmetric",
  "form",
  "largeop",
  "movablelimits",
  "accent",
  "accentunder",
  "linethickness",
  "width",
  "height",
  "depth",
  "lspace",
  "rspace",
  "minsize",
  "maxsize",
  "columnalign",
  "columnspan",
  "rowspan",
  "class",
]);

/** Attribute values are short words and measures (`0.1667em`, `true`, `m-box`): anything else is dropped. */
const SAFE_VALUE = /^[A-Za-z0-9 .%+-]{0,40}$/;

function rebuild(src: Element, doc: Document): Element | null {
  const name = src.localName;
  if (!ELEMENTS.has(name)) return null;
  const out = doc.createElementNS(MATHML, name);
  for (const a of Array.from(src.attributes)) {
    if (a.namespaceURI || !ATTRIBUTES.has(a.name) || !SAFE_VALUE.test(a.value)) continue;
    out.setAttribute(a.name, a.value);
  }
  for (const child of Array.from(src.childNodes)) {
    if (child.nodeType === Node.TEXT_NODE) out.append(doc.createTextNode(child.nodeValue ?? ""));
    else if (child.nodeType === Node.ELEMENT_NODE) {
      const c = rebuild(child as Element, doc);
      if (c) out.append(c);
    }
  }
  return out;
}

/**
 * A `<math>` element for the engine's MathML, or null when it isn't MathML at all. Elements and attributes
 * outside MathML Core's drawing set are left out; their text stays so nothing silently disappears.
 */
export function mathNode(mathml: string): Element | null {
  let parsed: Document;
  try {
    parsed = new DOMParser().parseFromString(mathml, "application/xml");
  } catch {
    return null;
  }
  const root = parsed.documentElement;
  if (!root || root.localName !== "math" || root.namespaceURI !== MATHML || parsed.querySelector("parsererror")) return null;
  return rebuild(root, document);
}
