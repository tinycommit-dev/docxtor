// Mailings, Edit recipient list: every recipient with a tick (in the merge or not), a search over the list, a filter (rules
// joined by and/or), a sort by a column (click its heading), and Select all and Clear all for what is shown. Nothing changes
// in the document until OK, which is one undo step.
import type { MergeData, MergeFilterRule, MergeSortKey } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import { h, modal, svg, toast } from "../ui/dom";
import { I } from "../ui/icons";
import { openDuplicates } from "./duplicates-dialog";
import { FILTER_COMPARISONS } from "./form";
import type { MergeSession } from "./session";

const PAGE = 100;

/** A rule with something to compare to (a new rule that is still empty filters nothing). */
const usable = (r: MergeFilterRule) => !!r.field && (r.op === "empty" || r.op === "not_empty" || r.value !== "");

let seq = 0;

export function openRecipients(s: MergeSession) {
  const eng = s.engine;
  if (!eng || !s.needRecipients()) return;
  const data: MergeData | null = eng.mergeData();
  if (!data) return;
  const id = `mmr-${++seq}`;
  const active = data.active.slice();
  let filter: MergeFilterRule[] = data.filter.map((r) => ({ ...r }));
  let sort: MergeSortKey[] = data.sort.map((k) => ({ ...k }));
  let query = "";
  let page = 0;
  let order: number[] = eng.mergeOrder(filter.filter(usable), sort);

  const sub = h("p", { class: "sub", "aria-live": "polite" });
  const search = h("input", { type: "search", id: `${id}-q`, placeholder: t("Find in the list"), "aria-label": t("Find in the list"), spellcheck: "false", autocomplete: "off" });
  const selectAll = h("button", { class: "btn sm", type: "button" }, t("Select all"));
  const clearAll = h("button", { class: "btn sm", type: "button" }, t("Clear all"));
  const filterBtn = h("button", { class: "btn sm", type: "button", "aria-expanded": "false", "aria-controls": `${id}-filter` }, t("Filter…"));
  const dupBtn = h("button", { class: "btn sm", type: "button" }, t("Find duplicates…"));
  const panel = h("div", { class: "mm-filter", id: `${id}-filter`, hidden: true });
  const table = h("table", { class: "mm-table" });
  const scroller = h("div", { class: "mm-table-wrap", tabindex: "0", role: "region", "aria-label": t("Recipients") }, table);
  const pager = h("div", { class: "mm-pager" });
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));

  const shownRows = (): number[] => {
    const q = query.trim().toLowerCase();
    return q ? order.filter((r) => data.rows[r].some((v) => v.toLowerCase().includes(q))) : order;
  };
  const count = () => order.filter((r) => active[r]).length;

  const paintSub = () => {
    const filtered = filter.some(usable);
    sub.textContent = filtered
      ? t("{n} of {total} recipients are in the merge. The filter shows {shown}.", { n: count(), total: data.rows.length, shown: order.length })
      : tn(data.rows.length, "{n} of {total} recipient is in the merge.", "{n} of {total} recipients are in the merge.", { n: count(), total: data.rows.length });
  };

  const paintTable = () => {
    const rows = shownRows();
    const pages = Math.max(1, Math.ceil(rows.length / PAGE));
    page = Math.min(page, pages - 1);
    const slice = rows.slice(page * PAGE, page * PAGE + PAGE);
    const sorted = (c: string) => sort[0] && sort[0].field === c;
    const head = h(
      "tr",
      {},
      h("th", { class: "mm-tick", scope: "col" }, h("span", { class: "sr-only" }, t("In the merge"))),
      ...data.columns.map((c) => {
        const on = sorted(c);
        const b = h("button", { class: `mm-sort${on ? " on" : ""}`, type: "button", title: t("Sort by {column}", { column: c }), "aria-label": t("Sort by {column}", { column: c }) }, c, on ? svg(I.chevron) : null);
        if (on && !sort[0].desc) b.classList.add("up");
        b.addEventListener("click", () => {
          // Ascending, then descending, then the order of the list.
          const next: MergeSortKey[] = !on ? [{ field: c }] : !sort[0].desc ? [{ field: c, desc: true }] : [];
          sort = next;
          reorder();
        });
        return h("th", { scope: "col", "aria-sort": on ? (sort[0].desc ? "descending" : "ascending") : "none" }, b);
      }),
    );
    const body = slice.map((r) => {
      const box = h("input", { type: "checkbox", checked: active[r], "aria-label": t("Include recipient {n}", { n: r + 1 }) });
      box.addEventListener("change", () => {
        active[r] = box.checked;
        paintSub();
      });
      return h("tr", { class: active[r] ? "" : "off" }, h("td", { class: "mm-tick" }, box), ...data.rows[r].map((v) => h("td", {}, v)));
    });
    table.replaceChildren(h("thead", {}, head), h("tbody", {}, ...body));
    if (!rows.length) table.append(h("caption", {}, data.rows.length ? t("No recipient matches.") : t("The list is empty.")));
    // Pages of a hundred keep a long list quick to draw.
    pager.replaceChildren();
    if (pages > 1) {
      const prev = h("button", { class: "btn sm ghost", type: "button" }, t("Previous"));
      const next = h("button", { class: "btn sm ghost", type: "button" }, t("Next"));
      prev.disabled = page === 0;
      next.disabled = page >= pages - 1;
      prev.addEventListener("click", () => {
        page--;
        paintTable();
      });
      next.addEventListener("click", () => {
        page++;
        paintTable();
      });
      pager.append(prev, h("span", { class: "mm-pager-where" }, t("Recipients {from} to {to} of {total}", { from: page * PAGE + 1, to: page * PAGE + slice.length, total: rows.length })), next);
    }
  };

  const reorder = () => {
    try {
      order = eng.mergeOrder(filter.filter(usable), sort);
    } catch {
      order = Array.from(data.rows.keys());
    }
    page = 0;
    paintSub();
    paintTable();
    paintFilter();
  };

  const paintFilter = () => {
    const n = filter.filter(usable).length;
    filterBtn.classList.toggle("on", n > 0);
    filterBtn.textContent = n ? t("Filter ({n})", { n }) : t("Filter…");
    if (panel.hidden) return;
    const columns = data.columns.map((c) => h("option", { value: c }, c));
    const rows = filter.map((rule, i) => {
      const joinSel = h("select", { "aria-label": t("Join with the rule above") }, h("option", { value: "and" }, t("And")), h("option", { value: "or" }, t("Or")));
      joinSel.value = rule.join ?? "and";
      joinSel.addEventListener("change", () => {
        rule.join = joinSel.value as "and" | "or";
        reorder();
      });
      const field = h("select", { "aria-label": t("Column") }, ...columns.map((o) => o.cloneNode(true)));
      field.value = rule.field;
      field.addEventListener("change", () => {
        rule.field = field.value;
        reorder();
      });
      const op = h("select", { "aria-label": t("Comparison") }, ...FILTER_COMPARISONS.map((c) => h("option", { value: c.value }, t(c.label))));
      op.value = rule.op;
      const value = h("input", { type: "text", value: rule.value, "aria-label": t("Compare to"), spellcheck: "false", autocomplete: "off" });
      value.disabled = rule.op === "empty" || rule.op === "not_empty";
      op.addEventListener("change", () => {
        rule.op = op.value;
        value.disabled = rule.op === "empty" || rule.op === "not_empty";
        reorder();
      });
      value.addEventListener("input", () => {
        rule.value = value.value;
        reorder();
      });
      const remove = h("button", { class: "btn sm ghost", type: "button", "aria-label": t("Remove this rule"), title: t("Remove this rule") }, svg(I.close));
      remove.addEventListener("click", () => {
        filter.splice(i, 1);
        reorder();
      });
      return h("div", { class: "mm-rule" }, i === 0 ? h("span", { class: "mm-rule-join" }, t("Where")) : joinSel, field, op, value, remove);
    });
    const add = h("button", { class: "btn sm", type: "button" }, t("Add a rule"));
    add.addEventListener("click", () => {
      filter.push({ field: data.columns[0], op: "eq", value: "", ...(filter.length ? { join: "and" as const } : {}) });
      reorder();
    });
    const clear = h("button", { class: "btn sm ghost", type: "button" }, t("Clear the filter"));
    clear.disabled = !filter.length;
    clear.addEventListener("click", () => {
      filter = [];
      reorder();
    });
    panel.replaceChildren(...rows, h("div", { class: "mm-rule-tools" }, add, clear));
  };

  filterBtn.addEventListener("click", () => {
    panel.hidden = !panel.hidden;
    filterBtn.setAttribute("aria-expanded", String(!panel.hidden));
    if (!panel.hidden && !filter.length) filter.push({ field: data.columns[0], op: "eq", value: "" });
    if (panel.hidden) filter = filter.filter(usable);
    reorder();
  });
  dupBtn.addEventListener("click", async () => {
    const found = await openDuplicates(data, active);
    if (found === "none") toast(t("No two records look alike."));
    else if (found) {
      found.forEach((v, i) => (active[i] = v));
      paintSub();
      paintTable();
    }
  });
  search.addEventListener("input", () => {
    query = search.value;
    page = 0;
    paintTable();
  });
  selectAll.addEventListener("click", () => {
    for (const r of shownRows()) active[r] = true;
    paintSub();
    paintTable();
  });
  clearAll.addEventListener("click", () => {
    for (const r of shownRows()) active[r] = false;
    paintSub();
    paintTable();
  });

  const dialog = h(
    "div",
    { class: "dialog mm-dialog mm-recipients", role: "dialog", "aria-modal": "true", "aria-label": t("Recipients") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Recipients")), sub),
    h("div", { class: "dialog-body" }, h("div", { class: "mm-bar" }, search, selectAll, clearAll, filterBtn, dupBtn), panel, scroller, pager),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => s.x.editor.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    // Only what changed goes into the op.
    const same = <T,>(a: T, b: T) => JSON.stringify(a) === JSON.stringify(b);
    const cleanFilter = filter.filter(usable).map((r, i) => ({ ...r, join: i === 0 ? undefined : r.join ?? "and" }));
    const op: Record<string, unknown> = { op: "set_merge_recipients" };
    if (!same(active, data.active)) op.active = active;
    if (!same(cleanFilter.map(({ join, ...r }) => ({ ...r, join })), data.filter.map((r) => ({ ...r })))) op.filter = cleanFilter;
    if (!same(sort, data.sort)) op.sort = sort;
    close();
    if (Object.keys(op).length > 1) s.run(op as never);
  });
  paintSub();
  paintTable();
  paintFilter();
  search.focus();
}
