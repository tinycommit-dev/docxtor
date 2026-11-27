// Page setup (File, Page setup…): margins, paper and layout in three tabs, for the whole document, the section the caret
// is in, or from the caret on. Lengths follow the unit setting (units.ts); the file always holds twips.
import type { Op, PageSetup, SectionInfo } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { formatUnits, unitWord } from "../units";
import { h, modal } from "./dom";
import { applyScoped, check, choice, field, grid, group, scopeChoice, selectOf, tabs, twipsOf, twipsOrWas, unitInput, unitLabel } from "./form";
import { openLineNumbers } from "./line-numbers";
import { openPageBorders } from "./page-borders";
import { docGridGroup } from "./doc-grid";

/** Millimetres as twips. */
const mm = (n: number) => Math.round((n * 1440) / 25.4);

/** The papers Word lists, in portrait (width, height in twips), and the one that is typed in. */
export const PAPERS: [string, number, number][] = [
  [msg("Letter (8.5 × 11 in)"), 12240, 15840],
  [msg("Legal (8.5 × 14 in)"), 12240, 20160],
  [msg("Executive (7.25 × 10.5 in)"), 10440, 15120],
  [msg("A3 (297 × 420 mm)"), mm(297), mm(420)],
  ["A4 (210 × 297 mm)", 11906, 16838],
  [msg("A5 (148 × 210 mm)"), 8391, 11906],
  [msg("A6 (105 × 148 mm)"), mm(105), mm(148)],
  [msg("B4 JIS (257 × 364 mm)"), mm(257), mm(364)],
  [msg("B5 JIS (182 × 257 mm)"), mm(182), mm(257)],
  [msg("B5 ISO (176 × 250 mm)"), mm(176), mm(250)],
  [msg("Tabloid (11 × 17 in)"), 15840, 24480],
  [msg("Statement (5.5 × 8.5 in)"), 7920, 12240],
  [msg("Folio (8.5 × 13 in)"), 12240, 18720],
  [msg("Quarto (215 × 275 mm)"), mm(215), mm(275)],
  [msg("10 × 14 in"), 14400, 20160],
  [msg("Envelope #10 (4.1 × 9.5 in)"), 5940, 13680],
  [msg("Envelope DL (110 × 220 mm)"), mm(110), mm(220)],
  [msg("Envelope C5 (162 × 229 mm)"), mm(162), mm(229)],
  [msg("Envelope B5 (176 × 250 mm)"), mm(176), mm(250)],
  [msg("Envelope Monarch (3.9 × 7.5 in)"), 5580, 10800],
  [msg("Japanese postcard (100 × 148 mm)"), mm(100), mm(148)],
];

const MARGINS: [string, [number, number, number, number]][] = [
  [msg("Normal"), [1440, 1440, 1440, 1440]],
  [msg("Narrow"), [720, 720, 720, 720]],
  [msg("Moderate"), [1440, 1080, 1440, 1080]],
  [msg("Wide"), [1440, 2880, 1440, 2880]],
];

type Multiple = "normal" | "mirror" | "twoUp" | "bookFold";
type GutterAt = "left" | "top" | "right";

export function openPageSetup(ed: Editor, firstTab = 0) {
  const setup: PageSetup = ed.engine.pageSetup();
  // The dialog shows the section holding the caret; the settings that belong to the document come from the page setup.
  const cur: SectionInfo | PageSetup = setup.sections?.[ed.caretSection()] ?? setup;
  const info = cur as SectionInfo;
  const startWas = (info.start as string) || "nextPage";

  const scope = scopeChoice(ed);

  // ---- Margins
  const landscape = choice("Orientation", [["portrait", t("Portrait")], ["landscape", t("Landscape")]], cur.width > cur.height ? "landscape" : "portrait");
  landscape.el.setAttribute("aria-label", t("Orientation"));
  const multiWas: Multiple = setup.mirrorMargins ? "mirror" : setup.printTwoOnOne ? "twoUp" : setup.bookFold ? "bookFold" : "normal";
  const multiple = selectOf(
    "ps-multiple",
    [["normal", t("Normal")], ["mirror", t("Mirror margins")], ["twoUp", t("2 pages per sheet")], ["bookFold", t("Book fold")]],
    multiWas,
  );
  const sheets = selectOf("ps-sheets", [["0", t("All")], ["4", "4"], ["8", "8"], ["12", "12"], ["16", "16"], ["20", "20"], ["24", "24"], ["28", "28"], ["32", "32"]], String(setup.bookFoldSheets ?? 0));
  const margin = (i: number, side: string) => unitInput(`ps-m${i}`, cur.margin[i], t("{side} margin ({unit})", { side, unit: unitWord() }));
  const sides = [margin(0, t("Top")), margin(1, t("Right")), margin(2, t("Bottom")), margin(3, t("Left"))];
  const [top, right, bottom, left] = sides;
  const labels = [h("label", { for: "ps-m0" }), h("label", { for: "ps-m1" }), h("label", { for: "ps-m2" }), h("label", { for: "ps-m3" })];
  const gutter = unitInput("ps-gutter", info.gutter ?? 0, t("Gutter ({unit})", { unit: unitWord() }));
  const gutterWas: GutterAt = setup.gutterAtTop ? "top" : info.rtlGutter ? "right" : "left";
  const gutterAt = selectOf("ps-gutter-at", [["left", t("Left")], ["top", t("Top")], ["right", t("Right")]], gutterWas);
  const presets = h("div", { class: "seg" });
  for (const [label, m] of MARGINS) {
    const b = h("button", { type: "button" }, t(label));
    b.addEventListener("click", () => sides.forEach((x, i) => (x.value = formatUnits(m[i]))));
    presets.append(b);
  }
  const syncMargins = () => {
    const mirrored = multiple.value === "mirror";
    const names = [t("Top"), mirrored ? t("Outside") : t("Right"), t("Bottom"), mirrored ? t("Inside") : t("Left")];
    labels.forEach((l, i) => (l.textContent = unitLabel(names[i])));
    sides[1].setAttribute("aria-label", t("{side} margin ({unit})", { side: names[1], unit: unitWord() }));
    sides[3].setAttribute("aria-label", t("{side} margin ({unit})", { side: names[3], unit: unitWord() }));
    sheets.disabled = multiple.value !== "bookFold";
    // Facing pages and a booklet put the gutter by the fold.
    gutterAt.disabled = multiple.value !== "normal";
  };
  multiple.addEventListener("change", () => {
    if (multiple.value === "twoUp" || multiple.value === "bookFold") landscape.set("landscape");
    syncMargins();
  });
  const margins = h(
    "div",
    {},
    group(t("Margins"), h("div", { class: "field" }, presets), grid(h("div", { class: "field" }, labels[0], top), h("div", { class: "field" }, labels[2], bottom), h("div", { class: "field" }, labels[3], left), h("div", { class: "field" }, labels[1], right))),
    group(t("Binding"), grid(h("div", { class: "field" }, h("label", { for: "ps-gutter" }, unitLabel(t("Gutter"))), gutter), field(t("Gutter position"), gutterAt, "ps-gutter-at"))),
    group(t("Pages"), grid(field(t("Multiple pages"), multiple, "ps-multiple"), field(t("Sheets per booklet"), sheets, "ps-sheets")), h("div", { class: "field" }, h("label", {}, t("Orientation")), landscape.el)),
  );

  // ---- Paper
  let w = Math.min(cur.width, cur.height);
  let hgt = Math.max(cur.width, cur.height);
  const paper = h("select", { id: "ps-paper" });
  const matches = (pw: number, ph: number) => Math.abs(pw - w) < 20 && Math.abs(ph - hgt) < 20;
  const seen = new Set<string>();
  let matched: boolean = false;
  for (const [label, pw, ph] of PAPERS) {
    // Two names can share a size (B5 for paper and for envelopes): the first one is the one shown for it.
    const key = `${pw}x${ph}`;
    const dup = seen.has(key);
    seen.add(key);
    const sel: boolean = !dup && !matched && matches(pw, ph);
    matched ||= sel;
    paper.append(h("option", { value: dup ? `${key}-2` : key, selected: sel }, t(label)));
  }
  paper.append(h("option", { value: "custom", selected: !matched }, matched ? t("Custom size") : t("Custom ({width} × {height} {unit})", { width: formatUnits(w), height: formatUnits(hgt), unit: unitWord() })));
  const width = unitInput("ps-width", w, t("Width ({unit})", { unit: unitWord() }), 1);
  const height = unitInput("ps-height", hgt, t("Height ({unit})", { unit: unitWord() }), 1);
  const fillSize = () => {
    width.value = formatUnits(w);
    height.value = formatUnits(hgt);
  };
  fillSize();
  paper.addEventListener("change", () => {
    if (paper.value === "custom") return;
    const [pw, ph] = paper.value.split("-")[0].split("x").map(Number);
    [w, hgt] = [pw, ph];
    fillSize();
  });
  for (const input of [width, height]) {
    input.addEventListener("input", () => {
      w = twipsOf(width, w) || w;
      hgt = twipsOf(height, hgt) || hgt;
      paper.value = "custom";
    });
  }
  const paperTab = h("div", {}, group(t("Paper size"), field(t("Paper size"), paper, "ps-paper"), grid(h("div", { class: "field" }, h("label", { for: "ps-width" }, unitLabel(t("Width"))), width), h("div", { class: "field" }, h("label", { for: "ps-height" }, unitLabel(t("Height"))), height))));

  // ---- Layout
  const start = selectOf(
    "ps-start",
    [["nextPage", t("New page")], ["continuous", t("Continuous")], ["evenPage", t("Even page")], ["oddPage", t("Odd page")], ["nextColumn", t("New column")]],
    startWas,
  );
  const evenOdd = check("ps-even-odd", t("Different odd and even pages"), !!setup.evenAndOddHeaders);
  const firstPage = check("ps-first", t("Different first page"), !!info.titlePage);
  const header = unitInput("ps-header", cur.header, t("Header ({unit})", { unit: unitWord() }));
  const footer = unitInput("ps-footer", cur.footer, t("Footer ({unit})", { unit: unitWord() }));
  const valign = selectOf("ps-valign", [["top", t("Top")], ["center", t("Center")], ["both", t("Justified")], ["bottom", t("Bottom")]], info.vAlign ?? "top");
  // A direction the dialog has no name for (the file's own `tbRlV`, say) is shown as "Other" and kept unless the person picks another.
  const directionWas = info.textDirection || "lrTb";
  const directions: [string, string][] = [["lrTb", t("Horizontal")], ["tbRl", t("Vertical, lines from the right")], ["btLr", t("Vertical, text from the bottom")]];
  if (!directions.some(([v]) => v === directionWas)) directions.push([directionWas, t("Other")]);
  const direction = selectOf("ps-direction", directions, directionWas);
  // The text area as the dialog's paper and margins stand now (points), which the document grid's counts are worked out against.
  const textArea = () => {
    const [pw, ph] = landscape.value === "landscape" ? [Math.max(w, hgt), Math.min(w, hgt)] : [Math.min(w, hgt), Math.max(w, hgt)];
    const side = (i: 0 | 1 | 2 | 3) => twipsOrWas(sides[i], cur.margin[i]);
    return { width: (pw - side(1) - side(3) - twipsOrWas(gutter, info.gutter ?? 0)) / 20, height: (ph - side(0) - side(2)) / 20 };
  };
  const docGrid = docGridGroup(info, textArea);
  const lineNumbers = h("button", { class: "btn", type: "button" }, t("Line numbers…"));
  const borders = h("button", { class: "btn", type: "button" }, t("Page borders…"));
  const layout = h(
    "div",
    {},
    group(t("Section"), field(t("Section start"), start, "ps-start")),
    group(t("Headers and footers"), evenOdd.row, firstPage.row, grid(h("div", { class: "field" }, h("label", { for: "ps-header" }, unitLabel(t("Header from edge"))), header), h("div", { class: "field" }, h("label", { for: "ps-footer" }, unitLabel(t("Footer from edge"))), footer))),
    group(t("Page"), grid(field(t("Vertical alignment"), valign, "ps-valign"), field(t("Text direction"), direction, "ps-direction"))),
    docGrid.group,
    h("div", { class: "ps-more" }, lineNumbers, borders),
  );

  const view = tabs("ps", [[t("Margins"), margins], [t("Paper"), paperTab], [t("Layout"), layout]], firstTab);
  syncMargins();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog para-dialog ps-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Page setup") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Page setup"))),
    view.list,
    h("div", { class: "dialog-body" }, ...view.panes, scope.field),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  lineNumbers.addEventListener("click", () => openLineNumbers(ed));
  borders.addEventListener("click", () => openPageBorders(ed));
  apply.addEventListener("click", () => {
    const mirror = multiple.value === "mirror";
    const turned = landscape.value === "landscape";
    const [pw, ph] = turned ? [Math.max(w, hgt), Math.min(w, hgt)] : [Math.min(w, hgt), Math.max(w, hgt)];
    const m = [twipsOrWas(top, cur.margin[0]), twipsOrWas(right, cur.margin[1]), twipsOrWas(bottom, cur.margin[2]), twipsOrWas(left, cur.margin[3])] as [number, number, number, number];
    const g = twipsOrWas(gutter, info.gutter ?? 0);
    // The gutter's side is the control's only for ordinary pages; the fold decides it otherwise, and a file whose
    // multiple-pages choice is not being changed keeps the side it has.
    const at: GutterAt = multiple.value === "normal" ? (gutterAt.value as GutterAt) : multiple.value === multiWas ? gutterWas : "left";
    const book = multiple.value === "bookFold" ? "bookFold" : "none";
    close();
    // The document's own settings, written whatever the scope says.
    const settings: Record<string, unknown> = {};
    if (mirror !== !!setup.mirrorMargins) settings.mirror_margins = mirror;
    if ((at === "top") !== !!setup.gutterAtTop) settings.gutter_at_top = at === "top";
    if (evenOdd.input.checked !== !!setup.evenAndOddHeaders) settings.even_and_odd_headers = evenOdd.input.checked;
    if ((multiple.value === "twoUp") !== !!setup.printTwoOnOne) settings.print_two_on_one = multiple.value === "twoUp";
    if ((book === "bookFold") !== !!setup.bookFold) settings.book_fold = book;
    if (book === "bookFold" && Number(sheets.value) !== (setup.bookFoldSheets ?? 0)) settings.book_fold_sheets = Number(sheets.value);
    const ops: Op[] = Object.keys(settings).length ? [{ op: "set_layout_settings", ...settings }] : [];
    if (ops.length && scope.value !== "forward") ed.exec(ops);
    const gridPatch = docGrid.patch();
    const headerTw = twipsOrWas(header, cur.header);
    const footerTw = twipsOrWas(footer, cur.footer);
    const sectionOps = (section: number | undefined): Op[] => {
      const at1 = section === undefined ? {} : { section };
      const out: Op[] = [
        { op: "set_page_setup", ...at1, width: pw, height: ph, margins: m },
        {
          op: "set_section",
          ...at1,
          // Only what the person changed: the rest of the section stays as the file has it.
          ...(g !== (info.gutter ?? 0) ? { gutter: g } : {}),
          ...((at === "right") !== !!info.rtlGutter ? { rtl_gutter: at === "right" } : {}),
          ...(headerTw !== cur.header ? { header: headerTw } : {}),
          ...(footerTw !== cur.footer ? { footer: footerTw } : {}),
          ...(valign.value !== (info.vAlign ?? "top") ? { v_align: valign.value } : {}),
          ...(direction.value !== directionWas ? { text_direction: direction.value } : {}),
          ...(start.value !== startWas ? { start: start.value } : {}),
          ...(gridPatch ? { doc_grid: gridPatch } : {}),
        },
      ];
      if (firstPage.input.checked !== !!info.titlePage) out.push({ op: "set_title_page", on: firstPage.input.checked, ...at1 });
      return out;
    };
    applyScoped(ed, scope.value, (section) => (scope.value === "forward" && section !== undefined ? [...sectionOps(section)] : sectionOps(section)));
    // "This point forward" changes the document's settings in the same step as the section.
    if (ops.length && scope.value === "forward") {
      ed.exec(ops, { merge: true });
      ed.reloadPageSetup();
    }
    ed.applyAutoFit();
    ed.focus();
  });
}

/** Turn the page. Margins turn with it, the way Word does it (the left margin becomes the top one). */
export function setOrientation(ed: Editor, landscape: boolean) {
  const { width: w, height: hgt, margin: m } = ed.caretSectionInfo();
  if (landscape === w > hgt) return;
  const margins = landscape ? [m[3], m[0], m[1], m[2]] : [m[1], m[2], m[3], m[0]];
  ed.exec({ op: "set_page_setup", width: hgt, height: w, margins });
  ed.reloadPageSetup();
  ed.applyAutoFit();
}

export function isLandscape(ed: Editor): boolean {
  if (!ed.engine) return false;
  const { width, height } = ed.caretSectionInfo();
  return width > height;
}
