// The card a link shows when the caret is in it, as in Google Docs: where it goes, and Copy, Edit and Remove. A link
// inside the document (to a bookmark or a heading) shows the target's text. A title and description from the page
// itself come only from a `ctx.linkPreviews` provider, an extension that is off until turned on, since it asks the web.
import { t, type ExtensionHost } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { CHIP_ICONS } from "./chip-icons";
import { h, popover, toast } from "./dom";
import { I } from "./icons";

export class LinkCard {
  private close: (() => void) | null = null;
  private href = "";
  private ac: AbortController | null = null;

  constructor(
    private ed: Editor,
    private host: ExtensionHost,
    private edit: () => void,
  ) {}

  /** The caret moved: show the card for the link it is in, or none. */
  sync() {
    const ed = this.ed;
    const l = ed.engine && !ed.readOnly && ed.collapsed() ? ed.linkAt() : null;
    if (!l) return this.hide();
    if (l.href === this.href && this.close) return;
    this.hide();
    this.href = l.href;
    const el = ed.root.querySelector<HTMLElement>(`[data-p="${l.from.p}"] [data-href="${CSS.escape(l.href)}"]`);
    if (!el) return;
    const internal = l.href.startsWith("#");
    const target = internal ? ed.root.querySelector(`[data-bm~="${CSS.escape(l.href.slice(1))}"]`)?.textContent?.trim() : null;
    const domain = internal ? "" : (() => {
      try {
        return new URL(l.href).hostname.replace(/^www\./, "");
      } catch {
        return l.href;
      }
    })();
    const title = h("div", { class: "card-title" }, internal ? target || t("A place in this document") : domain);
    const sub = h("div", { class: "card-sub" }, internal ? t("Link in this document") : l.href);
    const desc = h("div", { class: "card-desc", hidden: true });
    const button = (label: string, icon: string, run: () => void) => {
      const b = h("button", { class: "btn sm", type: "button" }, h("span", { class: "at-icon", html: icon, "aria-hidden": "true" }), label);
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", run);
      return b;
    };
    const card = h(
      "div",
      { class: "chip-card link-card", role: "group", "aria-label": t("Link") },
      h("div", { class: "card-head" }, h("span", { class: "card-avatar", html: internal ? I.link : CHIP_ICONS.externalLink, "aria-hidden": "true" }), h("div", { class: "card-text" }, title, sub, desc)),
      h(
        "div",
        { class: "card-actions" },
        button(t("Copy link"), I.copy, () => {
          void navigator.clipboard?.writeText(l.href).then(() => toast(t("Link copied")));
        }),
        button(t("Edit link"), I.link, () => (this.hide(), this.edit())),
        button(t("Remove link"), CHIP_ICONS.unlink, () => (this.hide(), ed.removeLink())),
      ),
    );
    this.close = popover(el, card, { onClose: () => (this.close = null) });
    // The card is a reading aid: the keyboard stays in the text.
    ed.focus();
    if (!internal && this.host.linkPreviewProviders.list().length) {
      this.ac = new AbortController();
      void this.host.linkPreview(l.href, this.ac.signal).then((p) => {
        if (!p || this.href !== l.href) return;
        if (p.title) title.textContent = p.title;
        if (p.description) {
          desc.textContent = p.description;
          desc.hidden = false;
        }
      });
    }
  }

  hide() {
    this.ac?.abort();
    this.ac = null;
    this.href = "";
    const c = this.close;
    this.close = null;
    c?.();
  }
}
