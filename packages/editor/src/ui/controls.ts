// Content controls and legacy form fields at work: a tab above the control the caret is in (its name, a button that
// opens its list or calendar, Properties and Remove), check boxes that tick on a click or Space, lists and calendars,
// picture controls, repeating sections with their Add item button, placeholder text that is selected whole, and the
// blanks of legacy forms. The engine does the editing (ops of the `control` and `form_field` kinds); this is the part
// a person touches. Kinds that extensions add (`ctx.contentControls`) are drawn and opened through here too.
import { t, type ContentControlContribution, type ContentControlInfo } from "@docxtor/extensions";
import type { ControlProps, ControlRef, MPos, Op, RControl, RForm } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { controlName, TYPED } from "../editor/controls-render";
import { openCalendar } from "./control-dates";
import { CONTROL_ICONS } from "./control-icons";
import { openControlProperties, openFormProperties } from "./control-properties";
import { h, overlayLayer, popover } from "./dom";
import { I } from "./icons";

/** A control the caret is in, with what is needed to act on it. */
export interface Located {
  /** For ops. */
  ref: ControlRef;
  view: RControl;
  /** The elements that show it: its text segments, or its block, or its row. */
  els: HTMLElement[];
  /** The paragraph of an inline control. */
  p?: number;
}

const PT = 0.75;

export class ControlsTool {
  private tab: HTMLElement;
  private active: Located | null = null;
  private sticky: { p: number; n: number } | null = null;
  private settingSelection = false;
  private seen = new WeakSet<HTMLElement>();
  private seenFor = 0;

  constructor(
    private ed: Editor,
    private contributions: () => ContentControlContribution[] = () => [],
  ) {
    this.tab = h("div", { class: "ctl-tab", style: "display:none", role: "toolbar", "aria-label": t("Content control") });
    overlayLayer().append(ed.life.own(this.tab));
    const f = ed.flow;
    // Boxes tick on a click and keep the caret where it was.
    f.addEventListener("mousedown", (e) => {
      const el = e.target as HTMLElement;
      if (el.closest?.(".ctl-check, .frm-box, .frm-select, .ghost.ff[data-frm]")) e.preventDefault();
    });
    f.addEventListener("click", (e) => this.onClick(e));
    // The selection events can arrive after an edit that failed (a locked control): look again once the pointer or key is up.
    f.addEventListener("mouseup", () => setTimeout(() => this.refresh()));
    f.addEventListener("keyup", () => this.refresh());
    ed.canvas.addEventListener("scroll", () => this.place(), { passive: true });
    ed.life.on(window, "resize", () => this.place());
  }

  /** The kinds extensions offer changed: draw the controls again. */
  reapply() {
    this.seen = new WeakSet();
    this.seenFor = -1;
    this.ed.rerender();
    this.refresh();
  }

  // -- finding the control ------------------------------------------------------

  /** The inline controls of a paragraph that cover a range, innermost first. */
  private inlineAt(p: number, from: number, to: number, strict: boolean): RControl[] {
    const view = this.ed.paragraphView(p);
    const hits = (view?.controls ?? []).filter((c) => c.kind !== "bibliography" && c.kind !== "docPart" && c.kind !== "equation" && (strict ? (from > c.start && to < c.end) || (from === c.start && to === c.end && to > from) || (from > c.start && to <= c.end && from < c.end) : from >= c.start && to <= c.end));
    return hits.sort((a, b) => a.end - a.start - (b.end - b.start));
  }

  private paraEl(p: number): HTMLElement | null {
    return this.ed.paraElement(p);
  }

  private segsOf(p: number, c: RControl): HTMLElement[] {
    const el = this.paraEl(p);
    if (!el) return [];
    return Array.from(el.children).filter((s): s is HTMLElement => s instanceof HTMLElement && s.dataset.o !== undefined && Number(s.dataset.o) >= c.start && Number(s.dataset.o) < c.end);
  }

  /** The controls around a paragraph that are blocks or table rows, innermost first. */
  private blocksAround(p: number): Located[] {
    const el = this.paraEl(p);
    const out: Located[] = [];
    for (let n: HTMLElement | null = el; n && n !== this.ed.root; n = n.parentElement) {
      if (n.classList.contains("fc-ctl") && n.dataset.ctlId) {
        const id = Number(n.dataset.ctlId);
        const b = this.ed.blockView(id);
        if (b?.t === "c" && b.control) out.push({ ref: { block: id }, view: b.control, els: [n] });
      } else if (n.tagName === "TR" && n.dataset.ctlWrap) {
        const wrap = Number(n.dataset.ctlWrap);
        const view: RControl = { n: 0, kind: n.dataset.ck ?? "other", start: 0, end: 0, ctl: wrap, fixedItems: !!n.dataset.repFixed };
        out.push({ ref: { wrap }, view, els: [n] });
      }
    }
    return out;
  }

  /** The control a range sits in: an inline control in the paragraph, else the block around it. */
  locate(strict = true): Located | null {
    const r = this.ed.peekRange();
    if (!r) return null;
    const [a, b] = r;
    if (a.p === b.p) {
      const inl = this.inlineAt(a.p, a.o, b.o, strict)[0];
      if (inl) return { ref: { p: a.p, n: inl.n }, view: inl, els: this.segsOf(a.p, inl), p: a.p };
      // A click on a control keeps it active while the caret is still at its edge.
      if (this.sticky && this.sticky.p === a.p) {
        const c = this.ed.paragraphView(a.p)?.controls?.find((x) => x.n === this.sticky!.n);
        if (c && a.o >= c.start && b.o <= c.end) return { ref: { p: a.p, n: c.n }, view: c, els: this.segsOf(a.p, c), p: a.p };
      }
    }
    return this.blocksAround(a.p)[0] ?? null;
  }

  /** The controls of a repeating section around the caret: its item and the section. */
  private repeating(): { item?: Located; section?: Located } {
    const r = this.ed.peekRange();
    if (!r) return {};
    const chain = this.blocksAround(r[0].p);
    const item = chain.find((c) => c.view.kind === "repeatingItem");
    const sec = chain.find((c) => c.view.kind === "repeating");
    // A row that is an item has the section's id in its data.
    if (item && !sec && item.els[0].dataset.repSection) {
      return { item, section: { ref: { wrap: Number(item.els[0].dataset.repSection) }, view: { n: 0, kind: "repeating", start: 0, end: 0, fixedItems: !!item.els[0].dataset.repFixed }, els: item.els } };
    }
    return { item, section: sec };
  }

  // -- the tab ------------------------------------------------------------------

  /** Called after the document or the selection changed. */
  refresh() {
    this.applyContributions();
    if (this.ed.readOnly || !this.ed.engine) return this.hide();
    const r = this.ed.peekRange();
    if (r && !this.settingSelection) {
      const [a, b] = r;
      // Placeholder text is selected whole, as Word does when the caret lands in it.
      if (a.p === b.p && a.o === b.o) {
        const ph = this.inlineAt(a.p, a.o, a.o, true).find((c) => c.placeholder);
        if (ph && a.o > ph.start && a.o < ph.end) {
          this.settingSelection = true;
          this.ed.select({ anchor: { p: a.p, o: ph.start }, focus: { p: a.p, o: ph.end } });
          this.settingSelection = false;
        }
      }
    }
    const loc = this.locate();
    const rep = this.repeating();
    this.active = loc ?? rep.item ?? null;
    this.mark();
    if (!this.active) return this.hide();
    this.build(this.active, rep);
    this.place();
  }

  hide() {
    this.active = null;
    if (this.tab.style.display !== "none") this.tab.style.display = "none";
    this.unmark();
  }

  /** The elements that carry `ctl-on` now: found by what was marked, not by searching the page (a keystroke goes through here twice). */
  private marked: HTMLElement[] = [];

  private unmark() {
    for (const e of this.marked) e.classList.remove("ctl-on");
    this.marked = [];
  }

  private mark() {
    this.unmark();
    if (this.active && this.active.view.appearance !== "hidden") {
      for (const e of this.active.els) {
        e.classList.add("ctl-on");
        this.marked.push(e);
      }
    }
  }

  private btn(icon: string, label: string, run: (b: HTMLButtonElement) => void): HTMLButtonElement {
    const b = h("button", { class: "ctl-btn", type: "button", title: label, "aria-label": label, html: icon });
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => run(b));
    return b;
  }

  private build(loc: Located, rep: { item?: Located; section?: Located }) {
    const v = loc.view;
    const custom = this.custom(v);
    this.tab.textContent = "";
    if (v.appearance === "hidden") {
      this.tab.style.display = "none";
      return;
    }
    const name = h("span", { class: "ctl-name" }, custom?.label && !v.alias && !v.tag ? custom.label : controlName(v));
    this.tab.append(name);
    if (v.lock) this.tab.append(h("span", { class: "ctl-lock", title: v.lock === "contentLocked" ? t("Contents can't be edited") : v.lock === "sdtLocked" ? t("Can't be deleted") : t("Can't be deleted or edited"), html: I.lock }));
    const locked = v.lock === "contentLocked" || v.lock === "sdtContentLocked";
    if (!locked) {
      if (custom?.edit) this.tab.append(this.btn(I.chevron, t("Edit"), (b) => this.customEdit(loc, custom, b)));
      else if (v.kind === "dropDown" || v.kind === "comboBox") this.tab.append(this.btn(I.chevron, t("Choose an item"), (b) => this.openList(loc, b)));
      else if (v.kind === "date") this.tab.append(this.btn(CONTROL_ICONS.controlDate, t("Choose a date"), (b) => this.openDate(loc, b)));
      else if (v.kind === "picture") this.tab.append(this.btn(CONTROL_ICONS.controlPicture, t("Choose a picture"), () => this.choosePicture(loc)));
    }
    const sec = rep.section;
    if (sec && !sec.view.fixedItems) {
      this.tab.append(this.btn(I.plus, t("Add item"), () => this.addItem()));
      this.tab.append(this.btn(I.minus, t("Remove item"), () => this.removeItem()));
    }
    if (v.kind !== "repeating" && v.kind !== "repeatingItem") this.tab.append(this.btn(CONTROL_ICONS.controlProperties, t("Properties"), () => this.properties()));
    if (v.kind !== "repeatingItem" && v.lock !== "sdtLocked" && v.lock !== "sdtContentLocked") this.tab.append(this.btn(I.close, t("Remove content control"), () => void this.ed.command("control.remove")));
    this.tab.style.display = "";
  }

  private place() {
    if (!this.active || this.tab.style.display === "none") return;
    const el = this.active.els[0];
    if (!el?.isConnected) return this.hide();
    const r = el.getBoundingClientRect();
    const c = this.ed.canvas.getBoundingClientRect();
    if (r.bottom < c.top || r.top > c.bottom) {
      this.tab.style.visibility = "hidden";
      return;
    }
    this.tab.style.visibility = "";
    const h2 = this.tab.offsetHeight || 24;
    this.tab.style.left = `${Math.max(c.left + 4, Math.min(r.left, c.right - this.tab.offsetWidth - 8))}px`;
    this.tab.style.top = `${Math.max(c.top + 2, r.top - h2 - 2)}px`;
  }

  // -- what a click does ----------------------------------------------------------

  private onClick(e: MouseEvent) {
    if (this.ed.readOnly) return;
    const el = e.target as HTMLElement;
    const frm = el.closest?.<HTMLElement>(".frm, .ghost.ff[data-frm]");
    if (frm) return this.onForm(frm, e);
    const seg = el.closest?.<HTMLElement>(".ctl");
    const blockCtl = el.closest?.<HTMLElement>(".fc-ctl.ctl-ph");
    if (!seg) {
      if (blockCtl && !el.closest(".fc-label")) this.selectBlockPlaceholder(blockCtl);
      return;
    }
    const p = Number(seg.closest<HTMLElement>("[data-p]")?.dataset.p);
    const n = Number(seg.dataset.ctl);
    const view = this.ed.paragraphView(p)?.controls?.find((c) => c.n === n);
    if (!view || !Number.isFinite(p)) return;
    this.sticky = { p, n };
    const loc: Located = { ref: { p, n }, view, els: this.segsOf(p, view), p };
    // An extension that took over this kind opens its own editor.
    const custom = this.custom(view);
    if (custom?.edit) {
      this.ed.select({ anchor: { p, o: view.start }, focus: { p, o: view.end } });
      this.customEdit(loc, custom, loc.els[0] ?? seg);
      return;
    }
    if (view.lock === "contentLocked" || view.lock === "sdtContentLocked") return;
    if (view.kind === "checkbox") {
      e.preventDefault();
      this.toggle(loc);
    } else if (view.placeholder) {
      this.ed.select({ anchor: { p, o: view.start }, focus: { p, o: view.end } });
      if (view.kind === "picture") this.choosePicture(loc);
      else if (view.kind === "date") this.openDate(loc, seg);
      else if (view.kind === "dropDown") this.openList(loc, seg);
    } else if (view.kind === "dropDown") {
      this.ed.select({ anchor: { p, o: view.start }, focus: { p, o: view.end } });
      this.openList(loc, seg);
    } else if (view.kind === "date") {
      this.ed.select({ anchor: { p, o: view.start }, focus: { p, o: view.end } });
      this.openDate(loc, seg);
    } else if (view.kind === "picture" && !seg.closest(".atom.img")) {
      this.choosePicture(loc);
    }
  }

  private selectBlockPlaceholder(box: HTMLElement) {
    const ps = Array.from(box.querySelectorAll<HTMLElement>("[data-p]"));
    if (!ps.length) return;
    const first = Number(ps[0].dataset.p);
    const last = Number(ps[ps.length - 1].dataset.p);
    const endLen = Array.from(ps[ps.length - 1].children).reduce((m, c) => Math.max(m, c instanceof HTMLElement && c.dataset.o ? Number(c.dataset.o) + (c.textContent?.length ?? 1) : 0), 0);
    this.ed.select({ anchor: { p: first, o: 0 }, focus: { p: last, o: endLen } });
  }

  private onForm(frm: HTMLElement, e: MouseEvent) {
    const p = Number(frm.closest<HTMLElement>("[data-p]")?.dataset.p);
    const n = Number(frm.dataset.frm);
    const form = this.ed.paragraphView(p)?.forms?.find((f) => f.n === n);
    if (!form || form.disabled) return;
    e.preventDefault();
    if (form.kind === "checkbox") {
      this.ed.exec({ op: "form_field", action: "set", at: { p, o: 0 }, n, checked: !form.checked }, { select: this.ed.sel ?? undefined });
    } else if (form.kind === "dropdown") {
      this.listPopover(frm, form.items ?? [], form.selected ?? 0, (i) => this.ed.exec({ op: "form_field", action: "set", at: { p, o: 0 }, n, index: i }, { select: this.ed.sel ?? undefined }));
    }
  }

  // -- values --------------------------------------------------------------------

  private set(loc: Located, value: { text?: string; checked?: boolean; index?: number; date?: string; picture?: unknown }, select?: boolean): boolean {
    const op: Op = { op: "control", action: "set", target: loc.ref, ...value };
    const sel = select && loc.p !== undefined ? { anchor: { p: loc.p, o: loc.view.start }, focus: { p: loc.p, o: loc.view.start } } : undefined;
    return !!this.ed.exec(op, sel ? { select: sel } : {});
  }

  private toggle(loc: Located) {
    const p = loc.p;
    const ok = this.set(loc, { checked: !loc.view.checked });
    // The box stays selected, so Space ticks it again.
    if (ok && p !== undefined) {
      const c = this.ed.paragraphView(p)?.controls?.find((x) => x.n === loc.ref.n);
      if (c) this.ed.select({ anchor: { p, o: c.start }, focus: { p, o: c.end } });
      this.refresh();
    }
  }

  private listPopover(anchor: HTMLElement, labels: string[], current: number, pick: (i: number) => void): () => void {
    const list = h("div", { class: "ctl-list", role: "listbox" });
    labels.forEach((label, i) => {
      const b = h("button", { class: "menu-item", type: "button", role: "option", "aria-selected": String(i === current) }, h("span", { class: "mi-icon", html: i === current ? I.check : "" }), h("span", { class: "mi-label" }, label));
      b.addEventListener("click", () => {
        close();
        pick(i);
      });
      list.append(b);
    });
    if (!labels.length) list.append(h("div", { class: "menu-empty" }, t("This list has no items yet. Add some in Properties.")));
    const close = popover(anchor, list);
    return close;
  }

  private openList(loc: Located, anchor: HTMLElement) {
    const items = loc.view.items ?? [];
    const cur = loc.view.placeholder ? 0 : Math.max(0, items.findIndex((i) => i.display === this.textOf(loc)));
    this.listPopover(
      anchor,
      items.map((i) => i.display),
      cur,
      (i) => this.set(loc, { index: i }),
    );
  }

  private textOf(loc: Located): string {
    return loc.els.map((e) => e.textContent ?? "").join("");
  }

  private openDate(loc: Located, anchor: HTMLElement) {
    openCalendar(anchor, { lid: loc.view.lid, current: loc.view.fullDate, pick: (iso) => this.set(loc, { date: iso }) });
  }

  private choosePicture(loc: Located) {
    const input = h("input", { type: "file", accept: "image/*", style: "position:fixed;left:-9999px" });
    overlayLayer().append(input);
    input.addEventListener("change", async () => {
      const file = input.files?.[0];
      input.remove();
      if (file) await this.setPicture(loc, file);
    });
    input.addEventListener("cancel", () => input.remove());
    input.click();
  }

  private async setPicture(loc: Located, file: File) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const ext = (file.name.split(".").pop() || file.type.split("/")[1] || "png").toLowerCase();
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.src = url;
    await img.decode().catch(() => undefined);
    let w = (img.naturalWidth || 400) * PT;
    let hgt = (img.naturalHeight || 300) * PT;
    URL.revokeObjectURL(url);
    const max = 324;
    if (w > max) {
      hgt *= max / w;
      w = max;
    }
    const media = this.ed.engine.addMedia(ext === "jpeg" ? "jpg" : ext, bytes);
    this.set(loc, { picture: { media, width: Math.round(w), height: Math.round(hgt), alt: file.name } });
  }

  // -- repeating sections ---------------------------------------------------------

  addItem() {
    const { item, section } = this.repeating();
    const target = item?.ref ?? section?.ref;
    if (target) this.ed.exec({ op: "control", action: "repeat", target });
  }

  removeItem() {
    const { item } = this.repeating();
    if (item) this.ed.exec({ op: "control", action: "repeat", target: item.ref, remove: true });
  }

  // -- commands --------------------------------------------------------------------

  /** Properties of the control the caret is in. */
  properties() {
    const loc = this.locate();
    if (!loc) return;
    openControlProperties(this.ed, loc.view, (props: ControlProps) => {
      this.ed.exec({ op: "control", action: "props", target: loc.ref, props });
    });
  }

  /** Take the control the caret is in away; its content stays. */
  remove() {
    const loc = this.locate();
    if (!loc) return;
    this.ed.exec({ op: "control", action: "remove", target: loc.ref });
    this.sticky = null;
  }

  has(): boolean {
    return !!this.locate();
  }

  /** The selection is a control's placeholder text (the selection bubble would only cover the control's tab). */
  placeholderSelected(): boolean {
    // A caret is no selection (and reading the page's selection after a keystroke would make the browser lay it out).
    if (this.ed.collapsed()) return false;
    const r = this.ed.peekRange();
    if (!r || r[0].p !== r[1].p || r[0].o === r[1].o) return false;
    return this.inlineAt(r[0].p, r[0].o, r[1].o, true).some((c) => c.placeholder && c.start === r[0].o && c.end === r[1].o);
  }

  /** The caret is in a repeating section. */
  inRepeating(): boolean {
    return !!this.repeating().section;
  }

  /** The legacy form field the caret is in (a text blank by its result), if any. */
  formAtCaret(): { p: number; form: RForm } | null {
    const r = this.ed.peekRange();
    if (!r) return null;
    const forms = this.ed.paragraphView(r[0].p)?.forms ?? [];
    const f = forms.find((x) => (x.kind === "text" ? r[0].o >= x.start && r[1].o <= x.end : r[0].o === x.at));
    return f ? { p: r[0].p, form: f } : null;
  }

  formProperties() {
    const at = this.formAtCaret();
    if (!at) return;
    openFormProperties(this.ed, at.form, (props) => {
      this.ed.exec({ op: "form_field", action: "props", at: { p: at.p, o: 0 }, n: at.form.n, props });
    });
  }

  /**
   * Insert a content control at the caret (around the selection, for text kinds). A repeating section wraps the
   * caret's paragraph. The new control's placeholder is selected, so typing fills it in.
   */
  insert(kind: string, props: ControlProps = {}, block = false): boolean {
    const r = this.ed.peekRange();
    if (!r || this.ed.readOnly) return false;
    const [a, b] = r;
    const wraps = a.p === b.p && a.o !== b.o && (kind === "text" || kind === "richText" || kind === "group");
    const defaults: Record<string, string> = {
      text: t("Click or tap here to enter text."),
      richText: t("Click or tap here to enter text."),
      date: t("Click or tap to enter a date."),
      dropDown: t("Choose an item."),
      comboBox: t("Choose an item."),
      picture: t("Click or tap to add a picture."),
    };
    const p: ControlProps = { placeholder: defaults[kind], ...props };
    if (kind === "dropDown" || kind === "comboBox") p.items ??= [];
    const res = this.ed.exec({ op: "control", action: "insert", at: a, ...(wraps ? { to: b } : {}), kind, props: p, block: block || kind === "repeating" }, { select: a });
    if (!res) return false;
    // The new control's content is selected: its placeholder is replaced by typing, and a box is ticked with Space.
    const c = this.ed.paragraphView(a.p)?.controls?.find((x) => x.start >= a.o && x.kind === kind);
    if (c) this.ed.select({ anchor: { p: a.p, o: c.start }, focus: { p: a.p, o: c.end } });
    this.ed.focus();
    this.refresh();
    return true;
  }

  insertForm(kind: "text" | "checkbox" | "dropdown") {
    const r = this.ed.peekRange();
    if (!r || this.ed.readOnly) return false;
    const props = kind === "dropdown" ? { items: [t("Option 1"), t("Option 2")] } : {};
    return !!this.ed.exec({ op: "form_field", action: "insert", at: r[0], kind, props });
  }

  // -- typing ----------------------------------------------------------------------

  /** A key the controls take: Space or Enter on a check box, the arrow or Enter on a list or a date. */
  beforeKey(e: KeyboardEvent): boolean {
    if (this.ed.readOnly || e.metaKey || e.ctrlKey || e.altKey) return false;
    const loc = this.locate();
    if (!loc) {
      return false;
    }
    const v = loc.view;
    const locked = v.lock === "contentLocked" || v.lock === "sdtContentLocked";
    if (v.kind === "checkbox" && !locked && (e.key === " " || e.key === "Enter")) {
      this.toggle(loc);
      return true;
    }
    const anchor = loc.els[0];
    if (!locked && anchor && (e.key === "ArrowDown" || e.key === "Enter") && e.altKey === false && !this.custom(v)) {
      if (v.kind === "dropDown" || (v.kind === "comboBox" && e.key === "ArrowDown" && e.shiftKey)) {
        this.openList(loc, anchor);
        return true;
      }
      if (v.kind === "date") {
        this.openDate(loc, anchor);
        return true;
      }
    }
    return false;
  }

  /** Typing or deleting the controls take over: a blank in a form, or a control that is picked from and not typed into. */
  beforeEdit(e: InputEvent, from: MPos, to: MPos): boolean {
    // The blanks of a legacy form: typing at their start, or in an empty one, goes into the blank.
    if (from.p === to.p) {
      const f = this.ed.paragraphView(from.p)?.forms?.find((x) => x.kind === "text" && from.o >= x.start && to.o <= x.end);
      if (f && !f.disabled && this.formEdit(e, from, to, f)) return true;
    }
    const loc = this.locate(false);
    if (!loc || loc.p === undefined || from.p !== to.p) return false;
    const v = loc.view;
    // A plain text control is one paragraph: Enter does nothing, or (when it allows carriage returns) breaks the line.
    if (v.kind === "text" && from.o >= v.start && to.o <= v.end && (e.inputType === "insertParagraph" || e.inputType === "insertLineBreak")) {
      if (v.multiline) this.ed.exec([...(from.o !== to.o ? [{ op: "delete", from, to }] : []), { op: "insert_text", at: from, text: "\n" }]);
      return true;
    }
    if (TYPED.has(v.kind) || !(from.o >= v.start && to.o <= v.end)) return false;
    // A drop-down, date, picture or check box is not typed into. Backspace selects it; deleting it selected takes it away.
    if (e.inputType.startsWith("delete")) {
      if (from.o === v.start && to.o === v.end && v.end > v.start) {
        this.remove();
      } else this.ed.select({ anchor: { p: from.p, o: v.start }, focus: { p: from.p, o: v.end } });
      return true;
    }
    if (e.inputType === "insertText" && e.data === " " && v.kind === "checkbox") {
      this.toggle(loc);
    }
    return true;
  }

  private formEdit(e: InputEvent, from: MPos, to: MPos, f: RForm): boolean {
    const current = f.start === f.end ? "" : this.ed.engine.textRange({ p: from.p, o: f.start }, { p: from.p, o: f.end });
    const blank = current.replace(/ /g, "").length === 0;
    const base = blank ? "" : current;
    const a = blank ? 0 : from.o - f.start;
    const b = blank ? 0 : to.o - f.start;
    let text: string;
    let caret: number;
    if (e.inputType === "insertText" || e.inputType === "insertReplacementText") {
      const typed = e.data ?? "";
      text = base.slice(0, a) + typed + base.slice(b);
      caret = a + typed.length;
    } else if (e.inputType.startsWith("delete")) {
      if (a !== b) {
        text = base.slice(0, a) + base.slice(b);
        caret = a;
      } else if (e.inputType.includes("Backward")) {
        text = base.slice(0, Math.max(0, a - 1)) + base.slice(a);
        caret = Math.max(0, a - 1);
      } else {
        text = base.slice(0, a) + base.slice(a + 1);
        caret = a;
      }
    } else return false;
    if (!this.ed.exec({ op: "form_field", action: "set", at: { p: from.p, o: 0 }, n: f.n, text }, { select: { anchor: { p: from.p, o: f.start + caret }, focus: { p: from.p, o: f.start + caret } } })) return true;
    // The engine may have cut the text to the field's length.
    const now = this.ed.paragraphView(from.p)?.forms?.find((x) => x.n === f.n);
    const end = now ? Math.min(f.start + caret, now.end) : f.start + caret;
    this.ed.select({ anchor: { p: from.p, o: end }, focus: { p: from.p, o: end } });
    return true;
  }

  // -- extensions' kinds -------------------------------------------------------------

  private custom(v: RControl): ContentControlContribution | undefined {
    const list = this.contributions();
    return list.find((c) => c.kind === v.kind || (v.tag !== undefined && c.kind === v.tag));
  }

  private info(loc: Located): ContentControlInfo {
    const v = loc.view;
    const href = loc.els.find((e) => e.dataset.href)?.dataset.href;
    return { kind: v.kind, alias: v.alias, tag: v.tag, text: this.textOf(loc), placeholder: !!v.placeholder, items: v.items, dateFormat: v.dateFormat, checked: v.checked, locked: v.lock === "contentLocked" || v.lock === "sdtContentLocked", fullDate: v.fullDate, lid: v.lid, ...(href ? { href } : {}) };
  }

  private customEdit(loc: Located, custom: ContentControlContribution, anchor: HTMLElement) {
    custom.edit?.({
      control: this.info(loc),
      anchor,
      set: (value) => this.set(loc, value),
      exec: (ops) => this.ed.exec(ops),
      popover: (content) => popover(anchor, content),
      ref: loc.ref,
      range: { from: { p: loc.p ?? 0, o: loc.view.start }, to: { p: loc.p ?? 0, o: loc.view.end } },
      remove: (deleteContent) => !!this.ed.exec({ op: "control", action: "remove", target: loc.ref, ...(deleteContent ? { delete_content: true } : {}) }),
      props: (props) => !!this.ed.exec({ op: "control", action: "props", target: loc.ref, props }),
    });
  }

  /** Let extensions decorate the controls of paragraphs drawn since the last time. */
  private applyContributions() {
    const list = this.contributions();
    // Only what the editor drew since the last time (a keystroke draws one paragraph); the whole page when it says so, and in a header, footer or note being edited.
    let drawn = this.ed.takeDrawn();
    if (!list.length) return;
    // A kind added or taken away since: look at every paragraph again.
    if (list.length !== this.seenFor) {
      this.seenFor = list.length;
      this.seen = new WeakSet();
      drawn = null;
    }
    let paras: Iterable<HTMLElement>;
    if (drawn && this.ed.root === this.ed.flow) {
      const found: HTMLElement[] = [];
      for (const b of drawn) {
        if (!b.isConnected) continue;
        if (b.dataset.p !== undefined) found.push(b);
        b.querySelectorAll<HTMLElement>("[data-p]").forEach((x) => found.push(x));
      }
      paras = found;
    } else paras = this.ed.root.querySelectorAll<HTMLElement>("[data-p]");
    for (const el of paras) {
      if (this.seen.has(el)) continue;
      this.seen.add(el);
      const p = Number(el.dataset.p);
      for (const c of this.ed.paragraphView(p)?.controls ?? []) {
        const contribution = this.custom(c);
        if (!contribution?.render) continue;
        const els = this.segsOf(p, c);
        const loc: Located = { ref: { p, n: c.n }, view: c, els, p };
        try {
          contribution.render(els, this.info(loc));
        } catch (err) {
          console.warn(`Docxtor content control ${contribution.kind}:`, err);
        }
      }
    }
  }

  /** Say why an edit was refused, in the person's words. Returns the text to show. */
  static explain(message: string): string {
    if (/locked and can't be deleted/.test(message)) return t("This content control can't be deleted.");
    if (/locked and its content can't be edited|content can't be (edited|deleted)/.test(message)) return t("This content control's contents can't be edited.");
    if (/only form fields and content controls/.test(message)) return t("This document is protected. You can only fill in the form fields and content controls.");
    if (/only the editable ranges/.test(message)) return t("This document is protected. You can only change the parts marked as editable.");
    if (/formatting is limited/.test(message)) return t("Formatting is limited in this document. Apply one of the allowed styles instead.");
    if (/isn't allowed in this document/.test(message)) return t("That style isn't allowed in this document.");
    if (/at least one item/.test(message)) return t("A repeating section keeps at least one item.");
    return message;
  }
}

