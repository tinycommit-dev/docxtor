// Insert, Field: the field names by group on the left, the form for the chosen field on the right, the field code as it
// will be written (which can be edited) and a preview of the result. The field types are the host's registry
// (`ctx.fields`), so fields an extension adds appear here too.
import { buildInstruction, FIELD_CATEGORIES, parseInstruction, readValues, t, type FieldOption, type FieldType, type FieldValues } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { fieldInput, localIso } from "../editor/fields";
import { h, modal, onChoose } from "./dom";

let seq = 0;

/** The text a field shows in the list: its title, then its name. */
const titleOf = (f: FieldType) => (f.title ? t(f.title) : f.name);

export interface FieldDialogOptions {
  /** The field to start on (its name). */
  select?: string;
  /** A code to start from, such as the one of a field being changed. */
  code?: string;
}

export function openFieldDialog(ed: Editor, types: FieldType[], opts: FieldDialogOptions = {}) {
  const id = `fd-${++seq}`;
  const categories = [...FIELD_CATEGORIES.map((c) => ({ id: c.id, label: c.label })), ...[...new Set(types.map((x) => x.category))].filter((c) => !FIELD_CATEGORIES.some((f) => f.id === c)).map((c) => ({ id: c, label: c }))].filter((c) => types.some((x) => x.category === c.id));
  const formats = ed.engine.dateFormats(localIso());

  let category = "";
  let current: FieldType = types.find((x) => x.name === opts.select) ?? types[0];
  let values: FieldValues = {};
  let shown: FieldType[] = [];

  const catSel = h("select", { id: `${id}-cat` }, h("option", { value: "" }, t("All")), ...categories.map((c) => h("option", { value: c.id }, t(c.label))));
  const list = h("div", { class: "xr-list fd-names", role: "listbox", id: `${id}-names`, "aria-label": t("Field names"), tabindex: "0" });
  const desc = h("p", { class: "help fd-desc" });
  const form = h("div", { class: "fd-form" });
  const code = h("input", { type: "text", id: `${id}-code`, spellcheck: "false", autocomplete: "off", autocapitalize: "off" });
  const preview = h("p", { class: "cap-preview fd-preview", "aria-live": "polite" });
  const insert = h("button", { class: "btn primary", type: "button" }, t("Insert"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));

  const writeCode = () => {
    code.value = buildInstruction(current, values);
    paintPreview();
  };
  const paintPreview = () => {
    const instr = code.value.trim();
    insert.disabled = !instr;
    let text = "";
    try {
      text = instr ? ed.engine.fieldPreview(instr, fieldInput(ed)) : "";
    } catch {
      text = "";
    }
    preview.textContent = text || (instr ? t("Shown when the fields are updated") : "");
  };

  const control = (o: FieldOption): HTMLElement => {
    const cid = `${id}-o-${o.id}`;
    const set = (v: string | boolean | undefined) => {
      values = { ...values, [o.id]: v };
      writeCode();
    };
    const help = o.help ? h("p", { class: "help" }, t(o.help)) : null;
    if (o.kind === "toggle") {
      const box = h("input", { type: "checkbox", id: cid, checked: !!values[o.id] });
      box.addEventListener("change", () => set(box.checked));
      return h("div", { class: "field" }, h("label", { class: "check", for: cid }, box, h("span", {}, t(o.label))), help);
    }
    if (o.write === "date") {
      // The formats Word offers in the document's language, with today's text for each; or any picture of your own.
      const OTHER = "\u0000other";
      const sel = h("select", { id: cid }, ...formats.map(([p, s]) => h("option", { value: p }, s)), h("option", { value: OTHER }, t("Other…")));
      const custom = h("input", { type: "text", "aria-label": t("Your own format"), spellcheck: "false", autocomplete: "off", placeholder: "dddd, MMMM d, yyyy" });
      const cur = String(values[o.id] ?? "");
      const known = formats.some(([p]) => p === cur);
      sel.value = known ? cur : cur ? OTHER : formats[0]?.[0] ?? OTHER;
      custom.hidden = sel.value !== OTHER;
      custom.value = known ? "" : cur;
      if (!cur && sel.value !== OTHER) values = { ...values, [o.id]: sel.value };
      sel.addEventListener("change", () => {
        custom.hidden = sel.value !== OTHER;
        if (sel.value === OTHER) custom.focus();
        set(sel.value === OTHER ? custom.value : sel.value);
      });
      custom.addEventListener("input", () => set(custom.value));
      return h("div", { class: "field" }, h("label", { for: cid }, t(o.label)), sel, custom, help);
    }
    if (o.kind === "choice") {
      const sel = h("select", { id: cid }, ...(o.choices ?? []).map((c) => h("option", { value: c.value }, t(c.label))));
      sel.value = String(values[o.id] ?? o.default ?? o.choices?.[0]?.value ?? "");
      sel.addEventListener("change", () => set(sel.value));
      return h("div", { class: "field" }, h("label", { for: cid }, t(o.label)), sel, help);
    }
    const input = h("input", { type: "text", id: cid, value: String(values[o.id] ?? ""), spellcheck: "false", autocomplete: "off", placeholder: o.placeholder ? t(o.placeholder) : "" });
    input.addEventListener("input", () => set(input.value));
    return h("div", { class: "field" }, h("label", { for: cid }, t(o.label)), input, help);
  };

  const paintForm = () => {
    desc.textContent = current.description ? t(current.description) : "";
    form.replaceChildren(...(current.options ?? []).map(control));
    writeCode();
  };

  const choose = (f: FieldType, start?: string) => {
    current = f;
    values = {};
    for (const o of f.options ?? []) if (o.default !== undefined) values[o.id] = o.default;
    if (start) values = { ...values, ...readValues(f, start) };
    list.querySelectorAll<HTMLElement>(".xr-item").forEach((el) => {
      const on = el.dataset.name === f.name;
      el.classList.toggle("on", on);
      el.setAttribute("aria-selected", String(on));
    });
    list.querySelector<HTMLElement>(".xr-item.on")?.scrollIntoView({ block: "nearest" });
    paintForm();
    if (start) code.value = start.trim();
    paintPreview();
  };

  const paintList = () => {
    shown = types.filter((x) => !category || x.category === category);
    if (!shown.includes(current)) current = shown[0] ?? current;
    list.replaceChildren(
      ...shown.map((f) => {
        const b = h("button", { type: "button", role: "option", class: `xr-item${f === current ? " on" : ""}`, "data-name": f.name, "aria-selected": String(f === current), tabindex: "-1" }, h("span", { class: "xr-title" }, titleOf(f)), h("span", { class: "xr-detail" }, f.name));
        b.addEventListener("mousedown", (e) => e.preventDefault());
        onChoose(b, () => choose(f), go);
        return b;
      }),
    );
  };

  catSel.addEventListener("change", () => {
    category = catSel.value;
    paintList();
    if (shown[0] && !shown.includes(current)) choose(shown[0]);
  });
  list.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = shown.indexOf(current);
    const next = shown[Math.min(shown.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))];
    if (next) choose(next);
  });
  code.addEventListener("input", paintPreview);
  code.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      go();
    }
  });

  const dialog = h(
    "div",
    { class: "dialog fd-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Field") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Field")), h("p", { class: "sub" }, t("Pick a field and its options. It shows a value that Docxtor keeps up to date."))),
    h(
      "div",
      { class: "dialog-body fd-cols" },
      h("div", { class: "fd-side" }, h("div", { class: "field" }, h("label", { for: `${id}-cat` }, t("Category")), catSel), h("div", { class: "field" }, h("label", {}, t("Field names")), list)),
      h("div", { class: "fd-main" }, desc, form, h("div", { class: "field" }, h("label", { for: `${id}-code` }, t("Field code")), code), h("div", { class: "field" }, h("label", {}, t("Result")), preview)),
    ),
    h("div", { class: "dialog-actions" }, cancel, insert),
  );
  const dismiss = modal(dialog, () => ed.focus());
  const go = () => {
    const instr = code.value.trim();
    if (!instr) return;
    dismiss();
    // A link shows the selected text, else its address.
    const link = parseInstruction(instr);
    const result = link.code === "HYPERLINK" ? ed.selectedText() || link.args[0] || link.switches.l?.toString() : undefined;
    ed.insertField(instr, result);
  };
  cancel.addEventListener("click", dismiss);
  insert.addEventListener("click", go);
  paintList();
  choose(current, opts.code);
  list.querySelector<HTMLElement>(".xr-item.on")?.scrollIntoView({ block: "nearest" });
}
