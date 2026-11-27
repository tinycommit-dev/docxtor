// Tools > Inspect document: what the file carries besides what the page shows, listed by the inspectors that are on
// (Docxtor's own look for personal information, comments, tracked changes, hidden text, headers and footers, custom
// data and embedded objects; extensions add more). Each finding can be removed; the removal is one undo step.
import { t, tn, type InspectorReport } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal, toast } from "./dom";

export interface InspectApi {
  inspect(signal: AbortSignal): Promise<InspectorReport[]>;
  /** Take the findings of these inspectors out, as one undo step; resolves with the ids that finished. */
  remove(ids: string[], signal: AbortSignal): Promise<string[]>;
}

export function openInspect(ed: Editor, api: InspectApi) {
  if (!ed.engine) return;
  const abort = new AbortController();
  const list = h("div", { class: "insp-list", role: "list", "aria-busy": "true" }, h("p", { class: "insp-note" }, t("Inspecting…")));
  const removeAll = h("button", { class: "btn", type: "button", disabled: true }, t("Remove everything found"));
  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const dialog = h(
    "div",
    { class: "dialog insp-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Inspect document") },
    h(
      "div",
      { class: "dialog-head" },
      h("h2", {}, t("Inspect document")),
      h("p", { class: "sub" }, t("Check the file for personal information and hidden data before you share it. Removing is one step you can undo, until the file is saved.")),
    ),
    h("div", { class: "dialog-body" }, list),
    h("div", { class: "dialog-actions" }, removeAll, h("span", { class: "spacer" }), done),
  );
  const close = modal(dialog, () => {
    abort.abort();
    ed.focus();
  });
  done.addEventListener("click", close);

  let busy = false;
  const run = async (ids: string[]) => {
    if (busy) return;
    busy = true;
    list.setAttribute("aria-busy", "true");
    try {
      const finished = await api.remove(ids, abort.signal);
      if (finished.length) toast(finished.length === 1 ? t("Removed.") : tn(finished.length, "Removed from {n} place.", "Removed from {n} places."));
    } finally {
      busy = false;
      await show();
    }
  };

  const row = (r: InspectorReport) => {
    const found = r.count > 0;
    const status = r.error
      ? t("Couldn't check: {reason}", { reason: r.error })
      : found
        ? tn(r.count, "{n} item found", "{n} items found")
        : t("Nothing found");
    const button = h("button", { class: "btn sm", type: "button", "aria-label": t("Remove {kind}", { kind: r.label.toLowerCase() }) }, t("Remove"));
    button.addEventListener("click", () => void run([r.id]));
    return h(
      "div",
      { class: `insp-row${found ? " found" : ""}`, role: "listitem", "data-inspector": r.id },
      h(
        "div",
        { class: "insp-text" },
        h("p", { class: "insp-label" }, r.label),
        r.description ? h("p", { class: "insp-desc" }, r.description) : null,
        h("p", { class: "insp-status", role: "status" }, status),
        r.detail.length ? h("ul", { class: "insp-detail" }, ...r.detail.map((d) => h("li", {}, d))) : null,
      ),
      found && r.removable && !r.error ? button : r.removable ? null : found ? h("span", { class: "insp-keep" }, t("Can't be removed here")) : null,
    );
  };

  async function show() {
    const reports = await api.inspect(abort.signal);
    if (abort.signal.aborted) return;
    list.replaceChildren(...reports.map(row));
    list.setAttribute("aria-busy", "false");
    removeAll.disabled = !reports.some((r) => r.count > 0 && r.removable && !r.error);
    removeAll.onclick = async () => {
      if (await ed.ask("inspect.removeAll")) void run(reports.filter((r) => r.count > 0 && r.removable && !r.error).map((r) => r.id));
    };
  }
  void show();
  done.focus();
}
