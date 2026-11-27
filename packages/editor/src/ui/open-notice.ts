// A strip above the pages after a file was converted, repaired or read as Strict Open XML: it says so in one sentence
// and offers the details (what was fixed, what the conversion leaves out). It doesn't block anything. The same
// dialog shows why a file could not be opened.
import { t, type OpenNotice } from "@docxtor/extensions";
import { h, modal } from "./dom";

/** The notice in a dialog: its title, then one line for each thing that was done or left out. */
export function openNoticeDialog(notice: OpenNotice) {
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("About this file"), style: "width:min(var(--dialog-m),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("About this file")), h("p", { class: "sub" }, notice.title)),
    h("div", { class: "dialog-body" }, h("ul", { class: "open-notes" }, ...notice.notes.map((n) => h("li", {}, n)))),
    h("div", { class: "dialog-actions" }, ok),
  );
  const close = modal(dialog);
  ok.addEventListener("click", close);
  ok.focus();
}

/** Say why a file could not be opened. */
export function openFailedDialog(name: string, reason: string): Promise<void> {
  return new Promise((resolve) => {
    const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
    const dialog = h(
      "div",
      { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Couldn't open {name}", { name }), style: "width:min(var(--dialog-m),100%)" },
      h("div", { class: "dialog-head" }, h("h2", {}, t("Couldn't open {name}", { name })), h("p", { class: "sub open-failed" }, reason)),
      h("div", { class: "dialog-actions" }, ok),
    );
    const close = modal(dialog, resolve);
    ok.addEventListener("click", close);
    ok.focus();
  });
}

export class OpenNoticeBar {
  readonly el: HTMLElement;
  private notice: OpenNotice | null = null;

  constructor(private onClose: () => void) {
    this.el = h("div", { class: "open-notice", role: "status", hidden: true });
  }

  /** Show the notice for a document that was just opened (null hides the strip). */
  show(notice: OpenNotice | null) {
    this.notice = notice;
    if (!notice) {
      this.el.hidden = true;
      this.el.replaceChildren();
      return;
    }
    const details = h("button", { class: "btn", type: "button" }, t("Details…"));
    details.addEventListener("click", () => this.details());
    const dismiss = h("button", { class: "btn ghost", type: "button" }, t("Dismiss"));
    dismiss.addEventListener("click", () => {
      this.show(null);
      this.onClose();
    });
    this.el.replaceChildren(h("p", {}, notice.title), h("div", { class: "open-notice-actions" }, details, dismiss));
    this.el.hidden = false;
  }

  details() {
    if (this.notice) openNoticeDialog(this.notice);
  }
}
