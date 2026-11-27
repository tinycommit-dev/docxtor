// Writes the open document as a PDF, from the pages as the editor has laid them out.
//
// 1. Make a clean copy of the pages (no selection, no spelling underlines, no margin marks).
// 2. Find the fonts the text needs and read their files.
// 3. Walk the copy and write each page's content (see page.ts).
// 4. Add pictures and fonts, links, bookmarks and the document's details, and write the file.
import { currentLocale, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { stripFar } from "../editor/far";
import { renameIds } from "../editor/shapes";
import { discoverFaces } from "./fontload";
import { FontSet, type FontKey } from "./fontset.ts";
import { ImageStore } from "./images.ts";
import { fontKeyOf, PageWalker, type Sheet } from "./page.ts";
import { meetsPdfUa, srgbProfile, xmpPacket } from "./pdfa.ts";
import { StructTree } from "./structure.ts";
import { Tagger } from "./tagmap.ts";
import { mathItalic, shownChars } from "./textutil.ts";
import { num, PdfWriter, pdfDate, pdfString, ref } from "./writer.ts";

export interface PdfOptions {
  /** Called as the work goes on, with a fraction from 0 to 1. */
  progress?: (fraction: number) => void;
  /** Tag the file for screen readers and other tools: a structure tree of headings, paragraphs, lists, tables, figures and links. Default: yes. */
  tagged?: boolean;
  /** Make a PDF/A-2b archive file: an output intent, metadata that says so, and every font held in the file. Implies `tagged`. */
  pdfa?: boolean;
}

export interface PdfResult {
  bytes: Uint8Array;
  pages: number;
  /** Fonts the document asked for that the PDF draws in another font. */
  substituted: string[];
  /** Pictures linked at a web address that the site would not let the file keep: a placeholder box is drawn in their place. */
  unreadablePictures: number;
  /** The file is tagged. */
  tagged: boolean;
  /** The file says it is PDF/A-2b. False when it was asked for and could not be: text drawn in a font the file does not hold. */
  pdfa: boolean;
}

const PT = 0.75;

/** Let the browser draw and handle input for a moment. */
const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** A copy of the pages as the editor shows them, set apart from the screen, on white paper. */
function pageCopy(ed: Editor): { root: HTMLElement; stage: HTMLElement } {
  const root = document.createElement("div");
  root.className = "docxtor docxtor-pdf";
  root.dataset.theme = "light";
  root.style.cssText = "position:absolute;left:-200000px;top:0;pointer-events:none";
  const stage = ed.stageElement.cloneNode(true) as HTMLElement;
  stage.querySelectorAll("[contenteditable]").forEach((e) => e.removeAttribute("contenteditable"));
  stage.querySelectorAll(".ai-changed, .cell-sel").forEach((e) => e.classList.remove("ai-changed", "cell-sel"));
  stage.querySelectorAll(".page-num, .vruler, .np-layer, .bm-layer, .fm-layer, .sp-layer").forEach((e) => e.remove());
  // The copy shows every block, whatever the screen has left out.
  stripFar(stage);
  stage.classList.remove("story-mode");
  renameIds(stage, "-pdf");
  root.append(stage);
  document.body.append(root);
  return { root, stage };
}

/** The characters each font key draws in the copy, so their font files can be read before the walk. */
function charsByFont(stage: HTMLElement): Map<string, { key: FontKey; chars: Set<number> }> {
  const out = new Map<string, { key: FontKey; chars: Set<number> }>();
  const styles = new WeakMap<Element, CSSStyleDeclaration>();
  const walker = document.createTreeWalker(stage, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const parent = n.parentElement;
    const data = (n as Text).data;
    if (!parent || !data.trim() || parent.closest("svg, script, style")) continue;
    let cs = styles.get(parent);
    if (!cs) styles.set(parent, (cs = getComputedStyle(parent)));
    // A letter on its own in an equation is drawn in italic (MathML's math-auto), as the walk draws it.
    const key = mathItalic(cs, data) ? { ...fontKeyOf(cs), italic: true } : fontKeyOf(cs);
    const id = `${cs.fontFamily}|${key.weight}|${key.italic}`;
    let entry = out.get(id);
    if (!entry) out.set(id, (entry = { key, chars: new Set([0x20]) }));
    const transformed = (cs.textTransform !== "none" && cs.textTransform !== "math-auto") || cs.fontVariantCaps !== "normal";
    for (const ch of data) {
      if (!transformed) entry.chars.add(ch.codePointAt(0)!);
      else for (const w of [true, false]) for (const c of shownChars(ch, cs.textTransform, cs.fontVariantCaps, w).cps) entry.chars.add(c);
    }
  }
  return out;
}

type StdChar = { kindOf: "sans" | "serif" | "mono"; bold: boolean; italic: boolean; cp: number };

/** Measures characters in the standard fonts, as the browser's own Helvetica, Times and Courier draw them. */
function stdMeasurer() {
  const ctx = document.createElement("canvas").getContext("2d")!;
  const widths = new Map<string, number>();
  const bounds = new Map<string, [number, number, number, number] | null>();
  const stacks = { sans: 'Helvetica, Arial, "Liberation Sans", Arimo, sans-serif', serif: '"Times New Roman", Times, "Liberation Serif", Tinos, serif', mono: '"Courier New", Courier, "Liberation Mono", Cousine, monospace' };
  const font = (p: StdChar) => `${p.italic ? "italic " : ""}${p.bold ? "bold " : ""}1000px ${stacks[p.kindOf]}`;
  return {
    /** The advance of a character, in 1/1000 em. */
    advance(p: StdChar): number {
      const id = `${p.kindOf}${p.bold}${p.italic}${p.cp}`;
      let w = widths.get(id);
      if (w === undefined) {
        ctx.font = font(p);
        w = ctx.measureText(String.fromCodePoint(p.cp)).width;
        widths.set(id, w);
      }
      return w;
    },
    /** The box a character's ink fills, [left, bottom, right, top] in 1/1000 em from its origin, y up; null when it has none. */
    bounds(p: StdChar): [number, number, number, number] | null {
      const id = `${p.kindOf}${p.bold}${p.italic}${p.cp}`;
      let b = bounds.get(id);
      if (b === undefined) {
        ctx.font = font(p);
        const m = ctx.measureText(String.fromCodePoint(p.cp));
        const box: [number, number, number, number] = [-m.actualBoundingBoxLeft, -m.actualBoundingBoxDescent, m.actualBoundingBoxRight, m.actualBoundingBoxAscent];
        b = box[2] > box[0] && box[3] > box[1] ? box : null;
        bounds.set(id, b);
      }
      return b;
    },
  };
}

/** A URL a PDF viewer can open, or null. */
function safeUri(href: string): string | null {
  if (!/^(https?:|mailto:|tel:|ftp:)/i.test(href)) return null;
  try {
    return encodeURI(decodeURI(href)).replace(/[\x00-\x20\x7f-￿]/g, "");
  } catch {
    return null;
  }
}

export async function exportPdf(ed: Editor, opts: PdfOptions = {}): Promise<PdfResult> {
  const engine = ed.engine;
  if (!engine) throw new Error(t("No document is open"));
  const progress = opts.progress ?? (() => undefined);
  const wantPdfa = !!opts.pdfa;
  const tagged = wantPdfa || (opts.tagged ?? true);
  // Late-loading fonts and pictures change the layout, so let them finish before the pages are laid out.
  await document.fonts.ready;
  if (ed.story) ed.exitStory(false);
  ed.layoutNow();
  // What the document says about itself, before anything waits (the user can keep typing).
  const props = engine.docProps();
  // A document that names no language gets the one the interface is in: a tagged file must say what language it is in.
  const docLang = engine.pageSetup().lang;
  const lang = docLang || (tagged ? currentLocale() : undefined);
  const stem = engine.fileName.replace(/\.[^.]+$/, "") || "Untitled";
  const headingList = engine.xrefTargets().headings;
  const headingLevels = new Map(headingList.map((h) => [h.p, h.level] as const));

  // Every page is drawn while the copy is made (the one way to lay everything out for output).
  const { root, stage } = ed.withDrawn(() => pageCopy(ed));
  try {
    progress(0.02);
    await Promise.all([document.fonts.ready, ...Array.from(stage.querySelectorAll("img"), (i) => i.decode().catch(() => undefined))]);
    await tick();
    // The sheets as the copy lays them out: the pages of the PDF.
    const stageBox = stage.getBoundingClientRect();
    const boxes: Sheet[] = Array.from(stage.querySelectorAll(":scope > .sheets > .sheet"), (el) => {
      const r = el.getBoundingClientRect();
      return { left: r.left - stageBox.left, top: r.top - stageBox.top, w: r.width, h: r.height };
    });
    const pageCount = boxes.length;
    if (!pageCount) throw new Error(t("The document has no pages"));

    // Fonts: find the files, read the ones the text needs.
    const fonts = new FontSet(await discoverFaces());
    const needs = [...charsByFont(stage).values()];
    await Promise.all(needs.map((n) => fonts.prepare(n.key, n.chars)));
    progress(0.15);

    const w = new PdfWriter();
    const images = new ImageStore(w);
    const alphas = new Map<number, string>();
    const alpha = (a: number) => {
      const k = Math.round(a * 1000) / 1000;
      let name = alphas.get(k);
      if (!name) alphas.set(k, (name = `GS${alphas.size + 1}`));
      return name;
    };
    const total = stage.querySelectorAll("*").length || 1;
    let seen = 0;
    const tree = tagged ? new StructTree(pageCount) : null;
    const tagger = tree ? new Tagger(tree, headingLevels, lang) : null;
    const std = stdMeasurer();
    const walker = new PageWalker({
      stage,
      boxes,
      fonts,
      images,
      alpha,
      stdAdvance: std.advance,
      stdBounds: std.bounds,
      tagging: tree && tagger ? { tree, enter: (el, parent) => tagger.enter(el, parent) } : undefined,
      tick: async () => {
        seen += 128;
        progress(0.15 + 0.6 * Math.min(1, seen / total));
        await tick();
      },
    });
    await walker.run();

    // Where things are, for bookmarks and links inside the document.
    const flow = stage.querySelector(".flow") ?? stage;
    const anchors = new Map<string, { page: number; x: number; y: number }>();
    flow.querySelectorAll("[data-bm]").forEach((e) => {
      const at = walker.locate(e);
      for (const name of (e.getAttribute("data-bm") ?? "").split(/\s+/)) if (name && !anchors.has(name)) anchors.set(name, at);
    });
    const headings = headingList
      .map((h) => {
        const el = flow.querySelector(`[data-id="${h.p}"]`);
        return el ? { text: (h.number ? `${h.number} ` : "") + h.text, level: Math.max(1, Math.min(9, h.level)), at: walker.locate(el) } : null;
      })
      .filter((h): h is NonNullable<typeof h> => !!h && !!h.text.trim());

    progress(0.8);
    const imageXObjects = await images.finish();
    const fontEntries = await fonts.write(w);
    const gsEntries = [...alphas].map(([a, name]) => `/${name} << /Type /ExtGState /ca ${num(a)} /CA ${num(a)} >>`).join(" ");
    const shadingEntries = walker.shadings.map((s) => `/${s.name} ${s.dict}`).join(" ");
    const resources = w.add(
      `<< /ProcSet [/PDF /Text /ImageB /ImageC /ImageI] /Font << ${fontEntries} >> /XObject << ${imageXObjects} >> /ExtGState << ${gsEntries} >>${shadingEntries ? ` /Shading << ${shadingEntries} >>` : ""} >>`,
    );

    // Pages.
    const pagesObj = w.alloc();
    const pageObjs = boxes.map(() => w.alloc());
    const target = (a: { page: number; x: number; y: number }) => `[${ref(pageObjs[a.page])} /XYZ ${num(Math.max(0, a.x))} ${num(a.y)} null]`;
    const annots: number[][] = boxes.map(() => []);
    for (const l of walker.links) {
      // A link is an annotation; in a tagged file it is the content of its Link element, and says in words what it is.
      const rect = `/Rect [${l.rect.map(num).join(" ")}] /Border [0 0 0] /F 4`;
      let action = "";
      if (l.href.startsWith("#")) {
        const at = anchors.get(decodeURIComponent(l.href.slice(1)));
        if (at) action = `/Dest ${target(at)}`;
      } else {
        const uri = safeUri(l.href);
        if (uri) action = `/A << /S /URI /URI ${pdfString(uri)} >>`;
      }
      if (!action) continue;
      const obj = w.alloc();
      const parent = tree && l.node ? ` /StructParent ${tree.annotate(l.node, l.page, obj)}` : "";
      w.put(obj, `<< /Type /Annot /Subtype /Link ${rect} ${action}${parent}${tagged && l.text ? ` /Contents ${pdfString(l.text.slice(0, 200))}` : ""} >>`);
      annots[l.page].push(obj);
    }
    // An archive file holds every font it draws with; a standard font is only named.
    const pdfa = wantPdfa && !fonts.usesStandardFonts;
    for (let k = 0; k < pageCount; k++) {
      const content = await w.addStream("", new TextEncoder().encode(`${walker.stream(k, tree)}\n`));
      const b = boxes[k];
      w.put(
        pageObjs[k],
        `<< /Type /Page /Parent ${ref(pagesObj)} /MediaBox [0 0 ${num(b.w * PT)} ${num(b.h * PT)}] /Resources ${ref(resources)} /Contents ${ref(content)}` +
          `${annots[k].length ? ` /Annots [${annots[k].map(ref).join(" ")}]` : ""}${tree ? ` /StructParents ${tree.structParents(k)} /Tabs /S` : ""}${pdfa ? " /Group << /S /Transparency /CS /DeviceRGB >>" : ""} >>`,
      );
      if ((k & 7) === 7) progress(0.8 + 0.18 * (k / pageCount));
    }
    w.put(pagesObj, `<< /Type /Pages /Kids [${pageObjs.map(ref).join(" ")}] /Count ${pageCount} >>`);

    const outline = headings.length ? outlineObjects(w, headings, target) : null;
    const structRoot = tree ? tree.write(w, pageObjs) : null;
    const now = new Date();
    const title = props.title?.trim() || stem;
    const info = w.add(
      `<< /Title ${pdfString(title)}${props.author ? ` /Author ${pdfString(props.author)}` : ""}${props.subject ? ` /Subject ${pdfString(props.subject)}` : ""}${props.keywords ? ` /Keywords ${pdfString(props.keywords)}` : ""} ` +
        `/Creator ${pdfString("Docxtor")} /Producer ${pdfString("Docxtor")} /CreationDate ${pdfDate(now)} /ModDate ${pdfDate(now)} >>`,
    );
    // Metadata: the file's details, and the standards it says it follows (a tagged file, an archive file).
    let metadata = "";
    if (tagged) {
      // The PDF/UA claim is made only when the file backs it; otherwise the file stays tagged and says nothing.
      const pdfua = !!tagger && meetsPdfUa({ usesStandardFonts: fonts.usesStandardFonts, headingLevels: headingList.map((h) => h.level), figureWithoutAlt: tagger.figureWithoutAlt, headerlessTable: tagger.headerlessTable });
      const packet = xmpPacket({ title, author: props.author, subject: props.subject, keywords: props.keywords, creatorTool: "Docxtor", producer: "Docxtor", created: now, modified: now, lang, pdfa, pdfua });
      metadata = ` /Metadata ${ref(await w.addStream("/Type /Metadata /Subtype /XML", new TextEncoder().encode(packet), false))}`;
    }
    let intent = "";
    if (pdfa) {
      const profile = await w.addStream("/N 3 /Alternate /DeviceRGB", srgbProfile());
      intent = ` /OutputIntents [<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier ${pdfString("sRGB IEC61966-2.1")} /Info ${pdfString("sRGB IEC61966-2.1")} /DestOutputProfile ${ref(profile)} >>]`;
    }
    const catalog = w.add(
      `<< /Type /Catalog /Pages ${ref(pagesObj)}${outline ? ` /Outlines ${ref(outline)} /PageMode /UseOutlines` : ""}${lang ? ` /Lang ${pdfString(lang)}` : ""} /ViewerPreferences << /DisplayDocTitle true >>` +
        `${structRoot ? ` /MarkInfo << /Marked true >> /StructTreeRoot ${ref(structRoot)}` : ""}${metadata}${intent} >>`,
    );
    const id = new Uint8Array(16);
    crypto.getRandomValues(id);
    const bytes = w.build(catalog, info, id);
    progress(1);
    return { bytes, pages: pageCount, unreadablePictures: images.unreadable, substituted: [...fonts.substituted].sort((a, b) => a.localeCompare(b)), tagged, pdfa };
  } finally {
    root.remove();
  }
}

/** The bookmarks panel: one entry per heading, nested by level. Returns the outline's object number. */
function outlineObjects(w: PdfWriter, headings: { text: string; level: number; at: { page: number; x: number; y: number } }[], target: (a: { page: number; x: number; y: number }) => string): number {
  interface Node {
    obj: number;
    h: (typeof headings)[number];
    kids: Node[];
    parent: Node | null;
  }
  const rootObj = w.alloc();
  const root: Node = { obj: rootObj, h: headings[0], kids: [], parent: null };
  // A heading nests under the nearest one above it that has a lower level.
  const stack: { level: number; node: Node }[] = [];
  for (const h of headings) {
    while (stack.length && stack[stack.length - 1].level >= h.level) stack.pop();
    const parent = stack.length ? stack[stack.length - 1].node : root;
    const node: Node = { obj: w.alloc(), h, kids: [], parent };
    parent.kids.push(node);
    stack.push({ level: h.level, node });
  }
  const count = (n: Node): number => n.kids.reduce((a, k) => a + 1 + count(k), 0);
  const write = (n: Node) => {
    n.kids.forEach((k, i) => {
      const prev = n.kids[i - 1];
      const next = n.kids[i + 1];
      const first = k.kids[0];
      const lastKid = k.kids[k.kids.length - 1];
      w.put(
        k.obj,
        `<< /Title ${pdfString(k.h.text.slice(0, 200))} /Parent ${ref(n.obj)}${prev ? ` /Prev ${ref(prev.obj)}` : ""}${next ? ` /Next ${ref(next.obj)}` : ""}` +
          `${first ? ` /First ${ref(first.obj)} /Last ${ref(lastKid.obj)} /Count ${count(k)}` : ""} /Dest ${target(k.h.at)} >>`,
      );
      write(k);
    });
  };
  w.put(rootObj, `<< /Type /Outlines /First ${ref(root.kids[0].obj)} /Last ${ref(root.kids[root.kids.length - 1].obj)} /Count ${count(root)} >>`);
  write(root);
  return rootObj;
}
