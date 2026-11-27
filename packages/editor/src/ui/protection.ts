// Document protection as a person meets it: a strip above the pages that says what the document restricts, the parts
// anyone may edit tinted with brackets, and a document marked as final opened read-only with an Edit anyway button.
// The engine enforces the rules (an edit outside what is allowed is refused), so this only shows them and offers
// the way out.
import { t } from "@docxtor/extensions";
import type { MPos, ProtectionInfo } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { modelToDom } from "../editor/selection";
import { h } from "./dom";
import { openRestrictEditing } from "./restrict-editing";
import { paintRange } from "../editor/far";

export class ProtectionTool {
  readonly el: HTMLElement;
  info: ProtectionInfo | null = null;
  private trackedNoticeSeen = false;
  private timer = 0;
  private highlight = true;
  /** Lets go of the reason that keeps a document marked as final read-only (null while none is held). */
  private releaseFinal: (() => void) | null = null;
  private finalHeld = false;
  private marked: HTMLElement[] = [];
  private listeners = new Set<(info: ProtectionInfo | null) => void>();

  constructor(private ed: Editor, private lock: (reason: string) => () => void) {
    this.el = h("div", { class: "notice-strip", role: "status", hidden: true });
  }

  /** A new document is open: forget what was said about the last. */
  reset() {
    this.releaseFinal?.();
    this.releaseFinal = null;
    this.trackedNoticeSeen = false;
    this.finalHeld = false;
    this.info = null;
    this.refresh();
  }

  onChange(fn: (info: ProtectionInfo | null) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** Look again after an edit (once a burst of them pauses). */
  schedule() {
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.refresh(), 120);
  }

  /** The editor is going away: a look still scheduled would read a closed document. */
  destroy() {
    window.clearTimeout(this.timer);
  }

  refresh() {
    window.clearTimeout(this.timer);
    const eng = this.ed.engine;
    const before = JSON.stringify(this.info && { ...this.info, ranges: this.info.ranges.length });
    this.info = eng ? eng.protectionInfo() : null;
    const info = this.info;
    // A document marked as final opens read-only.
    if (info?.markedFinal && !this.finalHeld) {
      this.finalHeld = true;
      this.releaseFinal = this.lock("marked as final");
    } else if (!info?.markedFinal && this.finalHeld) {
      this.finalHeld = false;
      this.releaseFinal?.();
      this.releaseFinal = null;
    }
    this.strip();
    this.paint();
    if (before !== JSON.stringify(info && { ...info, ranges: info.ranges.length })) for (const fn of this.listeners) fn(info);
  }

  /** What protection says in words, for the status bar and the strip; null when the document is not protected. */
  get status(): string | null {
    const i = this.info;
    if (!i) return null;
    if (i.markedFinal) return t("Marked as final");
    if (i.enforced && i.edit === "forms") return t("Protected: forms only");
    if (i.enforced && (i.edit === "readOnly" || i.edit === "comments")) return t("Protected: read only");
    if (i.enforced && i.edit === "trackedChanges") return t("Protected: tracked changes");
    if (i.enforced && i.formatting) return t("Protected: limited formatting");
    return null;
  }

  private strip() {
    const i = this.info;
    this.el.textContent = "";
    this.el.hidden = true;
    if (!i) return;
    const actions = h("div", { class: "notice-actions" });
    let text = "";
    if (i.markedFinal) {
      text = t("This document is marked as final. Editing is turned off.");
      const edit = h("button", { class: "btn", type: "button" }, t("Edit anyway"));
      edit.addEventListener("click", () => this.editAnyway());
      actions.append(edit);
    } else if (i.enforced && (i.edit === "readOnly" || i.edit === "comments")) {
      text = i.ranges.length ? t("This document is protected. You can only edit the highlighted parts.") : t("This document is protected. It can't be edited.");
      actions.append(this.stopButton());
    } else if (i.enforced && i.edit === "forms") {
      text = t("This document is protected. You can only fill in the form fields.");
      actions.append(this.stopButton());
    } else if (i.enforced && i.edit === "trackedChanges") {
      if (this.trackedNoticeSeen) return;
      text = t("This document asks for tracked changes. Docxtor can't track changes yet, so what you change here won't be tracked.");
      const ok = h("button", { class: "btn", type: "button" }, t("Got it"));
      ok.addEventListener("click", () => {
        this.trackedNoticeSeen = true;
        this.strip();
      });
      actions.append(ok, this.stopButton());
    } else if (i.enforced && i.formatting) {
      text = t("Formatting is limited to some styles in this document.");
      actions.append(this.stopButton());
    } else return;
    this.el.append(h("p", {}, text), actions);
    this.el.hidden = false;
  }

  private stopButton(): HTMLButtonElement {
    const b = h("button", { class: "btn ghost", type: "button" }, t("Stop protection…"));
    b.addEventListener("click", () => this.open());
    return b;
  }

  /** Marked as final: turn editing back on, and take the mark off (as Word's Edit Anyway does). */
  editAnyway() {
    if (!this.info?.markedFinal) return;
    this.releaseFinal?.();
    this.releaseFinal = null;
    this.finalHeld = false;
    this.ed.exec({ op: "protection", action: "mark_final", on: false });
    this.refresh();
    this.ed.focus();
  }

  markFinal(on = true): boolean {
    if (this.ed.readOnly && !this.finalHeld) return false;
    // Marking again goes through the engine, which refuses while the document is read-only: let go of its own reason first.
    const wasFinal = this.finalHeld;
    if (wasFinal) {
      this.releaseFinal?.();
      this.releaseFinal = null;
    }
    const done = this.ed.exec({ op: "protection", action: "mark_final", on });
    if (wasFinal && !done) this.releaseFinal = this.lock("marked as final");
    this.refresh();
    return !!done;
  }

  /** The Restrict editing dialog. */
  open() {
    openRestrictEditing(this.ed, this);
  }

  // -- the editable ranges ---------------------------------------------------------

  setHighlight(on: boolean) {
    this.highlight = on;
    this.paint();
  }

  get highlighting() {
    return this.highlight;
  }

  /** Tint the parts anyone may edit and put a bracket at each end, the way Word marks them. */
  private paint() {
    for (const m of this.marked) m.classList.remove("perm-start", "perm-end");
    this.marked = [];
    const reg = (globalThis as { CSS?: { highlights?: Map<string, unknown> } }).CSS?.highlights;
    reg?.delete("docxtor-perm");
    const i = this.info;
    if (!i || !i.enforced || !i.ranges.length || !this.highlight) return;
    const HL = (globalThis as { Highlight?: new (...r: AbstractRange[]) => unknown }).Highlight;
    const hl = HL ? new HL() : null;
    const edge = (pos: MPos, cls: string, end: boolean) => {
      const at = modelToDom(this.ed.root, pos);
      const seg = at?.node instanceof Element ? at.node.closest<HTMLElement>("[data-o]") : at?.node.parentElement?.closest<HTMLElement>("[data-o]");
      if (seg) {
        seg.classList.add(cls);
        this.marked.push(seg);
      }
      return end;
    };
    for (const r of i.ranges) {
      const a = modelToDom(this.ed.root, r.from);
      const b = modelToDom(this.ed.root, r.to);
      if (!a || !b) continue;
      try {
        const range = document.createRange();
        range.setStart(a.node, a.offset);
        range.setEnd(b.node, b.offset);
        // A fixed range: it keeps its text when the block moves into or out of a far box (see `paintRange`).
        (hl as { add(r: AbstractRange): void } | null)?.add(paintRange(range));
      } catch {
        // A range whose ends moved out of the page being drawn is left out.
      }
      edge(r.from, "perm-start", false);
      // The end bracket belongs to the last character, so look one back.
      edge({ p: r.to.p, o: Math.max(0, r.to.o - 1) }, "perm-end", true);
    }
    if (hl && reg) reg.set("docxtor-perm", hl);
  }
}
