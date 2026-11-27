// Building blocks for the layout dialogs (page setup, page borders, columns, note options, hyphenation, drop caps, fill
// effects): labelled fields, selects, number and length inputs, check boxes, a row of buttons that picks one value, tabs,
// and the choice of where a change applies.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { formatUnits, parseLength, unitStep, unitWord } from "../units";
import { h } from "./dom";

export const field = (label: string, control: HTMLElement, id?: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);

export const selectOf = (id: string, items: readonly (readonly [string, string])[], value: string, label?: string) => {
  const el = h("select", { id, "aria-label": label }, ...items.map(([v, text]) => h("option", { value: v, selected: v === value }, text)));
  el.value = value;
  return el;
};

/** A number field in the current unit (see units.ts): its value is a number of inches or centimetres. */
export const unitInput = (id: string, twips: number, label: string, min = 0) =>
  h("input", { id, type: "number", min: String(min), step: String(unitStep()), value: formatUnits(twips), "aria-label": label, style: "width:100%;min-width:0" });

/** The twips a unit field holds, or `fallback` when it holds nothing readable. */
export function twipsOf(input: HTMLInputElement, fallback: number): number {
  const v = parseLength(input.value);
  return v === null ? fallback : Math.max(0, v);
}

/**
 * Like `twipsOf`, but a field still showing what it was given for `was` gives back `was` itself: the field shows a
 * rounded number (0.35 in for 500 twips), and an untouched value must not drift to what the rounding says.
 */
export function twipsOrWas(input: HTMLInputElement, was: number): number {
  return input.value === formatUnits(was) ? was : twipsOf(input, was);
}

export const unitLabel = (text: string) => t("{label} ({unit})", { label: text, unit: unitWord() });

export const check = (id: string, label: string, on: boolean) => {
  const input = h("input", { id, type: "checkbox", checked: on });
  return { input, row: h("label", { class: "check", for: id }, input, h("span", {}, label)) };
};

/** A row of buttons of which one is on, like the orientation switch in the page setup. */
export function choice<T extends string>(label: string, items: readonly (readonly [T, string])[], initial: T, change?: (v: T) => void) {
  const el = h("div", { class: "seg", role: "radiogroup", "aria-label": label });
  let value = initial;
  const buttons = items.map(([v, text]) => {
    const b = h("button", { type: "button", role: "radio", "aria-checked": String(v === value) }, text);
    b.addEventListener("click", () => {
      set(v);
      change?.(v);
    });
    el.append(b);
    return [v, b] as const;
  });
  const paint = () => {
    for (const [v, b] of buttons) {
      b.classList.toggle("on", v === value);
      b.setAttribute("aria-checked", String(v === value));
    }
  };
  const set = (v: T) => {
    value = v;
    paint();
  };
  paint();
  return {
    el,
    get value() {
      return value;
    },
    set,
  };
}

/** Tabs in a dialog: a row of buttons and one pane for each, one shown at a time. */
export function tabs(id: string, items: readonly (readonly [string, HTMLElement])[], first = 0, onShow?: (n: number) => void) {
  const list = h("div", { class: "pd-tablist seg", role: "tablist" });
  const panes = items.map(([, pane], i) => {
    pane.classList.add("pd-pane");
    pane.setAttribute("role", "tabpanel");
    pane.id = `${id}-pane-${i}`;
    pane.setAttribute("aria-labelledby", `${id}-tab-${i}`);
    return pane;
  });
  const buttons = items.map(([label], i) => {
    const b = h("button", { type: "button", role: "tab", id: `${id}-tab-${i}`, "aria-controls": `${id}-pane-${i}`, "aria-selected": "false" }, label);
    b.addEventListener("click", () => show(i));
    list.append(b);
    return b;
  });
  const show = (n: number) => {
    buttons.forEach((b, i) => {
      b.classList.toggle("on", i === n);
      b.setAttribute("aria-selected", String(i === n));
    });
    panes.forEach((p, i) => (p.hidden = i !== n));
    onShow?.(n);
  };
  show(first);
  return { list, panes, show };
}

export const group = (title: string, ...body: (HTMLElement | null)[]) => h("div", { class: "pd-group" }, h("h3", {}, title), ...body);
export const grid = (...cells: (HTMLElement | null)[]) => h("div", { class: "pd-grid" }, ...cells);

/** Where a change applies: the whole document, the section the caret is in, or from the caret on (which starts a section there). */
export type Scope = "document" | "section" | "forward";

/**
 * The "Apply to" choice. A document of one section offers the whole document and this point forward; more sections add
 * this section. In a header, footer or note there is no caret in the text, so there is only the whole document and the
 * section (when there are several); nothing is shown when there would be no choice.
 */
export function scopeChoice(ed: Editor, label = t("Apply to")) {
  const several = (ed.pageInfo.sections?.length ?? 1) > 1;
  const inBody = ed.caretParagraph() !== undefined;
  const items: [Scope, string][] = [["document", t("Whole document")]];
  if (several && inBody) items.push(["section", t("This section")]);
  if (inBody) items.push(["forward", t("This point forward")]);
  const c = choice<Scope>(label, items, "document");
  // `c.value` is a getter: it has to stay one.
  return Object.assign(c, { shown: items.length > 1, field: items.length > 1 ? h("div", { class: "field" }, h("label", {}, label), c.el) : null });
}

/**
 * Apply a change to the sections a scope names. `build` makes the ops for one section (`undefined` is the whole
 * document, as the ops take it). "This point forward" starts a new section at the caret first (a next-page break) and
 * changes it and every section after it, in one undo step.
 */
export function applyScoped(ed: Editor, scope: Scope, build: (section: number | undefined) => unknown[]) {
  const ops: unknown[] = [];
  if (scope === "document") {
    ops.push(...build(undefined));
    ed.exec(ops as never);
    ed.reloadPageSetup();
    return;
  }
  if (scope === "section") {
    ops.push(...build(ed.caretParagraph()));
    ed.exec(ops as never);
    ed.reloadPageSetup();
    return;
  }
  const at = ed.range()?.[0];
  if (!at) return;
  const made = ed.exec({ op: "insert_break", at, kind: "next_page" });
  const caret = made?.find((r) => r.caret)?.caret;
  ed.reloadPageSetup();
  const first = caret ? ed.engine.formatAt(caret)?.section : undefined;
  const sections = ed.pageInfo.sections ?? [];
  if (first === undefined) return;
  const rest: unknown[] = [];
  for (let i = first; i < sections.length; i++) rest.push(...build(ed.sectionAnchor(i)));
  ed.exec(rest as never, { merge: true });
  ed.reloadPageSetup();
}
