// Tools > Word count: pages, words, characters, paragraphs and lines of the main text, and of the selection when there
// is one ("12 of 1,204"), like Word. A checkbox adds the footnotes, endnotes and text boxes to the totals, as Word's
// "Include textboxes, footnotes and endnotes" does. Headers and footers are never counted.
//
// The engine counts the text; lines depend on the layout, so they are counted here, from the pages on screen.
import { currentLocale, t } from "@docxtor/extensions";
import type { TextCounts } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { linesIn } from "../editor/lines";
import { h, modal } from "./dom";

export { linesIn };

export interface Counts {
  words: number;
  chars: number;
  charsNoSpaces: number;
}

/** Words are runs of non-space characters, the way Word counts them. Line breaks aren't characters. */
export function count(text: string): Counts {
  const flat = text.replace(/[\r\n]+/g, " ").trim();
  const chars = text.replace(/[\r\n]/g, "");
  return {
    words: flat ? flat.split(/\s+/).length : 0,
    chars: [...chars].length,
    charsNoSpaces: [...chars.replace(/\s/g, "")].length,
  };
}

export interface LineCounts {
  main: number;
  notes: number;
  boxes: number;
}

/**
 * Lines of the main text, of the footnotes and endnotes, and of the text boxes, as the pages show them. The text and its boxes are
 * counted from the page map (the paginator records each block's lines when it measures it); the notes are on the sheets.
 */
export function countLines(ed: Editor): LineCounts {
  const out: LineCounts = { main: 0, notes: 0, boxes: 0 };
  const text = ed.lineTotals();
  out.main = text.main;
  out.boxes = text.boxes;
  for (const el of Array.from(ed.sheetsElement.querySelectorAll<HTMLElement>(".fp"))) {
    // Headers and footers are not counted.
    if (el.closest(".hf, .hdr-repeat, .hdr, .ftr")) continue;
    const kind = el.closest(".shape-text") ? "boxes" : el.closest(".notes") ? "notes" : "main";
    out[kind] += linesIn(el);
  }
  return out;
}

const plus = (a: TextCounts, b: TextCounts): TextCounts => ({ paragraphs: a.paragraphs + b.paragraphs, words: a.words + b.words, chars: a.chars + b.chars, charsNoSpaces: a.charsNoSpaces + b.charsNoSpaces });

export function openWordCount(ed: Editor, opts: { extras?: boolean; onExtras?: (on: boolean) => void } = {}) {
  if (!ed.engine) return;
  const all = ed.engine.counts();
  const lines = countLines(ed);
  const selected = ed.collapsed() ? null : count(ed.selectedText());
  const selectedParagraphs = ed.collapsed() ? 0 : ed.selectedParagraphs().length;
  let nf: Intl.NumberFormat;
  try {
    nf = new Intl.NumberFormat(currentLocale());
  } catch {
    nf = new Intl.NumberFormat();
  }
  const n = (x: number) => nf.format(x);
  const value = (part: number | null, whole: number) => (part === null ? n(whole) : t("{part} of {whole}", { part: n(part), whole: n(whole) }));
  const extras = h("input", { type: "checkbox", id: "wc-extras", checked: opts.extras ?? true });
  const hasExtras = all.notes.paragraphs + all.boxes.paragraphs > 0;
  const table = h("dl", { class: "word-count" });
  const dds = new Map<string, HTMLElement>();
  const paint = () => {
    const total = extras.checked ? plus(plus(all, all.notes), all.boxes) : { paragraphs: all.paragraphs, words: all.words, chars: all.chars, charsNoSpaces: all.charsNoSpaces };
    const lineTotal = lines.main + (extras.checked ? lines.notes + lines.boxes : 0);
    const rows: [string, string][] = [
      [t("Pages"), n(ed.pages)],
      [t("Words"), value(selected?.words ?? null, total.words)],
      [t("Characters"), value(selected?.chars ?? null, total.chars)],
      [t("Characters excluding spaces"), value(selected?.charsNoSpaces ?? null, total.charsNoSpaces)],
      [t("Paragraphs"), value(selected ? selectedParagraphs : null, total.paragraphs)],
      [t("Lines"), n(lineTotal)],
    ];
    if (!dds.size) {
      for (const [k, v] of rows) {
        const dd = h("dd", { class: "readout" }, v);
        dds.set(k, dd);
        table.append(h("dt", {}, k), dd);
      }
    } else for (const [k, v] of rows) dds.get(k)!.textContent = v;
  };
  extras.addEventListener("change", () => {
    paint();
    opts.onExtras?.(extras.checked);
  });
  paint();
  const ok = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const sub = selected ? t("The selection, then the whole document. Headers and footers aren't counted.") : t("The main text. Headers and footers aren't counted.");
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Word count"), style: "width:min(var(--dialog-s),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Word count")), h("p", { class: "sub" }, sub)),
    h(
      "div",
      { class: "dialog-body" },
      table,
      h("div", { class: "pref-check wc-extras" }, h("label", { class: "check", for: "wc-extras" }, extras, h("span", {}, t("Include textboxes, footnotes and endnotes"))), hasExtras ? null : h("p", { class: "pref-help" }, t("This document has none."))),
    ),
    h("div", { class: "dialog-actions" }, ok),
  );
  const close = modal(dialog, () => ed.focus());
  ok.addEventListener("click", close);
  ok.focus();
}
