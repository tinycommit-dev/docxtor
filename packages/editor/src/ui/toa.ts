// The table of authorities, as lawyers use it in Word: Mark citation (the long and short citation of the selected text,
// in a category; Mark, Mark all, Next citation; the dialog stays open while you go through the document), the
// categories 8 to 16 named, and the Table of authorities dialog that inserts or changes the table. The marks are hidden
// TA fields; the table is a TOA field built by category with the page numbers, updated with the other fields.
import type { MPos, ToaOptions } from "@docxtor/core";
import { msg, t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { LEADERS } from "../editor/lists";
import { h, modal, toast } from "./dom";

/** Category names in the interface language: Word's seven, then the document's own for 8 to 16. */
const FIXED = [msg("Cases"), msg("Statutes"), msg("Other authorities"), msg("Rules"), msg("Treatises"), msg("Regulations"), msg("Constitutional provisions")];

export function categoryNames(ed: Editor): string[] {
  const own = ed.engine?.toaCategoryNames() ?? [];
  return Array.from({ length: 16 }, (_, i) => (i < 7 ? t(FIXED[i]) : own[i] && !/^Category \d+$/.test(own[i]) ? own[i] : t("Category {n}", { n: i + 1 })));
}

const LOOKS: [string, ReturnType<typeof msg>][] = [
  ["", msg("From the document")],
  ["classic", msg("Classic")],
  ["distinctive", msg("Distinctive")],
  ["formal", msg("Formal")],
  ["simple", msg("Simple")],
];

/** The table of authorities the caret is in, if any. */
export function caretToa(ed: Editor): number | null {
  if (ed.story || !ed.engine) return null;
  const r = ed.range();
  if (!r) return null;
  // With a paragraph, the engine answers only for a table that paragraph is in.
  return ed.engine.toaOptions(r[0].p) ? r[0].p : null;
}

/** Insert a table of authorities at the caret (before the paragraph it is in), then fix its page numbers. */
export function insertToa(ed: Editor, options: ToaOptions): boolean {
  if (ed.readOnly || ed.story) return false;
  const r = ed.range();
  if (!r) return false;
  const caret = ed.sel;
  if (!ed.exec({ op: "toa", action: "insert", before: ed.outerBlock(r[0].p), options, pages: ed.pageMap() }, { select: caret })) return false;
  ed.runUpdateFields(true, caret);
  return true;
}

/** Change the table of authorities the caret is in (or the first one) and build it again. */
export function setToa(ed: Editor, options: ToaOptions): boolean {
  if (ed.readOnly) return false;
  const at = caretToa(ed) ?? undefined;
  const caret = ed.sel;
  if (!ed.exec({ op: "toa", action: "set", ...(at !== undefined ? { at } : {}), options, pages: ed.pageMap() }, { select: caret })) return false;
  ed.runUpdateFields(true, caret);
  return true;
}

/** Build every table of authorities again (Word's Update table), with the page numbers as they are laid out. */
export function updateToa(ed: Editor): boolean {
  if (ed.readOnly || !ed.engine?.toaOptions()) return false;
  const caret = ed.sel;
  if (!ed.exec({ op: "toa", action: "update", pages: ed.pageMap() }, { select: caret })) return false;
  ed.runUpdateFields(true, caret);
  return true;
}

/** Name categories 8 to 16. */
export function openCategories(ed: Editor, after?: () => void) {
  const names = categoryNames(ed);
  const list = h("select", { id: "toa-cat-list", size: "9" }, ...names.slice(7).map((n, i) => h("option", { value: String(i + 8) }, n)));
  list.value = "8";
  const name = h("input", { type: "text", id: "toa-cat-name", value: names[7], autocomplete: "off" });
  list.addEventListener("change", () => (name.value = names[Number(list.value) - 1]));
  const replace = h("button", { class: "btn", type: "button" }, t("Replace"));
  const done = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const changed: { category: number; name: string }[] = [];
  replace.addEventListener("click", () => {
    const n = Number(list.value);
    const v = name.value.trim();
    names[n - 1] = v || t("Category {n}", { n });
    (list.options[n - 8] as HTMLOptionElement).textContent = names[n - 1];
    changed.push({ category: n, name: v });
  });
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Edit category") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Edit category")), h("p", { class: "sub" }, t("Categories 8 to 16 can have names of your own. They are kept in this document."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "toa-cat-list" }, t("Category")), list), h("div", { class: "field" }, h("label", { for: "toa-cat-name" }, t("Replace with")), name)),
    h("div", { class: "dialog-actions" }, replace, done),
  );
  const close = modal(dialog, () => after?.());
  done.addEventListener("click", () => {
    if (changed.length) ed.exec({ op: "toa", action: "set_category_names", names: changed });
    close();
  });
}

/** Mark citation (Alt+Shift+I): stays open, so citation after citation can be marked. */
export function openMarkCitation(ed: Editor) {
  if (ed.story || !ed.engine) return;
  let range: [MPos, MPos] | null = null;
  const pickSelection = () => {
    const r = ed.range();
    range = r && !ed.collapsed() ? [r[0], r[1]] : null;
    return range ? ed.selectedText().replace(/\s+/g, " ").trim() : "";
  };
  const picked = pickSelection();
  const long = h("textarea", { id: "toa-long", rows: "3", spellcheck: "false" }, picked);
  long.value = picked;
  const short = h("input", { type: "text", id: "toa-short", value: shortForm(picked), autocomplete: "off", spellcheck: "false" });
  const category = h("select", { id: "toa-category" });
  const fillCategories = (keep = category.value || "1") => {
    category.replaceChildren(...categoryNames(ed).map((n, i) => h("option", { value: String(i + 1) }, n)));
    category.value = keep;
  };
  fillCategories();
  const status = h("p", { class: "help", role: "status", "aria-live": "polite" });
  const next = h("button", { class: "btn", type: "button" }, t("Next citation"));
  const cats = h("button", { class: "btn", type: "button" }, t("Category…"));
  const markAll = h("button", { class: "btn", type: "button" }, t("Mark all"));
  const mark = h("button", { class: "btn primary", type: "button" }, t("Mark"));
  const closeBtn = h("button", { class: "btn ghost", type: "button" }, t("Close"));
  const check = () => {
    const none = !long.value.trim();
    mark.disabled = none;
    markAll.disabled = none;
  };
  long.addEventListener("input", check);
  long.addEventListener("input", () => (short.value = shortForm(long.value)));
  check();
  const dialog = h(
    "div",
    { class: "dialog toa-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Mark citation") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Mark citation")), h("p", { class: "sub" }, t("The citation is hidden in the text and listed under its category, with its page numbers, in the table of authorities."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: "toa-long" }, t("Long citation")), long, h("p", { class: "help" }, t("How the table lists it: the full citation."))),
      h("div", { class: "field" }, h("label", { for: "toa-short" }, t("Short citation")), short, h("p", { class: "help" }, t("Later mentions in this short form count for the same citation."))),
      h("div", { class: "field" }, h("label", { for: "toa-category" }, t("Category")), category),
      status,
    ),
    h("div", { class: "dialog-actions" }, next, cats, closeBtn, markAll, mark),
  );
  const close = modal(dialog, () => ed.focus());
  closeBtn.addEventListener("click", close);
  cats.addEventListener("click", () => openCategories(ed, () => fillCategories()));
  const run = (all: boolean) => {
    const text = long.value.trim();
    if (!text) return;
    const s = short.value.trim();
    const cat = Number(category.value);
    if (all) {
      const res = ed.exec({ op: "toa", action: "mark_all", long: text, ...(s ? { short: s } : {}), category: cat, ...(range ? { from: range[0], to: range[1] } : {}) }, { select: ed.sel });
      const n = res?.[0]?.count ?? 0;
      status.textContent = !res ? t("That citation could not be marked.") : n ? tn(n, "Marked {n} place.", "Marked {n} places.") : t("No other place has this citation.");
    } else {
      const at = range ? range[1] : ed.range()?.[1];
      if (!at) return;
      if (ed.exec({ op: "toa", action: "mark_citation", at, ...(range ? { from: range[0] } : {}), long: text, ...(s ? { short: s } : {}), category: cat }, { select: at })) status.textContent = t("Marked.");
    }
  };
  mark.addEventListener("click", () => run(false));
  markAll.addEventListener("click", () => run(true));
  next.addEventListener("click", () => {
    const from = ed.range()?.[1];
    const found = from && ed.engine?.nextCitation(from);
    if (!found) {
      status.textContent = t("No more likely citations after this point.");
      return;
    }
    ed.select({ anchor: found[0], focus: found[1] });
    const text = pickSelection();
    long.value = text;
    short.value = shortForm(text);
    status.textContent = "";
    check();
  });
  long.focus();
}

/** A short form to start from: a case's first party ("Smith" for "Smith v. Jones, ..."), else the text up to a comma. */
function shortForm(long: string): string {
  const s = long.trim();
  const vs = /^(.+?)\s+v\.?\s+/.exec(s);
  if (vs) return vs[1];
  return s.split(",")[0].slice(0, 60);
}

/** Insert table of authorities, or change the one the caret is in. */
export function openToaDialog(ed: Editor) {
  if (ed.story || !ed.engine) return;
  const at = caretToa(ed);
  const editing = at !== null;
  const start: ToaOptions = (editing ? ed.engine.toaOptions(at!) : null) ?? { category: 0, passim: true, keepFormatting: true, headings: true, rightAlign: true, leader: "dot" };
  const category = h("select", { id: "toa-cat" }, h("option", { value: "0" }, t("All")), ...categoryNames(ed).map((n, i) => h("option", { value: String(i + 1) }, n)));
  category.value = String(start.category ?? 0);
  const passim = h("input", { type: "checkbox", id: "toa-passim", checked: !!start.passim });
  const keep = h("input", { type: "checkbox", id: "toa-keep", checked: !!start.keepFormatting });
  const leader = h("select", { id: "toa-leader" }, ...LEADERS.map(([v, label]) => h("option", { value: v }, t(label))));
  leader.value = start.leader ?? "dot";
  const look = h("select", { id: "toa-look" }, ...LOOKS.map(([v, label]) => h("option", { value: v }, t(label))));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, editing ? t("Apply") : t("Insert"));
  const dialog = h(
    "div",
    { class: "dialog toa-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Table of authorities") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Table of authorities")), h("p", { class: "sub" }, editing ? t("Change the table the caret is in. Update fields keeps it current.") : t("Lists the citations you marked, by category, with their page numbers, before the paragraph you are in."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: "toa-cat" }, t("Category")), category),
      h("label", { class: "check", for: "toa-passim" }, passim, h("span", {}, t("Use passim (five or more pages on one citation)"))),
      h("label", { class: "check", for: "toa-keep" }, keep, h("span", {}, t("Keep original formatting"))),
      h("div", { class: "field" }, h("label", { for: "toa-leader" }, t("Line before the number")), leader),
      h("div", { class: "field" }, h("label", { for: "toa-look" }, t("Look")), look),
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    // What the dialog doesn't show (no headings, a comma before the pages, a bookmark, a sequence) stays as the table has it.
    const { look: _look, ...kept } = start;
    const options: ToaOptions = {
      headings: true,
      rightAlign: true,
      ...kept,
      category: Number(category.value),
      passim: passim.checked,
      keepFormatting: keep.checked,
      leader: leader.value as ToaOptions["leader"],
      ...(look.value ? { look: look.value } : {}),
    };
    close();
    if (editing) setToa(ed, options);
    else insertToa(ed, options);
    // A table with no marked citations says so in the document; say how to mark them.
    if (!editing && ed.engine.text().includes("No table of authorities entries found")) toast(t("Mark the citations first: References, Mark citation."));
  });
  category.focus();
}
