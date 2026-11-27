// A small form in a dialog, for the rules and the other mail merge dialogs that ask for a few words: a heading, the fields, and
// OK or Cancel. It resolves with what was typed, or null when the person gave up.
import { msg, t } from "@docxtor/extensions";
import { h, modal, svg } from "../ui/dom";
import { MAIL_ICONS } from "./icons";

export type FormField =
  | { id: string; label: string; kind: "text"; value?: string; placeholder?: string; help?: string }
  | { id: string; label: string; kind: "textarea"; value?: string; rows?: number; help?: string }
  | { id: string; label: string; kind: "select"; value?: string; options: { value: string; label: string }[]; help?: string }
  | { id: string; label: string; kind: "check"; value?: boolean; help?: string };

export type FormValues = Record<string, string | boolean>;

export interface FormOptions {
  title: string;
  sub?: string;
  fields: FormField[];
  ok: string;
  /** A message when the values aren't usable (shown in the dialog), or null. */
  validate?(values: FormValues): string | null;
  /** A field that shows only while this returns true (given the current values). */
  visible?(id: string, values: FormValues): boolean;
  /** A live preview under the fields: what the values give for a recipient, with buttons to move between recipients. */
  preview?: { count(): number; text(values: FormValues, position: number): string };
  /** A line of plain explanation under the fields that follows the values (what a choice does, a warning). */
  notes?(values: FormValues): string;
  /** Buttons at the left of the dialog's own (a button that opens another dialog). */
  extra?: HTMLElement[];
}

let seq = 0;

export function formDialog(opts: FormOptions, onClose?: () => void): Promise<FormValues | null> {
  return new Promise((resolve) => {
    const uid = `mmf-${++seq}`;
    const values: FormValues = {};
    const rows = new Map<string, HTMLElement>();
    const error = h("p", { class: "help mm-error", role: "alert" });
    const note = h("p", { class: "help mm-note", "aria-live": "polite" });
    const controls = opts.fields.map((f) => {
      const cid = `${uid}-${f.id}`;
      const help = f.help ? h("p", { class: "help" }, f.help) : null;
      let row: HTMLElement;
      if (f.kind === "check") {
        values[f.id] = !!f.value;
        const box = h("input", { type: "checkbox", id: cid, checked: !!f.value });
        box.addEventListener("change", () => {
          values[f.id] = box.checked;
          refresh();
        });
        row = h("div", { class: "field" }, h("label", { class: "check", for: cid }, box, h("span", {}, f.label)), help);
      } else if (f.kind === "select") {
        values[f.id] = f.value ?? f.options[0]?.value ?? "";
        const sel = h("select", { id: cid }, ...f.options.map((o) => h("option", { value: o.value }, o.label)));
        sel.value = String(values[f.id]);
        sel.addEventListener("change", () => {
          values[f.id] = sel.value;
          refresh();
        });
        row = h("div", { class: "field" }, h("label", { for: cid }, f.label), sel, help);
      } else if (f.kind === "textarea") {
        values[f.id] = f.value ?? "";
        const area = h("textarea", { id: cid, rows: f.rows ?? 4, spellcheck: "false" });
        area.value = String(values[f.id]);
        area.addEventListener("input", () => {
          values[f.id] = area.value;
          refresh();
        });
        row = h("div", { class: "field" }, h("label", { for: cid }, f.label), area, help);
      } else {
        values[f.id] = f.value ?? "";
        const input = h("input", { type: "text", id: cid, value: f.value ?? "", spellcheck: "false", autocomplete: "off", placeholder: f.placeholder ?? "" });
        input.addEventListener("input", () => {
          values[f.id] = input.value;
          refresh();
        });
        row = h("div", { class: "field" }, h("label", { for: cid }, f.label), input, help);
      }
      rows.set(f.id, row);
      return row;
    });
    const ok = h("button", { class: "btn primary", type: "button" }, opts.ok);
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    // The preview follows the values and the record shown.
    const pre = opts.preview ? h("pre", { class: "mm-preview", "aria-live": "polite", "aria-label": t("Preview") }) : null;
    const nav = opts.preview ? recordNav(opts.preview.count, () => paintPreview()) : null;
    const paintPreview = () => {
      if (!pre || !nav || !opts.preview) return;
      let text = "";
      try {
        text = opts.preview.text(values, nav.position());
      } catch {
        text = "";
      }
      pre.textContent = text;
    };
    const refresh = () => {
      for (const [id, row] of rows) row.hidden = opts.visible ? !opts.visible(id, values) : false;
      error.textContent = "";
      note.textContent = opts.notes?.(values) ?? "";
      note.hidden = !note.textContent;
      paintPreview();
    };
    let done = false;
    const finish = (v: FormValues | null) => {
      if (done) return;
      done = true;
      close();
      resolve(v);
    };
    const submit = () => {
      const problem = opts.validate?.(values) ?? null;
      if (problem) {
        error.textContent = problem;
        return;
      }
      finish({ ...values });
    };
    const dialog = h(
      "div",
      { class: "dialog mm-dialog", role: "dialog", "aria-modal": "true", "aria-label": opts.title },
      h("div", { class: "dialog-head" }, h("h2", {}, opts.title), opts.sub ? h("p", { class: "sub" }, opts.sub) : null),
      h("div", { class: "dialog-body" }, ...controls, pre ? h("div", { class: "field" }, h("label", {}, t("Preview")), nav!.el, pre) : null, note, error),
      h("div", { class: "dialog-actions" }, ...(opts.extra ?? []), cancel, ok),
    );
    dialog.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !(e.target instanceof HTMLTextAreaElement) && !(e.target instanceof HTMLButtonElement)) {
        e.preventDefault();
        submit();
      }
    });
    ok.addEventListener("click", submit);
    cancel.addEventListener("click", () => finish(null));
    const close = modal(dialog, () => {
      if (!done) {
        done = true;
        resolve(null);
      }
      onClose?.();
    });
    refresh();
  });
}

/** The comparisons the rules offer, as the engine names them. */
export const COMPARISONS: { value: string; label: string }[] = [
  { value: "eq", label: msg("Equal to") },
  { value: "ne", label: msg("Not equal to") },
  { value: "lt", label: msg("Less than") },
  { value: "gt", label: msg("Greater than") },
  { value: "le", label: msg("Less than or equal to") },
  { value: "ge", label: msg("Greater than or equal to") },
  { value: "empty", label: msg("Is blank") },
  { value: "not_empty", label: msg("Is not blank") },
];

/** The comparisons the recipient filter offers (the rules' ones and text matching). */
export const FILTER_COMPARISONS: { value: string; label: string }[] = [
  ...COMPARISONS,
  { value: "contains", label: msg("Contains") },
  { value: "not_contains", label: msg("Does not contain") },
];

/** A record navigator for dialogs that preview the first, previous, next and last records. */
export function recordNav(count: () => number, onMove: (position: number) => void): { el: HTMLElement; position: () => number; set(position: number): void } {
  let pos = 0;
  const label = h("span", { class: "mm-nav-count", "aria-live": "polite" });
  const mk = (icon: string, title: string, fn: () => number) => {
    const b = h("button", { class: "btn sm ghost", type: "button", title, "aria-label": title }, svg(icon));
    b.addEventListener("click", () => set(fn()));
    return b;
  };
  const set = (p: number, notify = true) => {
    const n = count();
    pos = Math.max(0, Math.min(n - 1, p));
    label.textContent = n ? t("Record {n} of {total}", { n: pos + 1, total: n }) : t("No recipients");
    if (notify) onMove(pos);
  };
  const el = h(
    "div",
    { class: "mm-nav", role: "group", "aria-label": t("Preview a recipient") },
    mk(MAIL_ICONS.mmFirst, t("First record"), () => 0),
    mk(MAIL_ICONS.mmPrevious, t("Previous record"), () => pos - 1),
    label,
    mk(MAIL_ICONS.mmNext, t("Next record"), () => pos + 1),
    mk(MAIL_ICONS.mmLast, t("Last record"), () => count() - 1),
  );
  set(0, false);
  return { el, position: () => pos, set };
}
