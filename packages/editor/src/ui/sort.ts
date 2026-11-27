// Sort paragraphs: the selected paragraphs (or list items) by their text, as text, numbers or dates, in up to
// three keys, ascending or descending, with the first line kept as a header if asked. One undoable step.
// The same dialog sorts the rows of a table by its columns (`openSort(ed, { … })`).
import type { SortKey, SortOptions } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { paraLength } from "../editor/selection";
import { h, modal } from "./dom";

type Row = { field: HTMLSelectElement; type: HTMLSelectElement; order: HTMLElement; descending: boolean };

/** Sorting the rows of a table: the paragraphs that mark the first and last row to sort (the whole table when `to` is left out), and the table's size. */
export interface TableSortTarget {
  from: number;
  to?: number;
  columns: number;
  /** The first row looks like a header, so it starts kept in place. */
  header: boolean;
}

export function openSort(ed: Editor, table?: TableSortTarget) {
  const ids = table ? [] : ed.selectedParagraphs();
  if (!table && ids.length < 2) return;

  const rows: Row[] = [];
  const keyRows: HTMLElement[] = [];
  const makeRow = (i: number) => {
    const field = h("select", { id: `so-field-${i}`, "aria-label": i === 0 ? t("Sort by") : t("Then by") });
    if (i > 0) field.append(h("option", { value: "-1" }, t("None")));
    if (table) for (let n = 1; n <= table.columns; n++) field.append(h("option", { value: String(n) }, t("Column {n}", { n })));
    else {
      field.append(h("option", { value: "0" }, t("Paragraphs")));
      for (let n = 1; n <= 9; n++) field.append(h("option", { value: String(n) }, t("Field {n}", { n })));
    }
    field.value = i === 0 ? (table ? "1" : "0") : "-1";
    const type = h("select", { id: `so-type-${i}`, "aria-label": t("Type") });
    for (const [v, label] of [["text", t("Text")], ["number", t("Number")], ["date", t("Date")]] as const) type.append(h("option", { value: v }, label));
    const order = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Order") });
    const row: Row = { field, type, order, descending: false };
    for (const [desc, label] of [[false, t("Ascending")], [true, t("Descending")]] as const) {
      const b = h("button", { type: "button", role: "radio", "aria-checked": "false", "data-desc": String(desc) }, label);
      b.addEventListener("click", () => {
        row.descending = desc;
        paint();
      });
      order.append(b);
    }
    field.addEventListener("change", paint);
    rows.push(row);
    return h(
      "div",
      { class: "field so-key" },
      h("label", { for: `so-field-${i}` }, i === 0 ? t("Sort by") : t("Then by")),
      h("div", { class: "so-controls" }, field, type, order),
    );
  };
  for (let i = 0; i < 3; i++) keyRows.push(makeRow(i));

  let header = table?.header ?? false;
  const headerSeg = h("div", { class: "seg", role: "radiogroup", "aria-label": t("My list has") });
  for (const [v, label] of [[true, t("Header row")], [false, t("No header row")]] as const) {
    const b = h("button", { type: "button", role: "radio", "aria-checked": "false", "data-v": String(v) }, label);
    b.addEventListener("click", () => {
      header = v;
      paint();
    });
    headerSeg.append(b);
  }
  const separator = h("select", { id: "so-sep", "aria-label": t("Fields are separated by") }, h("option", { value: "tabs" }, t("Tabs")), h("option", { value: "commas" }, t("Commas")), h("option", { value: "other" }, t("Another character")));
  const other = h("input", { type: "text", maxlength: "1", id: "so-other", "aria-label": t("The separating character"), value: ";", style: "width:48px;min-width:0" });
  separator.addEventListener("change", paint);
  const caseSensitive = h("input", { type: "checkbox", id: "so-case" });

  function paint() {
    rows.forEach((r, i) => {
      r.order.querySelectorAll("button").forEach((b) => {
        const on = (b as HTMLElement).dataset.desc === String(r.descending);
        b.classList.toggle("on", on);
        b.setAttribute("aria-checked", String(on));
      });
      const off = i > 0 && r.field.value === "-1";
      r.type.disabled = off;
      r.order.querySelectorAll("button").forEach((b) => ((b as HTMLButtonElement).disabled = off));
    });
    headerSeg.querySelectorAll("button").forEach((b) => {
      const on = (b as HTMLElement).dataset.v === String(header);
      b.classList.toggle("on", on);
      b.setAttribute("aria-checked", String(on));
    });
    other.hidden = separator.value !== "other";
  }
  paint();

  const error = h("p", { class: "dl-error", role: "alert", hidden: true });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("Sort"));
  const dialog = h(
    "div",
    { class: "dialog so-dialog", role: "dialog", "aria-modal": "true", "aria-label": table ? t("Sort table") : t("Sort paragraphs") },
    h("div", { class: "dialog-head" }, h("h2", {}, table ? t("Sort table") : t("Sort paragraphs")), h("p", { class: "sub" }, table ? t("Puts the rows of the table in order by the columns you choose. The cells of a row stay together.") : t("Puts the selected paragraphs in order. List items keep their numbers in the new order."))),
    h(
      "div",
      { class: "dialog-body" },
      ...keyRows,
      h("div", { class: "field" }, h("label", {}, table ? t("My table has") : t("My list has")), headerSeg),
      table ? null : h("div", { class: "field" }, h("label", { for: "so-sep" }, t("Fields are separated by")), h("div", { class: "so-controls" }, separator, other), h("span", { class: "help" }, t("Only needed to sort by a field."))),
      h("label", { class: "check", for: "so-case" }, caseSensitive, h("span", {}, t("Case sensitive"))),
      error,
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const keys: SortKey[] = [];
    rows.forEach((r) => {
      const field = Number(r.field.value);
      if (field < 0) return;
      keys.push({ type: r.type.value as SortKey["type"], descending: r.descending, field });
    });
    const options: SortOptions = { keys, case_sensitive: caseSensitive.checked, header, separator: separator.value as SortOptions["separator"] };
    if (separator.value === "other") options.separator_char = other.value || ";";
    const onError = (message: string) => {
      error.textContent = message;
      error.hidden = false;
    };
    if (table) {
      const res = ed.exec({ op: "sort_table", from: table.from, ...(table.to !== undefined ? { to: table.to } : {}), keys, case_sensitive: caseSensitive.checked, header }, { onError });
      if (!res) return;
      close();
      ed.focus();
      return;
    }
    const res = ed.exec({ op: "sort_paragraphs", ids, ...options }, { onError });
    if (!res) return;
    close();
    // The same paragraphs are selected, now in the new order: from the one first on the page to the last.
    const all = Array.from(ed.root.querySelectorAll<HTMLElement>("[data-p]")).map((e) => Number(e.dataset.p));
    const at = ids.map((i) => all.indexOf(i)).filter((i) => i >= 0);
    if (at.length) {
      const first = all[Math.min(...at)];
      const last = all[Math.max(...at)];
      ed.select({ anchor: { p: first, o: 0 }, focus: { p: last, o: paraLength(ed.root, last) } });
    }
    ed.focus();
  });
}
