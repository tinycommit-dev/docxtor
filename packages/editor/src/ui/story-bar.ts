// Contextual strip shown while a header, footer or the footnotes of a page are being
// edited: fields, first-page option, remove, and the way back to the body.
import { msg, t } from "@docxtor/extensions";
import type { Editor, StoryEdit } from "../editor/editor";
import { storyLink, toggleLinkToPrevious } from "../editor/story-links";
import { h, overlayLayer, svg } from "./dom";
import { I } from "./icons";

export class StoryBar {
  readonly el: HTMLElement;
  private story: StoryEdit | null = null;

  constructor(private ed: Editor) {
    this.el = h("div", { class: "story-strip", style: "display:none", role: "toolbar", "aria-label": t("Header and footer") });
    overlayLayer().append(ed.life.own(this.el));
    ed.canvas.addEventListener("scroll", () => this.place(), { passive: true });
    ed.life.on(window, "resize", () => this.place());
  }

  show(story: StoryEdit | null) {
    this.story = story;
    if (!story) {
      this.el.style.display = "none";
      return;
    }
    this.build(story);
    this.el.style.display = "";
    this.place();
  }

  private build(st: StoryEdit) {
    const ed = this.ed;
    const btn = (label: string, title: string, fn: () => void, cls = "tb") => {
      const b = h("button", { class: cls, type: "button", title }, label);
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", fn);
      return b;
    };
    const v = st.variant === "first" || st.variant === "even" ? st.variant : "default";
    const where = {
      header: { default: msg("Header on page {page}"), first: msg("First page header on page {page}"), even: msg("Even page header on page {page}") },
      footer: { default: msg("Footer on page {page}"), first: msg("First page footer on page {page}"), even: msg("Even page footer on page {page}") },
      notes: { default: msg("Footnotes on page {page}"), first: msg("Footnotes on page {page}"), even: msg("Footnotes on page {page}") },
      endnotes: { default: msg("Endnotes on page {page}"), first: msg("Endnotes on page {page}"), even: msg("Endnotes on page {page}") },
    }[st.zone][v];
    const label = h("span", { class: "story-where" }, t(where, { page: st.page + 1 }));
    const done = btn(t("Done"), t("Back to the document (Esc)"), () => ed.exitStory(), "tb done");
    if (st.zone === "notes" || st.zone === "endnotes") {
      this.el.replaceChildren(label, h("span", { class: "story-hint" }, t("Numbering follows the order of the marks in the text")), done);
      return;
    }
    const fields = h(
      "div",
      { class: "seg", role: "group", "aria-label": t("Insert field") },
      btn(t("Page #"), t("Insert the page number"), () => ed.insertField("PAGE")),
      btn(t("Page count"), t("Insert the number of pages"), () => ed.insertField("NUMPAGES")),
      btn(t("Date"), t("Insert today's date"), () => ed.insertField("DATE")),
    );
    // The options belong to the section of the page being edited.
    const info = ed.pageInfo;
    const box = ed.pageBox(st.page);
    const several = (info.sections?.length ?? 1) > 1;
    const sec = info.sections?.[box.section] ?? info;
    const titlePage = sec.titlePage;
    const scope = (i: number) => {
      const section = several ? ed.sectionAnchor(i) : undefined;
      return section !== undefined ? { section } : {};
    };
    const first = btn(titlePage ? `✓ ${t("Different first page")}` : t("Different first page"), t("Use a separate header and footer on the first page"), () => {
      const { zone, page } = st;
      ed.exitStory(false);
      ed.exec({ op: "set_title_page", on: !titlePage, ...scope(box.section) }, { select: null });
      // Turning it on while on the section's first page: continue in the (new) first-page version.
      if ((zone === "header" || zone === "footer") && box.first) ed.enterStory(zone, page);
    });
    if (titlePage) first.classList.add("on");
    // Different headers and footers on odd and even pages: a setting of the whole document.
    const evenOdd = !!info.evenAndOddHeaders;
    const odd = btn(evenOdd ? `✓ ${t("Different odd and even")}` : t("Different odd and even"), t("Use a separate header and footer on even pages"), () => {
      const { zone, page } = st;
      ed.exitStory(false);
      ed.exec({ op: "set_layout_settings", even_and_odd_headers: !evenOdd }, { select: null });
      // Turning it on from an even page: carry on in the (new) even-page version.
      if (!evenOdd && (zone === "header" || zone === "footer") && box.num % 2 === 0) ed.enterStory(zone, page);
    });
    if (evenOdd) odd.classList.add("on");
    odd.setAttribute("aria-pressed", String(evenOdd));
    // "Link to previous": a section after the first can show the header or footer of the section before, or have its own.
    const link = storyLink(ed);
    let linkBtn: HTMLElement | null = null;
    if (link && several) {
      const header = st.zone === "header";
      const hint = !link.linkable
        ? header
          ? t("The first section has no section before it to share a header with")
          : t("The first section has no section before it to share a footer with")
        : link.linked
          ? header
            ? t("Give this section a header of its own")
            : t("Give this section a footer of its own")
          : header
            ? t("Use the header of the section before")
            : t("Use the footer of the section before");
      const b = btn(link.linked ? `✓ ${t("Link to previous")}` : t("Link to previous"), hint, () => toggleLinkToPrevious(ed));
      b.setAttribute("aria-pressed", String(link.linked));
      if (link.linked) b.classList.add("on");
      if (!link.linkable) (b as HTMLButtonElement).disabled = true;
      linkBtn = b;
    }
    const remove = h("button", { class: "tb", type: "button", title: st.zone === "header" ? t("Remove this header") : t("Remove this footer"), "aria-label": st.zone === "header" ? t("Remove header") : t("Remove footer") }, svg(I.close));
    remove.addEventListener("mousedown", (e) => e.preventDefault());
    remove.addEventListener("click", () => {
      const kind = st.zone;
      const variant = st.variant ?? "default";
      // A section without its own header shows the one before it: remove it where it is defined.
      const map = (i: number) => (kind === "header" ? info.sections?.[i]?.headers : info.sections?.[i]?.footers) ?? {};
      let owner = box.section;
      while (owner > 0 && map(owner - 1)[variant] && map(owner - 1)[variant] === map(owner)[variant]) owner--;
      // The command asks first; the strip stays when the person says no.
      void ed.command(kind === "header" ? "insert.removeHeader" : "insert.removeFooter", { variant, ...scope(owner), leaveStory: true });
    });
    this.el.replaceChildren(...[label, fields, first, odd, linkBtn, remove, done].filter((x): x is HTMLElement => !!x));
  }

  /** Keep the strip next to the area being edited: below a header, above a footer or notes. */
  place() {
    const area = this.ed.storyElement;
    if (!this.story || !area) return;
    const r = area.getBoundingClientRect();
    const c = this.ed.canvas.getBoundingClientRect();
    const sw = this.el.offsetWidth || 420;
    const sh = this.el.offsetHeight || 36;
    const below = this.story.zone === "header";
    let top = below ? r.bottom + 10 : r.top - sh - 10;
    top = Math.max(c.top + 4, Math.min(c.bottom - sh - 4, top));
    this.el.style.left = `${Math.max(8, Math.min(window.innerWidth - sw - 8, r.left))}px`;
    this.el.style.top = `${top}px`;
    this.el.style.visibility = r.bottom < c.top || r.top > c.bottom ? "hidden" : "";
  }
}
