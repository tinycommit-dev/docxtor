// File > Document details: what and where the document is (name, size, dates, pages, words, fonts, how safely Docxtor keeps
// it) and the properties it carries (title, subject, author, keywords, category, status, company, manager, comments and
// custom properties). The properties are the file's own, the ones Word shows in its Properties dialog and other programs read;
// each change is an undoable edit as it is made.
import { currentLocale, fontStatus, h, modal, t, type CustomProp, type DocProps, type Editor } from "@docxtor/editor";

export interface DocDetails {
  name: string;
  /** Where the document lives, in words ("your Library"). */
  where: string;
  bytes: number;
  fonts: string[];
  /** Fonts extensions offer, which count as available. */
  offered: string[];
  /** The round-trip fidelity as a percentage, "Created in Docxtor", or "Couldn't measure". */
  fidelity: string;
  /** The name from Tools, User information, shown as a hint under Author. */
  user?: string;
}

function numbers(options?: Intl.NumberFormatOptions): Intl.NumberFormat {
  try {
    return new Intl.NumberFormat(currentLocale(), options);
  } catch {
    return new Intl.NumberFormat(undefined, options);
  }
}

/** "48 KB", "1.2 MB": the size as a person reads it. */
function size(bytes: number): string {
  const units: [string, number][] = [
    ["gigabyte", 1e9],
    ["megabyte", 1e6],
    ["kilobyte", 1e3],
  ];
  for (const [unit, per] of units) {
    if (bytes >= per) {
      const v = bytes / per;
      return numbers({ style: "unit", unit, unitDisplay: "short", maximumFractionDigits: v < 10 ? 1 : 0 }).format(v);
    }
  }
  return numbers({ style: "unit", unit: "byte", unitDisplay: "short" }).format(bytes);
}

/** A date from the file's properties (W3CDTF) as the interface language writes dates; the text itself when it is not a date. */
export function when(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const options: Intl.DateTimeFormatOptions = { dateStyle: "long", timeStyle: "short" };
  try {
    return new Intl.DateTimeFormat(currentLocale(), options).format(d);
  } catch {
    return new Intl.DateTimeFormat(undefined, options).format(d);
  }
}

/** The text-valued properties of the summary, in the order the form shows them. */
const SUMMARY: { key: keyof Pick<DocProps, "title" | "subject" | "author" | "manager" | "company" | "category" | "keywords" | "status" | "description">; label: () => string; wide?: boolean }[] = [
  { key: "title", label: () => t("Title") },
  { key: "subject", label: () => t("Subject") },
  { key: "author", label: () => t("Author") },
  { key: "manager", label: () => t("Manager") },
  { key: "company", label: () => t("Company") },
  { key: "category", label: () => t("Category") },
  { key: "keywords", label: () => t("Keywords"), wide: true },
  { key: "status", label: () => t("Status") },
  { key: "description", label: () => t("Comments"), wide: true },
];

/** The kinds of value a custom property can hold, as Word's Custom tab names them. */
const KINDS: [CustomProp["type"], () => string][] = [
  ["text", () => t("Text")],
  ["number", () => t("Number")],
  ["date", () => t("Date")],
  ["bool", () => t("Yes or no")],
];

export function openDocDetails(ed: Editor, d: DocDetails) {
  const engine = ed.engine;
  if (!engine) return;
  const counts = engine.counts();
  const n = numbers();
  const fonts = d.fonts.length ? d.fonts.map((f) => (fontStatus(f, d.offered) === "missing" ? t("{font} (not available)", { font: f }) : f)).join(", ") : t("None");
  const where = d.where.charAt(0).toLocaleUpperCase(currentLocale()) + d.where.slice(1);
  const props = engine.docProps();
  const rows: [string, string][] = [
    [t("Name"), d.name],
    [t("Where"), where],
    [t("Size"), size(d.bytes)],
    ...(props.created ? [[t("Created"), when(props.created)] as [string, string]] : []),
    ...(props.modified ? [[t("Last changed"), when(props.modified)] as [string, string]] : []),
    ...(props.lastModifiedBy ? [[t("Last changed by"), props.lastModifiedBy] as [string, string]] : []),
    [t("Pages"), n.format(ed.pages)],
    [t("Words"), n.format(counts.words)],
    [t("Characters"), n.format(counts.charsNoSpaces)],
    [t("Characters with spaces"), n.format(counts.chars)],
    [t("Fonts"), fonts],
    [t("Round-trip fidelity"), d.fidelity],
  ];
  const stats = h("dl", { class: "doc-details" }, ...rows.flatMap(([k, v]) => [h("dt", {}, k), h("dd", {}, v)]));

  // -- the properties: each field is applied when it is left, as one undo step ----------------------------------
  const problem = h("p", { class: "field-error", role: "alert", hidden: true });
  const say = (message: string | null) => {
    problem.textContent = message ?? "";
    problem.hidden = !message;
  };
  const current: DocProps = { ...props };
  const form = h("div", { class: "dd-form" });
  for (const f of SUMMARY) {
    const id = `dd-${f.key}`;
    const input = f.key === "description" ? h("textarea", { id, rows: 2, disabled: ed.readOnly }) : h("input", { type: "text", id, disabled: ed.readOnly, autocomplete: "off" });
    input.value = (current[f.key] as string | undefined) ?? "";
    const apply = () => {
      const next = input.value.trim();
      if ((current[f.key] ?? "") === next) return;
      say(null);
      if (ed.exec({ op: "set_doc_props", [f.key]: next }, { onError: say })) current[f.key] = next || undefined;
    };
    input.addEventListener("change", apply);
    if (f.key !== "description") input.addEventListener("keydown", (e) => (e as KeyboardEvent).key === "Enter" && input.blur());
    const hint =
      f.key === "title"
        ? t("Screen readers announce the title, and other programs show it as the file's title.")
        : f.key === "author" && d.user
          ? t("{name} is the name saved with changes you make. Change it under Tools, User information.", { name: d.user })
          : null;
    form.append(h("div", { class: `field${f.wide ? " wide" : ""}` }, h("label", { for: id }, f.label()), input, hint ? h("span", { class: "help" }, hint) : null));
  }

  // -- custom properties ----------------------------------------------------------------------------------------
  const customList = h("div", { class: "dd-custom", role: "list" });
  const valueInput = (type: CustomProp["type"], value: string, disabled: boolean): HTMLInputElement | HTMLSelectElement => {
    if (type === "bool") {
      const sel = h("select", { "aria-label": t("Value"), disabled }, h("option", { value: "true" }, t("Yes")), h("option", { value: "false" }, t("No")));
      sel.value = value === "false" ? "false" : "true";
      return sel;
    }
    const kind = type === "number" ? "number" : type === "date" ? "date" : "text";
    const input = h("input", { type: kind, "aria-label": t("Value"), disabled, autocomplete: "off", step: type === "number" ? "any" : undefined });
    input.value = value;
    return input;
  };
  const save = (set: CustomProp[], remove: string[] = []) => {
    say(null);
    const done = ed.exec({ op: "set_custom_props", set, remove }, { onError: say });
    if (done) {
      const names = new Set([...set.map((p) => p.name.toLowerCase()), ...remove.map((r) => r.toLowerCase())]);
      current.custom = [...(current.custom ?? []).filter((c) => !names.has(c.name.toLowerCase())), ...set];
    }
    return !!done;
  };
  const drawCustom = () => {
    customList.replaceChildren(
      ...(current.custom ?? []).map((c) => {
        const value = valueInput(c.type, c.value, ed.readOnly);
        value.addEventListener("change", () => save([{ ...c, value: value.value }]));
        const remove = h("button", { class: "btn ghost", type: "button", disabled: ed.readOnly, "aria-label": t("Remove {name}", { name: c.name }) }, t("Remove"));
        remove.addEventListener("click", () => {
          if (save([], [c.name])) drawCustom();
        });
        const kind = KINDS.find(([k]) => k === c.type)![1]();
        return h("div", { class: "dd-prop", role: "listitem" }, h("span", { class: "dd-prop-name" }, c.name), h("span", { class: "dd-prop-type" }, kind), value, remove);
      }),
    );
  };
  const newName = h("input", { type: "text", id: "dd-new-name", "aria-label": t("Name"), placeholder: t("Name"), autocomplete: "off", disabled: ed.readOnly });
  const newType = h("select", { "aria-label": t("Type"), disabled: ed.readOnly }, ...KINDS.map(([k, label]) => h("option", { value: k }, label())));
  let newValue = valueInput("text", "", ed.readOnly);
  const adder = h("div", { class: "dd-prop dd-add" });
  const drawAdder = () => adder.replaceChildren(newName, newType, newValue, add);
  const add = h("button", { class: "btn", type: "button", disabled: ed.readOnly }, t("Add"));
  newType.addEventListener("change", () => {
    newValue = valueInput(newType.value as CustomProp["type"], "", ed.readOnly);
    drawAdder();
  });
  add.addEventListener("click", () => {
    const name = newName.value.trim();
    if (!name) return say(t("Give the property a name."));
    const type = newType.value as CustomProp["type"];
    if (save([{ name, type, value: newValue.value }])) {
      newName.value = "";
      newValue = valueInput(type, "", ed.readOnly);
      drawAdder();
      drawCustom();
      newName.focus();
    }
  });
  drawAdder();
  drawCustom();

  const ok = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const dialog = h(
    "div",
    { class: "dialog dd-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Document details") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Document details"))),
    h(
      "div",
      { class: "dialog-body dd-body" },
      h("h3", { class: "pref-heading" }, t("Properties")),
      form,
      h("h3", { class: "pref-heading" }, t("Custom properties")),
      customList,
      adder,
      problem,
      h("h3", { class: "pref-heading" }, t("Statistics")),
      stats,
    ),
    h("div", { class: "dialog-actions" }, ok),
  );
  const close = modal(dialog, () => ed.focus());
  ok.addEventListener("click", close);
  ok.focus();
}
