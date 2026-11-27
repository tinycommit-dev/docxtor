// Controls for side panels such as Format shape: collapsible sections, labelled rows, number and length fields, sliders,
// selects, radio groups and colour fields. Each builder returns the row's element and a `set` that shows a value from the
// model without disturbing a control the person is typing in. A control commits on `change`, not on every keystroke.
import { t } from "@docxtor/extensions";
import { formatUnits, parseLength, unitName } from "../units";
import { h } from "./dom";
import { I } from "./icons";
import { pickColor } from "./shape-tool";
import { hex as hexColor } from "../editor/color.ts";

let counter = 0;
const uid = (kind: string) => `fs-${kind}-${++counter}`;

/** Which sections the person left open; kept for the session. */
const opened = new Map<string, boolean>();

export interface PanelSection {
  el: HTMLElement;
  body: HTMLElement;
  /** Show or hide the whole section (a shape that has no text frame has no Text box section). */
  setHidden(on: boolean): void;
  readonly isOpen: boolean;
}

/** A section with a heading button that opens and closes it. */
export function section(id: string, title: string, startOpen = false): PanelSection {
  const bodyId = uid("body");
  const body = h("div", { class: "fs-body", id: bodyId, role: "group", "aria-label": title });
  const btn = h("button", { type: "button", class: "fs-toggle", "aria-controls": bodyId, "aria-expanded": "false" }, h("span", { class: "fs-chevron", html: I.chevron }), h("span", {}, title));
  const el = h("section", { class: "fs-section", "data-section": id }, h("h3", { class: "fs-head" }, btn), body);
  const paint = (open: boolean) => {
    btn.setAttribute("aria-expanded", String(open));
    el.classList.toggle("open", open);
    body.hidden = !open;
  };
  const sec: PanelSection = {
    el,
    body,
    setHidden: (on) => {
      el.hidden = on;
    },
    get isOpen() {
      return opened.get(id) ?? startOpen;
    },
  };
  btn.addEventListener("click", () => {
    opened.set(id, !sec.isOpen);
    paint(sec.isOpen);
  });
  paint(sec.isOpen);
  return sec;
}

/** A labelled row: the label on the left, the controls on the right. `target` is the id of the control the label names. */
export function row(label: string, target: string | null, ...controls: (HTMLElement | null)[]): HTMLElement {
  const lab = target ? h("label", { for: target }, label) : h("span", { class: "fs-label", id: uid("lab") }, label);
  return h("div", { class: "fs-row" }, lab, h("div", { class: "fs-ctl" }, ...controls));
}

/** A small heading inside a section. */
export function subhead(text: string): HTMLElement {
  return h("div", { class: "fs-sub" }, text);
}

export interface Field<T> {
  el: HTMLElement;
  set(v: T): void;
  disable(on: boolean): void;
}

const same = (a: HTMLElement) => document.activeElement === a;

function trim(n: number, digits: number): string {
  const f = 10 ** digits;
  return String(Math.round(n * f) / f);
}

/** A number in a unit shown beside the field (`pt`, `%`, `°`). Commits on change (Enter or leaving the field). */
export function numberField(o: { label: string; unit?: string; min?: number; max?: number; step?: number; digits?: number; commit: (v: number) => void; blank?: boolean }): Field<number | undefined> {
  const id = uid("num");
  const digits = o.digits ?? 1;
  const input = h("input", { type: "number", id, class: "fs-num", step: o.step ?? 1, min: o.min, max: o.max, inputmode: "decimal", autocomplete: "off" });
  let last: number | undefined;
  const show = (v: number | undefined) => {
    input.value = v === undefined || !Number.isFinite(v) ? "" : trim(v, digits);
  };
  input.addEventListener("change", () => {
    let v = input.valueAsNumber;
    if (!Number.isFinite(v)) return show(last);
    if (o.min !== undefined) v = Math.max(o.min, v);
    if (o.max !== undefined) v = Math.min(o.max, v);
    v = Math.round(v * 10 ** digits) / 10 ** digits;
    show(v);
    if (v !== last) {
      last = v;
      o.commit(v);
    }
  });
  const unit = o.unit ? h("span", { class: "fs-unit", "aria-hidden": "true" }, o.unit) : null;
  return {
    el: row(o.unit ? `${o.label}` : o.label, id, input, unit),
    set(v) {
      last = v;
      if (!same(input)) show(v);
    },
    disable: (on) => {
      input.disabled = on;
    },
  };
}

/** A length in points, shown in the document's unit (inches or centimetres) and read with Word's unit suffixes. */
export function lengthField(o: { label: string; min?: number; commit: (pt: number) => void }): Field<number | undefined> {
  const id = uid("len");
  const input = h("input", { type: "text", id, class: "fs-num", inputmode: "decimal", autocomplete: "off" });
  let last: number | undefined;
  const show = (pt: number | undefined) => {
    input.value = pt === undefined || !Number.isFinite(pt) ? "" : formatUnits(pt * 20);
  };
  input.addEventListener("change", () => {
    const tw = parseLength(input.value);
    if (tw === null) return show(last);
    let pt = Math.round((tw / 20) * 10) / 10;
    if (o.min !== undefined) pt = Math.max(o.min, pt);
    show(pt);
    if (pt !== last) {
      last = pt;
      o.commit(pt);
    }
  });
  const unit = h("span", { class: "fs-unit", "aria-hidden": "true" }, unitName());
  return {
    el: row(o.label, id, input, unit),
    set(pt) {
      last = pt;
      unit.textContent = unitName();
      if (!same(input)) show(pt);
    },
    disable: (on) => {
      input.disabled = on;
    },
  };
}

/**
 * A slider with a number beside it. While the slider is dragged `live` runs for each step (the caller merges them into one
 * undo step); the number field and the end of a drag or a key press run `commit`.
 */
export function sliderField(o: { label: string; unit?: string; min: number; max: number; step?: number; digits?: number; live?: (v: number, first: boolean) => void; commit: (v: number) => void }): Field<number | undefined> {
  const id = uid("slider");
  const digits = o.digits ?? 0;
  const range = h("input", { type: "range", class: "fs-range", min: o.min, max: o.max, step: o.step ?? 1, "aria-label": o.label });
  const num = h("input", { type: "number", id, class: "fs-num fs-num-s", min: o.min, max: o.max, step: o.step ?? 1, inputmode: "decimal", autocomplete: "off" });
  let last: number | undefined;
  let dragging = false;
  const show = (v: number | undefined) => {
    const x = v === undefined || !Number.isFinite(v) ? o.min : v;
    if (!same(range)) range.value = String(x);
    if (!same(num)) num.value = v === undefined || !Number.isFinite(v) ? "" : trim(v, digits);
  };
  range.addEventListener("pointerdown", () => {
    dragging = false;
  });
  range.addEventListener("input", () => {
    const v = Number(range.value);
    num.value = trim(v, digits);
    if (o.live) {
      o.live(v, !dragging);
      dragging = true;
    }
  });
  range.addEventListener("change", () => {
    const v = Number(range.value);
    const wasLive = dragging;
    dragging = false;
    // A drag has been applied step by step already; a key press or a click on the track applies once.
    if (!wasLive || !o.live) {
      if (v !== last) o.commit(v);
    }
    last = v;
  });
  num.addEventListener("change", () => {
    let v = num.valueAsNumber;
    if (!Number.isFinite(v)) return show(last);
    v = Math.min(o.max, Math.max(o.min, v));
    range.value = String(v);
    num.value = trim(v, digits);
    if (v !== last) {
      last = v;
      o.commit(v);
    }
  });
  const unit = o.unit ? h("span", { class: "fs-unit", "aria-hidden": "true" }, o.unit) : null;
  return {
    el: h("div", { class: "fs-row fs-row-slider" }, h("label", { for: id }, o.label), h("div", { class: "fs-ctl" }, range, num, unit)),
    set(v) {
      last = v;
      show(v);
    },
    disable: (on) => {
      range.disabled = on;
      num.disabled = on;
    },
  };
}

/** A drop-down list. Options the model reports but the list lacks are added so the value always shows. */
export function selectField<T extends string>(o: { label: string; options: [T, string][]; groups?: { label: string; options: [T, string][] }[]; commit: (v: T) => void }): Field<T | undefined> & { select: HTMLSelectElement } {
  const id = uid("sel");
  const select = h("select", { id, class: "fs-select" });
  const add = (parent: HTMLElement, [v, label]: [T, string]) => parent.append(h("option", { value: v }, label));
  for (const opt of o.options) add(select, opt);
  for (const g of o.groups ?? []) {
    const og = h("optgroup", { label: g.label });
    for (const opt of g.options) add(og, opt);
    select.append(og);
  }
  select.addEventListener("change", () => o.commit(select.value as T));
  return {
    el: row(o.label, id, select),
    select,
    set(v) {
      if (v === undefined) return;
      let found = Array.from(select.options).some((x) => x.value === v);
      if (!found) {
        select.append(h("option", { value: v }, v));
        found = true;
      }
      select.value = v;
    },
    disable: (on) => {
      select.disabled = on;
    },
  };
}

/** A group of radio buttons, in one or two columns. */
export function radioField<T extends string>(o: { label: string; options: [T, string][]; columns?: 1 | 2; commit: (v: T) => void }): Field<T | undefined> {
  const name = uid("radio");
  const group = h("div", { class: `fs-radios${o.columns === 2 ? " two" : ""}`, role: "radiogroup", "aria-label": o.label });
  const inputs = o.options.map(([v, label]) => {
    const input = h("input", { type: "radio", name, value: v });
    input.addEventListener("change", () => {
      if (input.checked) o.commit(v);
    });
    group.append(h("label", { class: "fs-radio" }, input, h("span", {}, label)));
    return input;
  });
  return {
    el: group,
    set(v) {
      for (const i of inputs) i.checked = i.value === v;
    },
    disable: (on) => {
      for (const i of inputs) i.disabled = on;
    },
  };
}

/** A check box with its label beside it. */
export function checkField(o: { label: string; commit: (on: boolean) => void }): Field<boolean | undefined> & { input: HTMLInputElement } {
  const id = uid("chk");
  const input = h("input", { type: "checkbox", id });
  input.addEventListener("change", () => o.commit(input.checked));
  return {
    el: h("div", { class: "fs-check" }, input, h("label", { for: id }, o.label)),
    input,
    set(v) {
      input.checked = !!v;
    },
    disable: (on) => {
      input.disabled = on;
    },
  };
}

/** A segmented choice, for two or three short options. */
export function segField<T extends string>(o: { label: string; options: [T, string][]; commit: (v: T) => void }): Field<T | undefined> {
  const group = h("div", { class: "seg", role: "radiogroup", "aria-label": o.label });
  const buttons = o.options.map(([v, label]) => {
    const b = h("button", { type: "button", role: "radio", "aria-checked": "false" }, label);
    b.addEventListener("click", () => o.commit(v));
    group.append(b);
    return [v, b] as const;
  });
  return {
    el: row(o.label, null, group),
    set(v) {
      for (const [x, b] of buttons) {
        b.classList.toggle("on", x === v);
        b.setAttribute("aria-checked", String(x === v));
      }
    },
    disable: (on) => {
      for (const [, b] of buttons) b.disabled = on;
    },
  };
}

/** A button on its own line in a section. */
export function actionButton(label: string, run: () => void, extra = ""): HTMLButtonElement {
  const b = h("button", { type: "button", class: `btn fs-btn ${extra}`.trim() }, label);
  b.addEventListener("click", run);
  return b;
}

const HEX = /^[0-9a-f]{6}$/i;

/** A colour: a chip that opens the colour picker, and a field for the hex value. */
export function colorField(o: { label: string; none?: string; commit: (hex: string | null) => void }): Field<string | undefined> {
  const id = uid("hex");
  const chip = h("span", { class: "fs-chip none" });
  const btn = h("button", { type: "button", class: "fs-chipbtn", "aria-label": t("{label}: choose a color", { label: o.label }), title: t("Choose a color") }, chip);
  const input = h("input", { type: "text", id, class: "fs-hex", maxlength: 7, spellcheck: "false", autocomplete: "off", placeholder: o.none ? "" : "RRGGBB" });
  let last: string | undefined;
  const show = (hex: string | undefined) => {
    last = hex;
    chip.classList.toggle("none", !hex);
    chip.style.background = hexColor(hex) ?? "";
    if (!same(input)) input.value = hex ? hex.toUpperCase() : "";
  };
  btn.addEventListener("click", () => pickColor(btn, o.label, last, o.none ?? null, (c) => o.commit(c)));
  input.addEventListener("change", () => {
    const v = input.value.trim().replace(/^#/, "");
    if (!v && o.none) return o.commit(null);
    if (!HEX.test(v)) return show(last);
    o.commit(v.toUpperCase());
  });
  return {
    el: row(o.label, id, btn, input),
    set: show,
    disable: (on) => {
      btn.disabled = on;
      input.disabled = on;
    },
  };
}
