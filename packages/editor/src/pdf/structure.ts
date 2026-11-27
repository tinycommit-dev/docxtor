// The structure tree of a tagged PDF (the file's logical structure: headings, paragraphs, lists, tables, figures, links), and
// the marked content in the page streams that ties each piece of what is drawn to its place in the tree. A screen reader
// walks the tree; the marked content says which text and pictures belong to which element. Content that is not part of the
// document's text (backgrounds, borders, page numbers, headers and footers) is marked as an artifact, so it is skipped.
// No DOM: the walker fills the tree, this writes it.
import { num, pdfString, ref, type PdfWriter } from "./writer.ts";

/** What a structure element says about itself, beyond its type. */
export interface StructAttrs {
  /** The alternative text of a figure or a formula. */
  alt?: string;
  /** The language of this part, when it is not the document's. */
  lang?: string;
  /** A table header cell's scope. */
  scope?: "Column" | "Row" | "Both";
  /** A table cell spanning columns or rows. */
  colSpan?: number;
  rowSpan?: number;
  /** A list's numbering: `Decimal`, `Disc`... or `None`. */
  listNumbering?: string;
}

/** A piece of a page marked as the content of a structure element. */
interface Marked {
  kind: "mc";
  page: number;
  mcid: number;
  /** Where in reading order the content was met (see `StructTree.next`). */
  seq: number;
}

/** A link annotation that is the content of a Link element. */
interface Annotation {
  kind: "obj";
  page: number;
  obj: number;
  seq: number;
}

export interface StructNode {
  readonly type: string;
  readonly parent: StructNode | null;
  readonly attrs: StructAttrs;
  /** Children in reading order: elements, marked content, annotations. */
  readonly kids: (StructNode | Marked | Annotation)[];
  /** Where in reading order the element was met. */
  readonly seq: number;
}

/** Types that end up with no children are left out of the file, except the cells and rows of a table that has text. */
const KEEP_EMPTY = new Set(["TR", "TD", "TH"]);

export class StructTree {
  readonly root: StructNode = { type: "Document", parent: null, attrs: {}, kids: [], seq: 0 };
  private order = 1;
  /** How many pages the file has: annotations are numbered after them in the parent tree. */
  readonly pages: number;

  constructor(pages: number) {
    this.pages = pages;
  }
  /** Elements that come after everything else in reading order (footnotes: they sit on the page in front of the text in the page's markup). */
  private tail: StructNode | null = null;
  private nextMcid: number[] = [];
  /** The innermost element of every marked-content number, by page: the parent tree's first half. */
  private owners: StructNode[][] = [];
  private annotations: { key: number; node: StructNode }[] = [];

  /**
   * A number that puts things in reading order: elements get one when they are made, and drawing gets one when the page walker
   * meets it. Marked content is added when each page's stream is written, which is later than the elements are made, so
   * without these numbers a paragraph's text would all come after a link in the middle of it.
   */
  next(): number {
    return this.order++;
  }

  /** A new element, among the children of `parent` (kept in reading order when the tree is written). */
  node(parent: StructNode, type: string, attrs: StructAttrs = {}): StructNode {
    const n: StructNode = { type, parent, attrs, kids: [], seq: this.next() };
    parent.kids.push(n);
    return n;
  }

  /** The group for notes: a section at the end of the document, made when first needed. */
  notes(type = "Sect"): StructNode {
    this.tail ??= this.node(this.root, type);
    return this.tail;
  }

  /** The next marked-content number on a page, given to `node`. `seq` is where the content was met (see `next`). */
  mark(node: StructNode, page: number, seq = this.next()): number {
    const mcid = (this.nextMcid[page] ??= 0);
    this.nextMcid[page] = mcid + 1;
    node.kids.push({ kind: "mc", page, mcid, seq });
    (this.owners[page] ??= [])[mcid] = node;
    return mcid;
  }

  /** Make a link annotation the content of a Link element. Returns the annotation's `/StructParent` number. */
  annotate(node: StructNode, page: number, obj: number): number {
    node.kids.push({ kind: "obj", page, obj, seq: this.next() });
    const key = this.pages + this.annotations.length;
    this.annotations.push({ key, node });
    return key;
  }

  /** Whether an element or anything below it has marked content or an annotation. */
  private hasContent(n: StructNode): boolean {
    return n.kids.some((k) => ("type" in k ? this.hasContent(k) : true));
  }

  /** The `/StructParents` number of a page: its index, since the pages come first in the parent tree. */
  structParents(page: number): number {
    return page;
  }

  /**
   * Write the tree: the structure tree root, one element object per element that has content, and the parent tree that
   * goes from a page's marked content (and an annotation) back to its element. `pageObjs` are the objects of the pages.
   * Returns the object number of the structure tree root.
   */
  write(w: PdfWriter, pageObjs: number[]): number {
    // Elements and marked content were added in the order they were met on each page, and the pages' markup is not in reading
    // order everywhere; put each element's children in the order their numbers say.
    const order = (n: StructNode) => {
      n.kids.sort((a, b) => a.seq - b.seq);
      for (const k of n.kids) if ("type" in k) order(k);
    };
    order(this.root);
    // The notes come after the text that refers to them, though the pages' markup lists them first.
    if (this.tail) {
      const at = this.root.kids.indexOf(this.tail);
      if (at >= 0) this.root.kids.push(...this.root.kids.splice(at, 1));
    }
    const objs = new Map<StructNode, number>();
    const parentTree = w.alloc();
    const rootObj = w.alloc();
    const kept = (n: StructNode): boolean => n === this.root || this.hasContent(n);
    // A table keeps all its rows and cells once any of it has text, so the grid is whole.
    const keepAll = new Set<StructNode>();
    const mark = (n: StructNode, inTable: boolean) => {
      for (const k of n.kids) {
        if (!("type" in k)) continue;
        const table = inTable || (k.type === "Table" && this.hasContent(k));
        if (table && KEEP_EMPTY.has(k.type)) keepAll.add(k);
        mark(k, table);
      }
    };
    mark(this.root, false);
    const wanted = (n: StructNode) => kept(n) || keepAll.has(n);
    const assign = (n: StructNode) => {
      objs.set(n, w.alloc());
      for (const k of n.kids) if ("type" in k && wanted(k)) assign(k);
    };
    assign(this.root);

    const pageRef = (p: number) => ref(pageObjs[p]!);
    const body = (n: StructNode): string => {
      const parts: string[] = [];
      // `/Pg` names the page of the integer marked-content entries; entries from another page say theirs.
      const first = n.kids.find((k): k is Marked | Annotation => !("type" in k));
      const own = first?.page;
      for (const k of n.kids) {
        if ("type" in k) {
          if (wanted(k)) parts.push(ref(objs.get(k)!));
        } else if (k.kind === "mc") parts.push(k.page === own ? String(k.mcid) : `<< /Type /MCR /Pg ${pageRef(k.page)} /MCID ${k.mcid} >>`);
        else parts.push(`<< /Type /OBJR /Pg ${pageRef(k.page)} /Obj ${ref(k.obj)} >>`);
      }
      const attrs = n.attrs;
      const table = n.type === "TH" || n.type === "TD" ? [attrs.scope ? `/Scope /${attrs.scope}` : "", attrs.colSpan && attrs.colSpan > 1 ? `/ColSpan ${attrs.colSpan}` : "", attrs.rowSpan && attrs.rowSpan > 1 ? `/RowSpan ${attrs.rowSpan}` : ""].filter(Boolean) : [];
      const list = n.type === "L" && attrs.listNumbering ? [`/ListNumbering /${attrs.listNumbering}`] : [];
      const owner = table.length ? "Table" : list.length ? "List" : "";
      const a = owner ? ` /A << /O /${owner} ${[...table, ...list].join(" ")} >>` : "";
      return (
        `<< /Type /StructElem /S /${n.type} /P ${ref(n.parent ? objs.get(n.parent)! : rootObj)}` +
        `${own !== undefined ? ` /Pg ${pageRef(own)}` : ""}${a}${attrs.alt ? ` /Alt ${pdfString(attrs.alt)}` : ""}${attrs.lang ? ` /Lang ${pdfString(attrs.lang)}` : ""} /K [${parts.join(" ")}] >>`
      );
    };
    for (const [n, obj] of objs) w.put(obj, body(n));

    // The parent tree: for a page, the element of each marked-content number, in order; for an annotation, its element.
    const nums: string[] = [];
    const owned = (n: StructNode): StructNode => {
      // The nearest ancestor that was written (an element left out has its content in the next one up).
      let cur: StructNode | null = n;
      while (cur && !objs.has(cur)) cur = cur.parent;
      return cur ?? this.root;
    };
    for (let page = 0; page < pageObjs.length; page++) {
      const list = this.owners[page];
      if (!list?.length) continue;
      nums.push(`${page} [${Array.from(list, (n) => ref(objs.get(owned(n))!)).join(" ")}]`);
    }
    for (const a of this.annotations) nums.push(`${a.key} ${ref(objs.get(owned(a.node))!)}`);
    w.put(parentTree, `<< /Nums [${nums.join(" ")}] >>`);
    w.put(rootObj, `<< /Type /StructTreeRoot /K [${ref(objs.get(this.root)!)}] /ParentTree ${ref(parentTree)} /ParentTreeNextKey ${num(this.pages + this.annotations.length)} >>`);
    return rootObj;
  }
}

/** One piece of a page's drawing: operators, and the element the content belongs to (none: an artifact). */
export interface Item {
  s: string;
  /** The structure element this content belongs to. Undefined: an artifact (not part of the document's text). */
  tag?: StructNode;
  /** Graphics state and clipping operators: they stay outside any marked content. */
  raw?: boolean;
  /** Always an artifact, even in a file that is not tagged (the hyphen at the end of a line). */
  artifact?: boolean;
  /** Where in reading order it was met (`StructTree.next`). */
  seq?: number;
}

/**
 * The content stream of a page from its items. Runs of items for the same element become one marked-content sequence
 * with the next number on the page; runs of artifacts are wrapped as artifacts; state operators stay between them.
 * Without a tree it is the operators as they are.
 */
export function pageStream(items: Item[], page: number, tree: StructTree | null): string {
  if (!tree) return items.map((i) => (i.artifact ? `/Artifact BMC ${i.s} EMC` : i.s)).join("\n");
  const out: string[] = [];
  for (let i = 0; i < items.length; ) {
    const it = items[i]!;
    if (it.raw) {
      out.push(it.s);
      i++;
      continue;
    }
    let j = i;
    const group: string[] = [];
    while (j < items.length && !items[j]!.raw && items[j]!.tag === it.tag) group.push(items[j++]!.s);
    if (it.tag) out.push(`/${it.tag.type} << /MCID ${tree.mark(it.tag, page, it.seq)} >> BDC`, ...group, "EMC");
    else out.push("/Artifact BMC", ...group, "EMC");
    i = j;
  }
  return out.join("\n");
}
