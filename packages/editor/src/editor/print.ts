// Printing: the pages go to the browser's print dialog exactly as they look in the editor, and
// that dialog also saves a PDF.
import { Engine, type MPos } from "@docxtor/core";
import type { Editor } from "./editor";
import { imposition, type Multiple } from "./imposition";
import { stripFar } from "./far";
import { renameIds } from "./shapes";

/**
 * Print the pages exactly as they look here: each printed page is a copy of the stage cut to one
 * sheet, on paper of its own size.
 */
export function print(ed: Editor, after?: () => void) {
  const pages = printPages(ed);
  if (!pages) return;
  const size = document.createElement("style");
  const sheet = sheetOf(ed);
  size.textContent = sheet ? `@page { ${paper(sheet.w, sheet.h)} }` : pageRules(ed);
  document.head.append(size);
  document.body.append(pages);
  document.documentElement.classList.add("docxtor-printing");
  const done = () => {
    pages.remove();
    size.remove();
    document.documentElement.classList.remove("docxtor-printing");
    window.removeEventListener("afterprint", done);
    after?.();
  };
  window.addEventListener("afterprint", done);
  window.print();
}

/**
 * Print only the text between two positions. The selection becomes a copy of the document of its own (the engine keeps
 * the styles, headers and page setup), which is laid out in an editor out of sight, so it breaks into pages as a
 * document that holds just that text; then that editor's pages are printed. `make` builds an editor in a host element.
 */
export function printSelection(ed: Editor, from: MPos, to: MPos, make: (host: HTMLElement) => Editor): boolean {
  if (!ed.engine) return false;
  const engine = Engine.open(ed.engine.selectionCopy(from, to), ed.engine.fileName || "Selection.docx");
  const host = document.createElement("div");
  host.className = "docxtor docxtor-offscreen";
  host.dataset.theme = "light";
  document.body.append(host);
  const copy = make(host);
  const finish = () => {
    copy.destroy();
    engine.dispose();
    host.remove();
  };
  try {
    copy.load(engine);
    copy.layoutNow();
    print(copy, finish);
  } catch (e) {
    finish();
    throw e;
  }
  return true;
}

/** The paper size of a sheet, in the units CSS paper sizes use. */
const paper = (w: number, h: number) => `size: ${w / 96}in ${h / 96}in; margin: 0;`;

/** Sheets of one size share a named page; with a single size there are no names and the plain page rule is all there is. */
function pageNames(ed: Editor): (string | undefined)[] {
  const sizes = new Map<string, string>();
  const names: string[] = [];
  for (let k = 0; k < ed.pages; k++) {
    const b = ed.pageBox(k);
    const key = `${b.w}x${b.h}`;
    if (!sizes.has(key)) sizes.set(key, `s${sizes.size}`);
    names[k] = sizes.get(key)!;
  }
  return sizes.size > 1 ? names : [];
}

/** The `@page` rules: the first sheet's paper, and when sheets differ, one named page per paper (`page: sN` on the sheets). */
function pageRules(ed: Editor): string {
  const names = pageNames(ed);
  const first = ed.pageBox(0);
  const rules = [`@page { ${paper(first.w, first.h)} }`];
  const done = new Set<string>();
  names.forEach((n, k) => {
    if (!n || done.has(n)) return;
    done.add(n);
    const b = ed.pageBox(k);
    rules.push(`@page ${n} { ${paper(b.w, b.h)} }`);
  });
  return rules.join("\n");
}

/** How pages go two to a sheet, when the document asks for it, and the paper (in pixels). */
function sheetOf(ed: Editor): { mode: Multiple; w: number; h: number; booklet: number } | null {
  const setup = ed.engine?.pageSetup();
  if (!setup || (!setup.bookFold && !setup.printTwoOnOne)) return null;
  const mode: Multiple = setup.bookFold === "bookFoldRev" ? "bookFoldRev" : setup.bookFold ? "bookFold" : "twoUp";
  return { mode, w: setup.width / 15, h: setup.height / 15, booklet: setup.bookFoldSheets ?? 0 };
}

/** One page of the stage, as a window of its own size (the stage moved so the page is in it). */
function pageWindow(ed: Editor, k: number): HTMLElement {
  const b = ed.pageBox(k);
  const win = document.createElement("div");
  win.className = "print-half";
  win.style.cssText = `width:${b.w}px;height:${b.h}px`;
  const copy = cleanCopy(ed, k);
  copy.style.top = `${-b.top}px`;
  copy.style.left = `${-b.left}px`;
  win.append(copy);
  return win;
}

/** A copy of the stage for page k, without what is only for the screen. */
function cleanCopy(ed: Editor, k: number): HTMLElement {
  const copy = ed.stageElement.cloneNode(true) as HTMLElement;
  // The copy shows every block, whatever the screen has left out.
  stripFar(copy);
  copy.querySelectorAll("[contenteditable]").forEach((e) => e.removeAttribute("contenteditable"));
  renameIds(copy, `-p${k}`);
  copy.querySelectorAll(".ai-changed, .cell-sel").forEach((e) => e.classList.remove("ai-changed", "cell-sel"));
  copy.querySelectorAll(".ctl-on, .perm-start, .perm-end").forEach((e) => e.classList.remove("ctl-on", "perm-start", "perm-end"));
  copy.querySelectorAll(".page-num, .vruler, .np-layer, .bm-layer, .fm-layer, .sp-layer, .xs-layer, .pg-guides").forEach((e) => e.remove());
  return copy;
}

/** The pages to print: one sheet-sized window onto a copy of the stage per page, or two to a sheet when the document asks. */
export function printPages(ed: Editor): HTMLElement | null {
  if (!ed.engine) return null;
  if (ed.story) ed.exitStory(false);
  ed.layoutNow();
  // Every page is drawn while the copies are made (the one way to lay everything out for output).
  return ed.withDrawn(() => printedPages(ed));
}

function printedPages(ed: Editor): HTMLElement | null {
  const sheet = sheetOf(ed);
  if (sheet) {
    const box = document.createElement("div");
    box.className = "docxtor docxtor-print";
    box.dataset.theme = "light";
    for (const side of imposition(ed.pages, sheet.mode, sheet.booklet)) {
      const page = document.createElement("div");
      page.className = "print-page print-sheet";
      page.style.cssText = `width:${sheet.w}px;height:${sheet.h}px;flex-direction:${sheet.w >= sheet.h ? "row" : "column"}`;
      for (const k of side) {
        if (k === null) {
          const blank = document.createElement("div");
          blank.className = "print-half";
          blank.style.cssText = sheet.w >= sheet.h ? `width:${sheet.w / 2}px;height:${sheet.h}px` : `width:${sheet.w}px;height:${sheet.h / 2}px`;
          page.append(blank);
        } else page.append(pageWindow(ed, k));
      }
      box.append(page);
    }
    return box;
  }
  const names = pageNames(ed);
  // Pages are always paper white with black text, whatever the interface theme.
  const box = document.createElement("div");
  box.className = "docxtor docxtor-print";
  box.dataset.theme = "light";
  for (let k = 0; k < ed.pages; k++) {
    const b = ed.pageBox(k);
    const page = document.createElement("div");
    page.className = "print-page";
    page.style.cssText = `width:${b.w}px;height:${b.h}px`;
    // Each sheet prints on paper of its own size and orientation.
    if (names[k]) page.style.setProperty("page", names[k]!);
    const copy = cleanCopy(ed, k);
    copy.style.top = `${-b.top}px`;
    copy.style.left = `${-b.left}px`;
    page.append(copy);
    box.append(page);
  }
  return box;
}
