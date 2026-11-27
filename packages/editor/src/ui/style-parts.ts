// The small dialogs the Modify style dialog opens for the parts of a style that have no dialog of their own: the language
// and the list the style puts its paragraphs in. Each works on a draft and hands back what changed. (Font, Paragraph,
// Tabs and Borders and shading are the dialogs the selection uses, pointed at the draft.)
import type { ListDefinition } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { MENU_FONTS } from "../fonts";
import { languageChoices } from "./language";
import { h, modal } from "./dom";

/** The faces a font menu offers: Docxtor's list, the document's own fonts and the one chosen now. */
export function fontOptions(used: string[], current?: string): string[] {
  const all = [...MENU_FONTS];
  for (const f of [...used, current ?? ""]) if (f && !all.includes(f)) all.push(f);
  return all;
}

/** Choose the language of a style's text. `apply("")` takes the language off the style. */
export function openStyleLanguage(o: { current?: string; apply(lang: string): void; done(): void }) {
  const choices = languageChoices(o.current);
  const select = h("select", { id: "sl-lang" }, h("option", { value: "", selected: !o.current }, t("Same as the document")), ...choices.map((c) => h("option", { value: c.tag, selected: c.tag.toLowerCase() === o.current?.toLowerCase() }, c.label)));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Language") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Language")), h("p", { class: "sub" }, t("Spelling, grammar and hyphenation use this language for text in the style."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "sl-lang" }, t("Language")), select)),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, o.done);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    close();
    if (select.value !== (o.current ?? "")) o.apply(select.value);
  });
}

/** What the Numbering dialog can choose: no list, a list the document has, or a new one of a few common kinds. */
export type StyleList = { kind: "none" } | { kind: "existing"; numId: number } | { kind: "new"; format: string; text: string };

const NEW_LISTS: { id: string; label: string; format: string; text: string }[] = [
  { id: "bullet", label: msg("Bullets"), format: "bullet", text: "•" },
  { id: "decimal", label: msg("Numbers (1. 2. 3.)"), format: "decimal", text: "%1." },
  { id: "lowerLetter", label: msg("Lowercase letters (a. b. c.)"), format: "lowerLetter", text: "%1." },
  { id: "upperLetter", label: msg("Uppercase letters (A. B. C.)"), format: "upperLetter", text: "%1." },
  { id: "lowerRoman", label: msg("Lowercase Roman numerals (i. ii. iii.)"), format: "lowerRoman", text: "%1." },
];

/** Choose the list a style puts its paragraphs in. */
export function openStyleNumbering(o: { currentNumId?: number; lists: ListDefinition[]; apply(list: StyleList): void; done(): void }) {
  const choices: { value: string; label: string; list: StyleList }[] = [{ value: "none", label: t("No list"), list: { kind: "none" } }];
  for (const l of o.lists) choices.push({ value: `n${l.numId}`, label: l.name || t("List {n} in this document", { n: String(l.numId) }), list: { kind: "existing", numId: l.numId } });
  for (const n of NEW_LISTS) choices.push({ value: n.id, label: t(n.label), list: { kind: "new", format: n.format, text: n.text } });
  const start = o.currentNumId ? `n${o.currentNumId}` : "none";
  const select = h("select", { id: "sn-list" }, ...choices.map((c) => h("option", { value: c.value, selected: c.value === start || (start !== "none" && !choices.some((x) => x.value === start) && c.value === "none") }, c.label)));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Numbering") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Numbering")), h("p", { class: "sub" }, t("Paragraphs in the style join the list you choose."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "sn-list" }, t("List")), select)),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, o.done);
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    close();
    const pick = choices.find((c) => c.value === select.value);
    if (pick && pick.value !== start) o.apply(pick.list);
  });
}
