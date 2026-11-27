import { h, I, svg, t, type ExtensionHost, type Panel } from "@docxtor/editor";
import { storage, store } from "../storage";

interface Mounted {
  panel: Panel;
  el: HTMLElement;
  cleanup?: () => void;
}

/**
 * Left navigator: one tab per panel (Files, Outline and History are built in; extensions add more).
 * A tool panel (the accessibility check, the dictionary) has a tab, with a close button, only while
 * someone has it open.
 */
export class Sidebar {
  el: HTMLElement;
  private bar: HTMLElement;
  private body: HTMLElement;
  private mounted = new Map<string, Mounted>();
  /** Tool panels that have a tab right now. */
  private opened = new Set<string>();
  tab: string;

  constructor(private host: ExtensionHost) {
    const saved = storage("docxtor.sidebar", { tab: "files", open: window.innerWidth >= 1100 });
    this.tab = saved.tab;
    this.bar = h("div", { class: "tabs", role: "tablist", "aria-label": t("Navigator views") });
    this.body = h("div", { class: "tab-body", role: "tabpanel" });
    this.el = h("aside", { class: `sidebar${saved.open ? "" : " closed"}`, "aria-label": t("Navigator") }, this.bar, this.body);
    host.panels.onChange(() => this.render());
    this.render();
  }

  /** The panels with a tab: the views, and the tools someone opened. */
  private panels(): Panel[] {
    return [...this.host.panels.list()].filter((p) => !p.tool || this.opened.has(p.id)).sort((a, b) => (a.order ?? 100) - (b.order ?? 100));
  }

  private render() {
    // Release panels whose extension went away.
    const all = this.host.panels.list();
    for (const [id, m] of this.mounted) {
      if (all.includes(m.panel)) continue;
      m.cleanup?.();
      m.el.remove();
      this.mounted.delete(id);
      this.opened.delete(id);
    }
    const panels = this.renderBar();
    this.show(panels.some((p) => p.id === this.tab) ? this.tab : (panels[0]?.id ?? ""), false);
  }

  private renderBar(): Panel[] {
    const panels = this.panels();
    this.bar.replaceChildren(
      ...panels.map((p) => {
        const b = h("button", { class: "tab", type: "button", role: "tab", id: `tab-${p.id}`, "aria-controls": "navigator-view" }, p.title);
        b.addEventListener("click", () => this.show(p.id));
        b.addEventListener("keydown", (e) => {
          if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
          const i = panels.indexOf(p) + (e.key === "ArrowRight" ? 1 : -1);
          const next = panels[(i + panels.length) % panels.length];
          this.show(next.id);
          this.bar.querySelector<HTMLElement>(`#tab-${CSS.escape(next.id)}`)?.focus();
        });
        if (!p.tool) return b;
        // A tool's tab can be closed. The tab list may hold only tabs, so the close mark is a picture on the tab for the pointer
        // (hidden from screen readers) and the keyboard closes the tab with Delete, as the ARIA pattern for tabs says.
        const label = t("Close {panel}", { panel: p.title });
        const close = h("span", { class: "tab-close", title: label, "aria-hidden": "true" }, svg(I.close));
        close.addEventListener("click", (e) => {
          e.stopPropagation();
          this.close(p.id);
        });
        b.classList.add("closable");
        b.setAttribute("aria-keyshortcuts", "Delete");
        b.addEventListener("keydown", (e) => {
          if (e.key !== "Delete") return;
          e.preventDefault();
          this.close(p.id);
        });
        b.append(close);
        return b;
      }),
    );
    this.body.id = "navigator-view";
    return panels;
  }

  /** Take a tool's tab away; the first view comes back if it was showing. */
  close(id: string) {
    if (!this.opened.delete(id)) return;
    const was = this.tab === id;
    const panels = this.renderBar();
    this.show(was ? (panels[0]?.id ?? "") : this.tab, false);
  }

  show(id: string, notify = true) {
    // A tool gets its tab when it is first shown.
    if (!this.opened.has(id) && this.host.panels.list().some((p) => p.id === id && p.tool)) {
      this.opened.add(id);
      this.renderBar();
    }
    const panel = this.panels().find((p) => p.id === id);
    if (!panel) {
      this.body.replaceChildren();
      return;
    }
    this.tab = id;
    for (const b of this.bar.querySelectorAll<HTMLElement>(".tab")) {
      const on = b.id === `tab-${id}`;
      b.classList.toggle("on", on);
      b.setAttribute("aria-selected", String(on));
      b.tabIndex = on ? 0 : -1;
      if (on) (b.closest(".tab-group") ?? b).scrollIntoView?.({ block: "nearest", inline: "nearest" });
    }
    let m = this.mounted.get(id);
    if (!m || m.panel !== panel) {
      m?.cleanup?.();
      const el = h("div", { class: "panel-view" });
      const cleanup = panel.render(el) ?? undefined;
      m = { panel, el, cleanup };
      this.mounted.set(id, m);
    }
    if (this.body.firstChild !== m.el) this.body.replaceChildren(m.el);
    this.body.setAttribute("aria-labelledby", `tab-${id}`);
    this.persist();
    if (notify) panel.onShow?.();
  }

  get open() {
    return !this.el.classList.contains("closed");
  }

  toggle(open = !this.open) {
    this.el.classList.toggle("closed", !open);
    this.persist();
  }

  private persist() {
    store("docxtor.sidebar", { tab: this.tab, open: this.open });
  }
}
