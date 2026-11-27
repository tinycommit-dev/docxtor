// The menu bar: File, Edit, View, Insert, Design, Format, Tools, Extensions, Help. Every line is a command
// (or a submenu), so extensions add to any menu with `ctx.menus.register` and the menu, the command
// palette and the shortcut always agree. Menus keep their shape: commands that don't apply right now
// are greyed out rather than hidden. On narrow windows the bar folds into one "Menu" button.
//
// Keyboard: Tab to the bar, ←/→ between menus, ↓ or Enter to open, ↑/↓ through items, → into a
// submenu, ← back out, Esc to close (one level at a time, with the mouse too).
import { msg, t, type ExtensionHost, type MenuEntry, type MenuId } from "@docxtor/extensions";
import { coalesce, h, overlayLayer, svg, toast } from "./dom";
import { I } from "./icons";

export const MENU_BAR: { id: MenuId; title: string }[] = [
  { id: "file", title: msg("File") },
  { id: "edit", title: msg("Edit") },
  { id: "view", title: msg("View") },
  { id: "insert", title: msg("Insert") },
  { id: "design", title: msg("Design") },
  { id: "format", title: msg("Format") },
  { id: "mailings", title: msg("Mailings") },
  { id: "tools", title: msg("Tools") },
  { id: "extensions", title: msg("Extensions") },
  { id: "help", title: msg("Help") },
];

/** The icon slot at the start of a menu line: the item's icon, or a check mark when it's on. */
export function menuIcon(host: ExtensionHost, icon: string | undefined, checked: boolean): HTMLElement {
  const markup = checked ? (host.icon("check") ?? I.check) : icon;
  return h("span", { class: `menu-icon${checked ? " on" : ""}`, "aria-hidden": "true" }, markup ? svg(markup) : null);
}

/** The folded bar's single menu: one submenu per menu of the bar. */
const ROOT = "docxtor.menubar";

interface Panel {
  el: HTMLElement;
  /** The entry that opened it (submenus), or the bar button (top menus). */
  opener: HTMLElement;
  menu: string;
}

export interface MenuBarOptions {
  /** Give the document its focus back before a command runs (commands read its selection). */
  focusDocument?: () => void;
}

export class MenuBar {
  readonly el: HTMLElement;
  private bar: HTMLElement;
  private panels: Panel[] = [];
  private hoverTimer = 0;
  private cleanup: (() => void)[] = [];

  constructor(
    private host: ExtensionHost,
    private opts: MenuBarOptions = {},
  ) {
    this.bar = h("div", { class: "menubar-items" });
    this.el = h("nav", { class: "menubar", role: "menubar", "aria-label": t("Menu bar") }, this.bar);
    const refresh = coalesce(() => this.render());
    for (const sub of [host.menus.onChange(refresh), host.commands.onChange(refresh)]) this.cleanup.push(() => sub.dispose());
    this.render();
  }

  destroy() {
    this.close();
    this.cleanup.splice(0).forEach((f) => f());
    this.el.remove();
  }

  private tops(): { id: string; title: string }[] {
    return MENU_BAR.filter((m) => this.host.hasMenu(m.id)).map((m) => ({ id: m.id, title: t(m.title) }));
  }

  private render() {
    if (this.panels.length) return; // don't rebuild under an open menu
    const tops = this.tops();
    const buttons = tops.map((m, i) => this.topButton(m.id, m.title, i === 0));
    const folded = h("button", { class: "menubar-top menubar-fold", type: "button", "aria-haspopup": "menu", "aria-expanded": "false", tabindex: "-1" }, svg(I.menu), h("span", {}, t("Menu")));
    folded.addEventListener("mousedown", (e) => e.preventDefault());
    folded.addEventListener("click", () => (this.panels.length ? this.close() : this.open(ROOT, folded, false)));
    folded.addEventListener("keydown", (e) => {
      if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        this.open(ROOT, folded, true);
      }
    });
    this.bar.replaceChildren(...(buttons.length ? [folded, ...buttons] : []));
  }

  private topButton(id: string, title: string, first: boolean): HTMLElement {
    const b = h("button", { class: "menubar-top", type: "button", role: "menuitem", "aria-haspopup": "menu", "aria-expanded": "false", tabindex: first ? "0" : "-1", "data-menu": id }, title);
    // Keep the document's selection while the menus are used with the mouse.
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => {
      const isOpen = this.panels[0]?.opener === b;
      this.close();
      if (!isOpen) this.open(id, b, false);
    });
    // With a menu open, pointing at another title opens that one instead.
    b.addEventListener("mouseenter", () => {
      if (this.panels.length && this.panels[0].opener !== b && this.panels[0].opener.classList.contains("menubar-top")) {
        this.close();
        this.open(id, b, false);
      }
    });
    b.addEventListener("keydown", (e) => {
      const tops = this.topButtons();
      const i = tops.indexOf(b);
      if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
        e.preventDefault();
        this.focusTop(tops[(i + (e.key === "ArrowRight" ? 1 : -1) + tops.length) % tops.length]);
      } else if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        this.close();
        this.open(id, b, true);
      }
    });
    return b;
  }

  private topButtons(): HTMLElement[] {
    return Array.from(this.bar.querySelectorAll<HTMLElement>(".menubar-top:not(.menubar-fold)"));
  }

  private focusTop(b: HTMLElement | undefined) {
    if (!b) return;
    for (const x of this.topButtons()) x.tabIndex = x === b ? 0 : -1;
    b.focus();
  }

  /** The entries of a menu; the folded bar's root lists the bar's menus as submenus. */
  private entries(menu: string): MenuEntry[][] {
    if (menu !== ROOT) return this.host.menu(menu);
    return [this.tops().map((m) => ({ item: { menu: ROOT, submenu: m.id, title: m.title }, title: m.title, shortcut: "", enabled: true, checked: false }))];
  }

  private open(menu: string, opener: HTMLElement, keyboard: boolean) {
    const groups = this.entries(menu);
    if (!groups.length) return;
    const level = this.panels.length;
    const el = h("div", { class: "pop menu-drop", role: "menu", "aria-label": opener.textContent ?? "" });
    el.addEventListener("mousedown", (e) => e.preventDefault());
    // Another popover opening closes the menus.
    el.addEventListener("close", () => this.close());
    groups.forEach((group, gi) => {
      if (gi) el.append(h("div", { class: "menu-sep", role: "separator" }));
      for (const entry of group) el.append(this.itemEl(entry, level));
    });
    overlayLayer().append(el);
    this.panels.push({ el, opener, menu });
    opener.setAttribute("aria-expanded", "true");
    opener.classList.add("open");
    this.place(el, opener, level > 0);
    el.addEventListener("keydown", (e) => this.onKey(e, level));
    if (level === 0) this.listenOutside();
    if (keyboard) this.items(el)[0]?.focus();
  }

  // Every line has an icon slot, so labels line up in every menu; a setting that's on shows a check
  // mark in place of its icon, as in Google Docs.
  private itemEl(entry: MenuEntry, level: number): HTMLElement {
    const sub = entry.item.submenu;
    const toggle = !!entry.command?.toggled;
    const b = h(
      "button",
      {
        class: "menu-item",
        type: "button",
        role: toggle ? "menuitemcheckbox" : "menuitem",
        disabled: !entry.enabled,
        "aria-checked": toggle ? String(entry.checked) : null,
        "aria-haspopup": sub ? "menu" : null,
        "aria-expanded": sub ? "false" : null,
        tabindex: "-1",
      },
      menuIcon(this.host, entry.icon, entry.checked),
      h("span", { class: "menu-label" }, entry.title),
      sub ? h("span", { class: "menu-sub", "aria-hidden": "true" }, svg(I.chevron)) : entry.shortcut ? h("span", { class: "hint" }, entry.shortcut) : null,
    );
    if (sub) {
      const openSub = (keyboard: boolean) => {
        if (this.panels[level + 1]?.opener === b) return;
        this.closeFrom(level + 1);
        this.open(sub, b, keyboard);
      };
      b.addEventListener("click", () => openSub(false));
      b.addEventListener("mouseenter", () => {
        clearTimeout(this.hoverTimer);
        this.hoverTimer = window.setTimeout(() => openSub(false), 120);
      });
      (b as HTMLElement & { openSub?: (k: boolean) => void }).openSub = openSub;
    } else {
      b.addEventListener("mouseenter", () => {
        clearTimeout(this.hoverTimer);
        this.hoverTimer = window.setTimeout(() => this.closeFrom(level + 1), 120);
      });
      b.addEventListener("click", () => {
        this.close();
        this.opts.focusDocument?.();
        this.host.run(entry).catch((err) => toast(String((err as Error)?.message ?? err)));
      });
    }
    return b;
  }

  private items(el: HTMLElement): HTMLButtonElement[] {
    return Array.from(el.querySelectorAll<HTMLButtonElement>(".menu-item:not(:disabled)"));
  }

  private onKey(e: KeyboardEvent, level: number) {
    const panel = this.panels[level];
    if (!panel) return;
    const items = this.items(panel.el);
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const go = (n: number) => items[(n + items.length) % items.length]?.focus();
    const current = items[i] as (HTMLButtonElement & { openSub?: (k: boolean) => void }) | undefined;
    const stop = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    switch (e.key) {
      case "ArrowDown":
        stop();
        go(i + 1);
        break;
      case "ArrowUp":
        stop();
        go(i < 0 ? -1 : i - 1);
        break;
      case "Home":
        stop();
        go(0);
        break;
      case "End":
        stop();
        go(-1);
        break;
      case "ArrowRight":
        stop();
        if (current?.openSub) current.openSub(true);
        else this.stepTop(1);
        break;
      case "ArrowLeft":
        stop();
        if (level > 0) {
          this.closeFrom(level);
          panel.opener.focus();
        } else this.stepTop(-1);
        break;
      case "Tab":
        e.preventDefault();
        break;
    }
  }

  /** ←/→ in an open top menu: open the next one along the bar. */
  private stepTop(d: number) {
    const opener = this.panels[0]?.opener;
    const tops = this.topButtons();
    const i = opener ? tops.indexOf(opener) : -1;
    if (i < 0) return;
    const next = tops[(i + d + tops.length) % tops.length];
    this.close();
    this.focusTop(next);
    this.open(next.dataset.menu!, next, true);
  }

  private place(el: HTMLElement, opener: HTMLElement, beside: boolean) {
    const r = opener.getBoundingClientRect();
    const w = el.offsetWidth;
    const hgt = el.offsetHeight;
    let left = beside ? r.right + 2 : r.left;
    let top = beside ? r.top - 4 : r.bottom + 4;
    if (left + w > window.innerWidth - 8) left = beside ? Math.max(8, r.left - w - 2) : Math.max(8, window.innerWidth - w - 8);
    if (top + hgt > window.innerHeight - 8) top = Math.max(8, window.innerHeight - hgt - 8);
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
  }

  private outside = (e: Event) => {
    const target = e.target as Node;
    if (this.panels.some((p) => p.el.contains(target)) || this.bar.contains(target)) return;
    this.close();
  };

  /**
   * Escape closes the innermost open panel, whether or not a menu holds the focus (a menu opened
   * with the mouse leaves it in the document). Listening on the document before the editor does
   * means the editor never sees the key. The focus goes back to the opener only when it was in the
   * menus: after a mouse-opened menu it stays where it was.
   */
  private onEscape = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || !this.panels.length) return;
    e.preventDefault();
    e.stopPropagation();
    const focusInMenu = this.panels.some((p) => p.el.contains(document.activeElement));
    const level = this.panels.length - 1;
    const opener = this.panels[level].opener;
    if (level > 0) this.closeFrom(level);
    else this.close();
    if (focusInMenu) opener.focus();
  };

  private listenOutside() {
    document.addEventListener("keydown", this.onEscape, true);
    document.addEventListener("mousedown", this.outside, true);
    window.addEventListener("blur", this.onBlur);
    window.addEventListener("resize", this.onBlur);
  }

  private onBlur = () => this.close();

  private closeFrom(level: number) {
    for (const p of this.panels.splice(level)) {
      p.el.remove();
      p.opener.setAttribute("aria-expanded", "false");
      p.opener.classList.remove("open");
    }
  }

  close() {
    clearTimeout(this.hoverTimer);
    if (!this.panels.length) return;
    this.closeFrom(0);
    document.removeEventListener("keydown", this.onEscape, true);
    document.removeEventListener("mousedown", this.outside, true);
    window.removeEventListener("blur", this.onBlur);
    window.removeEventListener("resize", this.onBlur);
  }
}
