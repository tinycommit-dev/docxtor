// Go to: jump to a page, section, line, bookmark, note, field, table, picture, equation or heading, as Word's Go to
// does. A number goes to that one (+3 and -2 move that far from where the caret is); Next and Previous step through
// the places of the kind chosen. The dialog stays open so several jumps can be made.
import type { GotoKind, GotoTarget, MPos } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { lineRows, type LineRow } from "../editor/lines";
import { domToModel, modelToDom } from "../editor/selection";
import { h, modal } from "./dom";

export type GoKind = "page" | "section" | "line" | "bookmark" | GotoKind;

const KINDS: { id: GoKind; label: string }[] = [
  { id: "page", label: msg("Page") },
  { id: "section", label: msg("Section") },
  { id: "line", label: msg("Line") },
  { id: "bookmark", label: msg("Bookmark") },
  { id: "footnote", label: msg("Footnote") },
  { id: "endnote", label: msg("Endnote") },
  { id: "field", label: msg("Field") },
  { id: "table", label: msg("Table") },
  { id: "graphic", label: msg("Graphic") },
  { id: "equation", label: msg("Equation") },
  { id: "heading", label: msg("Heading") },
];

/** A place the editor can select. */
interface Place {
  p: number;
  o: number;
  endP: number;
  endO: number;
  label: string;
}

export const KIND_NAMES: Record<GoKind, string> = Object.fromEntries(KINDS.map((k) => [k.id, k.label])) as Record<GoKind, string>;

/** The paragraph's place in the text, to tell what comes before and after the caret. */
function paraOrder(ed: Editor): Map<number, number> {
  return new Map(Array.from(ed.flow.querySelectorAll<HTMLElement>("[data-p]")).map((e, i) => [Number(e.dataset.p), i]));
}

/** The places of a kind in the main text, in order. */
export function placesOf(ed: Editor, kind: GoKind): Place[] {
  if (kind === "bookmark") return ed.bookmarks().filter((b) => !b.story).map((b) => ({ p: b.p, o: b.o, endP: b.endP, endO: b.endO, label: b.name }));
  if (kind === "page" || kind === "line") return [];
  return ed.engine.gotoTargets(kind).map((x: GotoTarget) => ({ p: x.p, o: x.o, endP: x.endP, endO: x.endO, label: x.label }));
}

/** Select a place and scroll to it. */
export function selectPlace(ed: Editor, at: Place): boolean {
  return ed.selectBookmark({ name: "", p: at.p, o: at.o, endP: at.endP, endO: at.endO, text: "" });
}

/** The index of the first place after the caret (or `places.length` if none), for stepping forward. */
export function indexAfter(ed: Editor, places: Place[]): number {
  const caret = ed.story ? null : ed.range()?.[1];
  const order = paraOrder(ed);
  const at = caret ? order.get(caret.p) : undefined;
  if (at === undefined || !caret) return 0;
  const i = places.findIndex((x) => {
    const a = order.get(x.p);
    return a !== undefined && (a > at || (a === at && x.o > caret.o));
  });
  return i < 0 ? places.length : i;
}

/** The index of the last place before the caret (or -1), for stepping back. */
export function indexBefore(ed: Editor, places: Place[]): number {
  const caret = ed.story ? null : ed.range()?.[0];
  const order = paraOrder(ed);
  const at = caret ? order.get(caret.p) : undefined;
  if (at === undefined || !caret) return places.length - 1;
  let last = -1;
  places.forEach((x, i) => {
    const a = order.get(x.p);
    if (a !== undefined && (a < at || (a === at && x.o < caret.o))) last = i;
  });
  return last;
}

/** Go to the next (+1) or previous (-1) place of a kind; false when there is none. */
export function stepPlace(ed: Editor, kind: GoKind, d: 1 | -1): boolean {
  const places = placesOf(ed, kind);
  if (!places.length) return false;
  const i = d > 0 ? indexAfter(ed, places) : indexBefore(ed, places);
  const at = places[(i + places.length) % places.length];
  return selectPlace(ed, at);
}

/** The lines of the text of the body, top to bottom, counted by paragraph: from the page map (see `Editor.paragraphLines`), so no line of a block that is not drawn has to be looked for. */
function lineIndex(ed: Editor): { p: number; n: number }[] {
  return ed.paragraphLines();
}

/** Every line of text in a paragraph as laid out now (the paragraph is drawn first). */
function linesOf(ed: Editor, el: HTMLElement): LineRow[] {
  ed.ensureDrawn(el);
  return lineRows(el);
}

/** The model position of the first character of a paragraph that is drawn on a line. */
function firstOnLine(ed: Editor, para: HTMLElement, line: LineRow): MPos | null {
  const walker = document.createTreeWalker(para, NodeFilter.SHOW_TEXT, { acceptNode: (n) => ((n.parentElement?.closest(".lbl, .pgsp") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT) as number) });
  const range = document.createRange();
  for (let n = walker.nextNode() as Text | null; n; n = walker.nextNode() as Text | null) {
    for (let i = 0; i < n.data.length; i++) {
      range.setStart(n, i);
      range.setEnd(n, i + 1);
      const r = range.getBoundingClientRect();
      const mid = (r.top + r.bottom) / 2;
      if (r.height > 0 && mid >= line.top - 1 && mid <= line.bottom + 1) return domToModel(ed.flow, n, i);
    }
  }
  return null;
}

/** Where the character after a position is drawn (the position itself at the end of the text): it says which line a caret at a wrap belongs to. */
function rectOfNextChar(para: HTMLElement, node: Node, offset: number): DOMRect | null {
  const range = document.createRange();
  const walker = document.createTreeWalker(para, NodeFilter.SHOW_TEXT);
  let text: Text | null = node.nodeType === Node.TEXT_NODE ? (node as Text) : null;
  let at = offset;
  if (!text) {
    range.setStart(node, offset);
    range.collapse(true);
    return range.getClientRects()[0] ?? null;
  }
  // At the end of a piece of text the next character is the first of the text after it.
  while (text && at >= text.data.length) {
    walker.currentNode = text;
    text = walker.nextNode() as Text | null;
    at = 0;
  }
  if (!text) {
    range.setStart(node, offset);
    range.collapse(true);
    return range.getClientRects()[0] ?? null;
  }
  range.setStart(text, at);
  range.setEnd(text, at + 1);
  return range.getClientRects()[0] ?? null;
}

/** The line number (from 1) the caret is on: the lines of every paragraph before it, from the page map, and the lines of its own paragraph down to the caret. */
export function currentLine(ed: Editor): number {
  const caret = ed.story ? null : ed.range()?.[0];
  if (!caret) return 1;
  const el = ed.flow.querySelector<HTMLElement>(`[data-p="${caret.p}"]`);
  if (!el) return 1;
  let before = 0;
  for (const l of lineIndex(ed)) {
    if (l.p === caret.p) break;
    before += l.n;
  }
  ed.ensureDrawn(el);
  let top = el.getBoundingClientRect().top;
  // The line the caret itself is on, read from where the position is drawn (the dialog may hold the focus).
  const at = modelToDom(ed.flow, caret);
  if (at) {
    const r = rectOfNextChar(el, at.node, at.offset);
    if (r && r.height) top = r.top;
  }
  let own = 0;
  for (const row of lineRows(el)) if (row.top <= top + 2) own++;
  return Math.max(1, before + own);
}

/** Put the caret at the start of a line of the body (from 1) and scroll to it. False when there is no such line. */
export function goToLine(ed: Editor, n: number): boolean {
  if (n < 1) return false;
  // Which paragraph holds the line, and which of its lines it is: counted from the page map.
  let left = n;
  let at: { p: number; k: number } | null = null;
  for (const l of lineIndex(ed)) {
    if (left <= l.n) {
      at = { p: l.p, k: left - 1 };
      break;
    }
    left -= l.n;
  }
  if (!at) return false;
  const para = ed.flow.querySelector<HTMLElement>(`[data-p="${at.p}"]`);
  if (!para) return false;
  const rows = linesOf(ed, para);
  const l = rows[Math.min(at.k, rows.length - 1)];
  if (!l) return false;
  const c = ed.canvas.getBoundingClientRect();
  ed.canvas.scrollTop += l.top - c.top - c.height / 3;
  // Find the first character on the line. The dialog covers the page, so the position can't be read off the screen by pointing.
  const now = linesOf(ed, para);
  const here = now[Math.min(at.k, now.length - 1)] ?? l;
  const pos: MPos | null = firstOnLine(ed, para, here) ?? { p: at.p, o: 0 };
  ed.select(pos);
  ed.focus();
  ed.emitSelection();
  return true;
}

/** Scroll to a page (from 1) and put the caret at the first text on it. False when there is no such page. */
export function goToPage(ed: Editor, n: number): boolean {
  if (n < 1 || n > ed.pages) return false;
  const box = ed.pageBox(n - 1);
  const stage = ed.stageElement.getBoundingClientRect();
  const c = ed.canvas.getBoundingClientRect();
  const top = stage.top + box.top * ed.zoom;
  ed.canvas.scrollTop += top - c.top - 16;
  const here = ed.stageElement.getBoundingClientRect().top + box.top * ed.zoom;
  // The first paragraph the page map puts on this page (it is drawn first), unless that one starts on a later page: the page holds no paragraph's start.
  const id = ed.firstParagraphOnPage(n - 1);
  const first = id === null ? null : ed.flow.querySelector<HTMLElement>(`[data-p="${id}"]`);
  if (first) {
    ed.ensureDrawn(first);
    const r = first.getBoundingClientRect();
    if (r.height > 0 && r.top >= here - 2 && r.top < here + box.h * ed.zoom) {
      ed.select({ p: Number(first.dataset.p), o: 0 });
      ed.focus();
      ed.emitSelection();
    }
  }
  return true;
}

export function openGoto(ed: Editor, start: GoKind = "page") {
  if (!ed.engine) return;
  let kind: GoKind = start;
  const kinds = h("div", { class: "goto-kinds", role: "listbox", "aria-label": t("Go to what") });
  const label = h("label", { for: "goto-input" });
  const input = h("input", { type: "text", id: "goto-input", autocomplete: "off", spellcheck: "false", inputmode: "text" });
  const list = h("select", { id: "goto-list", "aria-label": t("Choose one") });
  const listField = h("div", { class: "field", style: "margin:0" }, h("label", { for: "goto-list" }, t("Or choose")), list);
  const help = h("p", { class: "goto-help", role: "status", "aria-live": "polite" });
  const prev = h("button", { class: "btn", type: "button" }, t("Previous"));
  const next = h("button", { class: "btn", type: "button" }, t("Next"));
  const go = h("button", { class: "btn primary", type: "button" }, t("Go to"));
  const close = h("button", { class: "btn ghost", type: "button" }, t("Close"));
  const side = h("div", { class: "goto-side" }, h("div", { class: "field", style: "margin:0" }, label, input), listField, help);
  const dialog = h(
    "div",
    { class: "dialog goto-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Go to"), style: "width:min(var(--dialog-m),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Go to"))),
    h("div", { class: "dialog-body" }, h("div", { class: "goto-body" }, kinds, side)),
    h("div", { class: "dialog-actions" }, close, prev, next, go),
  );
  const closeDialog = modal(dialog);
  close.addEventListener("click", closeDialog);
  const say = (text: string) => (help.textContent = text);

  const listed = () => kind !== "page" && kind !== "line" && kind !== "section";
  const refresh = () => {
    kinds.querySelectorAll<HTMLElement>(".goto-kind").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.kind === kind)));
    const name = t(KIND_NAMES[kind]);
    input.value = "";
    listField.hidden = !listed();
    input.hidden = kind === "bookmark";
    label.hidden = kind === "bookmark";
    label.textContent = kind === "page" ? t("Page number") : kind === "line" ? t("Line number") : t("{kind} number", { kind: name });
    say(kind === "page" || kind === "line" ? t("Type a number, or + or - and a number to move that far from here.") : kind === "bookmark" ? "" : t("Type a number, or + or - and a number. Next and Previous move one at a time."));
    if (listed()) {
      const places = placesOf(ed, kind);
      list.replaceChildren(h("option", { value: "" }, places.length ? t("{n} in the document", { n: places.length }) : t("None in the document")), ...places.map((x, i) => h("option", { value: String(i) }, x.label)));
      list.disabled = !places.length;
    }
  };
  for (const k of KINDS) {
    const b = h("button", { class: "goto-kind", type: "button", role: "option", "data-kind": k.id, "aria-selected": "false" }, t(k.label));
    b.addEventListener("click", () => {
      kind = k.id;
      refresh();
      (kind === "bookmark" ? list : input).focus();
    });
    kinds.append(b);
  }
  list.addEventListener("change", () => {
    if (!list.value) return;
    const places = placesOf(ed, kind);
    const at = places[Number(list.value)];
    if (at && selectPlace(ed, at)) say(at.label);
    list.focus();
  });

  /** The number typed: [relative, n], or null when it isn't one. */
  const parsed = (): [boolean, number] | null => {
    const v = input.value.trim();
    if (!/^[+-]?\d+$/.test(v)) return null;
    return [v.startsWith("+") || v.startsWith("-"), Number(v)];
  };

  const goNumber = (): boolean => {
    const p = parsed();
    if (!p) {
      say(t("Type a number first."));
      return false;
    }
    const [relative, n] = p;
    if (kind === "page") {
      const target = relative ? ed.currentPage() + n : n;
      if (!goToPage(ed, target)) {
        say(t("The document has {n} pages.", { n: ed.pages }));
        return false;
      }
      say(t("Page {n}", { n: target }));
      return true;
    }
    if (kind === "line") {
      const target = relative ? currentLine(ed) + n : n;
      if (!goToLine(ed, target)) {
        say(t("There is no line {n}.", { n: target }));
        return false;
      }
      say(t("Line {n}", { n: target }));
      return true;
    }
    const places = placesOf(ed, kind);
    if (!places.length) {
      say(t("There are none in the document."));
      return false;
    }
    // Relative to where the caret is: +1 is the next one, -1 the previous.
    const target = relative ? (n > 0 ? indexAfter(ed, places) + n - 1 : indexBefore(ed, places) + n + 1) : n - 1;
    if (target < 0 || target >= places.length) {
      say(t("There are {n} in the document.", { n: places.length }));
      return false;
    }
    selectPlace(ed, places[target]);
    say(places[target].label);
    return true;
  };

  const step = (d: 1 | -1) => {
    if (kind === "page") {
      const target = ed.currentPage() + d;
      if (goToPage(ed, target)) say(t("Page {n}", { n: target }));
      else say(t("The document has {n} pages.", { n: ed.pages }));
    } else if (kind === "line") {
      const target = currentLine(ed) + d;
      if (goToLine(ed, target)) say(t("Line {n}", { n: target }));
      else say(t("There is no line {n}.", { n: target }));
    } else {
      const places = placesOf(ed, kind);
      if (!places.length) say(t("There are none in the document."));
      else {
        const i = d > 0 ? indexAfter(ed, places) : indexBefore(ed, places);
        const at = places[(i + places.length) % places.length];
        selectPlace(ed, at);
        say(at.label);
      }
    }
    // Keep the dialog usable from the keyboard: selecting moves focus into the text.
    (d > 0 ? next : prev).focus();
  };
  next.addEventListener("click", () => step(1));
  prev.addEventListener("click", () => step(-1));
  go.addEventListener("click", () => {
    if (kind === "bookmark" || (!input.value.trim() && listed() && list.value)) {
      const at = placesOf(ed, kind)[Number(list.value)];
      if (at && list.value) selectPlace(ed, at);
      go.focus();
      return;
    }
    if (!input.value.trim()) return step(1);
    goNumber();
    go.focus();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      go.click();
    }
  });
  refresh();
  setTimeout(() => (kind === "bookmark" ? list : input).focus());
}
