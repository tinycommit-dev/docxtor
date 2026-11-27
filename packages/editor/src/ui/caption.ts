// Caption: a numbered label and text above or below the selected image or table (or the paragraph), with a preview.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { captionNumber, CHAPTER_SEPARATORS } from "../editor/lists";
import { openAutoCaption } from "./auto-caption";
import { h, modal } from "./dom";

const NEW_LABEL = "\u0000new";

/** The labels the dialog offers: Word's three, then any others the document already uses. */
export function captionLabels(ed: Editor): string[] {
  const out = [t("Figure"), t("Table"), t("Equation")];
  for (const c of ed.xrefTargets().captions) if (!out.includes(c.label)) out.push(c.label);
  return out;
}

/** Tables are captioned above, everything else below (Word's defaults). */
const defaultPosition = (label: string): "above" | "below" => (label === t("Table") || label === "Table" ? "above" : "below");

export function openCaption(ed: Editor) {
  const target = ed.captionTarget();
  if (!target) return;
  const labels = captionLabels(ed);
  // A table asks for a table caption; an image for a figure.
  let label = target.kind === "table" ? t("Table") : t("Figure");
  let position: "above" | "below" = defaultPosition(label);
  let positionTouched = false;

  const select = h("select", { id: "cap-label" }, ...labels.map((l) => h("option", { value: l }, l)), h("option", { value: NEW_LABEL }, t("New label…")));
  select.value = label;
  const custom = h("input", { type: "text", id: "cap-custom", "aria-label": t("New label"), placeholder: t("Label, such as Chart"), spellcheck: "false", autocomplete: "off", hidden: true });
  const text = h("input", { type: "text", id: "cap-text", "aria-label": t("Caption text"), placeholder: t("Describe it"), autocomplete: "off" });
  const preview = h("p", { class: "cap-preview", "aria-live": "polite" });

  // Numbering by chapter ("Figure 2-1"): on when the label's captions already are numbered that way.
  const numbered = ed.xrefTargets().headings.some((x) => x.number);
  const used = ed.xrefTargets().captions.map((c) => ({ label: c.label, m: /^(\d+(?:\.\d+)*)([-.:\u2013\u2014])\d+$/.exec(c.number) }));
  const chapterOn = h("input", { type: "checkbox", id: "cap-chapter", disabled: !numbered });
  const chapterLevel = h("select", { id: "cap-level", "aria-label": t("Chapter starts with style") }, ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => h("option", { value: String(n) }, t("Heading {n}", { n }))));
  const chapterSep = h("select", { id: "cap-sep", "aria-label": t("Separator") }, ...CHAPTER_SEPARATORS.map(([v, name]) => h("option", { value: v }, t(name))));
  const chapterBox = h("div", { class: "cap-chapter", hidden: true }, h("label", { for: "cap-level" }, t("Chapter starts with style")), chapterLevel, h("label", { for: "cap-sep" }, t("Separator")), chapterSep);
  const chapterHelp = h("p", { class: "help" }, numbered ? "" : t("Number the headings first (Format, Bullets and numbering) to include the chapter number."));
  const syncChapter = () => {
    const l = current();
    const hit = used.find((u) => u.label === l && u.m);
    if (hit && !chapterOn.checked && !chapterOn.dataset.touched && numbered) {
      chapterOn.checked = true;
      chapterSep.value = hit.m![2];
    }
    chapterBox.hidden = !chapterOn.checked;
  };
  chapterOn.addEventListener("change", () => {
    chapterOn.dataset.touched = "1";
    paint();
  });
  chapterLevel.addEventListener("change", () => paint());
  chapterSep.addEventListener("change", () => paint());

  const seg = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Position") });
  const buttons = (
    [
      ["above", t("Above")],
      ["below", t("Below")],
    ] as const
  ).map(([v, text]) => {
    const b = h("button", { type: "button", role: "radio" }, text);
    b.addEventListener("click", () => {
      position = v;
      positionTouched = true;
      paint();
    });
    seg.append(b);
    return [v, b] as const;
  });
  const what = target.kind === "image" ? t("the selected image") : target.kind === "table" ? t("the table") : t("the paragraph");
  const where = h("p", { class: "help" });

  const current = () => (select.value === NEW_LABEL ? custom.value.trim() : select.value);
  const paint = () => {
    for (const [v, b] of buttons) {
      b.classList.toggle("on", v === position);
      b.setAttribute("aria-checked", String(v === position));
    }
    const l = current() || t("Label");
    syncChapter();
    const chapter = chapterOn.checked ? { level: Number(chapterLevel.value), separator: chapterSep.value } : undefined;
    const n = captionNumber(ed, l, target, position, chapter);
    preview.textContent = n === null ? t("There is no numbered heading of that level above this place.") : `${l} ${n}${text.value.trim() ? `: ${text.value.trim()}` : ""}`;
    where.textContent = position === "above" ? t("The caption goes above {what}.", { what }) : t("The caption goes below {what}.", { what });
    insert.disabled = !current() || (chapterOn.checked && captionNumber(ed, l, target, position, { level: Number(chapterLevel.value), separator: chapterSep.value }) === null);
  };
  select.addEventListener("change", () => {
    custom.hidden = select.value !== NEW_LABEL;
    if (!custom.hidden) custom.focus();
    if (!positionTouched && select.value !== NEW_LABEL) position = defaultPosition(select.value);
    paint();
  });
  custom.addEventListener("input", paint);
  text.addEventListener("input", paint);

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const auto = h("button", { class: "btn ghost", type: "button" }, t("AutoCaption…"));
  const insert = h("button", { class: "btn primary", type: "button" }, t("Insert"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Caption") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Caption"))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: "cap-label" }, t("Label")), select, custom),
      h("div", { class: "field" }, h("label", { for: "cap-text" }, t("Text")), text),
      h("div", { class: "field" }, h("label", {}, t("Position")), seg, where),
      h("div", { class: "field" }, h("label", { class: "check", for: "cap-chapter" }, chapterOn, h("span", {}, t("Include chapter number"))), chapterBox, chapterHelp),
      h("div", { class: "field" }, h("label", {}, t("Preview")), preview),
    ),
    h("div", { class: "dialog-actions" }, auto, cancel, insert),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  auto.addEventListener("click", () => {
    close();
    openAutoCaption(ed);
  });
  const go = () => {
    const l = current();
    if (!l || insert.disabled) return;
    close();
    ed.insertCaption({ label: l, text: text.value.trim(), position, target, ...(chapterOn.checked ? { chapter: { level: Number(chapterLevel.value), separator: chapterSep.value } } : {}) });
  };
  insert.addEventListener("click", go);
  for (const el of [text, custom]) {
    el.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        go();
      }
    });
  }
  paint();
  text.focus();
}
