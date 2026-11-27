// Table of contents: which levels and styles make entries, page numbers on or off and where they sit, the line
// before them, links, and a look. Opened on a table of contents it changes that one; otherwise it adds one.
import type { TocOptions } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { caretToc, LEADERS, setToc } from "../editor/lists";
import { h, modal } from "./dom";

const LOOKS: [string, ReturnType<typeof msg>][] = [
  ["", msg("From the document")],
  ["classic", msg("Classic")],
  ["formal", msg("Formal")],
  ["modern", msg("Modern")],
  ["simple", msg("Simple")],
];

/** Styles that make entries by themselves or belong to the lists: left out of the style list. */
const SKIP = /^(heading\s*\d|toc\s*\d|toc heading|table of figures|index\s*\d|index heading|bibliography|caption|header|footer)$/i;

let seq = 0;

export function openTocDialog(ed: Editor) {
  if (ed.story || !ed.engine) return;
  const id = `toc-${++seq}`;
  const at = caretToc(ed);
  const editing = at !== null;
  const start: TocOptions = (editing ? ed.engine.tocOptions(at!) : null) ?? { levels: 3, outline: true, styles: [], entries: false, links: true, pageNumbers: true, rightAlign: true, leader: "dot" };
  const levelOf = new Map(start.styles.map((s) => [s.style.toLowerCase(), s.level]));

  const check = (label: string, on: boolean, name: string) => {
    const input = h("input", { type: "checkbox", id: `${id}-${name}`, checked: on });
    return { input, row: h("label", { class: "check", for: `${id}-${name}` }, input, h("span", {}, label)) };
  };
  const pages = check(t("Show page numbers"), start.pageNumbers, "pages");
  const align = check(t("Right align page numbers"), start.rightAlign, "align");
  const links = check(t("Make entries links to their headings"), start.links, "links");
  const outline = check(t("Paragraphs with an outline level"), start.outline, "outline");
  const entries = check(t("Table entry fields (Insert, Add to contents)"), start.entries, "entries");

  const leader = h("select", { id: `${id}-leader` }, ...LEADERS.map(([v, label]) => h("option", { value: v }, t(label))));
  leader.value = start.leader;
  const look = h("select", { id: `${id}-look` }, ...LOOKS.map(([v, label]) => h("option", { value: v }, t(label))));
  const levels = h("select", { id: `${id}-levels` }, h("option", { value: "0" }, t("None")), ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => h("option", { value: String(n) }, t("Levels 1 to {n}", { n }))));
  levels.value = String(start.levels);

  // Other styles: each can make entries at a level.
  const styleRows = ed.engine
    .styles()
    .filter((s) => s.kind === "paragraph" && !s.hidden && !SKIP.test(s.name) && (s.quick || s.custom || levelOf.has(s.name.toLowerCase()) || levelOf.has(s.id.toLowerCase())))
    .map((s) => {
      const input = h("input", { type: "number", min: "1", max: "9", "aria-label": t("Level for {style}", { style: s.name }), value: String(levelOf.get(s.name.toLowerCase()) ?? levelOf.get(s.id.toLowerCase()) ?? ""), placeholder: "-" });
      return { name: s.name, input, row: h("div", { class: "toc-style" }, h("span", {}, s.name), input) };
    });
  const styleList = h("div", { class: "toc-styles", role: "group", "aria-label": t("Styles and their levels") }, ...styleRows.map((r) => r.row));
  const styleBox = h("details", { class: "toc-more", open: start.styles.length > 0 }, h("summary", {}, t("Other styles")), h("p", { class: "help" }, t("Give a style a level to list its paragraphs too.")), styleList);

  const refresh = () => {
    const on = pages.input.checked;
    align.input.disabled = !on;
    leader.disabled = !on || !align.input.checked;
  };
  pages.input.addEventListener("change", refresh);
  align.input.addEventListener("change", refresh);
  refresh();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const remove = h("button", { class: "btn", type: "button" }, t("Remove table of contents"));
  const ok = h("button", { class: "btn primary", type: "button" }, editing ? t("Apply") : t("Insert"));
  const dialog = h(
    "div",
    { class: "dialog toc-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Table of contents") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Table of contents")), h("p", { class: "sub" }, editing ? t("Change the table the caret is in. Update fields keeps it current.") : t("Lists the headings with their page numbers, above the paragraph you are in."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: `${id}-levels` }, t("Heading levels")), levels),
      h("div", { class: "field" }, h("label", {}, t("Also list")), outline.row, entries.row, styleBox),
      h("div", { class: "field" }, h("label", {}, t("Page numbers")), pages.row, align.row, h("div", { class: "toc-leader" }, h("label", { for: `${id}-leader` }, t("Line before the number")), leader)),
      links.row,
      h("div", { class: "field" }, h("label", { for: `${id}-look` }, t("Look")), look, h("span", { class: "help" }, t("A look sets the formatting of the contents styles."))),
    ),
    h("div", { class: "dialog-actions" }, editing ? remove : null, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  remove.addEventListener("click", () => {
    close();
    void ed.command("insert.removeToc");
  });
  ok.addEventListener("click", () => {
    const styles = styleRows.filter((r) => r.input.value).map((r) => ({ style: r.name, level: Math.min(9, Math.max(1, Number(r.input.value) || 1)) }));
    const options: TocOptions = {
      levels: Number(levels.value),
      outline: outline.input.checked,
      styles,
      entries: entries.input.checked,
      ...(start.entryId ? { entryId: start.entryId } : {}),
      links: links.input.checked,
      pageNumbers: pages.input.checked,
      rightAlign: align.input.checked,
      leader: leader.value as TocOptions["leader"],
      ...(start.hidePages ? { hidePages: start.hidePages } : {}),
      ...(look.value ? { look: look.value } : {}),
    };
    close();
    if (editing) setToc(ed, options);
    else ed.insertToc(t("Contents"), options);
  });
  levels.focus();
}
