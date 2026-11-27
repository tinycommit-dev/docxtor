// Version history: a timeline per document with diffs and restore.
import { wordDiff } from "../ai/diff";
import { listVersions, renameVersion, versionBytes, type Version } from "../files/versions";
import { currentLocale, escapeHtml, h, msg, promptDialog, svg, t, tn } from "@docxtor/editor";
import { I } from "@docxtor/editor";

export interface HistoryHost {
  /** Ask before a risky action, by its caution id (web/src/cautions.ts): true to go ahead. */
  ask(id: string, ...args: unknown[]): Promise<boolean>;
  docKey(): string | null;
  /** Diff JSON of the current document against older bytes. */
  diffAgainstCurrent(bytes: Uint8Array): DiffSummary | null;
  diffBetween(older: Uint8Array, newer: Uint8Array): DiffSummary | null;
  restore(v: Version, bytes: Uint8Array): Promise<void>;
  nameCurrent(label: string): Promise<void>;
  download(v: Version, bytes: Uint8Array): void;
}

import type { DiffEntry, DiffSummary } from "@docxtor/core";
export type { DiffEntry, DiffSummary };

const KIND_LABEL: Record<string, string> = {
  open: msg("Opened"),
  save: msg("Saved"),
  auto: msg("Checkpoint"),
  ai: msg("AI edit"),
  "before-ai": msg("Before AI edit"),
  named: msg("Named"),
  restore: msg("Restored"),
};

/** Labels the app writes into history (stored in English, shown translated). */
msg("Before AI edit");
msg("Before restoring");
msg("Saved as a new file");

function when(at: number): string {
  const d = new Date(at);
  const today = new Date();
  const locale = currentLocale() === "en-XA" ? undefined : currentLocale();
  const time = d.toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
  if (d.toDateString() === today.toDateString()) return t("Today at {time}", { time });
  const y = new Date(today);
  y.setDate(today.getDate() - 1);
  if (d.toDateString() === y.toDateString()) return t("Yesterday at {time}", { time });
  return t("{date} at {time}", { date: d.toLocaleDateString(locale, { month: "short", day: "numeric" }), time });
}

export function renderDiff(d: DiffSummary, limit = 60): HTMLElement {
  const list = h("div", { class: "review-list" });
  if (!d.entries.length) {
    list.append(h("div", { class: "empty-note" }, t("No text changes. (Formatting-only changes aren't listed yet.)")));
    return list;
  }
  for (const e of d.entries.slice(0, limit)) {
    const item = h("div", { class: "review-item" });
    if (e.kind === "added") item.innerHTML = `<span class="tag add">${escapeHtml(t("Added"))}</span><span><ins>${escapeHtml(e.after ?? "") || "¶"}</ins></span>`;
    else if (e.kind === "removed") item.innerHTML = `<span class="tag del">${escapeHtml(t("Removed"))}</span><span><del>${escapeHtml(e.before ?? "") || "¶"}</del></span>`;
    else item.innerHTML = `<span class="tag mod">${escapeHtml(t("Edited"))}</span><span>${wordDiff(e.before ?? "", e.after ?? "")}</span>`;
    list.append(item);
  }
  if (d.entries.length > limit) list.append(h("div", { class: "empty-note" }, tn(d.entries.length - limit, "…and {n} more", "…and {n} more")));
  return list;
}

export class HistoryPanel {
  el: HTMLElement;
  private selected: number | null = null;
  private mode: "current" | "previous" = "current";

  constructor(private host: HistoryHost) {
    this.el = h("div", { class: "history" });
  }

  async render() {
    const key = this.host.docKey();
    const mark = h("button", { class: "tb", type: "button", title: t("Name this version"), "aria-label": t("Name this version") }, svg(I.plus));
    mark.addEventListener("click", async () => {
      const label = await promptDialog({ title: t("Name this version"), label: t("Name"), value: "", ok: t("Save version") });
      if (label) {
        await this.host.nameCurrent(label);
        await this.render();
      }
    });
    const head = h("div", { class: "panel-title" }, h("span", {}, t("Versions")), key ? mark : null);
    if (!key) {
      this.el.replaceChildren(head, h("div", { class: "empty-note" }, t("Open a document to see its history.")));
      return;
    }
    const versions = await listVersions(key);
    if (!versions.length) {
      this.el.replaceChildren(head, h("div", { class: "empty-note" }, t("Versions are recorded when you save, every few minutes while you edit, and around every AI change.")));
      return;
    }
    const list = h("div", {});
    versions.forEach((v, i) => {
      const prev = versions[i + 1];
      const row = h(
        "div",
        { class: `ver ${v.kind === "ai" ? "ai" : v.kind === "named" ? "named" : ""}${this.selected === v.id ? " sel" : ""}`, role: "button", tabindex: "0" },
        h("span", { class: "pin" }),
        h("span", { class: "what", title: v.label }, v.label ? t(v.label) : t(KIND_LABEL[v.kind])),
        h("span", { class: "when" }, v.by === "You" ? tn(v.words, "{when}, by you, {n} word", "{when}, by you, {n} words", { when: when(v.at) }) : tn(v.words, "{when}, by {who}, {n} word", "{when}, by {who}, {n} words", { when: when(v.at), who: v.by })),
      );
      const toggle = () => {
        this.selected = this.selected === v.id ? null : v.id!;
        void this.render();
      };
      row.addEventListener("click", toggle);
      row.addEventListener("keydown", (e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), toggle()));
      list.append(row);
      if (this.selected === v.id) list.append(this.detail(v, prev));
    });
    this.el.replaceChildren(head, list);
  }

  private detail(v: Version, prev?: Version): HTMLElement {
    const box = h("div", { class: "ver-detail" });
    const seg = h("div", { class: "seg" });
    const modes: ["current" | "previous", string][] = [
      ["current", t("vs. now")],
      ["previous", t("vs. previous")],
    ];
    for (const [m, label] of modes) {
      const b = h("button", { type: "button", class: this.mode === m ? "on" : "", disabled: m === "previous" && !prev }, label);
      b.addEventListener("click", (e) => {
        e.stopPropagation();
        this.mode = m;
        void this.render();
      });
      seg.append(b);
    }
    const summary = h("div", { class: "readout", style: "margin:8px 0 6px;color:var(--muted)" }, t("Comparing…"));
    const diffBox = h("div", {});
    const restore = h("button", { class: "btn", type: "button" }, svg(I.undo), t("Restore"));
    const rename = h("button", { class: "btn ghost", type: "button" }, t("Name…"));
    const dl = h("button", { class: "btn ghost icon-btn", type: "button", title: t("Download this version"), "aria-label": t("Download this version") }, svg(I.download));
    box.append(seg, summary, diffBox, h("div", { class: "row" }, restore, rename, dl));
    box.addEventListener("click", (e) => e.stopPropagation());
    void (async () => {
      const bytes = await versionBytes(v.id!);
      if (!bytes) {
        summary.textContent = t("Version data is missing.");
        return;
      }
      let d: DiffSummary | null = null;
      if (this.mode === "previous" && prev) {
        const pb = await versionBytes(prev.id!);
        d = pb ? this.host.diffBetween(pb, bytes) : null;
        summary.textContent = d ? t("{changed} edited, {added} added and {removed} removed since the version before", { changed: d.changed, added: d.added, removed: d.removed }) : "";
      } else {
        d = this.host.diffAgainstCurrent(bytes);
        summary.textContent = d ? t("{changed} edited, {added} added and {removed} removed since this version", { changed: d.changed, added: d.added, removed: d.removed }) : "";
      }
      if (d) diffBox.replaceChildren(renderDiff(d));
      restore.addEventListener("click", async () => {
        if (await this.host.ask("history.restore")) {
          await this.host.restore(v, bytes);
          this.selected = null;
          await this.render();
        }
      });
      rename.addEventListener("click", async () => {
        const label = await promptDialog({ title: t("Name version"), label: t("Name"), value: v.label, ok: t("Save") });
        if (label) {
          await renameVersion(v, label);
          await this.render();
        }
      });
      dl.addEventListener("click", () => this.host.download(v, bytes));
    })();
    return box;
  }
}
