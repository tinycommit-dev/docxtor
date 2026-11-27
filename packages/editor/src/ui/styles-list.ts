// The Styles panel and Manage styles: every style of the document, each drawn the way it looks, to show in use or all,
// to sort in Word's recommended order or alphabetically, and to apply, modify, delete, select all instances of and clear.
// One list serves both the navigator panel (a click applies a style, as in Word's Styles pane) and the Manage styles
// dialog (a click picks a style for the buttons under the list).
import type { StyleLook } from "@docxtor/core";
import { t, tn, type ExtensionHost } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal, onChoose, svg } from "./dom";
import { I } from "./icons";
import { applyStyleLook, openStyleMenu } from "./style-gallery";
import { styleTitle, tileCss } from "./style-preview";

type Show = "use" | "gallery" | "all";
type Sort = "recommended" | "alphabetical";

export class StylesList {
  readonly el: HTMLElement;
  private list = h("div", { class: "styles-list", role: "listbox", "aria-label": t("Styles") });
  private show: Show = "gallery";
  private sort: Sort = "recommended";
  private query = "";
  private looks: StyleLook[] = [];
  private chosen: string | null = null;
  private onPick: (look: StyleLook | null) => void = () => undefined;

  constructor(
    private ed: Editor,
    private host: ExtensionHost,
    private mode: "panel" | "dialog",
  ) {
    const showSel = h("select", { id: `sl-show-${mode}`, "aria-label": t("Show") }, h("option", { value: "gallery" }, t("In the style gallery")), h("option", { value: "use" }, t("In use")), h("option", { value: "all" }, t("All styles")));
    const sortSel = h("select", { id: `sl-sort-${mode}`, "aria-label": t("Sort") }, h("option", { value: "recommended" }, t("Recommended order")), h("option", { value: "alphabetical" }, t("A to Z")));
    const search = h("input", { type: "search", placeholder: t("Search styles"), "aria-label": t("Search styles"), autocomplete: "off", spellcheck: "false" });
    showSel.addEventListener("change", () => {
      this.show = showSel.value as Show;
      this.render();
    });
    sortSel.addEventListener("change", () => {
      this.sort = sortSel.value as Sort;
      this.render();
    });
    search.addEventListener("input", () => {
      this.query = search.value.trim().toLowerCase();
      this.render();
    });
    const foot = h("div", { class: "styles-foot" });
    if (mode === "panel") {
      for (const [command, label] of [["format.newStyle", t("New style…")], ["format.manageStyles", t("Manage styles…")]] as const) {
        const b = h("button", { class: "btn", type: "button" }, label);
        b.addEventListener("click", () => void this.host.execute(command).then(() => this.update(), () => undefined));
        foot.append(b);
      }
    }
    this.el = h("div", { class: `styles-pane styles-${mode}` }, h("div", { class: "styles-bar" }, search, h("div", { class: "styles-filters" }, showSel, sortSel)), this.list, mode === "panel" ? foot : null);
    this.list.addEventListener("keydown", (ev) => this.keys(ev));
  }

  /** Called when the chosen style changes (Manage styles uses it to enable its buttons). */
  onChoose(fn: (look: StyleLook | null) => void) {
    this.onPick = fn;
  }

  /** The style picked in the list (Manage styles). */
  get choice(): StyleLook | null {
    return this.looks.find((l) => l.id === this.chosen) ?? null;
  }

  /** Read the document's styles again and draw the list. */
  update() {
    const engine = this.ed.engine;
    this.looks = engine ? engine.styleGallery().filter((l) => l.kind === "paragraph" || l.kind === "character") : [];
    if (this.chosen && !this.looks.some((l) => l.id === this.chosen)) this.chosen = null;
    this.render();
  }

  private visible(): StyleLook[] {
    const cur = this.ed.format();
    const here = new Set([cur?.style ?? "Normal", cur?.run.style ?? ""]);
    let rows = this.looks.filter((l) => {
      if (this.show === "all") return true;
      if (this.show === "use") return l.uses > 0 || here.has(l.id);
      return !l.latent && (l.quick || here.has(l.id));
    });
    if (this.query) rows = rows.filter((l) => styleTitle(l.name, l.custom).toLowerCase().includes(this.query));
    if (this.sort === "alphabetical") {
      const collator = new Intl.Collator(undefined, { sensitivity: "base" });
      rows = [...rows].sort((a, b) => collator.compare(styleTitle(a.name, a.custom), styleTitle(b.name, b.custom)));
    } else {
      const rank = (l: StyleLook) => (l.kind === "paragraph" && l.id === "Normal" ? -1 : l.priority);
      const order = new Map(this.looks.map((l, i) => [l.id, i]));
      rows = [...rows].sort((a, b) => Number(a.latent) - Number(b.latent) || rank(a) - rank(b) || (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0));
    }
    return rows;
  }

  private render() {
    const cur = this.ed.format();
    const rows = this.visible();
    const hadFocus = this.list.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset.style : undefined;
    this.list.replaceChildren();
    if (!rows.length) this.list.append(h("p", { class: "empty-note" }, this.query ? t("No style matches.") : t("No styles to show.")));
    let first = true;
    for (const look of rows) {
      const name = styleTitle(look.name, look.custom);
      const here = look.kind === "character" ? look.id === cur?.run.style : look.id === (cur?.style ?? "Normal");
      const picked = this.mode === "dialog" ? look.id === this.chosen : here;
      const sample = h("span", { class: "style-sample", style: tileCss(look.run, look.para, { min: 11, max: 17 }) }, name);
      const main = h("button", { class: "styles-main", type: "button", role: "option", "aria-selected": String(picked), "data-style": look.id, tabindex: first || picked ? 0 : -1 }, sample);
      first = false;
      const meta = h("span", { class: "styles-meta" }, h("span", { class: "styles-kind", title: look.kind === "character" ? t("Character style") : t("Paragraph style") }, look.kind === "character" ? "a" : "¶"), look.uses ? h("span", { class: "styles-count", title: tn(look.uses, "{n} use", "{n} uses") }, String(look.uses)) : null);
      // The list holds only options; this key is for the pointer. The keyboard opens the same menu from the option itself (the Menu key or Shift+F10).
      const more = h("button", { class: "styles-more", type: "button", tabindex: "-1", "aria-hidden": "true", title: t("Options for {style}", { style: name }) }, svg(this.host.icon("chevron") ?? I.chevron));
      const row = h("div", { class: `styles-row${picked ? " on" : ""}${look.latent ? " latent" : ""}`, "data-row": look.id }, main, meta, more);
      onChoose(
        main,
        () => {
          if (this.mode === "dialog") {
            this.chosen = look.id;
            this.render();
            this.onPick(look);
            this.list.querySelector<HTMLElement>(`[data-style="${CSS.escape(look.id)}"]`)?.focus();
          } else {
            applyStyleLook(this.ed, look);
            this.ed.focus();
            this.update();
          }
        },
        () => {
          if (this.mode === "dialog") void this.host.execute("format.modifyStyle", look.id).catch(() => undefined);
        },
      );
      const menu = (at: { x: number; y: number }) => openStyleMenu(this.host, look, at);
      main.setAttribute("aria-keyshortcuts", "Shift+F10");
      main.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        menu({ x: e.clientX, y: e.clientY });
      });
      main.addEventListener("keydown", (e) => {
        if (e.key !== "ContextMenu" && !(e.shiftKey && e.key === "F10")) return;
        e.preventDefault();
        const r = main.getBoundingClientRect();
        menu({ x: r.left, y: r.bottom });
      });
      more.addEventListener("click", () => {
        const r = more.getBoundingClientRect();
        menu({ x: r.left, y: r.bottom });
      });
      this.list.append(row);
    }
    if (hadFocus) this.list.querySelector<HTMLElement>(`[data-style="${CSS.escape(hadFocus)}"]`)?.focus();
    if (this.mode === "dialog") this.onPick(this.choice);
  }

  private keys(ev: KeyboardEvent) {
    const mains = Array.from(this.list.querySelectorAll<HTMLElement>(".styles-main"));
    const i = mains.indexOf(document.activeElement as HTMLElement);
    if (i < 0) return;
    let to = -1;
    if (ev.key === "ArrowDown") to = i + 1;
    else if (ev.key === "ArrowUp") to = i - 1;
    else if (ev.key === "Home") to = 0;
    else if (ev.key === "End") to = mains.length - 1;
    else return;
    ev.preventDefault();
    if (to < 0 || to >= mains.length) return;
    mains.forEach((m) => (m.tabIndex = -1));
    mains[to].tabIndex = 0;
    mains[to].focus();
  }
}

/** Manage styles: the list with buttons for the chosen style. */
export function openManageStyles(ed: Editor, host: ExtensionHost) {
  const list = new StylesList(ed, host, "dialog");
  const button = (label: string, command: string, args: () => unknown[], needsStyle = true, kind = "") => {
    const b = h("button", { class: `btn ${kind}`.trim(), type: "button" }, label);
    b.addEventListener("click", () => {
      const choice = list.choice;
      if (needsStyle && !choice) return;
      void host.execute(command, ...args()).then(() => list.update(), () => undefined);
    });
    return b;
  };
  const newStyle = button(t("New style…"), "format.newStyle", () => []);
  const modify = button(t("Modify…"), "format.modifyStyle", () => [list.choice?.id]);
  const select = button(t("Select all instances"), "format.selectStyleInstances", () => [list.choice?.id]);
  const clear = button(t("Clear formatting of instances"), "format.clearStyleInstances", () => [list.choice?.id]);
  const del = button(t("Delete…"), "format.deleteStyle", () => [list.choice?.id], true, "danger");
  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const sync = (look: StyleLook | null) => {
    modify.disabled = !look;
    select.disabled = !look || look.uses === 0;
    clear.disabled = !look || look.uses === 0;
    del.disabled = !look || !look.custom;
  };
  list.onChoose(sync);
  const dialog = h(
    "div",
    { class: "dialog manage-styles", role: "dialog", "aria-modal": "true", "aria-label": t("Manage styles") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Manage styles")), h("p", { class: "sub" }, t("Choose a style, then change it, delete it, or select the text that uses it."))),
    h("div", { class: "dialog-body" }, list.el, h("div", { class: "ms-actions" }, newStyle, modify, select, clear, del)),
    h("div", { class: "dialog-actions" }, done),
  );
  const close = modal(dialog, () => ed.focus());
  list.update();
  sync(list.choice);
  done.addEventListener("click", close);
}
