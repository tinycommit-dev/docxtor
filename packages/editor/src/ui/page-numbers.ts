// Page number options: how numbers look (1, i, I, a, A), whether a section starts counting again, and chapter numbers in
// front of the page number (2-1, 2-2).
import type { Op, PageNumberFormat } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { pageLabel } from "../editor/layout";
import { h, modal } from "./dom";
import { check, selectOf } from "./form";

const FORMATS: [PageNumberFormat, string][] = [
  ["decimal", "1, 2, 3"],
  ["lowerRoman", "i, ii, iii"],
  ["upperRoman", "I, II, III"],
  ["lowerLetter", "a, b, c"],
  ["upperLetter", "A, B, C"],
];

const SEPARATORS: [string, string, string][] = [
  ["hyphen", msg("Hyphen (-)"), "-"],
  ["period", msg("Period (.)"), "."],
  ["colon", msg("Colon (:)"), ":"],
  ["emDash", msg("Em dash (—)"), "—"],
  ["enDash", msg("En dash (–)"), "–"],
];

export function openPageNumbers(ed: Editor) {
  const setup = ed.engine.pageSetup();
  const si = ed.caretSection();
  const info = setup.sections?.[si];
  const several = (setup.sections?.length ?? 1) > 1;
  let format: PageNumberFormat = info?.pageFormat ?? "decimal";
  let restart = info?.pageStart !== undefined;
  // Numbering starts again per section, so a document of several sections changes the caret's one by default.
  let scope: "document" | "section" = several ? "section" : "document";

  const group = (label: string, items: [string, string][], get: () => string, set: (v: string) => void) => {
    const el = h("div", { class: "seg", role: "radiogroup", "aria-label": label });
    const buttons = items.map(([v, text]) => {
      const b = h("button", { type: "button", role: "radio" }, text);
      b.addEventListener("click", () => {
        set(v);
        paint();
      });
      el.append(b);
      return [v, b] as const;
    });
    const paint = () => {
      for (const [v, b] of buttons) {
        b.classList.toggle("on", v === get());
        b.setAttribute("aria-checked", String(v === get()));
      }
      start.disabled = !restart;
    };
    return { el, paint };
  };
  const start = h("input", { type: "number", min: "0", step: "1", value: String(info?.pageStart ?? 1), id: "pn-start", "aria-label": t("Start at") });
  const formats = group(t("Format"), FORMATS, () => format, (v) => (format = v as PageNumberFormat));
  const numbering = group(
    t("Numbering"),
    [
      ["continue", t(several ? msg("Continue from the previous section") : msg("Start at 1"))],
      ["restart", t("Start at")],
    ],
    () => (restart ? "restart" : "continue"),
    (v) => (restart = v === "restart"),
  );
  const applyTo = several
    ? group(t("Apply to"), [["document", t("Whole document")], ["section", t("This section")]], () => scope, (v) => (scope = v as typeof scope))
    : null;

  // Chapter numbers: the number of the last heading of a level goes before the page number, which then starts again in each chapter.
  const chapter = check("pn-chapter", t("Include chapter number"), info?.chapStyle !== undefined);
  const chapStyle = selectOf("pn-chap-style", [1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => [String(n), t("Heading {n}", { n })] as const), String(info?.chapStyle ?? 1));
  chapStyle.setAttribute("aria-label", t("Chapter starts with style"));
  const chapSep = selectOf("pn-chap-sep", SEPARATORS.map(([v, l]) => [v, t(l)] as const), info?.chapSep ?? "hyphen");
  chapSep.setAttribute("aria-label", t("Separator"));
  const example = h("p", { class: "help", id: "pn-example" });
  const paintChapter = () => {
    chapStyle.disabled = !chapter.input.checked;
    chapSep.disabled = !chapter.input.checked;
    const sep = SEPARATORS.find(([v]) => v === chapSep.value)?.[2] ?? "-";
    const page = (n: number) => pageLabel(n, format === "decimal" ? undefined : format);
    example.textContent = chapter.input.checked ? t("Examples: {a}, {b}", { a: `1${sep}${page(1)}`, b: `2${sep}${page(1)}` }) : "";
  };
  for (const el of [chapter.input, chapStyle, chapSep]) el.addEventListener("change", paintChapter);
  for (const g of [formats, numbering, applyTo]) g?.paint();
  paintChapter();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Page numbers") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Page numbers"))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", {}, t("Format")), formats.el),
      h("div", { class: "field" }, h("label", {}, t("Numbering")), numbering.el, h("div", { style: "margin-top:8px" }, start)),
      h("div", { class: "field" }, chapter.row, h("div", { class: "bs-cols" }, h("div", { class: "field" }, h("label", { for: "pn-chap-style" }, t("Chapter starts with style")), chapStyle), h("div", { class: "field" }, h("label", { for: "pn-chap-sep" }, t("Separator")), chapSep)), example),
      applyTo && h("div", { class: "field" }, h("label", {}, t("Apply to")), applyTo.el),
    ),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    const n = restart ? Math.max(0, Math.round(parseFloat(start.value) || 0)) : undefined;
    const chap = chapter.input.checked ? { chap_style: Number(chapStyle.value), chap_sep: chapSep.value } : { chap_style: 0 };
    const ops: Op[] = [];
    if (scope === "section") {
      const section = ed.sectionAnchor(si);
      ops.push({ op: "set_page_numbering", ...(section !== undefined ? { section } : {}), format, ...chap, ...(n !== undefined ? { start: n } : {}) });
    } else {
      // The whole document counts on from its first page, which may start at another number.
      ops.push({ op: "set_page_numbering", format, ...chap });
      const first = several ? ed.sectionAnchor(0) : undefined;
      if (n !== undefined) ops.push({ op: "set_page_numbering", ...(first !== undefined ? { section: first } : {}), format, start: n });
    }
    close();
    ed.exec(ops);
    ed.reloadPageSetup();
    ed.focus();
  });
}
