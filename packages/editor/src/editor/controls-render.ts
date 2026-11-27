// Content controls and legacy form fields on the page. The engine says which stretch of a paragraph each control
// covers (`RPara.controls`); the segments inside it get classes and data attributes here, so the controls tool
// (ui/controls.ts) can find them, outline them and open their editors. Nothing is wrapped around the text: the
// selection mapping counts on the paragraph's segments being direct children.
import { t } from "@docxtor/extensions";
import type { RControl, RForm, RPara, RSeg } from "@docxtor/core";
import { decorateChip } from "./chips";
import { hex } from "./render";

/** What each kind is called in the control's tab when the file gives it no title. */
export function kindLabel(kind: string): string {
  switch (kind) {
    case "checkbox":
      return t("Check box");
    case "dropDown":
      return t("Drop-down list");
    case "comboBox":
      return t("Combo box");
    case "date":
      return t("Date picker");
    case "text":
      return t("Plain text");
    case "richText":
      return t("Rich text");
    case "picture":
      return t("Picture");
    case "repeating":
      return t("Repeating section");
    case "repeatingItem":
      return t("Item");
    case "group":
      return t("Group");
    case "bibliography":
      return t("Bibliography");
    default:
      return t("Content control");
  }
}

/** What each smart chip is called to someone using a screen reader, and in its tooltip. */
export function chipLabel(kind: string): string {
  switch (kind) {
    case "person":
      return t("Person chip");
    case "date":
      return t("Date chip");
    case "file":
      return t("File chip");
    case "place":
      return t("Place chip");
    case "dropdown":
      return t("Drop-down chip");
    default:
      return t("Smart chip");
  }
}

/** Kinds that hold typed text; the others are picked from, ticked or filled by a button. */
export const TYPED = new Set(["text", "richText", "comboBox", "group"]);

/** The name a control shows in its tab. */
export function controlName(c: RControl): string {
  return c.alias || c.tag || kindLabel(c.kind);
}

/** Mark the segments of a rendered paragraph that lie inside its content controls. Innermost control wins. */
export function decorateControls(el: HTMLElement, p: RPara) {
  const controls = p.controls ?? [];
  if (!controls.length) return;
  const segEls = Array.from(el.children).filter((c): c is HTMLElement => c instanceof HTMLElement && c.dataset.o !== undefined);
  for (const c of controls) {
    if (c.kind === "citation" || c.kind === "bibliography") continue;
    const mine = segEls.filter((s) => {
      const o = Number(s.dataset.o);
      return o >= c.start && o < c.end;
    });
    mine.forEach((s, i) => {
      s.classList.add("ctl");
      s.dataset.ctl = String(c.n);
      s.dataset.ck = c.kind;
      if (i === 0) s.classList.add("ctl-start");
      if (i === mine.length - 1) s.classList.add("ctl-end");
      s.classList.toggle("ctl-ph", !!c.placeholder);
      s.classList.toggle("ctl-locked", !!c.lock);
      if (hex(c.color)) s.style.setProperty("--ctl-color", hex(c.color)!);
      if (c.kind === "checkbox") {
        s.classList.add("ctl-check");
        s.setAttribute("role", "checkbox");
        s.setAttribute("aria-checked", String(!!c.checked));
        s.setAttribute("aria-label", controlName(c));
      }
      if (c.appearance === "hidden") s.classList.add("ctl-hidden");
    });
    decorateChip(mine, c, chipLabel);
  }
}

/** The element of a drop-down form field, which has no text of its own: it shows the entry chosen. (A check box is drawn from its own `ff` segment.) */
export function formAtom(seg: RSeg, forms: RForm[] | undefined): HTMLElement {
  const [kind, n] = (seg.x ?? "").split(":");
  const f = forms?.find((x) => x.n === Number(n));
  const el = document.createElement("span");
  el.dataset.frm = n;
  el.dataset.fk = kind;
  el.contentEditable = "false";
  el.className = "atom frm frm-select";
  el.setAttribute("role", "combobox");
  el.setAttribute("aria-label", f?.name || t("Drop-down list"));
  el.append(Object.assign(document.createElement("span"), { textContent: seg.label || " " }), Object.assign(document.createElement("i"), { innerHTML: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 6.5 3.5 3.5 3.5-3.5" vector-effect="non-scaling-stroke"/></svg>' }));
  if (f?.disabled) el.classList.add("frm-off");
  return el;
}
