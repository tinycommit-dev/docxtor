// The Properties dialogs of a content control and of a legacy form field: what Word's dialogs ask, written back as Word writes it.
import { t } from "@docxtor/extensions";
import type { ControlProps, FormProps, RControl, RForm } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { kindLabel } from "../editor/controls-render";
import { DATE_FORMATS, DATE_LANGUAGES, formatDate } from "./control-dates";
import { h, modal } from "./dom";
import { I } from "./icons";

interface Symbols {
  id: string;
  label: () => string;
  on: { val: string; font: string };
  off: { val: string; font: string };
}

/** Check box symbols to choose from: the symbols are characters of a font, as in Word's Symbol dialog. */
const SYMBOLS: Symbols[] = [
  { id: "cross", label: () => t("Cross"), on: { val: "2612", font: "MS Gothic" }, off: { val: "2610", font: "MS Gothic" } },
  { id: "tick", label: () => t("Tick"), on: { val: "2714", font: "Segoe UI Symbol" }, off: { val: "2610", font: "MS Gothic" } },
  { id: "check", label: () => t("Check mark"), on: { val: "2713", font: "Segoe UI Symbol" }, off: { val: "2610", font: "MS Gothic" } },
  { id: "square", label: () => t("Filled square"), on: { val: "25A0", font: "Segoe UI Symbol" }, off: { val: "25A1", font: "Segoe UI Symbol" } },
];

const APPEARANCES: [string, () => string][] = [
  ["boundingBox", () => t("Bounding box")],
  ["tags", () => t("Start and end tags")],
  ["hidden", () => t("None")],
];

const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);

function select(id: string, options: [string, string][], value: string): HTMLSelectElement {
  const s = h("select", { id });
  for (const [v, l] of options) s.append(h("option", { value: v }, l));
  s.value = value;
  return s;
}

/** Open the Properties dialog of a content control. `apply` gets what changed. */
export function openControlProperties(ed: Editor, c: RControl, apply: (props: ControlProps) => void) {
  const alias = h("input", { id: "cc-alias", type: "text", value: c.alias ?? "", spellcheck: "false" });
  const tag = h("input", { id: "cc-tag", type: "text", value: c.tag ?? "", spellcheck: "false" });
  const hasColor = !!c.color;
  const color = h("input", { id: "cc-color", type: "color", value: `#${(c.color ?? "4472C4").toLowerCase()}`, "aria-label": t("Colour") });
  const noColor = h("input", { id: "cc-nocolor", type: "checkbox", checked: !hasColor });
  const appearance = select("cc-appearance", APPEARANCES.map(([v, l]) => [v, l()] as [string, string]), c.appearance ?? "boundingBox");
  const noDelete = h("input", { id: "cc-nodelete", type: "checkbox", checked: c.lock === "sdtLocked" || c.lock === "sdtContentLocked" });
  const noEdit = h("input", { id: "cc-noedit", type: "checkbox", checked: c.lock === "contentLocked" || c.lock === "sdtContentLocked" });
  const temporary = h("input", { id: "cc-temp", type: "checkbox", checked: !!c.temporary });
  const body: HTMLElement[] = [
    field(t("Title"), alias, "cc-alias"),
    field(t("Tag"), tag, "cc-tag"),
    h("div", { class: "field" }, h("label", { for: "cc-color" }, t("Colour")), h("div", { class: "row-inline" }, color, h("label", { class: "check", for: "cc-nocolor" }, noColor, h("span", {}, t("Automatic"))))),
    field(t("Show as"), appearance, "cc-appearance"),
  ];
  const textual = ["text", "richText", "date", "dropDown", "comboBox"].includes(c.kind);
  let placeholder: HTMLInputElement | null = null;
  if (textual || c.kind === "picture") {
    placeholder = h("input", { id: "cc-ph", type: "text", spellcheck: "true", placeholder: t("Click or tap here to enter text.") });
    // The text now showing is the placeholder only while it is one; the new text is for when the control is empty.
    body.push(field(t("Placeholder text"), placeholder, "cc-ph"));
  }
  let multiline: HTMLInputElement | null = null;
  if (c.kind === "text") {
    multiline = h("input", { id: "cc-multi", type: "checkbox", checked: !!c.multiline });
    body.push(h("label", { class: "check", for: "cc-multi" }, multiline, h("span", {}, t("Allow carriage returns (multiple paragraphs)"))));
  }
  // Date.
  let dateFormat: HTMLInputElement | null = null;
  let lid: HTMLSelectElement | null = null;
  if (c.kind === "date") {
    const today = new Date();
    const utc = new Date(Date.UTC(today.getFullYear(), today.getMonth(), today.getDate()));
    lid = select("cc-lid", DATE_LANGUAGES.map(([v, l]) => [v, l()] as [string, string]), c.lid ?? "en-US");
    dateFormat = h("input", { id: "cc-fmt", type: "text", value: c.dateFormat ?? "M/d/yyyy", spellcheck: "false" });
    const list = h("div", { class: "cc-formats", role: "listbox", "aria-label": t("Date formats") });
    const paint = () => {
      list.textContent = "";
      for (const f of DATE_FORMATS) {
        const b = h("button", { type: "button", role: "option", class: f === dateFormat!.value ? "on" : "", "aria-selected": String(f === dateFormat!.value) }, formatDate(f, lid!.value, utc));
        b.addEventListener("click", () => {
          dateFormat!.value = f;
          paint();
        });
        list.append(b);
      }
    };
    lid.addEventListener("change", paint);
    dateFormat.addEventListener("input", paint);
    paint();
    body.push(field(t("Display the date like this"), list, "cc-fmt"), field(t("Date format"), dateFormat, "cc-fmt"), field(t("Language"), lid, "cc-lid"));
  }
  // Check box symbols.
  let symbols: HTMLSelectElement | null = null;
  if (c.kind === "checkbox") {
    const current = SYMBOLS.find((s) => s.on.val === c.checkedSymbol?.val && s.off.val === c.uncheckedSymbol?.val)?.id ?? "cross";
    symbols = select("cc-sym", SYMBOLS.map((s) => [s.id, `${String.fromCodePoint(parseInt(s.on.val, 16))}  ${String.fromCodePoint(parseInt(s.off.val, 16))}  ${s.label()}`] as [string, string]), current);
    body.push(field(t("Symbols for checked and unchecked"), symbols, "cc-sym"));
  }
  // The entries of a list.
  let rows: { display: HTMLInputElement; value: HTMLInputElement }[] = [];
  let listBox: HTMLElement | null = null;
  if (c.kind === "dropDown" || c.kind === "comboBox") {
    listBox = h("div", { class: "cc-items" });
    // Word's first entry is the prompt; it is kept and not edited here.
    const items = (c.items ?? []).slice(1);
    const addRow = (display = "", value = "") => {
      const d = h("input", { type: "text", value: display, "aria-label": t("Display name"), placeholder: t("Display name") });
      const v = h("input", { type: "text", value, "aria-label": t("Value"), placeholder: t("Value") });
      const entry = { display: d, value: v };
      rows.push(entry);
      const up = h("button", { class: "ctl-btn", type: "button", "aria-label": t("Move up"), html: I.chevron, style: "transform:rotate(180deg)" });
      const down = h("button", { class: "ctl-btn", type: "button", "aria-label": t("Move down"), html: I.chevron });
      const rm = h("button", { class: "ctl-btn", type: "button", "aria-label": t("Remove"), html: I.close });
      const line = h("div", { class: "cc-item" }, d, v, up, down, rm);
      const move = (dir: number) => {
        const i = rows.indexOf(entry);
        const j = i + dir;
        if (j < 0 || j >= rows.length) return;
        [rows[i], rows[j]] = [rows[j], rows[i]];
        const sib = dir < 0 ? line.previousElementSibling : line.nextElementSibling;
        if (sib) dir < 0 ? sib.before(line) : sib.after(line);
      };
      up.addEventListener("click", () => move(-1));
      down.addEventListener("click", () => move(1));
      rm.addEventListener("click", () => {
        rows = rows.filter((r) => r !== entry);
        line.remove();
      });
      listBox!.append(line);
    };
    items.forEach((i) => addRow(i.display, i.value));
    const add = h("button", { class: "btn ghost", type: "button" }, t("Add item"));
    add.addEventListener("click", () => {
      addRow();
      rows[rows.length - 1].display.focus();
    });
    body.push(h("div", { class: "field" }, h("label", {}, t("List items")), listBox, add));
  }
  // Repeating section.
  let title: HTMLInputElement | null = null;
  let fixed: HTMLInputElement | null = null;
  if (c.kind === "repeating") {
    title = h("input", { id: "cc-title", type: "text", value: c.sectionTitle ?? "", spellcheck: "true" });
    fixed = h("input", { id: "cc-fixed", type: "checkbox", checked: !!c.fixedItems });
    body.push(field(t("Title for the repeating items"), title, "cc-title"), h("label", { class: "check", for: "cc-fixed" }, fixed, h("span", {}, t("Don't let people add or remove items"))));
  }
  body.push(
    h(
      "fieldset",
      { class: "cc-locking" },
      h("legend", {}, t("Locking")),
      h("label", { class: "check", for: "cc-nodelete" }, noDelete, h("span", {}, t("This control can't be deleted"))),
      h("label", { class: "check", for: "cc-noedit" }, noEdit, h("span", {}, t("The contents can't be edited"))),
    ),
  );
  if (textual) body.push(h("label", { class: "check", for: "cc-temp" }, temporary, h("span", {}, t("Remove the control when its contents are edited"))));

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog cc-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("{kind} properties", { kind: kindLabel(c.kind) }) },
    h("div", { class: "dialog-head" }, h("h2", {}, t("{kind} properties", { kind: kindLabel(c.kind) }))),
    h("div", { class: "dialog-body" }, ...body),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const props: ControlProps = { alias: alias.value.trim(), tag: tag.value.trim(), color: noColor.checked ? "" : color.value.replace("#", "").toUpperCase(), appearance: appearance.value };
    props.lock = noDelete.checked && noEdit.checked ? "sdtContentLocked" : noDelete.checked ? "sdtLocked" : noEdit.checked ? "contentLocked" : "unlocked";
    if (textual) props.temporary = temporary.checked;
    if (placeholder && placeholder.value.trim()) props.placeholder = placeholder.value.trim();
    if (multiline) props.multiline = multiline.checked;
    if (dateFormat && lid) {
      props.dateFormat = dateFormat.value.trim() || "M/d/yyyy";
      props.lid = lid.value;
    }
    if (symbols) {
      const s = SYMBOLS.find((x) => x.id === symbols!.value) ?? SYMBOLS[0];
      props.checkedSymbol = s.on;
      props.uncheckedSymbol = s.off;
    }
    if (listBox) {
      const prompt = c.items?.[0];
      const keep = prompt ? [prompt] : [];
      const entries = rows.filter((r) => r.display.value.trim() || r.value.value.trim()).map((r) => ({ display: r.display.value.trim() || r.value.value.trim(), value: r.value.value.trim() || r.display.value.trim() }));
      props.items = [...keep, ...entries];
    }
    if (title && fixed) {
      props.sectionTitle = title.value.trim();
      props.fixedItems = fixed.checked;
    }
    close();
    apply(props);
  });
}

/** Open the Properties dialog of a legacy form field. */
export function openFormProperties(ed: Editor, f: RForm, apply: (props: FormProps) => void) {
  const name = h("input", { id: "ff-name", type: "text", value: f.name ?? "", spellcheck: "false" });
  const off = h("input", { id: "ff-off", type: "checkbox", checked: !!f.disabled });
  const help = h("input", { id: "ff-help", type: "text", value: f.help ?? "" });
  const status = h("input", { id: "ff-status", type: "text", value: f.status ?? "" });
  const body: HTMLElement[] = [field(t("Name"), name, "ff-name")];
  let type: HTMLSelectElement | null = null;
  let def: HTMLInputElement | null = null;
  let max: HTMLInputElement | null = null;
  let format: HTMLSelectElement | null = null;
  if (f.kind === "text") {
    type = select("ff-type", [["regular", t("Regular text")], ["number", t("Number")], ["date", t("Date")], ["currentDate", t("Current date")], ["currentTime", t("Current time")], ["calculated", t("Calculation")]], f.textType ?? "regular");
    def = h("input", { id: "ff-def", type: "text", value: f.default ?? "" });
    max = h("input", { id: "ff-max", type: "number", min: "0", value: String(f.maxLength ?? 0), placeholder: t("Unlimited") });
    format = select("ff-format", [["", t("None")], ["UPPERCASE", t("UPPERCASE")], ["lowercase", t("lowercase")], ["First capital", t("First capital")], ["Title case", t("Title case")]], f.format ?? "");
    body.push(field(t("Type"), type, "ff-type"), field(t("Default text"), def, "ff-def"), field(t("Maximum length (0 for unlimited)"), max, "ff-max"), field(t("Text format"), format, "ff-format"));
  }
  let size: HTMLInputElement | null = null;
  let auto: HTMLInputElement | null = null;
  let ticked: HTMLInputElement | null = null;
  if (f.kind === "checkbox") {
    auto = h("input", { id: "ff-auto", type: "checkbox", checked: f.size === undefined });
    size = h("input", { id: "ff-size", type: "number", min: "1", step: "0.5", value: String(f.size ?? 11) });
    ticked = h("input", { id: "ff-ticked", type: "checkbox", checked: !!f.defaultChecked });
    body.push(h("label", { class: "check", for: "ff-auto" }, auto, h("span", {}, t("Automatic size"))), field(t("Size (points)"), size, "ff-size"), h("label", { class: "check", for: "ff-ticked" }, ticked, h("span", {}, t("Ticked by default"))));
  }
  let entries: HTMLTextAreaElement | null = null;
  if (f.kind === "dropdown") {
    entries = h("textarea", { id: "ff-items", rows: "5", "aria-label": t("Entries") });
    entries.value = (f.items ?? []).join("\n");
    body.push(field(t("Entries, one on each line"), entries, "ff-items"));
  }
  body.push(field(t("Help text"), help, "ff-help"), field(t("Status bar text"), status, "ff-status"), h("label", { class: "check", for: "ff-off" }, off, h("span", {}, t("Fill-in is switched off"))));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const title = f.kind === "text" ? t("Text field properties") : f.kind === "checkbox" ? t("Check box properties") : t("Drop-down properties");
  const dialog = h("div", { class: "dialog cc-dialog", role: "dialog", "aria-modal": "true", "aria-label": title }, h("div", { class: "dialog-head" }, h("h2", {}, title)), h("div", { class: "dialog-body" }, ...body), h("div", { class: "dialog-actions" }, cancel, ok));
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const props: FormProps = { name: name.value.trim(), help: help.value, status: status.value, disabled: off.checked };
    if (type && def && max && format) {
      props.textType = type.value;
      props.default = def.value;
      props.maxLength = Math.max(0, Math.round(Number(max.value) || 0));
      props.format = format.value;
    }
    if (size && auto && ticked) {
      props.size = auto.checked ? 0 : Math.max(1, Number(size.value) || 11);
      props.defaultChecked = ticked.checked;
    }
    if (entries) props.items = entries.value.split("\n").map((x) => x.trim()).filter(Boolean);
    close();
    apply(props);
  });
}
