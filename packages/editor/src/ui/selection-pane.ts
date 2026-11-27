// The Selection pane: every shape, text box, picture, chart and group of the document in a list, front to back as they
// are stacked, each with its kind, its name and an eye to hide it. A click selects the object (Shift or Ctrl adds to
// the selection), a double-click or F2 renames it, a row can be dragged up or down to change the stacking, and Show
// all and Hide all do the eyes at once. It lists the whole document, not one page, because the stacking order is the
// document's. Every change is an op (`set_object_props`, `move_object`), one undo step each.
import { t, tn } from "@docxtor/extensions";
import type { MPos, ObjectInfo, Op } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { ALIGN_ICONS } from "./align-icons";
import { openDock, type Dock } from "./dock";
import { h, overlayLayer, svg } from "./dom";
import type { ObjectSet } from "./object-set";
import type { ShapeTool } from "./shape-tool";

const ID = "selection-pane";
const key = (o: { p: number; o: number }) => `${o.p}:${o.o}`;

const KIND_ICON: Record<ObjectInfo["kind"], keyof typeof ALIGN_ICONS> = {
  shape: "kindShape",
  textbox: "kindTextBox",
  picture: "kindPicture",
  chart: "kindChart",
  group: "kindGroup",
  canvas: "kindGroup",
};

const kindLabel = (k: ObjectInfo["kind"]) =>
  ({ shape: t("Shape"), textbox: t("Text box"), picture: t("Picture"), chart: t("Chart"), group: t("Group"), canvas: t("Drawing canvas") })[k];

type Section = "front" | "behind" | "inline";

/** The panel for one editor. */
export class SelectionPane {
  private dock: Dock | null = null;
  private list!: HTMLElement;
  private count!: HTMLElement;
  private showAll!: HTMLButtonElement;
  private hideAll!: HTMLButtonElement;
  private items: ObjectInfo[] = [];
  private signature = "";
  /** A rename or a drag is under way: the list is not rebuilt under it. */
  private busy = false;
  private stops: (() => void)[] = [];

  constructor(
    private ed: Editor,
    private set: ObjectSet,
    private shapes: ShapeTool,
    private events: { on(event: "selection" | "change" | "open", fn: () => void): () => void },
  ) {}

  get isOpen(): boolean {
    return !!this.dock;
  }

  open() {
    if (!this.dock) {
      this.dock = openDock(this.ed, ID, t("Selection pane"), () => this.closed());
      this.build(this.dock);
      // The rows follow the selection on the page: shapes tell when theirs changes, pictures only through the browser's selection.
      const follow = () => requestAnimationFrame(() => this.mark());
      document.addEventListener("selectionchange", follow);
      // Beside another panel (Format shape) rather than over it.
      const stack = () => this.stack();
      const watch = new MutationObserver(stack);
      watch.observe(overlayLayer(), { childList: true });
      window.addEventListener("resize", stack);
      stack();
      this.stops = [() => watch.disconnect(), () => window.removeEventListener("resize", stack), this.events.on("change", () => this.sync()), this.events.on("selection", () => this.mark()), this.events.on("open", () => this.sync(true)), this.shapes.onSelection(() => this.mark()), () => document.removeEventListener("selectionchange", follow)];
    }
    this.sync(true);
    this.list.querySelector<HTMLElement>(".sp-row")?.focus({ preventScroll: true });
  }

  close() {
    this.dock?.close();
  }

  /** Put the pane to the left of any other panel that is open at the right edge, so they don't cover each other. */
  private stack() {
    const el = this.dock?.el;
    if (!el) return;
    const c = this.ed.canvas.getBoundingClientRect();
    let right = Math.max(8, window.innerWidth - c.right + 8);
    for (const other of Array.from(overlayLayer().querySelectorAll<HTMLElement>(".dock"))) {
      if (other !== el) right += other.offsetWidth + 8;
    }
    el.style.right = `${right}px`;
  }

  private closed() {
    for (const s of this.stops) s();
    this.stops = [];
    this.dock = null;
  }

  private build(dock: Dock) {
    this.showAll = h("button", { type: "button", class: "tb text-tb", title: t("Show every object") }, t("Show all"));
    this.hideAll = h("button", { type: "button", class: "tb text-tb", title: t("Hide every object") }, t("Hide all"));
    for (const [b, hidden] of [[this.showAll, false], [this.hideAll, true]] as const) {
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => this.setAll(hidden));
    }
    this.count = h("span", { class: "sp-count", "aria-live": "polite" });
    this.list = h("div", { class: "sp-list", role: "list", "aria-label": t("Objects") });
    this.list.addEventListener("keydown", (e) => this.onKey(e));
    dock.body.classList.add("sp-body");
    dock.body.append(h("div", { class: "sp-bar" }, this.count, h("span", { class: "sp-bar-gap" }), this.showAll, this.hideAll), this.list);
  }

  // -- the list ------------------------------------------------------------------------------------

  private selected(): Set<string> {
    return new Set(this.set.all().map(key));
  }

  /** Rebuild the rows when the objects changed; otherwise only the selection and the eyes. */
  private sync(force = false) {
    if (!this.dock || !this.ed.engine) return;
    const items = this.ed.engine.objects();
    const sig = JSON.stringify(items.map((i) => [i.p, i.o, i.name, i.hidden ?? false, i.z ?? null, i.behind ?? false, i.wrap, i.kind]));
    this.items = items;
    if (sig !== this.signature || force) {
      this.signature = sig;
      if (!this.busy) this.rebuild();
    }
    this.mark();
  }

  private sections(): { id: Section; title: string; items: ObjectInfo[] }[] {
    // Nearer the viewer first. Objects with equal values are drawn in document order, so the later one is nearer.
    const order = new Map(this.items.map((it, i) => [key(it), i]));
    const nearest = (a: ObjectInfo, b: ObjectInfo) => (b.z ?? 0) - (a.z ?? 0) || order.get(key(b))! - order.get(key(a))!;
    const front = this.items.filter((i) => i.floating && !i.behind).sort(nearest);
    const behind = this.items.filter((i) => i.floating && i.behind).sort(nearest);
    const inline = this.items.filter((i) => !i.floating);
    return [
      { id: "front" as Section, title: t("In front of the text"), items: front },
      { id: "behind" as Section, title: t("Behind the text"), items: behind },
      { id: "inline" as Section, title: t("In line with the text"), items: inline },
    ].filter((s) => s.items.length);
  }

  private rebuild() {
    const sections = this.sections();
    this.list.replaceChildren();
    if (!this.items.length) {
      this.list.append(h("p", { class: "sp-empty" }, t("No shapes, text boxes, pictures or charts in this document yet.")));
    }
    for (const s of sections) {
      // The headings tell the stacks apart only when there is more than one.
      if (sections.length > 1) this.list.append(h("div", { class: "sp-section", role: "presentation" }, s.title));
      for (const it of s.items) this.list.append(this.row(it, s.id));
    }
    // One stop for the Tab key; the arrow keys go from row to row.
    this.list.querySelector<HTMLElement>(".sp-row")?.setAttribute("tabindex", "0");
    const n = this.items.length;
    this.count.textContent = tn(n, "{n} object", "{n} objects");
    const hiddenCount = this.items.filter((i) => i.hidden).length;
    this.showAll.disabled = hiddenCount === 0;
    this.hideAll.disabled = hiddenCount === n;
  }

  private row(it: ObjectInfo, section: Section): HTMLElement {
    const eye = h("button", {
      type: "button",
      class: "sp-eye",
      "aria-pressed": String(!!it.hidden),
      "aria-label": it.hidden ? t("Show {name}", { name: it.name }) : t("Hide {name}", { name: it.name }),
      title: it.hidden ? t("Show") : t("Hide"),
    });
    eye.append(svg(it.hidden ? ALIGN_ICONS.eyeOff : ALIGN_ICONS.eye));
    eye.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
    });
    eye.addEventListener("click", (e) => {
      e.stopPropagation();
      this.setProps(it, { hidden: !it.hidden });
    });
    const handle = h("span", { class: "sp-grip", "aria-hidden": "true" });
    if (section !== "inline") handle.append(svg(ALIGN_ICONS.dragHandle));
    const icon = h("span", { class: "sp-kind", title: kindLabel(it.kind), "aria-hidden": "true" });
    icon.append(svg(ALIGN_ICONS[KIND_ICON[it.kind]]));
    const name = h("span", { class: "sp-name" }, it.name);
    const row = h("div", { class: `sp-row${it.hidden ? " hidden" : ""}`, role: "listitem", tabindex: "-1", "data-key": key(it), "data-section": section, title: it.hidden ? t("Hidden") : kindLabel(it.kind) }, handle, icon, name, eye);
    // Shift or Ctrl+click adds to the objects selected. Safari would also stretch the text selection from the page to the row, which takes the
    // selection off the objects before the click is heard, so the key's own selecting is held back.
    row.addEventListener("mousedown", (e) => {
      if (e.shiftKey || e.ctrlKey || e.metaKey) e.preventDefault();
    });
    row.addEventListener("click", (e) => this.pick(it, e));
    row.addEventListener("dblclick", (e) => {
      if ((e.target as HTMLElement).closest(".sp-eye")) return;
      this.rename(it, row);
    });
    if (section !== "inline") row.addEventListener("pointerdown", (e) => this.dragStart(e, it, row, section));
    return row;
  }

  /** Mark the rows of the selected objects. */
  private mark() {
    if (!this.dock) return;
    const sel = this.selected();
    for (const row of Array.from(this.list.querySelectorAll<HTMLElement>(".sp-row"))) {
      const on = sel.has(row.dataset.key ?? "");
      row.classList.toggle("on", on);
      if (on) row.setAttribute("aria-current", "true");
      else row.removeAttribute("aria-current");
    }
    // The first selected row stays in view when the page selection moves it.
    const first = this.list.querySelector<HTMLElement>(".sp-row.on");
    if (first && !this.busy && !this.list.contains(document.activeElement)) first.scrollIntoView({ block: "nearest" });
  }

  // -- acting on the objects -----------------------------------------------------------------------

  private pos(it: ObjectInfo): MPos {
    return { p: it.p, o: it.o };
  }

  /** Select the object a row stands for; Shift or Ctrl (Command) adds it to the selection or takes it out. */
  private pick(it: ObjectInfo, e: MouseEvent) {
    if ((e.target as HTMLElement).closest(".sp-eye, .sp-name-input")) return;
    if (it.hidden) {
      // A hidden object isn't on the page to select.
      this.markOnly(it);
      return;
    }
    const here = this.pos(it);
    const all = this.set.all();
    if (e.shiftKey || e.ctrlKey || e.metaKey) {
      const has = all.some((p) => p.p === here.p && p.o === here.o);
      const next = has ? all.filter((p) => !(p.p === here.p && p.o === here.o)) : [...all, here];
      if (next.length) this.set.select(next);
    } else this.set.select([here]);
    this.mark();
  }

  private markOnly(it: ObjectInfo) {
    for (const row of Array.from(this.list.querySelectorAll<HTMLElement>(".sp-row"))) {
      const on = row.dataset.key === key(it);
      row.classList.toggle("on", on);
      if (on) row.setAttribute("aria-current", "true");
      else row.removeAttribute("aria-current");
    }
  }

  /** Change one object's name or visibility. */
  private setProps(it: ObjectInfo, props: { name?: string; hidden?: boolean }) {
    const keep = this.ed.sel;
    this.ed.exec({ op: "set_object_props", at: this.pos(it), ...props } as Op, { select: keep });
    this.set.refresh();
  }

  private setAll(hidden: boolean) {
    const ops = this.items.filter((i) => !!i.hidden !== hidden).map((i) => ({ op: "set_object_props", at: this.pos(i), hidden }) as Op);
    if (!ops.length) return;
    this.ed.exec(ops, { select: this.ed.sel });
    this.set.refresh();
  }

  private rename(it: ObjectInfo, row: HTMLElement) {
    if (this.busy) return;
    const name = row.querySelector<HTMLElement>(".sp-name");
    if (!name) return;
    this.busy = true;
    const input = h("input", { type: "text", class: "sp-name-input", value: it.name, "aria-label": t("Name"), maxlength: "255" });
    name.replaceWith(input);
    input.focus();
    input.select();
    let done = false;
    const finish = (commit: boolean) => {
      if (done) return;
      done = true;
      this.busy = false;
      const v = input.value.trim();
      if (commit && v && v !== it.name) this.setProps(it, { name: v });
      else {
        input.replaceWith(name);
        this.sync(true);
      }
      row.focus({ preventScroll: true });
    };
    input.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Enter") {
        e.preventDefault();
        finish(true);
      } else if (e.key === "Escape") {
        e.preventDefault();
        finish(false);
      }
    });
    input.addEventListener("blur", () => finish(true));
    input.addEventListener("click", (e) => e.stopPropagation());
    input.addEventListener("dblclick", (e) => e.stopPropagation());
    input.addEventListener("pointerdown", (e) => e.stopPropagation());
  }

  private onKey(e: KeyboardEvent) {
    const row = (e.target as HTMLElement).closest<HTMLElement>(".sp-row");
    if (!row || (e.target as HTMLElement).closest("input")) return;
    const rows = Array.from(this.list.querySelectorAll<HTMLElement>(".sp-row"));
    const i = rows.indexOf(row);
    const it = this.items.find((x) => key(x) === row.dataset.key);
    if (e.altKey && (e.key === "ArrowDown" || e.key === "ArrowUp") && it && row.dataset.section !== "inline") {
      // Alt and an arrow moves the object one place in its stack, for the keyboard.
      e.preventDefault();
      const peers = rows.filter((r) => r.dataset.section === row.dataset.section);
      const k = peers.indexOf(row);
      const other = peers[e.key === "ArrowUp" ? k - 1 : k + 1];
      if (!other) return;
      const [p, o] = other.dataset.key!.split(":").map(Number);
      this.ed.exec({ op: "move_object", at: this.pos(it), [e.key === "ArrowUp" ? "above" : "below"]: { p, o } } as Op, { select: this.ed.sel });
      this.set.refresh();
      this.sync(true);
      this.list.querySelector<HTMLElement>(`.sp-row[data-key="${key(it)}"]`)?.focus({ preventScroll: true });
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      rows[Math.max(0, Math.min(rows.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))]?.focus();
    } else if (e.key === "F2" && it) {
      e.preventDefault();
      this.rename(it, row);
    } else if ((e.key === "Enter" || e.key === " ") && it) {
      e.preventDefault();
      this.pick(it, e as unknown as MouseEvent);
    }
  }

  // -- dragging a row ----------------------------------------------------------------------------------

  /** Press on a row: past a few pixels it becomes a drag that drops the object between two others of its stack. */
  private dragStart(e: PointerEvent, it: ObjectInfo, row: HTMLElement, section: Section) {
    if (e.button !== 0 || (e.target as HTMLElement).closest(".sp-eye, .sp-name-input") || this.busy) return;
    const peers = Array.from(this.list.querySelectorAll<HTMLElement>(`.sp-row[data-section="${section}"]`));
    const sy = e.clientY;
    let dragging = false;
    let slot = -1;
    const line = h("div", { class: "sp-drop", "aria-hidden": "true" });
    const move = (ev: PointerEvent) => {
      if (!dragging) {
        if (Math.abs(ev.clientY - sy) < 4) return;
        dragging = true;
        this.busy = true;
        row.classList.add("dragging");
        this.list.classList.add("sorting");
        this.list.append(line);
      }
      // Which gap the pointer is nearest: 0 above the first row of the stack, n below the last.
      let best = 0;
      let bestD = Infinity;
      for (let i = 0; i <= peers.length; i++) {
        const y = i < peers.length ? peers[i].offsetTop : peers[peers.length - 1].offsetTop + peers[peers.length - 1].offsetHeight;
        const d = Math.abs(ev.clientY - (this.list.getBoundingClientRect().top - this.list.scrollTop + y));
        if (d < bestD) (bestD = d), (best = i);
      }
      slot = best;
      const y = best < peers.length ? peers[best].offsetTop : peers[peers.length - 1].offsetTop + peers[peers.length - 1].offsetHeight;
      line.style.top = `${y}px`;
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      if (!dragging) return;
      row.classList.remove("dragging");
      this.list.classList.remove("sorting");
      line.remove();
      this.busy = false;
      this.drop(it, peers, slot);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  }

  /** Drop `it` into the gap `slot` of the rows `peers` (a stack, front to back). */
  private drop(it: ObjectInfo, peers: HTMLElement[], slot: number) {
    const keys = peers.map((r) => r.dataset.key!);
    const from = keys.indexOf(key(it));
    // Dropping where it already is changes nothing.
    if (slot < 0 || slot === from || slot === from + 1) return this.sync(true);
    const at = this.pos(it);
    const lookup = (k: string): MPos => {
      const [p, o] = k.split(":").map(Number);
      return { p, o };
    };
    // In the list the front is at the top: a gap above row `slot` puts the object directly in front of that row; at the end, behind the last.
    const op: Op = slot < keys.length ? ({ op: "move_object", at, above: lookup(keys[slot]) } as Op) : ({ op: "move_object", at, below: lookup(keys[keys.length - 1]) } as Op);
    this.ed.exec(op, { select: this.ed.sel });
    this.set.refresh();
    this.sync(true);
  }
}
