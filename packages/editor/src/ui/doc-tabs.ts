// Document tabs in the navigation pane, as in Google Docs: the tabs with their emoji, nested up to three levels, the open
// one marked; Add tab; and a menu on each (Rename, Choose emoji, Add subtab, Duplicate, Move up, Move down, Nest in the
// tab above, Move out, Copy link to tab, Delete). Every change is a `tab` op, so it undoes; showing a tab is not an op: each pane of a split keeps its own.
// It is a tree for the keyboard: up and down move, Enter opens, the context menu key opens the menu.
import type { TabInfo } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, popover, promptDialog, toast } from "./dom";
import { I } from "./icons";

/** The most characters a tab's name may have (the engine refuses more). */
const TAB_NAME_MAX = 100;

const EMOJI = ["📄", "📝", "📌", "📎", "📊", "📈", "📅", "✅", "⭐", "💡", "🔖", "📚", "🧭", "🎯", "🛠️", "🧪", "💬", "📣", "🔒", "🌍", "🏁", "❤️", "⚠️", "❓"];

export class DocTabs {
  readonly el: HTMLElement;
  private list: HTMLElement;

  private get ed(): Editor {
    return this.pane();
  }

  /** `pane` gives the pane that has the keyboard: each pane of a split shows its own tab, and the list is that pane's. */
  constructor(private pane: () => Editor) {
    const add = h("button", { class: "btn sm doc-tab-add", type: "button", title: t("Add tab") }, h("span", { html: I.plus, "aria-hidden": "true" }), t("Add tab"));
    add.addEventListener("click", () => void this.add());
    this.list = h("div", { class: "doc-tabs", role: "tree", "aria-label": t("Document tabs") });
    this.el = h("section", { class: "doc-tabs-pane" }, h("div", { class: "doc-tabs-head" }, h("span", { class: "doc-tabs-title" }, t("Document tabs")), add), this.list);
    this.list.addEventListener("keydown", (e) => this.key(e));
  }

  /** Draw the list again (the tabs or the open one changed). */
  refresh() {
    const { tabs, active } = this.ed.tabs;
    const had = this.list.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset.tab : undefined;
    this.list.replaceChildren();
    // A tree needs items: with none to show the list is a plain box with a note in it.
    if (tabs.length < 2) {
      this.list.removeAttribute("role");
      this.list.removeAttribute("aria-label");
      this.list.append(h("p", { class: "doc-tabs-empty" }, t("Add a tab to split the document into parts you can switch between.")));
      return;
    }
    this.list.setAttribute("role", "tree");
    this.list.setAttribute("aria-label", t("Document tabs"));
    for (const tab of tabs) {
      const on = tab.id === (active ?? tabs[0].id);
      const more = h("button", { class: "tb doc-tab-more", type: "button", "aria-label": t("Options for {name}", { name: tab.name }), tabindex: "-1", html: I.more });
      more.addEventListener("click", (e) => {
        e.stopPropagation();
        this.menu(tab, more);
      });
      const item = h(
        "div",
        { class: on ? "doc-tab on" : "doc-tab", role: "treeitem", "aria-selected": String(on), "aria-level": String(tab.depth + 1), tabindex: on ? "0" : "-1", "data-tab": tab.id, style: `--depth:${tab.depth}` },
        h("span", { class: "doc-tab-icon", "aria-hidden": "true" }, tab.icon ?? ""),
        h("span", { class: "doc-tab-name" }, tab.name),
        more,
      );
      item.addEventListener("click", () => this.open(tab.id));
      item.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        this.menu(tab, more);
      });
      this.list.append(item);
    }
    if (had) this.list.querySelector<HTMLElement>(`[data-tab="${CSS.escape(had)}"]`)?.focus();
  }

  private open(id: string) {
    // Showing a tab is not an edit: it works in a read-only document too.
    this.ed.openTab(id);
    const tab = this.ed.tabs.tabs.find((x) => x.id === id);
    if (tab) this.ed.select({ p: this.firstParagraph(tab.ids[0] ?? tab.start), o: 0 });
    this.ed.focus();
  }

  private firstParagraph(block: number): number {
    const el = this.ed.root.querySelector<HTMLElement>(`[data-id="${block}"]`);
    const p = el?.dataset.p ?? el?.querySelector<HTMLElement>("[data-p]")?.dataset.p;
    return p ? Number(p) : block;
  }

  private async add(parent?: string) {
    const name = await promptDialog({ title: parent ? t("Add subtab") : t("Add tab"), label: t("Tab name"), value: t("Untitled tab"), ok: t("Add") });
    if (!name) return this.ed.focus();
    const first = this.ed.tabs.tabs.length ? {} : { first_name: t("Tab {n}", { n: 1 }) };
    this.ed.exec({ op: "tab", action: "add", name, ...(parent ? { parent } : {}), ...first });
    const tab = this.ed.tabs.tabs.find((x) => x.id === this.ed.tabs.active);
    if (tab) this.ed.select({ p: this.firstParagraph(tab.ids[0] ?? tab.start), o: 0 });
    this.ed.focus();
  }

  private menu(tab: TabInfo, anchor: HTMLElement) {
    const { tabs } = this.ed.tabs;
    const i = tabs.indexOf(tab);
    const siblings = tabs.filter((x) => x.parent === tab.parent);
    const k = siblings.indexOf(tab);
    const above = siblings[k - 1];
    const below = siblings[k + 1];
    const afterBelow = siblings[k + 2];
    const item = (label: string, run: () => unknown, disabled = false) => {
      const b = h("button", { class: "menu-item", type: "button", role: "menuitem", disabled }, h("span", { class: "menu-label" }, label));
      b.addEventListener("click", () => {
        close();
        void run();
      });
      return b;
    };
    const move = (parent: string | undefined, before: string | undefined) => this.ed.exec({ op: "tab", action: "move", id: tab.id, ...(parent ? { parent } : {}), ...(before ? { before } : {}) });
    const parentTab = tabs.find((x) => x.id === tab.parent);
    const content = h(
      "div",
      { class: "menu doc-tab-menu" },
      item(t("Rename…"), async () => {
        const name = await promptDialog({ title: t("Rename tab"), label: t("Tab name"), value: tab.name, ok: t("Rename") });
        if (name) this.ed.exec({ op: "tab", action: "rename", id: tab.id, name });
      }),
      item(t("Choose emoji…"), () => this.emoji(tab, anchor)),
      item(t("Add subtab…"), () => this.add(tab.id), tab.depth >= 2),
      item(t("Duplicate"), () => this.ed.exec({ op: "tab", action: "duplicate", id: tab.id, name: [...t("Copy of {name}", { name: tab.name })].slice(0, TAB_NAME_MAX).join("") })),
      item(t("Move up"), () => move(tab.parent, above?.id), !above),
      item(t("Move down"), () => move(tab.parent, afterBelow?.id), !below),
      item(t("Nest in the tab above"), () => move(above?.id, undefined), !above || tab.depth >= 2),
      item(t("Move out"), () => move(parentTab?.parent, tabs.find((x, j) => j > i && x.depth < tab.depth && x.parent === parentTab?.parent)?.id), !parentTab),
      item(t("Copy link to tab"), () => {
        void navigator.clipboard?.writeText(`#_DocxtorTab_${tab.id}`).then(() => toast(t("Link copied")));
      }),
      item(t("Delete"), async () => {
        if (tabs.length < 2) return;
        void this.ed.command("tabs.delete", tab.id, tab.name);
      }),
    );
    const close = popover(anchor, content, { align: "right" });
    (content.querySelector("button:not([disabled])") as HTMLElement | null)?.focus();
  }

  private emoji(tab: TabInfo, anchor: HTMLElement) {
    const grid = h("div", { class: "doc-tab-emoji", role: "listbox", "aria-label": t("Choose emoji") });
    let close = () => {};
    const pick = (icon: string | null) => {
      close();
      this.ed.exec({ op: "tab", action: "set_icon", id: tab.id, ...(icon ? { icon } : {}) });
    };
    for (const e of EMOJI) {
      const b = h("button", { class: "tb", type: "button", role: "option", "aria-label": e }, e);
      b.addEventListener("click", () => pick(e));
      grid.append(b);
    }
    const none = h("button", { class: "btn sm", type: "button" }, t("No emoji"));
    none.addEventListener("click", () => pick(null));
    close = popover(anchor, h("div", {}, grid, none), { align: "right" });
    (grid.querySelector("button") as HTMLElement | null)?.focus();
  }

  private key(e: KeyboardEvent) {
    const items = [...this.list.querySelectorAll<HTMLElement>(".doc-tab")];
    const i = items.indexOf(document.activeElement as HTMLElement);
    if (i < 0) return;
    const go = (k: number) => {
      const to = items[Math.max(0, Math.min(items.length - 1, k))];
      items.forEach((x) => (x.tabIndex = -1));
      to.tabIndex = 0;
      to.focus();
    };
    if (e.key === "ArrowDown") go(i + 1);
    else if (e.key === "ArrowUp") go(i - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(items.length - 1);
    else if (e.key === "Enter" || e.key === " ") this.open(items[i].dataset.tab!);
    else if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
      const tab = this.ed.tabs.tabs.find((x) => x.id === items[i].dataset.tab);
      if (tab) this.menu(tab, items[i]);
    } else return;
    e.preventDefault();
  }
}
