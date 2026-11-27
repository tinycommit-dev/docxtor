// The style gallery (the toolbar's style picker): every style drawn the way it looks in the document (the font, size,
// colour and weight the engine resolved for it), the document's own styles first, then Word's built-in styles it does
// not define yet. Picking one applies it; a right-click offers Modify, Update to match, Select all and Delete. The
// lines under the tiles are commands (Create a style, Modify, Update, Manage styles), so they have shortcuts and
// can be found in the menu and the command palette.
import type { FormatState, StyleLook } from "@docxtor/core";
import { t, tn, type ExtensionHost } from "@docxtor/extensions";
import { applyStyle } from "../editor/charcmds";
import type { Editor } from "../editor/editor";
import { h, popover } from "./dom";
import { styleTitle, tileCss } from "./style-preview";

/** The tiles to show, in Word's order: styles in the gallery by priority (the default style first), then the built-in ones not yet in the file. The style the caret has is always among them. */
export function galleryStyles(looks: StyleLook[], current: { paragraph?: string; character?: string }): StyleLook[] {
  // A character style linked to a paragraph style is that paragraph style's twin: the one tile stands for both, as in Word.
  const paragraphs = new Set(looks.filter((l) => l.kind === "paragraph").map((l) => l.id));
  const twin = (l: StyleLook) => l.kind === "character" && !!l.link && paragraphs.has(l.link);
  const usable = (l: StyleLook) => (l.kind === "paragraph" || l.kind === "character") && !twin(l);
  const rank = (l: StyleLook) => (l.kind === "paragraph" && l.id === "Normal" ? -1 : l.priority);
  const mine = looks.filter((l) => usable(l) && !l.latent && (l.quick || l.id === current.paragraph || l.id === current.character) && !(l.hidden && !l.quick));
  const order = new Map(looks.map((l, i) => [l.id, i]));
  mine.sort((a, b) => rank(a) - rank(b) || (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0));
  const more = looks.filter((l) => usable(l) && l.latent);
  return [...mine, ...more];
}

/** What the caret's text has now: its paragraph style and character style. */
export function currentStyles(state: FormatState | null, defaultPara = "Normal"): { paragraph: string; character?: string } {
  return { paragraph: state?.style ?? defaultPara, character: state?.charStyle };
}

/** Give the selection a style: a paragraph style to its paragraphs, a character style to its text, and a linked style as either, as in Word (see `applyStyle`). */
export function applyStyleLook(ed: Editor, look: Pick<StyleLook, "id" | "kind" | "link">) {
  applyStyle(ed, look);
}

/** The mark after a name: ¶ for a paragraph style, a for a character style, ¶a for a linked one, as Word shows them. */
export function kindMark(look: Pick<StyleLook, "kind" | "link">): HTMLElement {
  const linked = look.kind === "paragraph" && !!look.link;
  const title = look.kind === "character" ? t("Character style") : linked ? t("Linked style") : t("Paragraph style");
  return h("span", { class: "style-kind", title, "aria-label": title }, look.kind === "character" ? "a" : linked ? "\u00b6a" : "\u00b6");
}

/** One tile: the style's name in its own look on a white tile. */
export function styleTile(look: StyleLook, o: { current?: boolean; tag?: "button" | "div" } = {}): HTMLElement {
  const name = styleTitle(look.name, look.custom);
  const tile = h(
    o.tag ?? "button",
    {
      class: `style-tile${look.kind === "character" ? " char" : ""}${look.latent ? " latent" : ""}`,
      ...(o.tag === "div" ? {} : { type: "button" }),
      role: "option",
      "aria-selected": String(!!o.current),
      "data-style": look.id,
      title: look.kind === "character" ? t("{style} (character style)", { style: name }) : name,
    },
    h("span", { class: "style-sample", style: tileCss(look.run, look.para) }, name),
    kindMark(look),
  );
  return tile;
}

export interface GalleryOptions {
  ed: Editor;
  host: ExtensionHost;
  /** Where the caret's style is (the toolbar passes what it shows). */
  state: FormatState | null;
}

/** A line under the tiles that runs a command. */
function commandLine(host: ExtensionHost, id: string, title: string, close: () => void, args: unknown[] = []): HTMLElement | null {
  const cmd = host.command(id);
  if (!cmd) return null;
  const item = h("button", { class: "menu-item", type: "button", "data-command": id }, h("span", {}, title));
  item.disabled = !host.enabled(cmd);
  const key = host.shortcut(id);
  if (key) item.append(h("span", { class: "hint" }, key));
  item.addEventListener("click", () => {
    close();
    void host.execute(id, ...args).catch(() => undefined);
  });
  return item;
}

/** The menu a style's right-click gives. */
export function openStyleMenu(host: ExtensionHost, look: StyleLook, at: { x: number; y: number }, done: () => void = () => {}) {
  const list = h("div", {});
  const close = popover(at, list);
  const name = styleTitle(look.name, look.custom);
  const add = (id: string, title: string, args: unknown[]) => {
    const line = commandLine(host, id, title, () => {
      close();
      done();
    }, args);
    if (line) list.append(line);
  };
  add("format.modifyStyle", t("Modify “{style}”…", { style: name }), [look.id]);
  if (look.kind === "paragraph" || look.kind === "character") add("format.updateStyle", t("Update “{style}” to match the selection", { style: name }), [look.id]);
  if (look.uses > 0) add("format.selectStyleInstances", tn(look.uses, "Select all {n} instance", "Select all {n} instances"), [look.id]);
  add("format.styleInGallery", look.quick ? t("Remove from the style gallery") : t("Add to the style gallery"), [look.id, !look.quick]);
  if (look.custom) {
    list.append(h("div", { class: "menu-sep" }));
    add("format.renameStyle", t("Rename “{style}”…", { style: name }), [look.id]);
    add("format.deleteStyle", t("Delete “{style}”…", { style: name }), [look.id]);
  }
  return close;
}

/** Open the gallery under `anchor`. Returns a function that closes it. */
export function openStyleGallery({ ed, host, state }: GalleryOptions, anchor: HTMLElement): () => void {
  const engine = ed.engine;
  const looks = engine ? engine.styleGallery() : [];
  const defaultPara = looks.find((l) => l.kind === "paragraph" && l.id === "Normal")?.id ?? "Normal";
  const current = currentStyles(state, defaultPara);
  const shown = galleryStyles(looks, current);
  const root = h("div", { class: "style-gallery", role: "group", "aria-label": t("Styles") });
  const grid = h("div", { class: "style-grid", role: "listbox", "aria-label": t("Styles") });
  const more = h("div", { class: "style-grid", role: "listbox", "aria-label": t("More styles") });
  const tiles: HTMLElement[] = [];
  let close = () => {};
  for (const look of shown) {
    const on = look.kind === "character" ? look.id === current.character : look.id === current.paragraph || (!!look.link && look.link === current.character);
    const tile = styleTile(look, { current: on });
    tile.tabIndex = on ? 0 : -1;
    tile.addEventListener("click", () => {
      close();
      applyStyleLook(ed, look);
      ed.focus();
    });
    tile.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      openStyleMenu(host, look, { x: (e as MouseEvent).clientX, y: (e as MouseEvent).clientY });
    });
    tiles.push(tile);
    (look.latent ? more : grid).append(tile);
  }
  if (!tiles.some((x) => x.tabIndex === 0) && tiles[0]) tiles[0].tabIndex = 0;
  const scroll = h("div", { class: "style-scroll" }, h("div", { class: "menu-title" }, t("Styles")), grid);
  if (more.childElementCount) scroll.append(h("div", { class: "menu-title" }, t("More built-in styles")), more);
  root.append(scroll, h("div", { class: "menu-sep" }));
  const style = looks.find((l) => l.id === current.paragraph);
  const name = style ? styleTitle(style.name, style.custom) : current.paragraph;
  const lines = [
    state?.charStyle ? commandLine(host, "format.clearCharStyle", t("Clear character style"), () => close()) : null,
    commandLine(host, "format.createStyle", t("Create a style from the selection…"), () => close()),
    commandLine(host, "format.modifyStyle", t("Modify “{style}”…", { style: name }), () => close(), [current.paragraph]),
    commandLine(host, "format.updateStyle", t("Update “{style}” to match the selection", { style: name }), () => close(), [current.paragraph]),
    commandLine(host, "format.manageStyles", t("Manage styles…"), () => close()),
    commandLine(host, "format.clear", t("Clear formatting"), () => close()),
  ];
  for (const l of lines) if (l) root.append(l);
  root.addEventListener("keydown", (ev) => {
    const tile = document.activeElement as HTMLElement | null;
    if (!tile || !tiles.includes(tile)) return;
    const i = tiles.indexOf(tile);
    const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: 2, ArrowUp: -2 };
    let to = -1;
    if (ev.key in step) to = i + step[ev.key];
    else if (ev.key === "Home") to = 0;
    else if (ev.key === "End") to = tiles.length - 1;
    else return;
    ev.preventDefault();
    if (to < 0 || to >= tiles.length) return;
    for (const x of tiles) x.tabIndex = -1;
    tiles[to].tabIndex = 0;
    tiles[to].focus();
  });
  close = popover(anchor, root, { onClose: () => ed.focus() });
  root.querySelector<HTMLElement>('.style-tile[tabindex="0"]')?.focus();
  return close;
}
