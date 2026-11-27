// Cross-reference: a live reference to a heading, bookmark, caption or note, shown as its text, number or page.
// The reference type narrows the list of targets; what to show depends on the type.
import type { XrefShow, XrefTarget } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal, onChoose } from "./dom";

interface Item {
  title: string;
  detail?: string;
  indent?: number;
  number?: string;
  target: XrefTarget;
}

const SHOWS: Record<string, [XrefShow, string][]> = {
  heading: [
    ["text", msg("Heading text")],
    ["number", msg("Heading number")],
    ["number_no_context", msg("Heading number (no context)")],
    ["number_full_context", msg("Heading number (full context)")],
    ["page", msg("Page number")],
  ],
  numbered: [
    ["text", msg("Paragraph text")],
    ["number", msg("Paragraph number")],
    ["number_no_context", msg("Paragraph number (no context)")],
    ["number_full_context", msg("Paragraph number (full context)")],
    ["page", msg("Page number")],
  ],
  bookmark: [
    ["text", msg("Bookmark text")],
    ["page", msg("Page number")],
  ],
  caption: [
    ["label_number", msg("Only label and number")],
    ["text", msg("Entire caption")],
    ["caption_text", msg("Only caption text")],
    ["page", msg("Page number")],
  ],
  footnote: [
    ["number", msg("Footnote number")],
    ["number_formatted", msg("Footnote number (formatted)")],
    ["page", msg("Page number")],
  ],
  endnote: [
    ["number", msg("Endnote number")],
    ["number_formatted", msg("Endnote number (formatted)")],
    ["page", msg("Page number")],
  ],
};

let seq = 0;

export function openCrossReference(ed: Editor) {
  const id = `xr-${++seq}`;
  const targets = ed.xrefTargets();
  const clip = (s: string, n = 90) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

  // Types with something to point at come first; Heading, Bookmark and the notes are always offered.
  const types: { value: string; label: string; items: Item[] }[] = [
    { value: "heading", label: t("Heading"), items: targets.headings.map((x) => ({ title: x.text || t("(empty heading)"), indent: Math.max(0, x.level - 1), number: x.number, detail: x.number, target: { type: "heading", p: x.p } })) },
    { value: "numbered", label: t("Numbered item"), items: ed.engine.numberedItems().map((x) => ({ title: clip(x.text) || x.full, indent: x.level, number: x.number, detail: x.full, target: { type: "numbered", p: x.p } })) },
    { value: "bookmark", label: t("Bookmark"), items: targets.bookmarks.map((x) => ({ title: x.name, detail: clip(x.text), target: { type: "bookmark", name: x.name } })) },
  ];
  const labels = [...new Set(targets.captions.map((c) => c.label))];
  for (const l of labels) types.push({ value: `caption:${l}`, label: l, items: targets.captions.filter((c) => c.label === l).map((c) => ({ title: clip(c.text), target: { type: "caption", p: c.p } })) });
  types.push(
    { value: "footnote", label: t("Footnote"), items: targets.footnotes.map((x) => ({ title: `${x.label}  ${clip(x.text)}`, target: { type: "footnote", id: x.id } })) },
    { value: "endnote", label: t("Endnote"), items: targets.endnotes.map((x) => ({ title: `${x.label}  ${clip(x.text)}`, target: { type: "endnote", id: x.id } })) },
  );

  const typeSel = h("select", { id: `${id}-type` }, ...types.map((x) => h("option", { value: x.value }, x.label)));
  typeSel.value = (types.find((x) => x.items.length) ?? types[0]).value;
  const showSel = h("select", { id: `${id}-show` });
  const filter = h("input", { type: "text", "aria-label": t("Search the list"), placeholder: t("Search"), role: "combobox", "aria-expanded": "true", "aria-controls": `${id}-list`, autocomplete: "off", spellcheck: "false" });
  const list = h("div", { class: "xr-list", role: "listbox", id: `${id}-list`, "aria-label": t("Reference to") });
  const link = h("input", { type: "checkbox", id: `${id}-link`, checked: true });
  const above = h("input", { type: "checkbox", id: `${id}-above` });

  const kindOf = () => typeSel.value.split(":")[0];
  const current = () => types.find((x) => x.value === typeSel.value)!;
  let shown: Item[] = [];
  let active = 0;

  const insert = h("button", { class: "btn primary", type: "button" }, t("Insert"));
  const close = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));

  const fillShow = () => {
    const keep = showSel.value;
    const item = shown[active];
    // A heading's number only exists when it is numbered.
    const options = SHOWS[kindOf()].filter(([v]) => kindOf() !== "heading" || !["number", "number_no_context", "number_full_context"].includes(v) || !!item?.number);
    showSel.replaceChildren(...options.map(([v, label]) => h("option", { value: v }, t(label))));
    showSel.value = options.some(([v]) => v === keep) ? keep : options[0][0];
  };

  const paintList = () => {
    const words = filter.value.toLowerCase().split(/\s+/).filter(Boolean);
    shown = current().items.filter((it) => words.every((w) => `${it.title} ${it.detail ?? ""}`.toLowerCase().includes(w)));
    active = Math.min(active, Math.max(0, shown.length - 1));
    list.replaceChildren(
      ...(shown.length
        ? shown.map((it, i) => {
            const b = h(
              "button",
              { type: "button", role: "option", class: `xr-item${i === active ? " on" : ""}`, id: `${id}-o${i}`, "aria-selected": String(i === active), style: it.indent ? `padding-left:${12 + it.indent * 14}px` : undefined },
              h("span", { class: "xr-title" }, it.title),
              it.detail && it.detail !== it.title ? h("span", { class: "xr-detail" }, it.detail) : null,
            );
            b.addEventListener("mousedown", (e) => e.preventDefault());
            onChoose(
              b,
              () => {
                active = i;
                select();
              },
              go,
            );
            return b;
          })
        : [h("p", { class: "xr-empty" }, current().items.length ? t("Nothing matches.") : emptyText(kindOf()))]),
    );
    filter.setAttribute("aria-activedescendant", shown.length ? `${id}-o${active}` : "");
    insert.disabled = !shown.length;
    fillShow();
  };
  const select = () => {
    list.querySelectorAll<HTMLElement>(".xr-item").forEach((el, i) => {
      el.classList.toggle("on", i === active);
      el.setAttribute("aria-selected", String(i === active));
    });
    list.querySelector<HTMLElement>(".xr-item.on")?.scrollIntoView({ block: "nearest" });
    filter.setAttribute("aria-activedescendant", `${id}-o${active}`);
    fillShow();
  };

  const go = () => {
    const item = shown[active];
    if (!item) return;
    const show = showSel.value as XrefShow;
    dismiss();
    ed.insertCrossReference({ target: item.target, show, link: link.checked, aboveBelow: above.checked });
  };
  typeSel.addEventListener("change", () => {
    active = 0;
    filter.value = "";
    paintList();
  });
  filter.addEventListener("input", () => {
    active = 0;
    paintList();
  });
  filter.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!shown.length) return;
      active = (active + (e.key === "ArrowDown" ? 1 : -1) + shown.length) % shown.length;
      select();
    } else if (e.key === "Enter") {
      e.preventDefault();
      go();
    }
  });

  const dialog = h(
    "div",
    { class: "dialog xr-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Cross-reference") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Cross-reference")), h("p", { class: "sub" }, t("Point to a heading, numbered item, caption, bookmark or note. It follows the text when you update fields."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "xr-row" }, h("div", { class: "field" }, h("label", { for: `${id}-type` }, t("Reference type")), typeSel), h("div", { class: "field" }, h("label", { for: `${id}-show` }, t("Insert reference to")), showSel)),
      h("div", { class: "field" }, h("label", {}, t("For which item")), filter, list),
      h("label", { class: "check", for: `${id}-link` }, link, h("span", {}, t("Insert as hyperlink"))),
      h("label", { class: "check", for: `${id}-above` }, above, h("span", {}, t("Include above or below"))),
    ),
    h("div", { class: "dialog-actions" }, close, insert),
  );
  const dismiss = modal(dialog, () => ed.focus());
  close.addEventListener("click", dismiss);
  insert.addEventListener("click", go);
  paintList();
  filter.focus();
}

function emptyText(kind: string): string {
  if (kind === "heading") return t("No headings yet. Give a paragraph a heading style first.");
  if (kind === "numbered") return t("No numbered paragraphs yet. Number a list first.");
  if (kind === "bookmark") return t("No bookmarks yet. Use Insert, Bookmark to add one.");
  if (kind === "footnote") return t("No footnotes yet.");
  if (kind === "endnote") return t("No endnotes yet.");
  return t("No captions with this label yet.");
}
