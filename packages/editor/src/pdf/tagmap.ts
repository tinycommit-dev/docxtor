// Decides which structure element each part of the laid-out pages belongs to, for a tagged PDF. The walker (page.ts) asks
// `enter` for every element it visits and carries the answer down: a paragraph is a P or a heading, a list item is an LI with
// its label and body, a table is a Table with rows and cells, a picture with alternative text is a Figure, a link is a Link.
// Page furniture (headers, footers, borders, page and line numbers) is an artifact. Reading order is the page markup's order.
import type { StructAttrs, StructNode, StructTree } from "./structure.ts";

/** Parts of the page that are not the document's text. */
const ARTIFACT = ".pgsp, .crule, .lnum, .vruler, .np-layer, .bm-layer, .sp-layer, .page-num, .fc-label, .notes-sep, .notes-line, .notes-notice, td.gap, .hdr-repeat, [aria-hidden='true']";

/** Elements a block cannot sit inside: a paragraph in a text box in a paragraph goes beside the outer one. */
const INLINE = new Set(["P", "H1", "H2", "H3", "H4", "H5", "H6", "Span", "Lbl", "Link", "Figure", "Formula"]);

const FIGURES = new Set(["Figure", "Formula"]);

export class Tagger {
  private lists = new WeakMap<Element, StructNode>();
  private bodies = new WeakMap<StructNode, StructNode>();
  private tree: StructTree;
  private headings: ReadonlyMap<number, number>;
  private lang?: string;
  /** Tables made, and those with a header cell: a table without one cannot claim PDF/UA. */
  private tables = new Set<StructNode>();
  private headed = new Set<StructNode>();
  private undescribed = 0;

  /** A picture was tagged as a figure with no description. */
  get figureWithoutAlt(): boolean {
    return this.undescribed > 0;
  }

  /** Some table has no header cell. */
  get headerlessTable(): boolean {
    return [...this.tables].some((t) => !this.headed.has(t));
  }

  /**
   * @param headings each heading paragraph's id and its level (1 to 9)
   * @param lang the document's language, so only passages in another language say theirs
   */
  constructor(tree: StructTree, headings: ReadonlyMap<number, number>, lang?: string) {
    this.tree = tree;
    this.headings = headings;
    this.lang = lang;
  }

  /** The first element of the walk: everything is in the document. */
  get root(): StructNode {
    return this.tree.root;
  }

  /** The element an element belongs to, given its parent's. Null: an artifact, and so is everything inside it. */
  enter(el: Element, parent: StructNode | null): StructNode | null {
    if (!parent) return null;
    const cl = el.classList;
    // What sits on the sheets themselves (headers, footers, watermarks, page borders) is page furniture; the notes are text.
    if (el.parentElement?.classList.contains("sheet")) return cl.contains("notes") ? this.tree.notes() : null;
    if (el.matches(ARTIFACT)) return null;
    // Inside a list item: the label, and everything else is the item's body.
    if (parent.type === "LI" && !cl.contains("fp")) {
      if (cl.contains("lbl")) return this.tree.node(parent, "Lbl");
      parent = this.body(parent);
    }
    const tag = el.tagName.toLowerCase();
    // A drawing is part of a figure or a formula, or it is decoration.
    if (tag === "svg" || tag === "img" || tag === "canvas" || tag === "math") return FIGURES.has(parent.type) ? parent : null;
    if (cl.contains("atom")) {
      if (cl.contains("img")) {
        const alt = (el as HTMLElement).dataset.alt || el.querySelector("img")?.getAttribute("alt") || el.getAttribute("aria-label") || "";
        // A picture with no description is still a figure, so a screen reader knows it is there; a checker flags the missing text.
        if (!alt.trim()) this.undescribed++;
        return this.tree.node(parent, "Figure", { alt: alt.trim() || undefined });
      }
      if (cl.contains("math")) return this.tree.node(parent, "Formula", { alt: (el as HTMLElement).dataset.latex?.trim() || undefined });
      if (cl.contains("shape")) {
        const alt = el.getAttribute("aria-label")?.trim();
        return alt ? this.tree.node(parent, "Figure", { alt }) : parent;
      }
    }
    if (el.hasAttribute("data-href")) return this.tree.node(parent, "Link", { alt: (el.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 200) || undefined });
    if (cl.contains("fp")) return this.paragraph(el as HTMLElement, parent);
    if (tag === "table" && cl.contains("ft")) {
      const table = this.tree.node(this.block(parent), "Table");
      this.tables.add(table);
      return table;
    }
    if (tag === "tr") return parent.type === "Table" ? this.tree.node(parent, "TR") : parent;
    if (tag === "td") return parent.type === "TR" ? this.cell(el as HTMLTableCellElement, parent) : parent;
    return parent;
  }

  /** The nearest element a paragraph or table can go in: not inside another paragraph, a figure or a link. */
  private block(n: StructNode): StructNode {
    let cur: StructNode = n;
    while (INLINE.has(cur.type) && cur.parent) cur = cur.parent;
    return cur;
  }

  private body(item: StructNode): StructNode {
    let body = this.bodies.get(item);
    if (!body) this.bodies.set(item, (body = this.tree.node(item, "LBody")));
    return body;
  }

  private paragraph(el: HTMLElement, parent: StructNode): StructNode {
    const host = this.block(parent);
    const level = this.headings.get(Number(el.dataset.id));
    const lang = el.dataset.lang && el.dataset.lang !== this.lang ? el.dataset.lang : undefined;
    if (level) return this.tree.node(host, `H${Math.min(Math.max(level, 1), 6)}`, { lang });
    const label = el.querySelector(":scope > .lbl");
    if (!label) return this.tree.node(host, "P", { lang });
    // The paragraphs of one list sit in one L, however far apart the page breaks put them.
    let prev = el.previousElementSibling;
    while (prev?.matches(".pgsp, .crule")) prev = prev.previousElementSibling;
    let list = prev ? this.lists.get(prev) : undefined;
    if (!list || list.parent !== host) list = this.tree.node(host, "L", { listNumbering: numbering(label.textContent ?? "", label.classList.contains("lbl-check")) });
    this.lists.set(el, list);
    return this.tree.node(list, "LI", { lang });
  }

  private cell(td: HTMLTableCellElement, row: StructNode): StructNode {
    const tr = td.parentElement as HTMLElement | null;
    const table = td.closest("table");
    let look: { firstRow?: boolean; firstColumn?: boolean } = {};
    try {
      look = JSON.parse((table as HTMLElement | null)?.dataset.look ?? "{}");
    } catch {
      /* a table with no design */
    }
    const first = tr ? Array.from(tr.children).filter((c) => !c.matches("td.gap"))[0] === td : false;
    const rowIndex = tr?.parentElement ? Array.from(tr.parentElement.children).filter((c) => !c.matches(".pgsp")).indexOf(tr) : -1;
    const header = !!tr?.dataset.header || (!!look.firstRow && rowIndex === 0);
    const attrs: StructAttrs = { colSpan: td.colSpan > 1 ? td.colSpan : undefined, rowSpan: td.rowSpan > 1 ? td.rowSpan : undefined };
    if (header || (look.firstColumn && first)) {
      if (row.parent) this.headed.add(row.parent);
      return this.tree.node(row, "TH", { ...attrs, scope: header ? "Column" : "Row" });
    }
    return this.tree.node(row, "TD", attrs);
  }
}

/** How a list counts, from its first label: what a PDF reader announces ("Decimal", "Disc"...). */
export function numbering(label: string, checklist = false): string | undefined {
  const l = label.trim();
  if (checklist) return "None";
  if (/^\(?\d+[.)]?$/.test(l)) return "Decimal";
  if (/^\(?[ivxlcdm]+[.)]?$/.test(l)) return "LowerRoman";
  if (/^\(?[IVXLCDM]+[.)]?$/.test(l)) return "UpperRoman";
  if (/^\(?[a-z][.)]?$/.test(l)) return "LowerAlpha";
  if (/^\(?[A-Z][.)]?$/.test(l)) return "UpperAlpha";
  if (l && !/[\p{L}\p{N}]/u.test(l)) return "Disc";
  return undefined;
}
