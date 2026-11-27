// Find & replace bar. The engine does the searching (`Engine.find`: plain text with special characters, wildcards,
// regular expressions, whole words, ignored punctuation, formatting, headers, footers, notes and text boxes) and the
// replacing (the `replace_text` op), so the interface, the AI tools and the CLI all behave the same. Matches are
// painted with the CSS Custom Highlight API so the editable DOM is never modified.
import type { BookmarkInfo, FindHit, FindOptions, Op, ReplaceTextOp, StyleInfo } from "@docxtor/core";
import { msg, t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { paraPoint } from "../editor/selection";
import { h, popover, svg, toast } from "./dom";
import { describeFind, describeReplace, emptyFormats, hasFormats, openFormatDialog, type FormatKind, type FormatSet, type FormatSide } from "./find-format";
import { paired, specialItems, type PatternMode } from "./find-special";
import { I } from "./icons";
import { paintRange } from "../editor/far";

type HighlightRegistry = { set(name: string, h: unknown): void; delete(name: string): void };
const highlights = (globalThis as unknown as { CSS?: { highlights?: HighlightRegistry } }).CSS?.highlights;
const HighlightCtor = (globalThis as unknown as { Highlight?: new (...r: AbstractRange[]) => unknown }).Highlight;

/** The most matches painted at once on a very long document. */
const PAINT_LIMIT = 4000;
/** Results shown in the list at first; "Show more" adds this many each time. */
const PAGE = 100;
/** The longest text to find, as in Word. A longer one is not searched. */
const MAX_FIND = 255;

/** The switches of the options panel. */
interface Switches {
  matchCase: boolean;
  wholeWord: boolean;
  wildcards: boolean;
  regex: boolean;
  prefix: boolean;
  suffix: boolean;
  ignorePunct: boolean;
  ignoreSpace: boolean;
  ignoreMarks: boolean;
  soundsLike: boolean;
  direction: "all" | "down" | "up";
  scope: "all" | "main";
}

const OPTION_ROWS: { key: keyof Switches; label: string }[] = [
  { key: "matchCase", label: msg("Match case") },
  { key: "wholeWord", label: msg("Whole words only") },
  { key: "wildcards", label: msg("Use wildcards") },
  { key: "regex", label: msg("Regular expression") },
  { key: "prefix", label: msg("Match prefix") },
  { key: "suffix", label: msg("Match suffix") },
  { key: "ignorePunct", label: msg("Ignore punctuation") },
  { key: "ignoreSpace", label: msg("Ignore white space") },
  { key: "ignoreMarks", label: msg("Ignore accents") },
  { key: "soundsLike", label: msg("Sounds like (English)") },
];

export class FindBar {
  el: HTMLElement;
  private input: HTMLInputElement;
  private replaceInput: HTMLInputElement;
  private count: HTMLElement;
  private selectAllBtn: HTMLButtonElement;
  private replaceBtn: HTMLButtonElement;
  private replaceAllBtn: HTMLButtonElement;
  private moreBtn: HTMLButtonElement;
  private listBtn: HTMLButtonElement;
  private morePanel: HTMLElement;
  private resultsPanel: HTMLElement;
  private findNote: HTMLElement;
  private replaceNote: HTMLElement;
  private checks = new Map<keyof Switches, HTMLInputElement>();
  private hits: FindHit[] = [];
  private truncated = false;
  private current = -1;
  private open = false;
  private moved = false;
  private error = "";
  private tooLong = false;
  private shown = PAGE;
  private timer = 0;
  private dock: { el: HTMLElement; show(): void } | null = null;
  /** A search is waiting: should it jump to the match after the caret (typing) or keep the current one (the text changed)? */
  private pending: "jump" | "keep" | null = null;
  private sw: Switches = { matchCase: false, wholeWord: false, wildcards: false, regex: false, prefix: false, suffix: false, ignorePunct: false, ignoreSpace: false, ignoreMarks: false, soundsLike: false, direction: "all", scope: "all" };
  private fmt: FormatSet = emptyFormats();

  constructor(private ed: Editor) {
    this.input = h("input", { type: "search", placeholder: t("Find"), "aria-label": t("Find"), spellcheck: "false", autocomplete: "off" });
    this.replaceInput = h("input", { type: "text", placeholder: t("Replace with"), "aria-label": t("Replace with"), spellcheck: "false", autocomplete: "off" });
    this.count = h("span", { class: "find-count", "aria-live": "polite" }, "");
    const btn = (icon: string, title: string, fn: () => void) => {
      const b = h("button", { class: "tb", type: "button", title, "aria-label": title });
      b.append(svg(icon));
      b.addEventListener("click", fn);
      return b;
    };
    const prev = btn(I.chevron, t("Previous match (⇧Enter)"), () => this.step(-1));
    prev.style.transform = "rotate(180deg)";
    this.selectAllBtn = h("button", { class: "btn find-select", type: "button" }, t("Select all"));
    this.selectAllBtn.addEventListener("click", () => this.selectAll());
    this.replaceBtn = h("button", { class: "btn", type: "button" }, t("Replace"));
    this.replaceAllBtn = h("button", { class: "btn", type: "button" }, t("All"));
    this.replaceBtn.addEventListener("click", () => this.replaceCurrent());
    this.replaceAllBtn.addEventListener("click", () => this.replaceAll());
    this.moreBtn = h("button", { class: "btn ghost find-more-btn", type: "button", "aria-expanded": "false" }, t("More"));
    this.moreBtn.addEventListener("click", () => this.toggleMore());
    this.listBtn = h("button", { class: "btn ghost", type: "button", "aria-expanded": "false" }, t("List results"));
    this.listBtn.addEventListener("click", () => this.toggleResults());
    this.findNote = h("div", { class: "find-note", hidden: true });
    this.replaceNote = h("div", { class: "find-note", hidden: true });
    this.morePanel = this.buildMore();
    this.resultsPanel = h("div", { class: "find-results", hidden: true, role: "region", "aria-label": t("Find in") });
    this.el = h(
      "div",
      { class: "find-bar", role: "search", style: "display:none" },
      h("div", { class: "find-row" }, this.input, this.count, prev, btn(I.chevron, t("Next match (Enter)"), () => this.step(1)), btn(I.close, t("Close (Esc)"), () => this.hide())),
      this.findNote,
      h("div", { class: "find-row" }, this.replaceInput, this.replaceBtn, this.replaceAllBtn),
      this.replaceNote,
      h("div", { class: "find-row find-row-tools" }, this.moreBtn, this.selectAllBtn, this.listBtn),
      this.morePanel,
      this.resultsPanel,
    );
    this.input.addEventListener("input", () => this.schedule(true));
    const keys = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        this.hide();
      } else if (e.key === "Enter" && e.target === this.input) {
        e.preventDefault();
        this.step(e.shiftKey ? -1 : 1);
      } else if (e.key === "Enter" && e.target === this.replaceInput) {
        e.preventDefault();
        this.replaceCurrent();
      }
    };
    this.input.addEventListener("keydown", keys);
    this.replaceInput.addEventListener("keydown", keys);
    this.morePanel.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        this.hide();
      }
    });
    this.syncSwitches();
  }

  // ---------------------------------------------------------------------------
  // The options panel
  // ---------------------------------------------------------------------------

  private buildMore(): HTMLElement {
    const checks = h("div", { class: "find-checks", role: "group", "aria-label": t("Search options") });
    for (const row of OPTION_ROWS) {
      const input = h("input", { type: "checkbox", "data-opt": row.key });
      input.addEventListener("change", () => {
        (this.sw[row.key] as boolean) = input.checked;
        // Wildcards and regular expressions exclude each other: the box just ticked turns the other off.
        if (input.checked && row.key === "wildcards") this.sw.regex = false;
        if (input.checked && row.key === "regex") this.sw.wildcards = false;
        this.syncSwitches();
        this.schedule(true);
      });
      this.checks.set(row.key, input);
      checks.append(h("label", { class: "check" }, input, h("span", {}, t(row.label))));
    }
    const mk = (cls: string, label: string, options: [string, string][], onChange: (v: string) => void) => {
      const sel = h("select", { class: cls, "aria-label": label }, ...options.map(([v, text]) => h("option", { value: v }, text)));
      sel.addEventListener("change", () => {
        onChange(sel.value);
        this.schedule(true);
      });
      return h("div", { class: "field find-select-field" }, h("label", {}, label), sel);
    };
    const direction = mk("find-direction", t("Search"), [["all", t("All")], ["down", t("Down")], ["up", t("Up")]], (v) => (this.sw.direction = v as Switches["direction"]));
    const scope = mk("find-scope", t("In"), [["all", t("All stories")], ["main", t("Main document only")]], (v) => (this.sw.scope = v as Switches["scope"]));
    const tool = (label: string, side: FormatSide, kind: "special" | "format", field: () => HTMLInputElement) => {
      const b = h("button", { class: "btn", type: "button", "aria-haspopup": "menu" }, label);
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => (kind === "special" ? this.specialMenu(b, side, field()) : this.formatMenu(b, side)));
      return b;
    };
    return h(
      "div",
      { class: "find-more", hidden: true },
      h("div", { class: "find-pair" }, direction, scope),
      checks,
      h("div", { class: "find-tools" }, h("span", { class: "find-tools-label" }, t("Find")), tool(t("Special"), "find", "special", () => this.input), tool(t("Format"), "find", "format", () => this.input)),
      h("div", { class: "find-tools" }, h("span", { class: "find-tools-label" }, t("Replace")), tool(t("Special"), "replace", "special", () => this.replaceInput), tool(t("Format"), "replace", "format", () => this.replaceInput)),
    );
  }

  private toggleMore(force?: boolean) {
    const on = force ?? this.morePanel.hidden;
    this.morePanel.hidden = !on;
    this.moreBtn.setAttribute("aria-expanded", String(on));
    this.moreBtn.textContent = on ? t("Fewer") : t("More");
  }

  /** Keep the switches consistent, as Word does: wildcards or a regular expression, not both; some switches don't go with them. */
  private syncSwitches() {
    const s = this.sw;
    const pattern = s.wildcards || s.regex;
    if (s.wholeWord) s.prefix = s.suffix = false;
    if (pattern || s.soundsLike) s.ignoreMarks = false;
    if (pattern) s.soundsLike = s.ignorePunct = s.ignoreSpace = false;
    if (s.soundsLike) s.wildcards = s.regex = false;
    if (s.wildcards) s.matchCase = true;
    const off: Partial<Record<keyof Switches, boolean>> = {
      wholeWord: false,
      prefix: s.wholeWord,
      suffix: s.wholeWord,
      matchCase: s.wildcards,
      wildcards: s.soundsLike,
      regex: s.soundsLike,
      ignorePunct: pattern,
      ignoreSpace: pattern,
      ignoreMarks: pattern || s.soundsLike,
      soundsLike: pattern,
    };
    for (const [key, input] of this.checks) {
      input.checked = !!s[key];
      input.disabled = !!off[key];
    }
    this.showNotes();
  }

  private mode(): PatternMode {
    return this.sw.regex ? "regex" : this.sw.wildcards ? "wildcards" : "plain";
  }

  private styles(): StyleInfo[] {
    try {
      return this.ed.engine.styles();
    } catch {
      return [];
    }
  }

  /** The lines under the fields that say which formatting is in force. */
  private showNotes() {
    const styles = this.styles();
    const f = describeFind(this.fmt.find, styles);
    this.findNote.hidden = !f;
    this.findNote.replaceChildren(...(f ? [h("span", {}, t("Format: {format}", { format: f })), this.clearButton("find")] : []));
    const r = describeReplace(this.fmt.run, this.fmt.para, this.fmt.lang, styles);
    this.replaceNote.hidden = !r;
    this.replaceNote.replaceChildren(...(r ? [h("span", {}, t("Format: {format}", { format: r })), this.clearButton("replace")] : []));
  }

  private clearButton(side: FormatSide): HTMLButtonElement {
    const b = h("button", { class: "tb", type: "button", title: t("No formatting"), "aria-label": t("No formatting") });
    b.append(svg(I.close));
    b.addEventListener("click", () => {
      if (side === "find") this.fmt.find = {};
      else {
        this.fmt.run = {};
        this.fmt.para = {};
        this.fmt.lang = undefined;
      }
      this.showNotes();
      this.schedule(true);
    });
    return b;
  }

  /** Put a special character code into the field that last had the caret. */
  private specialMenu(anchor: HTMLElement, side: FormatSide, field: HTMLInputElement) {
    const list = h("div", { class: "find-menu", role: "menu" });
    let close = () => {};
    for (const item of specialItems(side, this.mode())) {
      const b = h("button", { class: "menu-item", type: "button", role: "menuitem" }, h("span", {}, t(item.label)), h("span", { class: "hint" }, item.code));
      b.addEventListener("click", () => {
        close();
        const at = field.selectionStart ?? field.value.length;
        const end = field.selectionEnd ?? at;
        field.setRangeText(item.code, at, end, "end");
        if (paired(item.code)) {
          const inside = item.code === "{1}" || item.code === "{1,3}" ? at + 1 : at + (item.code === "[!]" ? 2 : 1);
          field.setSelectionRange(inside, item.code.startsWith("{") ? inside + item.code.length - 2 : inside);
        }
        field.dispatchEvent(new Event("input", { bubbles: true }));
        field.focus();
      });
      list.append(b);
    }
    close = popover(anchor, list);
  }

  private formatMenu(anchor: HTMLElement, side: FormatSide) {
    const list = h("div", { class: "find-menu", role: "menu" });
    let close = () => {};
    const items: [FormatKind, string][] = [
      ["font", t("Font…")],
      ["paragraph", t("Paragraph…")],
      ["style", t("Style…")],
      ["highlight", t("Highlight…")],
      ["language", t("Language…")],
    ];
    for (const [kind, label] of items) {
      const b = h("button", { class: "menu-item", type: "button", role: "menuitem" }, h("span", {}, label));
      b.addEventListener("click", () => {
        close();
        openFormatDialog(kind, side, this.fmt, this.styles(), (next) => {
          this.fmt = next;
          this.showNotes();
          this.toggleMore(true);
          this.schedule(true);
        });
      });
      list.append(b);
    }
    const none = h("button", { class: "menu-item", type: "button", role: "menuitem", disabled: !hasFormats(this.fmt, side) }, h("span", {}, t("No formatting")));
    none.addEventListener("click", () => {
      close();
      if (side === "find") this.fmt.find = {};
      else this.fmt = { ...this.fmt, run: {}, para: {}, lang: undefined };
      this.showNotes();
      this.schedule(true);
    });
    list.append(h("div", { class: "menu-sep", role: "separator" }), none);
    close = popover(anchor, list);
  }

  // ---------------------------------------------------------------------------
  // Showing and hiding
  // ---------------------------------------------------------------------------

  show() {
    this.open = true;
    this.moved = false;
    this.el.style.display = "";
    const sel = this.ed.selectedText();
    if (sel && !sel.includes("\n") && sel.length < 100) this.input.value = sel;
    this.input.focus();
    this.input.select();
    this.search(true);
  }

  /** Open the bar and go to the next or previous match of what was searched last. */
  stepFromOutside(d: number) {
    if (!this.open) this.show();
    this.step(d);
  }

  hide(refocus = true) {
    this.flush();
    if (refocus && this.open && this.moved && this.current >= 0) this.reveal(this.hits[this.current]);
    this.open = false;
    this.pending = null;
    window.clearTimeout(this.timer);
    this.el.style.display = "none";
    highlights?.delete("find");
    highlights?.delete("find-current");
    if (refocus && !this.ed.story) this.ed.focus();
  }

  get isOpen() {
    return this.open;
  }

  /** Re-run the search after document changes. */
  refresh() {
    if (this.open) this.schedule(false);
  }

  private schedule(jump: boolean) {
    window.clearTimeout(this.timer);
    this.pending = jump || this.pending === "jump" ? "jump" : "keep";
    this.timer = window.setTimeout(() => this.flush(), jump ? 40 : 120);
  }

  /** Run a search that is waiting for the typing to pause, now (before stepping or replacing, which need its matches). */
  private flush() {
    window.clearTimeout(this.timer);
    const mode = this.pending;
    this.pending = null;
    if (mode) this.search(mode === "jump");
  }

  // ---------------------------------------------------------------------------
  // Searching
  // ---------------------------------------------------------------------------

  private options(): FindOptions {
    const s = this.sw;
    const o: FindOptions = {};
    const pattern = s.wildcards || s.regex;
    if (!s.matchCase) o.ignore_case = true;
    if (s.wholeWord) o.whole_word = true;
    if (s.wildcards) o.wildcards = true;
    if (s.regex) o.regex = true;
    if (s.prefix) o.prefix = true;
    if (s.suffix) o.suffix = true;
    if (s.ignorePunct && !pattern) o.ignore_punct = true;
    if (s.ignoreSpace && !pattern) o.ignore_space = true;
    if (s.ignoreMarks && !pattern) o.ignore_marks = true;
    if (s.soundsLike && !pattern) o.sounds_like = true;
    if (s.scope === "main") o.main_only = true;
    if (Object.keys(this.fmt.find).length) o.find_format = this.fmt.find;
    if (s.direction !== "all") {
      const r = this.ed.range();
      if (r) {
        o.direction = s.direction;
        o.from = s.direction === "down" ? r[1] : r[0];
      }
    }
    return o;
  }

  private search(jump: boolean) {
    const q = this.input.value;
    const wasHit = this.hits[this.current];
    this.hits = [];
    this.truncated = false;
    this.error = "";
    this.tooLong = q.length > MAX_FIND;
    const formatOnly = !q && Object.keys(this.fmt.find).length > 0;
    if ((q || formatOnly) && this.ed.engine && !this.tooLong) {
      try {
        const res = this.ed.engine.find(q, this.options());
        this.hits = res.hits;
        this.truncated = res.truncated;
      } catch (e) {
        this.error = String((e as Error).message ?? e).replace(/^Error:\s*/, "");
      }
    }
    this.shown = PAGE;
    if (jump || this.current >= this.hits.length || !wasHit) {
      const caret = this.ed.story ? null : this.ed.range()?.[0];
      const order = new Map(Array.from(this.ed.flow.querySelectorAll<HTMLElement>("[data-p]")).map((e, i) => [Number(e.dataset.p), i]));
      const at = caret ? order.get(caret.p) : undefined;
      let i = 0;
      if (at !== undefined && caret) {
        const f = this.hits.findIndex((m) => {
          if (m.story) return true;
          const a = order.get(m.p);
          return a !== undefined && (a > at || (a === at && m.o >= caret.o));
        });
        i = f < 0 ? 0 : f;
      }
      this.current = this.hits.length ? i : -1;
      this.showCurrent();
    } else if (wasHit) {
      // The same match if it is still there (the text around it changed), else the one now in its place.
      const same = this.hits.findIndex((m) => m.p === wasHit.p && m.o === wasHit.o);
      this.current = same >= 0 ? same : Math.min(this.current, this.hits.length - 1);
    }
    this.paint();
  }

  /** Paint the matches in the pages: the text, headers, footers and notes drawn on them, and text boxes. */
  private paint() {
    const n = this.hits.length;
    this.count.classList.toggle("error", !!this.error || this.tooLong);
    const has = !!(this.input.value || Object.keys(this.fmt.find).length);
    if (this.tooLong) {
      this.count.textContent = t("Too long (255 characters at most)");
      this.count.title = "";
    } else if (this.error) {
      this.count.textContent = t("Not a valid pattern");
      this.count.title = this.error;
    } else {
      this.count.title = "";
      this.count.textContent = has ? (n ? (this.truncated ? t("{current} of {total}+", { current: this.current + 1, total: n }) : t("{current} of {total}", { current: this.current + 1, total: n })) : t("No results")) : "";
    }
    this.selectAllBtn.disabled = !n;
    this.replaceBtn.disabled = !n;
    this.replaceAllBtn.disabled = !n;
    if (!this.resultsPanel.hidden) this.renderResults();
    if (!highlights || !HighlightCtor) return;
    const { text, areas } = this.paragraphMaps();
    const ranges: Range[] = [];
    const cur: Range[] = [];
    const rangesOf = (m: FindHit): Range[] => {
      const out: Range[] = [];
      const els: HTMLElement[] = m.story && m.story.kind !== "textbox" ? (areas.get(m.p) ?? []) : [text.get(m.p) ?? areas.get(m.p)?.[0]].filter((x): x is HTMLElement => !!x);
      for (const el of els) {
        const r = this.rangeIn(el, m);
        if (r) out.push(r);
      }
      return out;
    };
    // The current match is measured where it is: its block is drawn first (before any range is made in it: drawing a far block moves it out of
    // its box, and a live range would fall out of it).
    const now = this.hits[this.current];
    const nowEl = now && (!now.story || now.story.kind === "textbox") ? (text.get(now.p) ?? areas.get(now.p)?.[0]) : undefined;
    if (nowEl) this.ed.ensureDrawn(nowEl);
    // Paint around the current match first so it is always there, then as many of the rest as is sensible. The highlights hold fixed ranges, which
    // keep their text when a block is drawn or left out later (see `paintRange`).
    const start = Math.max(0, this.current - Math.floor(PAINT_LIMIT / 2));
    for (let i = start; i < Math.min(n, start + PAINT_LIMIT); i++) {
      const rs = rangesOf(this.hits[i]);
      ranges.push(...rs);
      if (i === this.current) cur.push(...rs);
    }
    highlights.set("find", new HighlightCtor(...ranges.map(paintRange)));
    if (cur.length) {
      highlights.set("find-current", new HighlightCtor(...cur.map(paintRange)));
      const rect = cur[0].getBoundingClientRect();
      const c = this.ed.canvas.getBoundingClientRect();
      if (rect.height && (rect.top < c.top + 60 || rect.bottom > c.bottom - 60)) this.ed.canvas.scrollTop += rect.top - c.top - c.height / 3;
    } else highlights.delete("find-current");
  }

  /** Paragraph elements by id, found in one pass: the text (and the story being edited), and the read-only header, footer and note areas. */
  private paragraphMaps(): { text: Map<number, HTMLElement>; areas: Map<number, HTMLElement[]> } {
    const text = new Map<number, HTMLElement>();
    for (const root of new Set([this.ed.flow, this.ed.root])) for (const el of Array.from(root.querySelectorAll<HTMLElement>("[data-p]"))) text.set(Number(el.dataset.p), el);
    const areas = new Map<number, HTMLElement[]>();
    for (const el of Array.from(this.ed.stageElement.querySelectorAll<HTMLElement>(".hf [data-id], .notes [data-id]"))) {
      const id = Number(el.dataset.id);
      const list = areas.get(id);
      if (list) list.push(el);
      else areas.set(id, [el]);
    }
    return { text, areas };
  }

  private rangeIn(el: HTMLElement, m: FindHit): Range | null {
    const a = paraPoint(el, m.o);
    // A match that ends in another paragraph is painted to the end of its first one.
    const b = m.endP === m.p ? paraPoint(el, m.endO) : paraPoint(el, Number.MAX_SAFE_INTEGER);
    if (!a || !b) return null;
    const r = document.createRange();
    try {
      r.setStart(a.node, a.offset);
      r.setEnd(b.node, b.offset);
    } catch {
      return null;
    }
    return r;
  }

  private step(d: number) {
    this.flush();
    if (!this.hits.length) return;
    this.current = (this.current + d + this.hits.length) % this.hits.length;
    this.moved = true;
    this.showCurrent();
    this.paint();
  }

  /** The match moved to is shown: its tab opens and its folded heading unfolds, so it is painted and scrolled to. */
  private showCurrent() {
    const m = this.hits[this.current];
    if (m && !(m.story && m.story.kind !== "textbox")) this.ed.ensureDrawn(m.p);
  }

  /** Make the editor's selection the match. A match in a header, footer or note opens that for editing. */
  private reveal(m: FindHit | undefined) {
    if (!m) return;
    const story = m.story && m.story.kind !== "textbox" ? (m.story as BookmarkInfo["story"]) : undefined;
    this.ed.selectBookmark({ name: "", p: m.p, o: m.o, endP: m.endP, endO: m.endO, text: "", story });
  }

  /** Select every match at once (the current one is the real selection), so one edit changes them all. */
  private selectAll() {
    this.flush();
    // Several selections live in the text, in one paragraph each.
    const usable = this.hits.map((m, i) => ({ m, i })).filter(({ m }) => !m.story && m.p === m.endP);
    const n = usable.length;
    if (!n) {
      toast(t("Select all works on matches in the text. These are in headers, footers, notes or across paragraphs."));
      return;
    }
    if (this.ed.story) this.ed.exitStory(false);
    const ranges = usable.map(({ m }) => ({ from: { p: m.p, o: m.o }, to: { p: m.endP, o: m.endO } }));
    const primary = Math.max(0, usable.findIndex(({ i }) => i >= this.current));
    this.hide(false);
    const got = this.ed.selectRanges(ranges, primary);
    toast(got ? tn(got, "{n} match selected", "{n} matches selected") : t("The matches are in tabs that aren't shown."));
  }

  // ---------------------------------------------------------------------------
  // Replacing
  // ---------------------------------------------------------------------------

  /** The replace_text op for the current options and formatting. */
  private replaceOp(extra: Partial<ReplaceTextOp>): ReplaceTextOp {
    const s = this.sw;
    const pattern = s.wildcards || s.regex;
    const op: ReplaceTextOp = { op: "replace_text", find: this.input.value, replace: this.replaceInput.value, ...this.options(), ...extra };
    // Word gives the replacement the capitals of what it replaces when "Match case" is off.
    if (!s.matchCase && !pattern && !s.soundsLike) op.keep_case = true;
    if (Object.keys(this.fmt.run).length) op.format = this.fmt.run;
    if (Object.keys(this.fmt.para).length) op.para_format = this.fmt.para;
    if (this.fmt.lang) op.language = this.fmt.lang;
    return op;
  }

  private replaceCurrent() {
    this.flush();
    const m = this.hits[this.current];
    if (!m) return;
    // The engine replaces the first match at or after the start of this one, in the text this one is in.
    const op = this.replaceOp({ all: false, from: { p: m.p, o: m.o }, direction: "down" });
    const inStory = !!m.story;
    const res = this.ed.exec(op as unknown as Op, inStory ? { select: null } : {});
    if (!res) return;
    const index = this.current;
    this.search(!inStory);
    if (inStory && this.hits.length) this.current = Math.min(index, this.hits.length - 1);
    this.moved = true;
    this.paint();
    this.replaceInput.focus();
  }

  private replaceAll() {
    this.flush();
    if (!this.hits.length) return;
    const n = this.hits.length;
    const op = this.replaceOp({ all: true });
    const res = this.ed.exec(op as unknown as Op, { select: null });
    if (!res) return;
    const caret = res.find((r) => r.caret)?.caret;
    if (caret && this.ed.flow.querySelector(`[data-p="${caret.p}"]`) && !this.ed.story) this.ed.select(caret);
    const done = res[0]?.count ?? n;
    this.search(true);
    this.count.textContent = tn(done, "Replaced {n}", "Replaced {n}");
  }

  // ---------------------------------------------------------------------------
  // The list of results
  // ---------------------------------------------------------------------------

  private toggleResults(force?: boolean) {
    // With a navigator to hold the list, the button opens that.
    if (this.dock) {
      this.dock.show();
      this.renderResults();
      return;
    }
    const on = force ?? this.resultsPanel.hidden;
    this.resultsPanel.hidden = !on;
    this.listBtn.setAttribute("aria-expanded", String(on));
    if (on) this.renderResults();
  }

  /**
   * Put the list of results somewhere else (the app's navigator) instead of inside the bar: `el` holds it and `show`
   * brings that place into view. Pass null to put it back in the bar.
   */
  setDock(dock: { el: HTMLElement; show(): void } | null) {
    this.dock = dock;
    if (dock) {
      dock.el.append(this.resultsPanel);
      this.resultsPanel.hidden = false;
      this.listBtn.removeAttribute("aria-expanded");
      this.renderResults();
    } else {
      this.el.append(this.resultsPanel);
      this.resultsPanel.hidden = true;
      this.listBtn.setAttribute("aria-expanded", "false");
    }
  }

  /** Where in the document a match is, in a word or two. */
  private place(m: FindHit): string {
    switch (m.story?.kind) {
      case "header":
        return t("Header");
      case "footer":
        return t("Footer");
      case "footnote":
        return t("Footnote");
      case "endnote":
        return t("Endnote");
      case "textbox":
        return t("Text box");
      default:
        return "";
    }
  }

  private renderResults() {
    const n = this.hits.length;
    if (!n) {
      this.resultsPanel.replaceChildren(h("p", { class: "find-results-empty" }, this.input.value || Object.keys(this.fmt.find).length ? t("No results") : t("Type something to find.")));
      return;
    }
    const list = h("div", { class: "find-hits" });
    for (let i = 0; i < Math.min(n, this.shown); i++) {
      const m = this.hits[i];
      const place = this.place(m);
      const row = h(
        "button",
        { class: `find-hit${i === this.current ? " on" : ""}`, type: "button", "data-index": String(i) },
        place ? h("span", { class: "find-hit-place" }, place) : null,
        h("span", { class: "find-hit-text" }, h("span", {}, m.before.length > 22 ? `…${m.before.slice(-22)}` : m.before), h("mark", {}, m.text.length > 80 ? `${m.text.slice(0, 80)}…` : m.text), h("span", {}, m.after)),
      );
      // The click must not move the caret into the page before the match is chosen.
      row.addEventListener("click", () => {
        this.current = i;
        this.moved = true;
        this.paint();
        this.reveal(m);
      });
      list.append(row);
    }
    const head = h("div", { class: "find-results-head" }, tn(n, "{n} result", "{n} results") + (this.truncated ? "+" : ""));
    const more =
      n > this.shown
        ? h("button", { class: "btn ghost", type: "button" }, t("Show more"))
        : null;
    more?.addEventListener("click", () => {
      this.shown += PAGE * 2;
      this.renderResults();
    });
    this.resultsPanel.replaceChildren(head, list, ...(more ? [more] : []));
    list.querySelector<HTMLElement>(".find-hit.on")?.scrollIntoView({ block: "nearest" });
  }

  /** Show the list of results (the command "Show all results"). */
  showResults() {
    if (!this.open) this.show();
    this.toggleResults(true);
  }
}
