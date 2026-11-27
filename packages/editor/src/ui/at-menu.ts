// The @ menu, as in Google Docs: typing @ after a space (or at the start of a paragraph) opens a list at the caret of
// people, dates, files, drop-downs, building blocks and things to insert, filtered by what is typed after it. Arrow keys
// move, Enter or Tab chooses, Escape closes and keeps the typed text; a space after text nothing matches closes it too.
// The lines come from `ctx.atMenu` providers (ExtensionHost.atMenuSections), so extensions add their own.
import type { MPos } from "@docxtor/core";
import { t, type AtMenuItem, type AtMenuSection, type ExtensionHost } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, popover } from "./dom";

/** At most this many lines are shown, and this many a section until something is typed (the list scrolls). */
const MAX_LINES = 40;
const PER_SECTION = 4;

let seq = 0;

export class AtMenu {
  private start: MPos | null = null;
  private close: (() => void) | null = null;
  private list: HTMLElement | null = null;
  private items: AtMenuItem[] = [];
  private active = 0;
  private query = "";
  private asked = 0;
  private ac: AbortController | null = null;
  private readonly id = `at-menu-${++seq}`;

  constructor(
    private ed: Editor,
    private host: ExtensionHost,
  ) {}

  get open(): boolean {
    return !!this.close;
  }

  /** After the text changed or the caret moved: open the menu on a fresh @, follow what is typed, or close it. */
  check(typed: boolean) {
    const ed = this.ed;
    if (!ed.engine || ed.readOnly || !ed.collapsed()) return this.dismiss();
    const caret = ed.range()?.[1];
    if (!caret) return this.dismiss();
    if (this.start) {
      if (caret.p !== this.start.p || caret.o <= this.start.o) return this.dismiss();
      const text = ed.engine.textRange(this.start, caret);
      if (!text.startsWith("@") || text.length > 40 || text.includes("\n")) return this.dismiss();
      const q = text.slice(1);
      // A space after text that matches nothing: it was not meant for the menu.
      if (/\s$/.test(q) && !this.items.length) return this.dismiss();
      if (q !== this.query) void this.refresh(q);
      return;
    }
    if (!typed || caret.o < 1) return;
    const before = ed.engine.textRange({ p: caret.p, o: Math.max(0, caret.o - 2) }, caret);
    if (!/(^|\s)@$/.test(before)) return;
    this.start = { p: caret.p, o: caret.o - 1 };
    this.query = "\u0000";
    void this.refresh("");
  }

  /** Close without choosing: the typed text stays as it is. */
  dismiss() {
    this.start = null;
    this.items = [];
    this.query = "";
    this.ac?.abort();
    this.ac = null;
    const c = this.close;
    this.close = null;
    this.list = null;
    c?.();
    this.ed.flow.removeAttribute("aria-activedescendant");
    this.ed.flow.removeAttribute("aria-controls");
  }

  private async refresh(q: string) {
    this.query = q;
    const mine = ++this.asked;
    const sections = await this.host.atMenuSections(q.trim());
    if (mine !== this.asked || !this.start) return;
    this.items = [];
    this.paint(sections);
  }

  private paint(sections: AtMenuSection[]) {
    const list = h("div", { class: "at-menu", id: this.id, role: "listbox", "aria-label": t("Insert") });
    let n = 0;
    for (const s of sections) {
      if (n >= MAX_LINES) break;
      const group = h("div", { class: "at-group", role: "group", "aria-label": s.title }, h("div", { class: "at-title", "aria-hidden": "true" }, s.title));
      const shown = this.query.trim() ? s.items : s.items.slice(0, PER_SECTION);
      for (const item of shown) {
        if (n >= MAX_LINES) break;
        const i = n++;
        const icon = item.icon ? this.host.icon(item.icon) : "";
        const row = h(
          "div",
          { class: "at-item", role: "option", id: `${this.id}-${i}`, "aria-selected": "false" },
          h("span", { class: "at-icon", "aria-hidden": "true", html: icon }),
          h("span", { class: "at-label" }, item.label),
          item.detail ? h("span", { class: "at-detail" }, item.detail) : null,
        );
        row.addEventListener("mousedown", (e) => e.preventDefault());
        row.addEventListener("click", () => void this.choose(i));
        row.addEventListener("mousemove", () => this.highlight(i));
        group.append(row);
        this.items.push(item);
      }
      list.append(group);
    }
    if (!this.items.length) list.append(h("div", { class: "at-empty" }, t("Nothing matches “{query}”", { query: this.query.trim() })));
    this.active = 0;
    if (this.close && this.list) {
      this.list.replaceWith(list);
      this.list = list;
    } else {
      const at = this.caretPoint();
      this.list = list;
      this.close = popover(at, list, { onClose: () => this.closed() });
      this.ac = new AbortController();
      // The keys the menu uses while it is open; everything else types into the text.
      document.addEventListener("keydown", (e) => this.key(e), { capture: true, signal: this.ac.signal });
      this.ed.flow.setAttribute("aria-controls", this.id);
    }
    this.highlight(0);
  }

  /** The popover closed by itself (a click elsewhere). */
  private closed() {
    if (!this.close) return;
    this.close = null;
    this.dismiss();
  }

  private caretPoint(): { x: number; y: number } {
    const s = window.getSelection();
    const r = s && s.rangeCount ? s.getRangeAt(0).getBoundingClientRect() : null;
    if (r && (r.width || r.height)) return { x: r.left, y: r.bottom + 4 };
    const el = this.ed.flow.getBoundingClientRect();
    return { x: el.left + 40, y: el.top + 40 };
  }

  private highlight(i: number) {
    if (!this.list || !this.items.length) return;
    this.active = Math.max(0, Math.min(this.items.length - 1, i));
    this.list.querySelectorAll<HTMLElement>(".at-item").forEach((el, k) => {
      el.setAttribute("aria-selected", String(k === this.active));
      el.classList.toggle("on", k === this.active);
      if (k === this.active) el.scrollIntoView({ block: "nearest" });
    });
    this.ed.flow.setAttribute("aria-activedescendant", `${this.id}-${this.active}`);
  }

  private key(e: KeyboardEvent) {
    if (!this.open) return;
    switch (e.key) {
      case "ArrowDown":
        this.highlight(this.active + 1);
        break;
      case "ArrowUp":
        this.highlight(this.active - 1);
        break;
      case "Enter":
      case "Tab":
        if (!this.items.length) return this.dismiss();
        void this.choose(this.active);
        break;
      case "Escape":
        this.dismiss();
        break;
      default:
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  }

  /** Take the @ and what was typed after it away, then run the line chosen. */
  private async choose(i: number) {
    const item = this.items[i];
    const start = this.start;
    const caret = this.ed.range()?.[1];
    this.dismiss();
    if (!item || !start || !caret) return;
    this.ed.exec({ op: "delete", from: start, to: caret }, { select: start });
    try {
      await item.run();
    } finally {
      this.ed.focus();
    }
  }
}
