// Shared by the view specs (outline, zoom, views, navigation): documents with headings, and ways to read what the
// editor shows. Nothing here changes what the app does.
import { buildDocx, openBuilt, type DocSpec } from "./docx-builder";
import { chooseMenu, type Page } from "../helpers";

/** Heading styles with outline levels, as Word's own definitions carry them. */
export const HEADING_STYLES = [1, 2, 3, 4]
  .map((n) => `<w:style w:type="paragraph" w:styleId="Heading${n}"><w:name w:val="heading ${n}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="60"/><w:outlineLvl w:val="${n - 1}"/></w:pPr><w:rPr><w:b/><w:sz w:val="${40 - 4 * n}"/></w:rPr></w:style>`)
  .join("");

/** A heading paragraph. */
export const heading = (level: number, text: string) => ({ text, ppr: `<w:pStyle w:val="Heading${level}"/>` });

/** Paragraphs of filler text, `n` of them, each `Line k` so they stay one line. */
export const filler = (n: number, prefix = "Body") => Array.from({ length: n }, (_, i) => `${prefix} ${i + 1}`);

/** A document with a title-less outline: Alpha (with Alpha one and Alpha two), Beta and Gamma, each with a line of body text. */
export const OUTLINE_DOC: DocSpec = {
  styles: HEADING_STYLES,
  sections: [
    {
      paragraphs: ["Intro text", heading(1, "Alpha"), "alpha body", heading(2, "Alpha one"), "alpha one body", heading(2, "Alpha two"), "alpha two body", heading(1, "Beta"), "beta body", heading(1, "Gamma"), "gamma body"],
    },
  ],
};

/** The same, long enough to run over several pages. */
export const LONG_OUTLINE_DOC: DocSpec = {
  styles: HEADING_STYLES,
  sections: [
    {
      paragraphs: [
        "Intro text",
        heading(1, "Alpha"),
        ...filler(45, "alpha"),
        heading(2, "Alpha one"),
        ...filler(45, "alpha one"),
        heading(1, "Beta"),
        ...filler(45, "beta"),
        heading(1, "Gamma"),
        ...filler(45, "gamma"),
      ],
    },
  ],
};

export interface Block {
  id: number;
  level: number;
  kind: string;
  text: string;
}

interface ViewEditor {
  engine: { outlineBlocks(): Block[] };
  zoom: number;
  zoomer: { mode: string; across: number; set(z: number, anchor?: unknown): void; setMode(mode: string, across?: number): void };
  pages: number;
  continuous: boolean;
  readOnly: boolean;
  folds: { folded: Set<number>; levels: number; firstLine: boolean };
  select(p: { p: number; o: number }): void;
  sel: { focus: { p: number; o: number } } | null;
  range(): [{ p: number; o: number }, { p: number; o: number }] | null;
}
export interface ViewFe {
  view: string;
  compact: boolean;
  readOnly: boolean;
  editor: ViewEditor;
  setView(id: string): boolean;
  views(): { id: string; title: string }[];
  command(id: string): Promise<unknown>;
  use(ext: unknown): Promise<void>;
  extensions: { views: { list(): { id: string }[] }; remove(id: string): Promise<void> };
}

/** Run something against the docxtor editor in the page, with the view API typed. */
export const onFe = <R, A = undefined>(page: Page, fn: (fe: ViewFe, arg: A) => R | Promise<R>, arg?: A): Promise<R> =>
  page.evaluate(([src, a]) => new Function("fe", "a", `return (${src})(fe, a)`)((window as unknown as { docxtor: { fe: unknown } }).docxtor.fe, a), [fn.toString(), arg] as const) as Promise<R>;

/** The blocks of the document as the engine has them (levels, ids, text). */
export const blocks = (page: Page): Promise<Block[]> => onFe(page, (fe) => fe.editor.engine.outlineBlocks());

/** The text of the headings, each prefixed with its level: `1 Alpha`. */
export const headings = async (page: Page): Promise<string[]> => (await blocks(page)).filter((b) => b.level > 0).map((b) => `${b.level} ${b.text}`);

/** Every block's text, in order. */
export const order = async (page: Page): Promise<string[]> => (await blocks(page)).map((b) => b.text);

/** The id of the block with this text. */
export const idOf = async (page: Page, text: string): Promise<number> => {
  const hit = (await blocks(page)).find((b) => b.text === text);
  if (!hit) throw new Error(`no block says "${text}"`);
  return hit.id;
};

/** Put the caret at the start of the block with this text (by the model, as a click would). */
export const caretIn = async (page: Page, text: string) => {
  const id = await idOf(page, text);
  await onFe(page, (fe, p) => fe.editor.select({ p, o: 0 }), id);
};

/** Open a built document, and wait for it to be laid out. */
export async function openDoc(page: Page, spec: DocSpec = OUTLINE_DOC) {
  await openBuilt(page, spec);
}

/** Choose View, Layout, `name`. */
export const chooseLayout = (page: Page, name: string) => chooseMenu(page, "View", "Layout", name);

/** The built bytes, for tests that open a file themselves. */
export const bytesOf = (spec: DocSpec) => Array.from(buildDocx(spec));

/** The flow's top-level blocks that are shown (not folded away), as their text. */
export const shownText = (page: Page): Promise<string[]> =>
  page.evaluate(() =>
    Array.from(document.querySelectorAll<HTMLElement>(".flow > .fp"))
      .filter((el) => !el.dataset.fold)
      .map((el) => (el.textContent ?? "").trim()),
  );
