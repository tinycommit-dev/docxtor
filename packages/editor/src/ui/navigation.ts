// The Navigation pane: the headings (drag one to move its whole section), the pages as thumbnails, and a search
// whose results are listed with their surroundings. It lives in the navigator beside the document as a panel; its
// edits are ops (`move_blocks`), so they undo like any other.
import type { BookmarkInfo, FindHit, OutlineEntry } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import type { DocxtorEditor } from "../docxtor-editor";
import type { Editor } from "../editor/editor";
import { editorSnapshots, snapshotPage } from "../editor/snapshot";
import type { OutlineOps } from "../editor/outline-view";
import { h } from "./dom";
import { DocTabs } from "./doc-tabs";
import { goToPage } from "./goto";

type Tab = "headings" | "pages" | "results";
const TABS: { id: Tab; label: () => string }[] = [
  { id: "headings", label: () => t("Headings") },
  { id: "pages", label: () => t("Pages") },
  { id: "results", label: () => t("Results") },
];
/** The tab people left open, for the next time the pane is built. */
let lastTab: Tab = "headings";
/** Width of a page thumbnail in px. */
const THUMB = 112;
/** Results listed at most. */
const RESULT_LIMIT = 200;

export class NavigationPane {
  readonly el: HTMLElement;
  private tabs = new Map<Tab, HTMLButtonElement>();
  private body: HTMLElement;
  private tab: Tab = lastTab;
  private off: (() => void)[] = [];
  private timer = 0;
  private observer: IntersectionObserver | null = null;
  private query = "";
  private matchCase = false;
  private input: HTMLInputElement | null = null;
  /** The Results tab as drawn: redrawn for a change of the document, it searches again into these and keeps the search box (and what is being typed in it). */
  private results: { input: HTMLInputElement; list: HTMLElement; status: HTMLElement } | null = null;
  /** A search waiting for typing to pause. */
  private searchTimer = 0;
  /** Headings the pane has folded (only the pane's list; the document isn't touched). */
  private collapsed = new Set<number>();
  private dragged = 0;
  /** The document's tabs, above the headings. */
  private docTabs: DocTabs;
  /** The blocks as the engine had them at the last look: moving the caret doesn't ask again. */
  private blocks: OutlineEntry[] | null = null;

  constructor(
    private fe: DocxtorEditor,
    private ed: Editor,
    private ops: OutlineOps,
  ) {
    const bar = h("div", { class: "nav-tabs", role: "tablist", "aria-label": t("Navigation views") });
    for (const { id, label } of TABS) {
      const b = h("button", { class: "nav-tab", type: "button", role: "tab", id: `nav-tab-${id}`, "aria-controls": "nav-body" }, label());
      b.addEventListener("click", () => this.show(id));
      b.addEventListener("keydown", (e) => {
        if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
        const i = TABS.findIndex((x) => x.id === this.tab) + (e.key === "ArrowRight" ? 1 : -1);
        const next = TABS[(i + TABS.length) % TABS.length].id;
        this.show(next);
        this.tabs.get(next)?.focus();
      });
      this.tabs.set(id, b);
      bar.append(b);
    }
    this.body = h("div", { class: "nav-body", id: "nav-body", role: "tabpanel" });
    this.docTabs = new DocTabs(() => fe.editor);
    this.el = h("div", { class: "nav-pane" }, this.docTabs.el, bar, this.body);
    this.docTabs.refresh();
    this.off.push(
      fe.on("change", () => (this.changed(), this.docTabs.refresh())),
      fe.on("layout", () => this.docTabs.refresh()),
      fe.on("open", () => {
        this.collapsed.clear();
        this.changed();
      }),
      fe.on("selection", () => this.syncCurrent()),
      fe.on("layout", () => this.layoutChanged()),
    );
    const scroll = () => this.syncCurrent();
    ed.canvas.addEventListener("scroll", scroll, { passive: true });
    this.off.push(() => ed.canvas.removeEventListener("scroll", scroll));
    this.show(this.tab);
  }

  destroy() {
    window.clearTimeout(this.timer);
    this.observer?.disconnect();
    this.off.forEach((f) => f());
    this.off = [];
    this.el.remove();
  }

  show(tab: Tab) {
    this.tab = lastTab = tab;
    for (const [id, b] of this.tabs) {
      b.classList.toggle("on", id === tab);
      b.setAttribute("aria-selected", String(id === tab));
      b.tabIndex = id === tab ? 0 : -1;
    }
    this.body.setAttribute("aria-labelledby", `nav-tab-${tab}`);
    this.render();
  }

  /** Show the Results tab with a search for `text` (and run it). */
  search(text: string) {
    this.query = text;
    this.show("results");
    this.input?.focus();
  }

  private changed() {
    this.blocks = null;
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.render(), 300);
  }

  private layoutChanged() {
    if (this.tab === "pages") this.changed();
    else this.syncCurrent();
  }

  private render() {
    // A redraw that was waiting is this one.
    window.clearTimeout(this.timer);
    this.observer?.disconnect();
    this.observer = null;
    if (this.tab === "headings") this.renderHeadings();
    else if (this.tab === "pages") this.renderPages();
    else this.renderResults();
  }

  // -- headings -----------------------------------------------------------------------

  private entries(): OutlineEntry[] {
    return (this.blocks ??= this.ed.engine ? this.ops.entries() : []);
  }

  private headings(): OutlineEntry[] {
    return this.entries().filter((e) => e.level > 0 && e.text.trim());
  }

  private renderHeadings() {
    const heads = this.headings();
    if (!heads.length) {
      this.body.replaceChildren(h("p", { class: "nav-empty" }, t("Headings appear here as you add them. Use ⌘⌥1–3 to make one.")));
      return;
    }
    const list = h("div", { class: "nav-list", role: "tree", "aria-label": t("Headings") });
    heads.forEach((e, i) => {
      const next = heads[i + 1];
      const parent = next && next.level > e.level;
      // Under a heading the pane has folded.
      const gone = [...this.collapsed].some((id) => {
        const k = heads.findIndex((x) => x.id === id);
        if (k < 0 || k >= i) return false;
        for (let j = k + 1; j <= i; j++) if (heads[j].level <= heads[k].level) return false;
        return true;
      });
      const row = h("div", { class: "nav-item", role: "treeitem", "data-id": String(e.id), "data-level": String(e.level), "aria-level": String(e.level), "aria-expanded": parent ? String(!this.collapsed.has(e.id)) : undefined, draggable: "true", style: `padding-left:${4 + (e.level - 1) * 14}px` });
      if (gone) row.hidden = true;
      const toggle = h("button", { class: `nav-toggle${parent ? "" : " none"}${this.collapsed.has(e.id) ? " folded" : ""}`, type: "button", tabindex: "-1", "aria-label": this.collapsed.has(e.id) ? t("Expand {name}", { name: e.text }) : t("Collapse {name}", { name: e.text }) }, parent ? chevron() : "");
      if (parent) {
        toggle.addEventListener("click", () => {
          if (this.collapsed.has(e.id)) this.collapsed.delete(e.id);
          else this.collapsed.add(e.id);
          this.renderHeadings();
        });
      }
      const label = h("button", { class: "nav-label", type: "button", title: e.text }, e.text);
      label.addEventListener("click", () => this.ed.scrollToParagraph(e.id));
      label.addEventListener("keydown", (ev) => this.headingKey(ev, e));
      row.append(toggle, label);
      this.dragEvents(row, e, heads);
      list.append(row);
    });
    this.body.replaceChildren(list);
    this.syncCurrent();
  }

  /** Alt with the arrow keys moves the heading's section; the arrows alone walk the list. */
  private headingKey(ev: KeyboardEvent, e: OutlineEntry) {
    if (ev.altKey && (ev.key === "ArrowUp" || ev.key === "ArrowDown")) {
      ev.preventDefault();
      if (this.fe.readOnly) return;
      if (this.ops.moveHeading(e.id, ev.key === "ArrowUp" ? -1 : 1)) {
        this.render();
        queueMicrotask(() => this.body.querySelector<HTMLElement>(`.nav-item[data-id="${e.id}"] .nav-label`)?.focus());
      }
      return;
    }
    if (ev.key !== "ArrowUp" && ev.key !== "ArrowDown") return;
    const rows = Array.from(this.body.querySelectorAll<HTMLElement>(".nav-item:not([hidden]) .nav-label"));
    const at = rows.indexOf(ev.currentTarget as HTMLElement);
    const to = rows[at + (ev.key === "ArrowUp" ? -1 : 1)];
    if (to) {
      ev.preventDefault();
      to.focus();
    }
  }

  private dragEvents(row: HTMLElement, e: OutlineEntry, heads: OutlineEntry[]) {
    const clear = () => {
      for (const r of Array.from(this.body.querySelectorAll(".drop-before, .drop-after"))) r.classList.remove("drop-before", "drop-after");
    };
    row.addEventListener("dragstart", (ev) => {
      if (this.fe.readOnly) return ev.preventDefault();
      this.dragged = e.id;
      ev.dataTransfer?.setData("text/plain", e.text);
      if (ev.dataTransfer) ev.dataTransfer.effectAllowed = "move";
      row.classList.add("dragging");
    });
    row.addEventListener("dragend", () => {
      this.dragged = 0;
      row.classList.remove("dragging");
      clear();
    });
    row.addEventListener("dragover", (ev) => {
      if (!this.dragged || !this.canDrop(heads, this.dragged, e.id)) return;
      ev.preventDefault();
      clear();
      const r = row.getBoundingClientRect();
      row.classList.add(ev.clientY < r.top + r.height / 2 ? "drop-before" : "drop-after");
    });
    row.addEventListener("dragleave", () => row.classList.remove("drop-before", "drop-after"));
    row.addEventListener("drop", (ev) => {
      if (!this.dragged || !this.canDrop(heads, this.dragged, e.id)) return;
      ev.preventDefault();
      const before = row.classList.contains("drop-before");
      clear();
      const moved = this.dragged;
      this.dragged = 0;
      this.drop(moved, e.id, before);
    });
  }

  /** A heading can't go inside its own section. */
  private canDrop(heads: OutlineEntry[], from: number, onto: number): boolean {
    if (from === onto) return false;
    const i = heads.findIndex((e) => e.id === from);
    const j = heads.findIndex((e) => e.id === onto);
    if (i < 0 || j < 0) return false;
    for (let k = i + 1; k <= j; k++) if (heads[k].level <= heads[i].level) return true;
    return j < i;
  }

  private drop(from: number, onto: number, before: boolean) {
    if (before) {
      this.ops.moveSection(from, { before: onto });
      return;
    }
    // After the target's whole section: after the last block before the next heading that is not under it.
    const entries = this.entries();
    const i = entries.findIndex((e) => e.id === onto);
    let end = i + 1;
    while (end < entries.length && !(entries[end].level > 0 && entries[end].level <= entries[i].level)) end++;
    this.ops.moveSection(from, { after: entries[end - 1].id });
  }

  /** The heading the caret is in (or the last one above it) is marked. */
  private syncCurrent() {
    if (this.tab === "pages") {
      const page = this.ed.engine ? this.ed.currentPage() : 0;
      for (const b of Array.from(this.body.querySelectorAll<HTMLElement>(".nav-page"))) {
        const on = Number(b.dataset.page) === page;
        b.classList.toggle("current", on);
        if (on) b.setAttribute("aria-current", "page");
        else b.removeAttribute("aria-current");
      }
      return;
    }
    if (this.tab !== "headings" || !this.ed.engine) return;
    const sel = this.ed.sel;
    const block = sel ? this.ed.topBlockId(sel.focus.p) : null;
    const entries = this.entries();
    let i = entries.findIndex((e) => e.id === block);
    while (i > 0 && entries[i].level === 0) i--;
    const id = i >= 0 && entries[i].level > 0 ? entries[i].id : 0;
    for (const r of Array.from(this.body.querySelectorAll<HTMLElement>(".nav-item"))) {
      const on = Number(r.dataset.id) === id;
      r.classList.toggle("current", on);
      const label = r.querySelector(".nav-label");
      if (on) label?.setAttribute("aria-current", "location");
      else label?.removeAttribute("aria-current");
    }
  }

  // -- pages --------------------------------------------------------------------------

  private renderPages() {
    const ed = this.ed;
    if (!ed.engine) {
      this.body.replaceChildren();
      return;
    }
    const grid = h("div", { class: "nav-pages", role: "list", "aria-label": t("Pages") });
    const src = editorSnapshots(ed);
    this.observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const cell = entry.target as HTMLElement;
          if (entry.isIntersecting && !cell.querySelector(".pg-snap")) {
            const k = Number(cell.dataset.page) - 1;
            if (k >= ed.pages) continue;
            const b = ed.pageBox(k);
            const snap = snapshotPage(src, k);
            snap.style.transform = `scale(${THUMB / b.w})`;
            snap.style.transformOrigin = "0 0";
            cell.querySelector(".pg-thumb")?.append(snap);
          } else if (!entry.isIntersecting) cell.querySelector(".pg-snap")?.remove();
        }
      },
      { root: this.body, rootMargin: "300px 0px" },
    );
    for (let k = 0; k < ed.pages; k++) {
      const b = ed.pageBox(k);
      const label = t("Page {page}", { page: k + 1 });
      const thumb = h("div", { class: "pg-thumb" });
      thumb.style.width = `${THUMB}px`;
      thumb.style.height = `${Math.round(b.h * (THUMB / b.w))}px`;
      const cell = h("button", { class: "nav-page", type: "button", role: "listitem", "data-page": String(k + 1), "aria-label": label }, thumb, h("span", { class: "nav-page-num" }, String(k + 1)));
      cell.addEventListener("click", () => {
        goToPage(ed, k + 1);
      });
      grid.append(cell);
      this.observer.observe(cell);
    }
    this.body.replaceChildren(ed.continuous ? h("p", { class: "nav-empty" }, t("This layout has no pages. Switch to the print layout to see them.")) : grid);
    this.syncCurrent();
  }

  // -- results ------------------------------------------------------------------------

  private renderResults() {
    window.clearTimeout(this.searchTimer);
    const drawn = this.results;
    if (drawn && drawn.input.isConnected && this.body.contains(drawn.input)) {
      this.runSearch(drawn.list, drawn.status);
      return;
    }
    const input = h("input", { type: "search", class: "nav-search", placeholder: t("Search the document"), "aria-label": t("Search the document"), value: this.query, spellcheck: "false" });
    this.input = input;
    const matchCase = h("input", { type: "checkbox", id: "nav-case" });
    matchCase.checked = this.matchCase;
    const status = h("div", { class: "nav-status", role: "status", "aria-live": "polite" });
    const list = h("div", { class: "nav-results", role: "list", "aria-label": t("Results") });
    const find = h("button", { class: "nav-link", type: "button" }, t("Find and replace…"));
    find.addEventListener("click", () => void this.fe.command("find.open").catch(() => undefined));
    const run = () => {
      window.clearTimeout(this.searchTimer);
      this.query = input.value;
      this.matchCase = matchCase.checked;
      this.runSearch(list, status);
    };
    input.addEventListener("input", () => {
      // Kept at once, so nothing that draws the tab again in the meantime puts back an older search.
      this.query = input.value;
      window.clearTimeout(this.searchTimer);
      this.searchTimer = window.setTimeout(run, 200);
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        run();
        list.querySelector<HTMLElement>(".nav-result")?.focus();
      }
    });
    matchCase.addEventListener("change", run);
    this.body.replaceChildren(h("div", { class: "nav-searchbar" }, input, h("label", { class: "nav-case", for: "nav-case" }, matchCase, t("Match case"))), status, list, find);
    this.results = { input, list, status };
    this.runSearch(list, status);
  }

  private runSearch(list: HTMLElement, status: HTMLElement) {
    list.replaceChildren();
    const q = this.query;
    if (!q.trim() || !this.ed.engine) {
      status.textContent = "";
      list.append(h("p", { class: "nav-empty" }, t("Type to search the text, headers, footers and notes.")));
      return;
    }
    let hits: FindHit[] = [];
    try {
      hits = this.ed.engine.find(q, { ignore_case: !this.matchCase }).hits;
    } catch {
      status.textContent = t("That search can't be run.");
      return;
    }
    status.textContent = hits.length ? tn(hits.length, "{n} result", "{n} results") : t("No results");
    for (const m of hits.slice(0, RESULT_LIMIT)) {
      const row = h("button", { class: "nav-result", type: "button", role: "listitem" }, h("span", { class: "r-before" }, m.before), h("mark", {}, m.text), h("span", { class: "r-after" }, m.after));
      row.addEventListener("click", () => this.goTo(m));
      list.append(row);
    }
    if (hits.length > RESULT_LIMIT) list.append(h("p", { class: "nav-empty" }, t("Showing the first {n}.", { n: RESULT_LIMIT })));
  }

  private goTo(m: FindHit) {
    const ed = this.ed;
    if (ed.story && !m.story) ed.exitStory(false);
    const story = m.story && m.story.kind !== "textbox" ? (m.story as BookmarkInfo["story"]) : undefined;
    ed.selectBookmark({ name: "", p: m.p, o: m.o, endP: m.endP, endO: m.endO, text: "", story });
  }
}

function chevron(): SVGElement {
  const el = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  el.setAttribute("viewBox", "0 0 24 24");
  el.setAttribute("fill", "none");
  el.setAttribute("stroke", "currentColor");
  el.setAttribute("stroke-width", "2.25");
  el.setAttribute("stroke-linecap", "round");
  el.setAttribute("stroke-linejoin", "round");
  el.setAttribute("aria-hidden", "true");
  el.innerHTML = '<path d="m8 10 4 4 4-4"/>';
  return el;
}
