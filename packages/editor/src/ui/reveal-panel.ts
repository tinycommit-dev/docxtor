// Reveal formatting: for the selected text, which formatting it has and where each setting comes from (the document's
// defaults, the paragraph style, a character style, or formatting put on by hand), with a link to clear each layer.
// The engine does the sorting (`Engine.revealFormatting`); this draws it and offers the clears.
import type { Op, Reveal, RevealRow } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { paraText } from "../editor/multi-select";
import { formatUnits, unitName } from "../units";
import { h } from "./dom";
import { styleTitle } from "./style-preview";
import { hex } from "../editor/color.ts";

const LABELS: Record<string, () => string> = {
  font: () => t("Font"),
  size: () => t("Size"),
  bold: () => t("Bold"),
  italic: () => t("Italic"),
  underline: () => t("Underline"),
  strike: () => t("Strikethrough"),
  allCaps: () => t("All caps"),
  smallCaps: () => t("Small caps"),
  color: () => t("Font color"),
  highlight: () => t("Highlight"),
  shading: () => t("Text shading"),
  vertAlign: () => t("Position"),
  characterSpacing: () => t("Character spacing"),
  hidden: () => t("Hidden"),
  language: () => t("Language"),
  alignment: () => t("Alignment"),
  indentLeft: () => t("Left indent"),
  indentRight: () => t("Right indent"),
  firstLine: () => t("First line"),
  spaceBefore: () => t("Space before"),
  spaceAfter: () => t("Space after"),
  lineSpacing: () => t("Line spacing"),
  keepWithNext: () => t("Keep with next"),
  keepLinesTogether: () => t("Keep lines together"),
  pageBreakBefore: () => t("Page break before"),
  widowControl: () => t("Widow and orphan control"),
  outlineLevel: () => t("Outline level"),
  list: () => t("List"),
  borders: () => t("Borders"),
  paragraphShading: () => t("Paragraph shading"),
  tabs: () => t("Tab stops"),
  rightToLeft: () => t("Right to left"),
};

const pt = (twips: number) => `${Math.round((twips / 20) * 100) / 100} ${t("pt")}`;

/** A setting's value in words. */
export function revealValue(row: RevealRow): string {
  const v = row.value;
  if (typeof v === "boolean") return v ? t("On") : t("Off");
  if (typeof v === "object") {
    const exact = v.rule === "exact" || v.rule === "atLeast";
    return exact ? `${v.rule === "exact" ? t("Exactly") : t("At least")} ${pt(v.line)}` : t("{n} lines", { n: String(Math.round((v.line / 240) * 100) / 100) });
  }
  switch (row.key) {
    case "size":
      return `${v} ${t("pt")}`;
    case "color":
    case "shading":
    case "paragraphShading":
      return hex(String(v)) ?? String(v);
    case "indentLeft":
    case "indentRight":
    case "firstLine":
      return `${formatUnits(Math.abs(Number(v)))} ${unitName()}${row.key === "firstLine" ? ` ${Number(v) < 0 ? t("(hanging)") : t("(first line)")}` : ""}`;
    case "spaceBefore":
    case "spaceAfter":
    case "characterSpacing":
      return pt(Number(v));
    case "alignment": {
      const names: Record<string, string> = { left: t("Left"), start: t("Left"), center: t("Centered"), right: t("Right"), end: t("Right"), both: t("Justified"), distribute: t("Distributed") };
      return names[String(v)] ?? String(v);
    }
    case "tabs":
      return String(v);
    default:
      return String(v);
  }
}

export class RevealPanel {
  readonly el: HTMLElement;
  private body = h("div", { class: "reveal-body" });

  constructor(private ed: Editor) {
    this.el = h("div", { class: "reveal-pane" }, this.body);
  }

  /** The formatting at the selection, read again. */
  update() {
    const engine = this.ed.engine;
    const r = this.ed.range();
    if (!engine || !r) {
      this.body.replaceChildren(h("p", { class: "empty-note" }, t("Click in the text to see its formatting.")));
      return;
    }
    const collapsed = this.ed.collapsed();
    const reveal = engine.revealFormatting({ p: r[0].p, o: collapsed ? r[0].o : r[0].o + 1 });
    if (!reveal) {
      this.body.replaceChildren(h("p", { class: "empty-note" }, t("Click in the text to see its formatting.")));
      return;
    }
    this.draw(reveal);
  }

  private source(row: RevealRow): string {
    switch (row.source) {
      case "default":
        return t("Document default");
      case "paragraphStyle":
        return t("Style: {style}", { style: styleTitle(row.style ?? "", row.custom) });
      case "characterStyle":
        return t("Character style: {style}", { style: styleTitle(row.style ?? "", row.custom) });
      default:
        return t("Set by hand");
    }
  }

  private draw(reveal: Reveal) {
    const ed = this.ed;
    const clear = (label: string, run: () => void) => {
      const b = h("button", { class: "btn link", type: "button" }, label);
      b.addEventListener("click", () => {
        run();
        ed.focus();
        this.update();
      });
      return b;
    };
    const layer = (title: string, detail: string | null, action: HTMLElement | null) => h("div", { class: "reveal-layer" }, h("div", { class: "reveal-layer-text" }, h("span", { class: "reveal-layer-title" }, title), detail ? h("span", { class: "reveal-layer-detail" }, detail) : null), action);
    // With only a caret, the text layer is the paragraph's text.
    const sel = (): { from: { p: number; o: number }; to: { p: number; o: number } } | null => {
      const r = ed.range();
      if (!r) return null;
      if (!ed.collapsed()) return { from: r[0], to: r[1] };
      const el = ed.paraElement(r[0].p);
      const len = el ? paraText(el).length : r[0].o;
      return { from: { p: r[0].p, o: 0 }, to: { p: r[0].p, o: len } };
    };
    const layers = h("div", { class: "reveal-layers" });
    const para = reveal.paragraphStyle;
    const chain = [para, ...reveal.basedOn];
    layers.append(
      layer(
        t("Paragraph style"),
        chain.map((s) => styleTitle(s.name, s.custom)).join(", "),
        para.id !== "Normal" ? clear(t("Use Normal"), () => ed.setStyle("Normal")) : null,
      ),
    );
    if (reveal.characterStyle) {
      layers.append(layer(t("Character style"), styleTitle(reveal.characterStyle.name, reveal.characterStyle.custom), clear(t("Remove"), () => {
            const s = sel();
            if (s) ed.exec({ op: "format_text", from: s.from, to: s.to, format: { style: "" } }, { select: ed.sel, keepExtra: true });
          })));
    }
    layers.append(
      layer(
        t("Formatting by hand"),
        reveal.directRun || reveal.directParagraph ? [reveal.directParagraph ? t("paragraph") : "", reveal.directRun ? t("text") : ""].filter(Boolean).join(", ") : t("None"),
        reveal.directRun || reveal.directParagraph
          ? h(
              "span",
              { class: "reveal-clears" },
              reveal.directRun
                ? clear(t("Clear text"), () => {
                    const s = sel();
                    if (!s) return;
                    const ops: Op[] = [{ op: "format_text", from: s.from, to: s.to, format: { clear: true } }];
                    if (reveal.characterStyle) ops.push({ op: "format_text", from: s.from, to: s.to, format: { style: reveal.characterStyle.id } });
                    ed.exec(ops, { select: ed.sel, keepExtra: true });
                  })
                : null,
              reveal.directParagraph
                ? clear(t("Clear paragraph"), () => {
                    const ids = ed.selectedParagraphs();
                    ed.exec({ op: "format_paragraphs", ids, format: { clear: true, style: para.id } }, { select: ed.sel, keepExtra: true });
                  })
                : null,
            )
          : null,
      ),
    );
    const group = (title: string, rows: RevealRow[]) => {
      if (!rows.length) return null;
      return h(
        "section",
        { class: "reveal-group", "aria-label": title },
        h("h3", {}, title),
        ...rows.map((row) =>
          h(
            "div",
            { class: `reveal-row src-${row.source}`, "data-key": row.key },
            h("span", { class: "reveal-key" }, (LABELS[row.key] ?? (() => row.key))()),
            h("span", { class: "reveal-value" }, row.key === "color" || row.key === "shading" || row.key === "paragraphShading" ? h("i", { class: "reveal-swatch", style: hex(String(row.value)) ? `background:${hex(String(row.value))}` : "" }) : null, revealValue(row)),
            h("span", { class: "reveal-source" }, this.source(row)),
          ),
        ),
      );
    };
    this.body.replaceChildren(layers, ...[group(t("Text"), reveal.rows.filter((r) => r.group === "font")), group(t("Paragraph"), reveal.rows.filter((r) => r.group === "paragraph"))].filter((x): x is HTMLElement => !!x));
  }
}
