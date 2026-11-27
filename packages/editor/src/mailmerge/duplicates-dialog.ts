// Edit recipient list, Find duplicates: the records that look alike, in groups, each with a tick (in the merge or not). The first of
// each group starts ticked and the others do not; the person can tick any they want to keep. OK sets the ticks, which is the
// include/exclude tick of the recipient list.
import type { MergeData } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import { h, modal, toast } from "../ui/dom";
import { findDuplicates, keepFirst, type DuplicateGroup, type DuplicateReason } from "./duplicates";
import type { MergeSession } from "./session";

const WHY: Record<DuplicateReason, () => string> = {
  identical: () => t("All the details are the same"),
  nameAddress: () => t("Same name and address"),
  email: () => t("Same email address"),
};

/** The most columns a group shows; the rest are in the recipient list. */
const SHOWN = 6;

let seq = 0;

/**
 * Show the duplicates of a list. Resolves with the ticks the person chose for every record (what `active` was, changed in the groups),
 * or null when they cancelled; resolves with `"none"` after saying so when no record looks like another.
 */
export function openDuplicates(data: Pick<MergeData, "columns" | "rows" | "matched">, active: boolean[], onClose?: () => void): Promise<boolean[] | "none" | null> {
  return new Promise((resolve) => {
    const groups: DuplicateGroup[] = findDuplicates(data);
    if (!groups.length) {
      resolve("none");
      return;
    }
    const id = `mmd-${++seq}`;
    const ticks = keepFirst(active, groups);
    const count = groups.reduce((n, g) => n + g.rows.length, 0);
    const columns = data.columns.slice(0, SHOWN);
    const sub = h("p", { class: "sub", "aria-live": "polite" });
    const paintSub = () => {
      const kept = groups.reduce((n, g) => n + g.rows.filter((r) => ticks[r]).length, 0);
      sub.textContent = t("{groups} of records look alike: {count} records, {kept} of them in the merge.", {
        groups: tn(groups.length, "{n} group", "{n} groups"),
        count,
        kept,
      });
    };
    const boxes: [number, HTMLInputElement][] = [];
    const sections = groups.map((g, i) => {
      const rows = g.rows.map((r) => {
        const box = h("input", { type: "checkbox", checked: ticks[r], "aria-label": t("Keep recipient {n} in the merge", { n: r + 1 }) });
        box.addEventListener("change", () => {
          ticks[r] = box.checked;
          row.classList.toggle("off", !box.checked);
          paintSub();
        });
        boxes.push([r, box]);
        const row = h("tr", { class: ticks[r] ? "" : "off" }, h("td", { class: "mm-tick" }, box), h("td", { class: "mm-dup-n" }, String(r + 1)), ...columns.map((_, c) => h("td", {}, data.rows[r][c] ?? "")));
        return row;
      });
      const head = h("tr", {}, h("th", { class: "mm-tick", scope: "col" }, h("span", { class: "sr-only" }, t("Keep in the merge"))), h("th", { scope: "col" }, t("No.")), ...columns.map((c) => h("th", { scope: "col" }, c)));
      // Two records with everything the same are alike in every other way too; say only that.
      const reasons: DuplicateReason[] = g.rows.length === 2 && g.reasons.includes("identical") ? ["identical"] : g.reasons;
      return h(
        "div",
        { class: "mm-dup-group", role: "group", "aria-labelledby": `${id}-g${i}` },
        h("h3", { id: `${id}-g${i}` }, t("Group {n}: {why}", { n: i + 1, why: reasons.map((r) => WHY[r]()).join(", ").toLowerCase() })),
        h("div", { class: "mm-table-wrap" }, h("table", { class: "mm-table" }, h("thead", {}, head), h("tbody", {}, ...rows))),
      );
    });
    const sync = () => {
      for (const [r, box] of boxes) {
        box.checked = ticks[r];
        box.closest("tr")?.classList.toggle("off", !ticks[r]);
      }
      paintSub();
    };
    const keep = h("button", { class: "btn sm", type: "button" }, t("Keep the first of each group"));
    const all = h("button", { class: "btn sm", type: "button" }, t("Keep every record"));
    keep.addEventListener("click", () => {
      Object.assign(ticks, keepFirst(active, groups));
      sync();
    });
    all.addEventListener("click", () => {
      for (const g of groups) for (const r of g.rows) ticks[r] = true;
      sync();
    });
    const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const dialog = h(
      "div",
      { class: "dialog mm-dialog mm-recipients", role: "dialog", "aria-modal": "true", "aria-label": t("Find duplicates") },
      h("div", { class: "dialog-head" }, h("h2", {}, t("Find duplicates")), sub, h("p", { class: "help" }, t("The first record of each group stays in the merge. Tick any other you want to keep, or clear a tick to leave a record out."))),
      h("div", { class: "dialog-body" }, h("div", { class: "mm-bar" }, keep, all), h("div", { class: "mm-dup" }, ...sections)),
      h("div", { class: "dialog-actions" }, cancel, ok),
    );
    let done = false;
    const finish = (v: boolean[] | null) => {
      if (done) return;
      done = true;
      close();
      resolve(v);
    };
    const close = modal(dialog, () => {
      if (!done) {
        done = true;
        resolve(null);
      }
      onClose?.();
    });
    cancel.addEventListener("click", () => finish(null));
    ok.addEventListener("click", () => finish(ticks.slice()));
    paintSub();
    ok.focus();
  });
}

/** The command: find the duplicates of the recipient list and set the ticks, as one undo step. */
export async function findDuplicatesOfList(s: MergeSession) {
  const data = s.engine && s.needRecipients() ? s.engine.mergeData() : null;
  if (!data) return;
  const found = await openDuplicates(data, data.active, () => s.x.editor.focus());
  if (found === "none") toast(t("No two records look alike."));
  else if (found && found.some((v, i) => v !== data.active[i])) s.run({ op: "set_merge_recipients", active: found });
}
