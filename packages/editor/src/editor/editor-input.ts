// Keyboard, composition, typing, deleting, autoformat, clipboard and cell movement.
import type { Editor } from "./editor";
import { caretRangeAt, domToModel, orderPositions, paraIn, paraLength, readSelection, textBefore, writeSelection } from "./selection";
import type { MPos, Op } from "@docxtor/core";
import { renderBlock } from "./render";
import { HORIZON, autoformatEnter, autoformatTyped } from "./autoformat";
import type { AutoContext, AutoPlan, AutoformatSettings } from "./autoformat";
import { stripFar } from "./far";
import { FRAGMENT_MIME, clipFromData, foldedFragment } from "./paste-convert";

/** The keys that move the caret by a line, a character or a page, for `ensureNear`. */
const ARROW_KEYS: Record<string, "up" | "down" | "left" | "right" | "pageup" | "pagedown"> = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", PageUp: "pageup", PageDown: "pagedown" };
const isMac = navigator.platform.toLowerCase().includes("mac");

export const inputMethods = {
  bind(this: Editor): void {
    const f = this.flow;
    const ro = new ResizeObserver(() => {
      this.reflowContinuous();
      this.applyAutoFit();
      // The window changed size: more or fewer pages are in view.
      this.farSoon();
    });
    ro.observe(this.canvas);
    this.life.defer(() => ro.disconnect());
    // Ctrl and the wheel, and a pinch, zoom.
    this.life.defer(this.zoomer.bind(this.canvas));
    // Input listeners sit on the stage and serve whichever editing root is active:
    // the body flow, or a header/footer/notes area while it is being edited.
    const st = this.stage;
    const mine = (e: Event) => this.root.contains(e.target as Node);
    st.addEventListener("beforeinput", (e) => mine(e) && this.onBeforeInput(e));
    st.addEventListener("compositionstart", (e) => {
      if (!mine(e)) return;
      window.clearTimeout(this.layoutTimer);
      this.extra.clear();
      this.composing = readSelection(this.root) ?? this.sel;
    });
    st.addEventListener("compositionend", (e) => mine(e) && this.onCompositionEnd(e));
    st.addEventListener("keydown", (e) => mine(e) && this.onKeyDown(e));
    st.addEventListener("copy", (e) => mine(e) && this.onCopy(e, false));
    st.addEventListener("cut", (e) => mine(e) && this.onCopy(e, true));
    st.addEventListener("paste", (e) => mine(e) && this.onPaste(e));
    st.addEventListener("dragstart", (e) => (mine(e) ? this.dnd.start(e) : e.preventDefault()));
    st.addEventListener("dragover", (e) => mine(e) && this.dnd.over(e));
    st.addEventListener("dragleave", (e) => mine(e) && this.dnd.leave(e));
    st.addEventListener("dragend", (e) => this.dnd.end(e));
    this.painter.bind();
    // A click puts the selection where it lands, so the other selected ranges go. A right-click on one of them keeps all of them (the menu is for the whole selection).
    this.canvas.addEventListener(
      "mousedown",
      (e) => {
        if (!this.multi) return;
        if (e.button === 2 && this.extra.hit(e.clientX, e.clientY)) e.preventDefault();
        else if (e.button === 0 && !e.ctrlKey) this.extra.clear();
      },
      true,
    );
    // Cmd/Ctrl-click on the mark at the start of a note goes back to its reference in the text.
    // The note's own mark is a few pixels wide (a small roman numeral), so a click a pixel or two off still counts.
    st.addEventListener("click", (e) => {
      if (!this.story || !(e.metaKey || e.ctrlKey)) return;
      let mark = (e.target as HTMLElement).closest<HTMLElement>(".atom.note");
      if (!mark) {
        for (const m of Array.from(this.storyEl?.querySelectorAll<HTMLElement>(".atom.note") ?? [])) {
          const r = m.getBoundingClientRect();
          if (e.clientX >= r.left - 3 && e.clientX <= r.right + 3 && e.clientY >= r.top - 2 && e.clientY <= r.bottom + 2) mark = m;
        }
      }
      if (mark && this.storyEl?.contains(mark) && this.followNoteMark(mark)) e.preventDefault();
    });
    // Double-click a page's top or bottom margin to edit its header/footer, or a note mark to edit the note.
    this.canvas.addEventListener("dblclick", (e) => this.onDoubleClick(e));
    this.canvas.addEventListener(
      "mousedown",
      (e) => {
        const t = e.target as HTMLElement;
        if (!this.story || this.storyEl?.contains(t)) return;
        // A click in the text puts the caret where it lands; one on the desk round the pages puts it back where it was in the text.
        if (f.contains(t)) this.exitStory(false);
        else if (t === this.canvas || t === this.stage || t === this.zoomEl || t === this.fitEl || t === this.viewEl) this.exitStory(true);
      },
      true,
    );
    // Clicking empty paper below the text puts the caret at the end of the document.
    this.canvas.addEventListener("mousedown", (e) => {
      if (this.story || f.contains(e.target as Node)) return;
      const ps = f.querySelectorAll<HTMLElement>("[data-p]");
      const last = ps[ps.length - 1];
      if (!last) return;
      const rect = f.getBoundingClientRect();
      if (e.clientY < rect.bottom) return;
      e.preventDefault();
      const id = Number(last.dataset.p);
      const end = { p: id, o: paraLength(f, id) };
      this.sel = { anchor: end, focus: end };
      this.focus();
      this.emitSelection();
    });
    f.addEventListener("drop", (e) => {
      const file = e.dataTransfer?.files?.[0];
      if (file && file.type.startsWith("image/")) {
        e.preventDefault();
        e.stopPropagation();
        const pos = caretRangeAt(e.clientX, e.clientY);
        if (pos) {
          const m = domToModel(f, pos.startContainer, pos.startOffset);
          if (m) this.sel = { anchor: m, focus: m };
        }
        void this.insertImage(file);
      } else if (!file) this.dnd.drop(e);
    });
    // An equation is one unit: a click selects all of it (a double-click then opens it for editing).
    f.addEventListener("mousedown", (e) => {
      const eq = (e.target as HTMLElement).closest?.<HTMLElement>(".atom.math, .atom.ruby");
      if (!eq || this.readOnly || e.button !== 0 || e.shiftKey || e.metaKey || e.ctrlKey || e.altKey) return;
      const p = Number(eq.closest<HTMLElement>("[data-p]")?.dataset.p);
      const o = Number(eq.dataset.o);
      if (!Number.isFinite(p) || !Number.isFinite(o)) return;
      e.preventDefault();
      this.select({ anchor: { p, o }, focus: { p, o: o + 1 } });
      this.focus();
      this.emitSelection();
    });
    // The box of a checklist item ticks with a click; the caret stays where it was.
    const boxOf = (e: Event) => (e.target as HTMLElement).closest?.<HTMLElement>('.lbl[role="checkbox"]') ?? null;
    f.addEventListener("mousedown", (e) => {
      if (boxOf(e)) e.preventDefault();
    });
    f.addEventListener("click", (e) => {
      const box = boxOf(e);
      const id = Number(box?.closest<HTMLElement>("[data-p]")?.dataset.p);
      if (box && !this.readOnly && Number.isFinite(id)) {
        e.preventDefault();
        const caret = { p: id, o: paraLength(f, id) };
        this.exec({ op: "set_checked", ids: [id], checked: box.getAttribute("aria-checked") !== "true" }, { select: this.sel ?? { anchor: caret, focus: caret } });
        this.focus();
        return;
      }
      const mark = (e.target as HTMLElement).closest<HTMLElement>(".atom.note");
      if (mark && (e.metaKey || e.ctrlKey) && this.followNoteMark(mark)) return;
      // A citation is one unit: a click selects all of it and opens its popover.
      const cite = (e.target as HTMLElement).closest<HTMLElement>(".cite");
      if (cite && !this.readOnly && !e.shiftKey && !e.altKey && !e.metaKey && !e.ctrlKey && window.getSelection()?.isCollapsed !== false) {
        this.openCitation(cite);
        return;
      }
      const link = (e.target as HTMLElement).closest<HTMLElement>(".lnk");
      if (link && (e.metaKey || e.ctrlKey)) {
        this.openAddress(link.dataset.href!);
      }
    });
    this.life.on(document, "selectionchange", () => {
      if (this.composing) return;
      const s = readSelection(this.root);
      if (!s) return;
      const moved = !this.sel || s.focus.p !== this.sel.focus.p || s.focus.o !== this.sel.focus.o || s.anchor.o !== this.sel.anchor.o || s.anchor.p !== this.sel.anchor.p;
      this.sel = s;
      if (moved) {
        // The person moved the selection (a click, an arrow key, shift-select): the other ranges go.
        this.extra.clear();
        this.pending = null;
        this.lastAuto = null;
        this.emitSelection();
        this.farSoon();
      }
    });
  },
  onCompositionEnd(this: Editor, e: CompositionEvent): void {
    const sel = this.composing;
    this.composing = null;
    if (!sel) return;
    const [a, b] = orderPositions(this.root, sel.anchor, sel.focus);
    const ops: Op[] = [];
    if (a.p !== b.p || a.o !== b.o) ops.push({ op: "delete", from: a, to: b });
    if (e.data) ops.push({ op: "insert_text", at: a, text: e.data, ...(this.pending ? { format: this.pending } : {}) });
    // The browser mutated the DOM during composition; force the paragraph to be rebuilt.
    const blockOf = (id: number) => this.root.querySelector(`[data-p="${id}"]`)?.closest(".flow > *, .flow > .cols > *") as HTMLElement | null;
    const touched = blockOf(a.p);
    if (ops.length) this.exec(ops);
    else if (touched) this.refresh();
    if (touched && touched.isConnected) {
      // Same block element survived (no engine change): re-render it from the model.
      const id = Number(touched.dataset.id);
      const b = this.json.get(id);
      if (b) {
        const el = renderBlock(b, this.env);
        touched.replaceWith(el);
        this.blocks.set(id, el);
        this.layoutNow();
      }
    }
  },
  targetRange(this: Editor, e: InputEvent): [MPos, MPos] | null {
    const rs = e.getTargetRanges?.();
    if (!rs || !rs.length) return null;
    const r = rs[0];
    const a = domToModel(this.root, r.startContainer, r.startOffset);
    const b = domToModel(this.root, r.endContainer, r.endOffset);
    if (!a || !b) return null;
    return orderPositions(this.root, a, b);
  },
  onBeforeInput(this: Editor, e: InputEvent): void {
    if (e.inputType === "insertCompositionText" || this.composing) return;
    e.preventDefault();
    if (this.readOnly) return;
    const s = readSelection(this.root) ?? this.sel;
    if (!s) return;
    this.sel = s;
    if (this.multi && this.multiInput(e)) return;
    const [from, to] = orderPositions(this.root, s.anchor, s.focus);
    if (this.hooks.beforeEdit?.(e, from, to)) return;
    const collapsed = from.p === to.p && from.o === to.o;
    const del: Op[] = collapsed ? [] : [{ op: "delete", from, to }];
    const fmt = this.pending ? { format: this.pending } : {};
    switch (e.inputType) {
      case "insertText":
      case "insertReplacementText": {
        let text = e.data ?? e.dataTransfer?.getData("text/plain") ?? "";
        let range: [MPos, MPos] = [from, to];
        if (e.inputType === "insertReplacementText") range = this.targetRange(e) ?? range;
        if (!text) return;
        text = text.replace(/\r\n?/g, "\n");
        const rcol = range[0].p === range[1].p && range[0].o === range[1].o;
        const now = performance.now();
        // 0 means "not typing": the clock itself starts near 0 after a page load.
        const merge = rcol && collapsed && this.lastTyping > 0 && now - this.lastTyping < 1500 && !/^\s/.test(text) && this.atTypedEnd();
        this.lastTyping = now;
        this.lastDelete = 0;
        const ops: Op[] = rcol ? [] : [{ op: "delete", from: range[0], to: range[1] }];
        ops.push({ op: "insert_text", at: range[0], text, ...fmt });
        // Typed one character at a time over nothing: what the typing substitutions look at.
        const typed = e.inputType === "insertText" && rcol && collapsed && this.autoformat && text.length <= 2 ? range[0] : null;
        const done = this.exec(ops, { merge, typing: true });
        if (done && typed) this.afterTyped(typed, text);
        return;
      }
      case "insertParagraph": {
        this.lastTyping = 0;
        this.lastDelete = 0;
        this.recent.s = "";
        if (collapsed && this.autoformat) {
          const plan = autoformatEnter(this.autoContext(from), this.autoformat);
          if (plan?.consumesEnter) {
            this.runAuto(plan, from);
            return;
          }
          // A link or a capital letter before the break is a step of its own, ahead of it.
          if (plan) this.runAuto(plan, from);
        }
        this.exec([...del, { op: "split_paragraph", at: from }]);
        return;
      }
      case "insertLineBreak": {
        this.exec([...del, { op: "insert_text", at: from, text: "\n", ...fmt }]);
        return;
      }
      case "deleteContentBackward":
      case "deleteContentForward":
      case "deleteWordBackward":
      case "deleteWordForward":
      case "deleteSoftLineBackward":
      case "deleteSoftLineForward":
      case "deleteHardLineBackward":
      case "deleteHardLineForward":
      case "deleteByCut":
      case "deleteContent": {
        this.lastTyping = 0;
        if (!collapsed) {
          this.exec({ op: "delete", from, to }, { select: from });
          return;
        }
        // Backspace right after a substitution takes the substitution back.
        const auto = this.lastAuto;
        if (e.inputType === "deleteContentBackward" && auto && auto.caret.p === from.p && auto.caret.o === from.o) {
          this.undo();
          return;
        }
        const backward = e.inputType.includes("Backward");
        // A citation is one unit: Backspace after it, or Delete before it, takes all of it.
        const unit = this.citationBeside(from, backward);
        if (unit) {
          this.exec({ op: "delete", from: { p: unit.p, o: unit.o }, to: { p: unit.p, o: unit.o + unit.len } }, { select: { p: unit.p, o: unit.o } });
          return;
        }
        const tr = this.targetRange(e);
        const single = e.inputType === "deleteContentBackward" || e.inputType === "deleteContentForward";
        if (tr && (tr[0].p !== tr[1].p || tr[0].o !== tr[1].o) && tr[0].p === from.p && tr[1].p === from.p) {
          this.exec({ op: "delete", from: tr[0], to: tr[1] }, { select: tr[0], merge: single && this.deleteMerge(), typing: true });
          return;
        }
        if (backward) this.deleteBackward(from);
        else this.deleteForward(from);
        return;
      }
      case "historyUndo":
        this.undo();
        return;
      case "historyRedo":
        this.redo();
        return;
      case "formatBold":
        this.toggleMark("bold");
        return;
      case "formatItalic":
        this.toggleMark("italic");
        return;
      case "formatUnderline":
        this.toggleMark("underline");
        return;
      case "formatStrikeThrough":
        this.toggleMark("strike");
        return;
      default:
        // insertFromPaste / insertFromDrop are handled by their own events.
        return;
    }
  },
  /** Turn the typing substitutions on (with these settings) or off (null). */
  setAutoformat(this: Editor, settings: AutoformatSettings | null): void {
    this.autoformat = settings;
    this.lastAuto = null;
  },
  /** What the typing substitutions may look at before `at`: the text, the language, the formatting. */
  autoContext(this: Editor, at: MPos): AutoContext {
    const p = at.p;
    return {
      p,
      end: at.o,
      text: textBefore(this.root, at, HORIZON),
      lang: this.setup?.lang,
      state: () => this.engine.formatAt(at),
      formatAt: (i) => this.engine.formatAt({ p, o: i + 1 }),
      listOps: (id, style) => this.listStyleOps([id], style),
    };
  },
  /** A character was typed at `at`: apply the substitution it completes, if any, as a step of its own. */
  afterTyped(this: Editor, at: MPos, text: string): void {
    const settings = this.autoformat;
    if (!settings || this.composing) return;
    const caret = { p: at.p, o: at.o + text.length };
    const r = this.recent;
    r.s = (r.p === at.p && r.o === at.o ? r.s + text : text).slice(-3);
    r.p = caret.p;
    r.o = caret.o;
    // Letters and digits complete nothing, except the one after "--" (word--word).
    if (/^[\p{L}\p{N}]/u.test(text) && !/--[\p{L}\p{N}]$/u.test(r.s)) return;
    const plan = autoformatTyped(this.autoContext(caret), settings);
    if (plan) this.runAuto(plan, caret);
  },
  /** Apply a substitution as its own undo step; `restore` is the caret to return to when it is taken back. */
  runAuto(this: Editor, plan: AutoPlan, restore: MPos): void {
    if (!this.exec(plan.ops, { select: plan.caret ?? undefined, typing: true })) return;
    this.lastTyping = 0;
    this.lastDelete = 0;
    this.recent.s = "";
    if (plan.pending) {
      this.pending = { ...(this.pending ?? {}), ...plan.pending };
      this.hooks.onSelection?.(this.formatWithPending());
    }
    const caret = this.sel?.focus;
    if (caret) this.lastAuto = { caret: { ...caret }, restore };
  },
  allParas(this: Editor): HTMLElement[] {
    return Array.from(this.root.querySelectorAll<HTMLElement>("[data-p]"));
  },
  /** Consecutive single-character deletions form one undo step, like typing. */
  deleteMerge(this: Editor): boolean {
    const now = performance.now();
    const merge = now - this.lastDelete < 1500 && this.atTypedEnd();
    this.lastDelete = now;
    return merge;
  },
  /**
   * Is the caret where the last key that typed or deleted left it? Typing goes on in one undo step only there: after a click or an arrow key elsewhere,
   * what is typed is a step of its own, as in Word, however soon it follows (an undo then takes back one place, not two far apart).
   */
  atTypedEnd(this: Editor): boolean {
    const e = this.typedEnd;
    const s = this.sel;
    return !!e && !!s && s.anchor.p === e.p && s.anchor.o === e.o && s.focus.p === e.p && s.focus.o === e.o;
  },
  deleteBackward(this: Editor, at: MPos): void {
    if (at.o > 0) {
      const prev = textBefore(this.root, at, 2);
      const n = prev.length === 2 && /[\uDC00-\uDFFF]/.test(prev[1]) && /[\uD800-\uDBFF]/.test(prev[0]) ? 2 : 1;
      this.exec({ op: "delete", from: { p: at.p, o: at.o - n }, to: at }, { select: { p: at.p, o: at.o - n }, merge: this.deleteMerge(), typing: true });
      return;
    }
    const f = this.engine.formatAt(at);
    if (f?.list) {
      // Backspace at the start of a list item removes the bullet first.
      this.exec({ op: "set_list_style", ids: [at.p], style: "none" }, { select: at });
      return;
    }
    const paras = this.allParas();
    const idx = paras.findIndex((p) => Number(p.dataset.p) === at.p);
    if (idx <= 0) return;
    const cur = paras[idx];
    const prev = paras[idx - 1];
    // The first paragraph of a text box has nothing before it to join: Backspace stays in the box.
    if (cur.parentElement?.classList.contains("shape-text") && prev.parentElement !== cur.parentElement) return;
    const prevId = Number(prev.dataset.p);
    const prevLen = paraLength(this.root, prevId);
    // Backspace right after a horizontal line takes the line away (joining text into it would put a rule under it).
    if (prev.dataset.hr && prevLen === 0 && prev.parentElement === cur.parentElement) {
      this.exec({ op: "delete_blocks", ids: [prevId] }, { select: at });
      return;
    }
    if (prev.parentElement === cur.parentElement) {
      this.exec({ op: "delete", from: { p: prevId, o: prevLen }, to: at }, { select: { p: prevId, o: prevLen } });
    } else if (paraLength(this.root, at.p) === 0 && cur.parentElement === this.root) {
      // Empty paragraph right after a table: remove it and jump into the table.
      this.exec({ op: "delete_blocks", ids: [at.p] }, { select: { p: prevId, o: prevLen } });
    } else {
      this.sel = { anchor: { p: prevId, o: prevLen }, focus: { p: prevId, o: prevLen } };
      writeSelection(this.root, this.sel);
    }
  },
  deleteForward(this: Editor, at: MPos): void {
    const len = paraLength(this.root, at.p);
    if (at.o < len) {
      const p = this.root.querySelector(`[data-p="${at.p}"]`) as HTMLElement;
      const after = textBefore(this.root, { p: at.p, o: Math.min(len, at.o + 2) }, 2);
      const n = after.length === 2 && /[\uD800-\uDBFF]/.test(after[0]) && p ? 2 : 1;
      this.exec({ op: "delete", from: at, to: { p: at.p, o: at.o + n } }, { select: at, merge: this.deleteMerge(), typing: true });
      return;
    }
    const paras = this.allParas();
    const idx = paras.findIndex((p) => Number(p.dataset.p) === at.p);
    const next = paras[idx + 1];
    if (!next || next.parentElement !== paras[idx].parentElement) return;
    // Delete on a horizontal line removes the line, not the text after it into the line.
    if (paras[idx].dataset.hr && len === 0) {
      const to = { p: Number(next.dataset.p), o: 0 };
      this.exec({ op: "delete_blocks", ids: [at.p] }, { select: to });
      return;
    }
    this.exec({ op: "delete", from: at, to: { p: Number(next.dataset.p), o: 0 } }, { select: at });
  },
  onKeyDown(this: Editor, e: KeyboardEvent): void {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const k = e.key.toLowerCase();
    // Moving the caret ends the moment in which Backspace takes a substitution back.
    if (/^(Arrow|Home$|End$|Page)/.test(e.key)) this.lastAuto = null;
    if (this.composing || e.isComposing) return;
    const dir = ARROW_KEYS[e.key];
    if (dir) this.ensureNear(dir);
    if (this.keyHook?.(e)) return;
    if (this.hooks.beforeKey?.(e)) {
      e.preventDefault();
      return;
    }
    const run = (fn: () => void) => {
      e.preventDefault();
      fn();
    };
    if (e.key === "Escape" && this.multi) return run(() => this.extra.clear());
    if (e.key === "Escape" && this.story) return run(() => this.exitStory());
    // Enter on a selected equation opens it for editing.
    if (e.key === "Enter" && !mod && !e.shiftKey && !e.altKey && !this.readOnly) {
      const eq = this.equationAtCaret();
      const el = eq && this.equationElement(eq.at);
      if (eq && el) return run(() => this.hooks.onEquation?.(eq, el));
      const guide = this.rubyAtCaret();
      const gel = guide && this.rubyElement(guide.at);
      if (guide && gel) return run(() => this.hooks.onRuby?.(guide, gel));
    }
    if (mod && e.shiftKey && k === "v") this.plainPaste = true;
    // Shift+Arrow onto a picture, shape or other object that is one unit: Firefox leaves the selection where it was (it does not extend a
    // selection over a box that can't be edited), so when nothing has moved the editor extends it by the one unit.
    if (e.shiftKey && !mod && !e.altKey && (e.key === "ArrowRight" || e.key === "ArrowLeft") && this.sel && !this.composing) {
      const start = this.sel;
      const step = e.key === "ArrowRight" ? 1 : -1;
      window.setTimeout(() => {
        const now = readSelection(this.root);
        if (!now || now.anchor.p !== start.anchor.p || now.anchor.o !== start.anchor.o || now.focus.p !== start.focus.p || now.focus.o !== start.focus.o) return;
        const para = paraIn(this.root, now.focus.p);
        const unit = step > 0 ? now.focus.o : now.focus.o - 1;
        if (!para || unit < 0 || !para.querySelector(`.atom[data-o="${unit}"]`)) return;
        this.select({ anchor: now.anchor, focus: { p: now.focus.p, o: now.focus.o + step } });
        this.emitSelection();
      }, 0);
    }
    // Backspace in the first paragraph of the text, or Delete in the last, when it is empty: Safari gives no `beforeinput` for it and
    // takes the paragraph out of the page itself, which leaves the page without the paragraph the model still has (the text typed
    // next goes nowhere). The editor does what it does for the same key elsewhere, and the browser does nothing.
    if ((e.key === "Backspace" || e.key === "Delete") && !mod && !e.altKey && !e.shiftKey && !this.readOnly && this.sel && this.collapsed() && this.edgeEmpty(e.key === "Backspace")) {
      e.preventDefault();
      this.onBeforeInput(new InputEvent("beforeinput", { inputType: e.key === "Backspace" ? "deleteContentBackward" : "deleteContentForward", cancelable: true }));
      return;
    }
    // Backspace or Delete on a selection of nothing but objects (a picture, a shape, a text box): Safari gives no `beforeinput` for one that
    // floats (it has no place in the line to take text from), so the shape stayed. The editor deletes it, as the other browsers' event does.
    if ((e.key === "Backspace" || e.key === "Delete") && !mod && !e.altKey && !e.shiftKey && !this.readOnly && this.onlyObjectsSelected()) {
      e.preventDefault();
      this.onBeforeInput(new InputEvent("beforeinput", { inputType: e.key === "Backspace" ? "deleteContentBackward" : "deleteContentForward", cancelable: true }));
      return;
    }
    // Formatting and insert shortcuts (⌘B, ⌘⌥2…) are commands, resolved by DocxtorEditor's
    // extension host when this event reaches it; only keys tied to editing state are handled here.
    if (e.key === "Tab" && !mod) {
      const f = this.format();
      if (f?.inTable) {
        return run(() => this.moveCell(e.shiftKey ? -1 : 1));
      }
      if (f?.list) return run(() => this.indent(e.shiftKey ? -1 : 1));
      if (!e.shiftKey) {
        return run(() => {
          const r = this.range();
          if (r) this.exec([...(this.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }]), { op: "insert_text", at: r[0], text: "\t" }]);
        });
      }
    }
  },
  /** The selection is a range in one paragraph in which every unit is an object (an `.atom`: picture, shape, equation, field...). */
  onlyObjectsSelected(this: Editor): boolean {
    const s = this.sel;
    if (!s || s.anchor.p !== s.focus.p || s.anchor.o === s.focus.o) return false;
    const para = paraIn(this.root, s.focus.p);
    if (!para) return false;
    for (let o = Math.min(s.anchor.o, s.focus.o); o < Math.max(s.anchor.o, s.focus.o); o++) if (!Array.from(para.querySelectorAll<HTMLElement>(`.atom[data-o="${o}"]`)).some((a) => a.closest("[data-p]") === para)) return false;
    return true;
  },
  /** Is the caret in an empty paragraph that is the first (`first`) or the last of the text being edited? */
  edgeEmpty(this: Editor, first: boolean): boolean {
    const ps = this.root.querySelectorAll<HTMLElement>("[data-p]");
    const edge = ps[first ? 0 : ps.length - 1];
    return !!edge && !!this.sel && this.sel.focus.p === Number(edge.dataset.p) && paraLength(this.root, this.sel.focus.p) === 0;
  },
  moveCell(this: Editor, dir: number): void {
    const r = this.range();
    if (!r) return;
    const p = this.root.querySelector(`[data-p="${r[0].p}"]`);
    const td = p?.closest("td");
    const table = td?.closest("table");
    if (!td || !table) return;
    const cells = Array.from(table.querySelectorAll("td")).filter((c) => c.closest("table") === table && !c.closest("tr.pgsp, tr.hdr-repeat"));
    const i = cells.indexOf(td as HTMLTableCellElement);
    const target = cells[i + dir];
    if (!target) {
      if (dir > 0) this.tableAction("row_below");
      return;
    }
    const tp = target.querySelector<HTMLElement>("[data-p]");
    if (!tp) return;
    const id = Number(tp.dataset.p);
    this.sel = { anchor: { p: id, o: 0 }, focus: { p: id, o: paraLength(this.root, id) } };
    writeSelection(this.root, this.sel);
  },
  /**
   * What copying (or dragging) the selection puts on the clipboard: the text, HTML for other programs, and Docxtor's own
   * format (JSON), which keeps everything when it is pasted into a Docxtor document.
   */
  copyPayload(this: Editor, r: [MPos, MPos]): { text: string; html: string; fragment: string } {
    const text = this.engine.textRange(r[0], r[1]);
    const s = window.getSelection();
    let html = "";
    if (s && s.rangeCount) {
      const div = document.createElement("div");
      div.append(s.getRangeAt(0).cloneContents());
      div.querySelectorAll(".pgsp,.lbl,br[data-ph]").forEach((x) => x.remove());
      // A block left out of the layout (a far page) is not left out of the text that is copied.
      stripFar(div);
      // Pictures are shown from addresses that only this page knows; other programs get the picture itself.
      div.querySelectorAll<HTMLImageElement>('img[src^="blob:"]').forEach((img) => {
        const uri = this.engine.mediaDataUri(img.getAttribute("src") ?? "");
        if (uri) img.setAttribute("src", uri);
      });
      html = div.innerHTML;
    }
    let fragment = "";
    try {
      const f = this.engine.copyFragment(r[0], r[1]);
      if (f) fragment = JSON.stringify(f);
    } catch {
      /* the plain formats still work */
    }
    // Clipboards that carry only the standard formats still bring the rich content: it rides in the HTML.
    if (fragment && html && fragment.length < 1_500_000) html = `<div data-docxtor-fragment="${foldedFragment(fragment)}">${html}</div>`;
    return { text, html, fragment };
  },
  onCopy(this: Editor, e: ClipboardEvent, cut: boolean): void {
    const r = this.range();
    if (!r || this.collapsed()) return;
    e.preventDefault();
    // Several ranges: their text, one per line, in document order.
    if (this.multi) {
      e.clipboardData?.setData("text/plain", this.rangesText());
      if (cut) this.deleteRanges(this.ranges());
      return;
    }
    const p = this.copyPayload(r);
    e.clipboardData?.setData("text/plain", p.text);
    if (p.html) e.clipboardData?.setData("text/html", p.html);
    if (p.fragment) e.clipboardData?.setData(FRAGMENT_MIME, p.fragment);
    if (cut) this.exec({ op: "delete", from: r[0], to: r[1] }, { select: r[0] });
  },
  onPaste(this: Editor, e: ClipboardEvent): void {
    e.preventDefault();
    const dt = e.clipboardData;
    if (!dt) return;
    const file = Array.from(dt.files ?? []).find((f) => f.type.startsWith("image/"));
    if (file) {
      void this.insertImage(file);
      return;
    }
    const plain = this.plainPaste;
    this.plainPaste = false;
    // Several ranges: the plain text goes in place of each of them.
    if (this.multi) {
      const text = dt.getData("text/plain");
      if (text) this.replaceRanges(this.ranges(), text.replace(/\r\n?/g, "\n"));
      return;
    }
    void this.pasteFlow.paste(clipFromData(dt), { plain });
  },
};

declare module "./editor" {
  interface Editor {
    bind(): void;
    onCompositionEnd(e: CompositionEvent): void;
    targetRange(e: InputEvent): [MPos, MPos] | null;
    onBeforeInput(e: InputEvent): void;
    setAutoformat(settings: AutoformatSettings | null): void;
    autoContext(at: MPos): AutoContext;
    afterTyped(at: MPos, text: string): void;
    runAuto(plan: AutoPlan, restore: MPos): void;
    allParas(): HTMLElement[];
    deleteMerge(): boolean;
    atTypedEnd(): boolean;
    deleteBackward(at: MPos): void;
    deleteForward(at: MPos): void;
    onKeyDown(e: KeyboardEvent): void;
    onlyObjectsSelected(): boolean;
    edgeEmpty(first: boolean): boolean;
    moveCell(dir: number): void;
    copyPayload(r: [MPos, MPos]): { text: string; html: string; fragment: string; };
    onCopy(e: ClipboardEvent, cut: boolean): void;
    onPaste(e: ClipboardEvent): void;
  }
}
