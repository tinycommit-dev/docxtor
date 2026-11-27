// Pasting: read the clipboard (or a drop) through the converters, put the content in the document as one
// undoable step, and offer "Paste options" next to it so the person can paste it again another way.
import { Engine, type Fragment, type MPos } from "@docxtor/core";
import { t, type PasteContent, type PasteConverter } from "@docxtor/extensions";
import { h, overlayLayer, popover, svg, toast } from "../ui/dom";
import { I } from "../ui/icons";
import type { Editor } from "./editor";
import { BUILTIN_CONVERTERS, clipFromText, convertClip, warningText, type Clip, type Converted } from "./paste-convert";
import { textToMarkdown } from "./paste";

/** How a paste takes the source's formatting. */
export type PasteChoice = "keep" | "merge" | "text";

export const PASTE_CHOICES: { id: PasteChoice; label: () => string; help: () => string }[] = [
  { id: "keep", label: () => t("Keep source formatting"), help: () => t("Fonts, sizes, colours, styles and lists as they were.") },
  { id: "merge", label: () => t("Merge formatting"), help: () => t("Keep bold, italic, lists and links. Take this document's fonts and paragraph styles.") },
  { id: "text", label: () => t("Keep text only"), help: () => t("Just the words.") },
];

export function normalizeChoice(v: unknown): PasteChoice {
  return v === "merge" || v === "text" ? v : "keep";
}

interface LastPaste {
  conv: Converted;
  choice: PasteChoice;
  /** The selection the paste replaced. */
  range: [MPos, MPos];
  /** Where the caret was left. */
  end: MPos;
  /** The undo depth after the paste, to tell whether anything has happened since. */
  depth: number;
}

export class PasteFlow {
  /** What a plain Paste does. */
  choice: PasteChoice = "keep";
  /** The converters in force (Docxtor's own when an extension host supplies none). */
  converters: () => PasteConverter[] = () => [];
  private last: LastPaste | null = null;
  private busy = false;
  private chip: HTMLButtonElement | null = null;
  private closeMenu: (() => void) | null = null;
  private watch = 0;

  constructor(private ed: Editor) {}

  /** The converters in force. */
  list(): PasteConverter[] {
    const l = this.converters();
    return l.length ? l : BUILTIN_CONVERTERS;
  }

  /** Paste what a clip holds at the caret, replacing the selection. */
  async paste(clip: Clip, o: { plain?: boolean; choice?: PasteChoice; only?: string } = {}) {
    const ed = this.ed;
    if (ed.readOnly || !ed.engine) return;
    const choice = o.plain ? "text" : (o.choice ?? this.choice);
    let conv: Converted | null = null;
    const plain = clip.get("text/plain");
    if (choice === "text" && plain) conv = { content: { text: plain }, by: "docxtor.text", clip };
    else conv = await convertClip(clip, this.list(), ed.engine, o.only);
    if (!conv && plain && !o.only) conv = { content: { text: plain }, by: "docxtor.text", clip };
    if (!conv) return;
    this.apply(conv, choice);
  }

  /** Put converted content in the text. */
  apply(conv: Converted, choice: PasteChoice, at?: [MPos, MPos]) {
    const ed = this.ed;
    const range = at ?? ed.range();
    if (!range) return;
    this.busy = true;
    try {
      const done = this.insert(conv, choice, at);
      if (!done) return;
      const fragment = "fragment" in conv.content ? conv.content.fragment : null;
      const warning = fragment ? warningText(fragment) : null;
      if (warning) toast(warning, 5000);
      const rich = !!fragment || conv.clip.types.some((x) => x === "text/html" || x === "text/rtf");
      const end = ed.range()?.[0];
      if (rich && end && !ed.multi) this.show({ conv, choice, range, end, depth: ed.engine.raw.checkpoint() });
      else this.hide();
    } finally {
      this.busy = false;
    }
  }

  private insert(conv: Converted, choice: PasteChoice, at?: [MPos, MPos]): boolean {
    const ed = this.ed;
    const c: PasteContent = conv.content;
    const textOf = (): string => conv.clip.get("text/plain") ?? ("fragment" in c ? Engine.fragmentText(c.fragment) : "markdown" in c ? c.markdown : c.text);
    if (at) ed.select({ anchor: at[0], focus: at[1] });
    if ("fragment" in c && choice !== "text") return ed.insertFragmentAtCaret(c.fragment, choice === "merge" ? "merge" : "keep");
    if ("markdown" in c && choice !== "text") {
      ed.insertMarkdownAtCaret(c.markdown);
      return true;
    }
    ed.insertMarkdownAtCaret(textToMarkdown(textOf().replace(/\r\n?/g, "\n")));
    return true;
  }

  /** Paste again another way, replacing the earlier paste with one undoable change. */
  reapply(choice: PasteChoice) {
    const l = this.last;
    const ed = this.ed;
    this.hideMenu();
    if (!l || !ed.engine || ed.engine.raw.checkpoint() !== l.depth) return this.hide();
    this.busy = true;
    try {
      ed.undo();
    } finally {
      this.busy = false;
    }
    this.apply(l.conv, choice, l.range);
  }

  // -- the Paste options button --------------------------------------------------------------

  /** Called by the editor before any edit that is not the paste itself. */
  edited() {
    if (!this.busy) this.hide();
  }

  /** Called when the selection moves. */
  moved() {
    const l = this.last;
    if (!l || this.busy) return;
    const r = this.ed.range();
    if (!r || !this.ed.collapsed() || r[0].p !== l.end.p || r[0].o !== l.end.o) this.hide();
  }

  private show(l: LastPaste) {
    this.hide();
    this.last = l;
    const b = h("button", { class: "paste-chip", type: "button", "aria-label": t("Paste options"), title: t("Paste options"), "aria-haspopup": "menu" }, svg(I.paste), svg(I.chevron));
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => this.menu(b));
    overlayLayer().append(b);
    this.chip = b;
    this.place();
    const again = () => {
      window.cancelAnimationFrame(this.watch);
      this.watch = window.requestAnimationFrame(() => this.place());
    };
    this.ed.canvas.addEventListener("scroll", again, { passive: true });
    window.addEventListener("resize", again);
    this.unwatch = () => {
      this.ed.canvas.removeEventListener("scroll", again);
      window.removeEventListener("resize", again);
    };
    window.addEventListener("keydown", this.onKey, true);
  }

  private unwatch: (() => void) | null = null;

  private onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" && this.chip) {
      this.hide();
    }
  };

  private place() {
    const b = this.chip;
    if (!b) return;
    const s = window.getSelection();
    if (!s || !s.rangeCount) return;
    let r = s.getRangeAt(0).getBoundingClientRect();
    if (!r.width && !r.height) {
      const para = this.ed.paraElement(this.last?.end.p ?? -1);
      if (para) r = para.getBoundingClientRect();
    }
    const view = this.ed.canvas.getBoundingClientRect();
    const out = r.bottom < view.top || r.top > view.bottom;
    b.hidden = out;
    b.style.left = `${Math.min(r.right + 6, window.innerWidth - 56)}px`;
    b.style.top = `${r.bottom + 4}px`;
  }

  private menu(anchor: HTMLElement) {
    if (this.closeMenu) return this.hideMenu();
    const l = this.last;
    if (!l) return;
    const list = h("div", { class: "paste-menu" });
    for (const c of PASTE_CHOICES) {
      const item = h("button", { class: "menu-item", type: "button", role: "menuitemradio", "aria-checked": String(c.id === l.choice) }, h("span", { class: "menu-label" }, c.label()));
      if (c.id === l.choice) item.classList.add("active");
      item.title = c.help();
      item.addEventListener("click", () => this.reapply(c.id));
      list.append(item);
    }
    this.closeMenu = popover(anchor, list, {
      onClose: () => {
        this.closeMenu = null;
      },
    });
  }

  private hideMenu() {
    this.closeMenu?.();
    this.closeMenu = null;
  }

  hide() {
    this.hideMenu();
    this.chip?.remove();
    this.chip = null;
    this.last = null;
    this.unwatch?.();
    this.unwatch = null;
    window.removeEventListener("keydown", this.onKey, true);
    window.cancelAnimationFrame(this.watch);
  }

  /** The text of a clip, as a clip of its own (for pasting it plain). */
  static plainClip(text: string): Clip {
    return clipFromText(text);
  }
}

export type { Fragment };
