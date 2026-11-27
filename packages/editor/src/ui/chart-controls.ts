// Small controls the Edit chart dialog is made of: labelled fields, checkboxes, selects and number inputs that can
// be left on Automatic.
import { h } from "./dom";

let uid = 0;
export const uniq = (prefix: string): string => `${prefix}-${++uid}`;

export function field(label: string, control: HTMLElement, help?: string): HTMLElement {
  if (!control.id) control.id = uniq("cf");
  return h("div", { class: "field" }, h("label", { for: control.id }, label), control, help ? h("p", { class: "help" }, help) : null);
}

export function check(label: string, checked: boolean, onChange: (on: boolean) => void, disabled = false): HTMLLabelElement {
  const input = h("input", { type: "checkbox", disabled });
  input.checked = checked;
  input.addEventListener("change", () => onChange(input.checked));
  return h("label", { class: "check" }, input, h("span", {}, label));
}

export function select<T extends string>(options: [T, string][], value: T, onChange: (v: T) => void, label?: string): HTMLSelectElement {
  const s = h("select", { "aria-label": label }, ...options.map(([v, l]) => h("option", { value: v }, l)));
  s.value = value;
  s.addEventListener("change", () => onChange(s.value as T));
  return s;
}

/** Text that is a number, or empty for Automatic. */
export function numberInput(value: number | undefined, onChange: (v: number | undefined) => void, opts: { placeholder?: string; label?: string; min?: number; max?: number; step?: number } = {}): HTMLInputElement {
  const input = h("input", { type: "text", inputmode: "decimal", autocomplete: "off", spellcheck: "false", placeholder: opts.placeholder, "aria-label": opts.label, value: value === undefined ? "" : String(value) });
  input.addEventListener("input", () => {
    const raw = input.value.trim().replace(",", ".");
    if (raw === "") {
      input.removeAttribute("aria-invalid");
      onChange(undefined);
      return;
    }
    const n = Number(raw);
    const ok = Number.isFinite(n) && /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(raw) && (opts.min === undefined || n >= opts.min) && (opts.max === undefined || n <= opts.max);
    input.toggleAttribute("aria-invalid", !ok);
    if (ok) onChange(n);
  });
  return input;
}

export function group(title: string, ...children: (Node | null | false)[]): HTMLElement {
  return h("fieldset", { class: "chart-group" }, h("legend", {}, title), ...children);
}

export function row(...children: (Node | null | false)[]): HTMLElement {
  return h("div", { class: "chart-two" }, ...children);
}
