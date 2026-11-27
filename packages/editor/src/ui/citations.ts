// Citations in the text: the short lines that name a source, the popover that opens on a citation (its
// sources, page, what to leave out), and the plain picker behind Insert, Citation when the host has no
// Citations panel.
import type { CitationInfo, Source, SourceName } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, popover, svg, toast } from "./dom";
import { I } from "./icons";
import { quickPick } from "./quick-pick";

const nameOf = (n: SourceName) => ("literal" in n ? n.literal : n.family);

/** Who wrote it, in a few words: "Smith", "Smith and Lee", "Smith et al." (the editors when there are no authors). */
export function sourceAuthors(s: Source): string {
  const names = (s.authors.length ? s.authors : (s.editors ?? [])).map(nameOf).filter(Boolean);
  if (names.length === 0) return "";
  if (names.length === 1) return names[0];
  if (names.length === 2) return t("{a} and {b}", { a: names[0], b: names[1] });
  return t("{name} et al.", { name: names[0] });
}

/** The title, or a placeholder for a source that has none yet. */
export const sourceTitle = (s: Source): string => s.title.trim() || t("Untitled source");

/** The quiet line under a title: "Smith and Lee, 2020". */
export const sourceDetail = (s: Source): string => [sourceAuthors(s), s.year ?? ""].filter(Boolean).join(", ");

export interface CitationPopoverOptions {
  /** Offer "Edit source" on each source; the host opens its own form. */
  editSource?: (tag: string) => void;
}

/**
 * The popover on a citation: the sources it cites (each can be taken out or edited), a field to add another,
 * the page, and what to leave out. Every change is an `edit_citation` op; the citation stays selected.
 */
export function openCitationPopover(ed: Editor, citation: CitationInfo, anchor: HTMLElement | { x: number; y: number }, opts: CitationPopoverOptions = {}) {
  let cur = citation;
  let gone = false;
  const readOnly = ed.readOnly;

  const list = h("div", { class: "cite-sources" });
  const add = h("select", { class: "cite-add", "aria-label": t("Add a source"), disabled: readOnly });
  const page = h("input", { type: "text", class: "cite-input", spellcheck: "false", autocomplete: "off", "aria-label": t("Page"), placeholder: t("For example 23 or 23-25"), disabled: readOnly });
  const author = h("input", { type: "checkbox", disabled: readOnly });
  const year = h("input", { type: "checkbox", disabled: readOnly });
  const remove = h("button", { class: "btn ghost danger", type: "button", disabled: readOnly }, t("Remove citation"));
  const box = h(
    "div",
    { class: "cite-pop" },
    h("div", { class: "menu-title" }, t("Citation")),
    list,
    h("div", { class: "cite-field" }, add),
    h("label", { class: "cite-field" }, h("span", { class: "cite-label" }, t("Page")), page),
    h("label", { class: "cite-check" }, author, h("span", {}, t("Omit author"))),
    h("label", { class: "cite-check" }, year, h("span", {}, t("Omit year"))),
    h("div", { class: "bm-actions" }, remove),
  );

  const close = popover(anchor, box, {
    onClose: () => {
      ed.flow.querySelectorAll(".cite-on").forEach((el) => el.classList.remove("cite-on"));
      // The caret goes to the end of the citation, so typing carries on after it rather than replacing it.
      if (!gone) {
        const end = ed.engine.citations().find((c) => c.p === cur.p && c.o === cur.o);
        ed.select({ p: cur.p, o: end ? end.o + end.len : cur.o });
      }
      ed.focus();
    },
  });

  /** Run a change; keep the keyboard focus where it was (the document is drawn again). */
  let applying = false;
  const apply = (patch: Parameters<Editor["editCitation"]>[1]) => {
    // Drawing the document again moves the selection, which can blur a field and make it commit a second time.
    if (applying) return;
    applying = true;
    const was = document.activeElement as HTMLElement | null;
    try {
      const next = ed.editCitation(cur, patch);
      if (next) cur = next;
      paint();
    } finally {
      applying = false;
    }
    if (was?.isConnected && document.activeElement !== was) was.focus({ preventScroll: true });
  };

  const paint = () => {
    const all = ed.engine.sources();
    const rows = cur.tags.map((tag) => {
      const s = all.find((x) => x.tag === tag);
      const only = cur.tags.length < 2;
      const edit = opts.editSource && s ? h("button", { class: "btn ghost sm", type: "button", disabled: readOnly }, t("Edit source")) : null;
      edit?.addEventListener("click", () => {
        close();
        opts.editSource?.(tag);
      });
      const out = h("button", { class: "tb", type: "button", title: only ? t("A citation needs a source. Remove the citation instead.") : t("Take this source out of the citation"), "aria-label": t("Remove {title} from this citation", { title: s ? sourceTitle(s) : tag }), disabled: readOnly || only }, svg(I.close));
      out.addEventListener("click", () => apply({ tags: cur.tags.filter((x) => x !== tag) }));
      return h(
        "div",
        { class: "cite-src", "data-tag": tag },
        h("div", { class: "cite-src-info" }, h("span", { class: "cite-src-title" }, s ? sourceTitle(s) : t("Missing source {tag}", { tag })), s && sourceDetail(s) ? h("span", { class: "cite-src-detail" }, sourceDetail(s)) : null),
        edit,
        out,
      );
    });
    list.replaceChildren(...rows);
    const free = all.filter((s) => !cur.tags.includes(s.tag));
    add.replaceChildren(h("option", { value: "" }, t("Add a source…")), ...free.map((s) => h("option", { value: s.tag }, [sourceTitle(s), sourceDetail(s)].filter(Boolean).join(", "))));
    add.value = "";
    (add.parentElement as HTMLElement).hidden = !free.length;
    // The citation being edited keeps its outline while focus is in the popover.
    ed.flow.querySelectorAll(".cite-on").forEach((el) => el.classList.remove("cite-on"));
    ed.citationElement(cur)?.classList.add("cite-on");
    page.value = cur.page ?? "";
    author.checked = !!cur.suppressAuthor;
    year.checked = !!cur.suppressYear;
  };

  add.addEventListener("change", () => add.value && apply({ tags: [...cur.tags, add.value] }));
  // Enter and leaving the field both commit; only a different page is a change (and an undo step).
  const commitPage = () => {
    const value = page.value.trim();
    if (value !== (cur.page ?? "")) apply({ page: value });
  };
  page.addEventListener("change", commitPage);
  page.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commitPage();
    }
  });
  author.addEventListener("change", () => apply({ suppress_author: author.checked }));
  year.addEventListener("change", () => apply({ suppress_year: year.checked }));
  remove.addEventListener("click", () => {
    gone = true;
    close();
    void ed.command("citation.remove", cur);
  });
  paint();
  if (!readOnly) setTimeout(() => page.focus());
}

/** Open the popover for the citation the caret is in, near the caret. False when it isn't in one. */
export function openCitationAtCaret(ed: Editor, opts: CitationPopoverOptions = {}): boolean {
  const c = ed.citationAtCaret();
  if (!c) return false;
  ed.selectCitation(c);
  const el = ed.citationElement(c);
  const rect = !el && window.getSelection()?.rangeCount ? window.getSelection()!.getRangeAt(0).getBoundingClientRect() : null;
  openCitationPopover(ed, c, el ?? { x: rect?.left ?? 120, y: rect?.bottom ?? 120 }, opts);
  return true;
}

/** Choose one of the document's sources from a list and cite it at the caret. */
export async function pickSourceToCite(ed: Editor) {
  const sources = ed.engine.sources();
  if (!sources.length) return toast(t("This document has no sources yet."));
  const pick = await quickPick(
    sources.map((s) => ({ label: sourceTitle(s), detail: sourceDetail(s), tag: s.tag })),
    { placeholder: t("Search your sources"), label: t("Cite a source") },
  );
  if (pick) ed.insertCitation([pick.tag]);
}
