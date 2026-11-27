// The strip over the page while a merged record is shown in place of the template: which record, the way to the first,
// previous, next and last, Find recipient, and the way back to the template.
import type { MergePreview } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { h, overlayLayer, svg } from "../ui/dom";
import { MAIL_ICONS } from "./icons";
import type { MergeSession } from "./session";

export class PreviewStrip {
  readonly el: HTMLElement;
  private shown = false;

  constructor(private s: MergeSession) {
    this.el = h("div", { class: "mm-strip", style: "display:none", role: "toolbar", "aria-label": t("Preview results") });
    const ed = s.x.editor;
    overlayLayer().append(ed.life.own(this.el));
    ed.canvas.addEventListener("scroll", () => this.place(), { passive: true });
    ed.life.on(window, "resize", () => this.place());
  }

  show(p: MergePreview) {
    const s = this.s;
    const key = (icon: string, label: string, to: "first" | "previous" | "next" | "last", disabled: boolean) => {
      const b = h("button", { class: "tb", type: "button", title: label, "aria-label": label }, svg(icon));
      b.disabled = disabled;
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => s.go(to));
      return b;
    };
    const where =
      p.used > 1
        ? t("Records {from} to {to} of {total}", { from: p.position + 1, to: Math.min(p.count, p.position + p.used), total: p.count })
        : t("Record {n} of {total}", { n: p.position + 1, total: p.count });
    const label = h("span", { class: "mm-strip-where", "aria-live": "polite" }, where);
    const note = p.skipped ? h("span", { class: "mm-strip-note" }, t("A rule leaves this record out")) : null;
    const find = h("button", { class: "tb", type: "button", title: t("Find a recipient") }, t("Find recipient"));
    find.addEventListener("mousedown", (e) => e.preventDefault());
    find.addEventListener("click", () => void s.x.fe.command("mailings.find"));
    const done = h("button", { class: "tb done", type: "button", title: t("Show the document with its merge fields again") }, t("Done"));
    done.addEventListener("mousedown", (e) => e.preventDefault());
    done.addEventListener("click", () => {
      s.stopPreview();
      s.x.editor.focus();
    });
    this.el.replaceChildren(
      h("div", { class: "seg", role: "group", "aria-label": t("Go to a record") }, key(MAIL_ICONS.mmFirst, t("First record"), "first", p.position === 0), key(MAIL_ICONS.mmPrevious, t("Previous record"), "previous", p.position === 0), key(MAIL_ICONS.mmNext, t("Next record"), "next", p.position >= p.count - 1), key(MAIL_ICONS.mmLast, t("Last record"), "last", p.position >= p.count - 1)),
      label,
      ...(note ? [note] : []),
      find,
      done,
    );
    this.shown = true;
    this.el.style.display = "";
    this.place();
  }

  hide() {
    this.shown = false;
    this.el.style.display = "none";
  }

  /** Keep the strip at the top of the pages. */
  place() {
    if (!this.shown) return;
    const c = this.s.x.editor.canvas.getBoundingClientRect();
    const w = this.el.offsetWidth || 420;
    this.el.style.left = `${Math.max(8, c.left + (c.width - w) / 2)}px`;
    // Under the ruler.
    this.el.style.top = `${c.top + 40}px`;
  }
}
