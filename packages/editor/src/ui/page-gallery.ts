// The cover page, header and footer galleries (Insert, Cover page; Insert, Header; Insert, Footer). Each tile is the real
// design laid out in this document's theme, paper and properties: the engine makes a small document of every design, an
// editor out of sight lays it out, and each tile shows one of its pages, shrunk. Cover pages saved by the person (the
// snippet stores' "Cover pages" gallery, and the document's own building blocks) come after Docxtor's.
import type { GalleryLabels } from "@docxtor/core";
import { Engine } from "@docxtor/core";
import { msg, t, type ExtensionHost } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { stripFar } from "../editor/far";
import { listParts, type Part } from "../editor/quick-parts";
import type { DocxtorEditor } from "../docxtor-editor";
import { offscreenEditor } from "../mailmerge/offscreen";
import { h, modal } from "./dom";

export type GalleryKind = "cover" | "header" | "footer";

/** The names of Docxtor's designs, for translating. */
export const DESIGN_NAMES: Record<string, string> = {
  Simple: msg("Simple"),
  Band: msg("Band"),
  "Side bar": msg("Side bar"),
  Frame: msg("Frame"),
  Academic: msg("Academic"),
  Report: msg("Report"),
  Blocks: msg("Blocks"),
  Centered: msg("Centered"),
  Abstract: msg("Abstract"),
  Blank: msg("Blank"),
  "Blank (three columns)": msg("Blank (three columns)"),
  Title: msg("Title"),
  "Title and date": msg("Title and date"),
  Banded: msg("Banded"),
  "Title and page number": msg("Title and page number"),
  "Page number": msg("Page number"),
  "Author and page number": msg("Author and page number"),
  "Company and date": msg("Company and date"),
};

/** The placeholder texts and words of the designs in the interface language. */
export function galleryLabels(ed: Editor): GalleryLabels {
  return {
    title: t("[Document title]"),
    subtitle: t("[Document subtitle]"),
    author: t("[Author name]"),
    company: t("[Company name]"),
    date: t("[Date]"),
    abstract: t("[Abstract: a short summary of the document]"),
    course: t("[Course name]"),
    supervisor: t("[Supervisor name]"),
    type: t("[Type here]"),
    page: t("Page"),
    of: t("of"),
    submitted_by: t("Submitted by"),
    course_label: t("Course"),
    supervisor_label: t("Supervisor"),
    abstract_label: t("Abstract"),
    dateFormat: "MMMM d, yyyy",
    lang: ed.pageInfo?.lang || navigator.language || "en-US",
  };
}

/** The galleries' names for the snippets a person saved. */
export const SAVED_GALLERY: Record<GalleryKind, string> = { cover: "Cover pages", header: "Headers", footer: "Footers" };

interface Choice {
  /** One of Docxtor's designs. */
  design?: string;
  /** A cover page the person saved. */
  part?: Part;
}

/** Fill the tiles with pictures of the pages: lay the preview document out in an editor out of sight. */
async function drawTiles(fe: DocxtorEditor, kind: GalleryKind, tiles: HTMLElement[], gone: () => boolean) {
  const ed = fe.editor;
  if (!ed.engine || !tiles.length) return;
  const bytes = ed.engine.pageGalleryPreview(kind, galleryLabels(ed));
  const off = await offscreenEditor({ x: { fe } });
  try {
    await off.fe.open(bytes, "gallery.docx");
    if (gone()) return;
    const o = off.fe.editor;
    o.layoutNow();
    // The top margin in pixels (twips over 15): where a header or footer design starts on its page.
    const top = (o.pageInfo?.margin?.[0] ?? 1440) / 15;
    // A cover page is a page each; the headers and footers follow one another, each starting a page.
    const pages = Math.min(o.pages, tiles.length);
    for (let k = 0; k < pages; k++) {
      const b = o.pageBox(k);
      const tile = tiles[k];
      const frame = tile.querySelector<HTMLElement>(".pg-page")!;
      const width = frame.clientWidth || 150;
      const scale = width / b.w;
      // A header or footer shows the strip of page it fills.
      const cropTop = kind === "cover" ? 0 : Math.max(0, top - 18);
      const cropH = kind === "cover" ? b.h : Math.min(b.h, 110);
      frame.style.height = `${Math.round(cropH * scale)}px`;
      const copy = o.withDrawn(() => o.stageElement.cloneNode(true) as HTMLElement);
      stripFar(copy);
      copy.querySelectorAll("[contenteditable]").forEach((e) => e.removeAttribute("contenteditable"));
      copy.querySelectorAll("[id]").forEach((e) => e.removeAttribute("id"));
      copy.querySelectorAll(".page-num, .vruler, .np-layer, .bm-layer, .fm-layer, .sp-layer, .xs-layer, .fc-label").forEach((e) => e.remove());
      copy.classList.add("pg-stage");
      copy.style.transform = `scale(${scale})`;
      copy.style.left = `${-b.left * scale}px`;
      copy.style.top = `${-(b.top + cropTop) * scale}px`;
      copy.setAttribute("aria-hidden", "true");
      copy.inert = true;
      frame.replaceChildren(copy);
      frame.classList.add("drawn");
    }
  } finally {
    await off.dispose();
  }
}

/**
 * Show a gallery. `pick` gets the design or saved part chosen; `extra` buttons go at the foot (Remove current cover page,
 * Edit header...).
 */
export async function openPageGallery(fe: DocxtorEditor, host: ExtensionHost, kind: GalleryKind, pick: (c: Choice) => void, extra: HTMLElement[] = []) {
  const ed = fe.editor;
  const title = kind === "cover" ? t("Cover page") : kind === "header" ? t("Header") : t("Footer");
  const designs = Engine.pageGallery(kind);
  const saved = kind === "cover" ? (await listParts(host, ed)).filter((p) => p.gallery === SAVED_GALLERY.cover || p.gallery === t(SAVED_GALLERY.cover)) : [];
  let closed = false;
  const tiles: HTMLElement[] = [];
  const grid = h("div", { class: `pg-grid pg-${kind}`, role: "listbox", "aria-label": title });
  const choose = (c: Choice) => {
    close();
    pick(c);
  };
  const tile = (name: string, c: Choice, i: number, words?: string) => {
    const el = h(
      "button",
      { class: "pg-tile", type: "button", role: "option", "aria-selected": "false", tabindex: i === 0 ? 0 : -1, "data-design": c.design ?? c.part?.id ?? "" },
      h("span", { class: "pg-page", "aria-hidden": "true" }, words ? h("span", { class: "pg-words" }, words) : null),
      h("span", { class: "pg-name" }, name),
    );
    el.addEventListener("click", () => choose(c));
    grid.append(el);
    return el;
  };
  designs.forEach((d, i) => tiles.push(tile(t(DESIGN_NAMES[d.name] ?? d.name), { design: d.id }, i)));
  const savedTiles = saved.map((p, i) => tile(p.name, { part: p }, designs.length + i, p.text));
  grid.addEventListener("keydown", (e) => {
    const all = [...tiles, ...savedTiles];
    const at = all.indexOf(document.activeElement as HTMLElement);
    if (at < 0) return;
    const across = Math.max(1, Math.round(grid.clientWidth / (all[0].offsetWidth || 1)));
    const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: across, ArrowUp: -across };
    let to = -1;
    if (e.key in step) to = at + step[e.key];
    else if (e.key === "Home") to = 0;
    else if (e.key === "End") to = all.length - 1;
    else return;
    e.preventDefault();
    if (to < 0 || to >= all.length) return;
    all.forEach((x) => (x.tabIndex = -1));
    all[to].tabIndex = 0;
    all[to].focus();
  });
  const cancel = h("button", { class: "btn", type: "button" }, t("Cancel"));
  const dialog = h(
    "div",
    { class: "dialog pg-dialog", role: "dialog", "aria-modal": "true", "aria-label": title },
    h("div", { class: "dialog-head" }, h("h2", {}, title), h("p", { class: "sub" }, kind === "cover" ? t("Its boxes show the document's title, author and date. Type in one to change it everywhere.") : t("Shown in this document's colours and fonts."))),
    h("div", { class: "dialog-body" }, saved.length ? h("h3", { class: "pg-group" }, t("Built-in")) : null, grid),
    h("div", { class: "dialog-actions" }, ...extra, cancel),
  );
  if (savedTiles.length) grid.insertBefore(h("h3", { class: "pg-group" }, t("Saved")), savedTiles[0]);
  const close = modal(dialog, () => {
    closed = true;
    ed.focus();
  });
  cancel.addEventListener("click", close);
  for (const b of extra) b.addEventListener("click", close);
  tiles[0]?.focus();
  try {
    await drawTiles(fe, kind, tiles, () => closed);
  } catch (e) {
    console.warn(`Docxtor: the gallery pictures could not be drawn: ${(e as Error).message ?? e}`);
  }
}
