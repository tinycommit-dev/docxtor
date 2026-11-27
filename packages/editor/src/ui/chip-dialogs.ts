// The two small dialogs smart chips need: adding a person by hand, and making or changing a drop-down chip
// (its name, and each option with a colour).
import { CHIP_COLORS, t, type ChipColor, type DropdownOption, type Person } from "@docxtor/extensions";
import { h, modal } from "./dom";
import { I } from "./icons";

/** What each colour is called. */
export function colorName(c: ChipColor): string {
  switch (c) {
    case "blue":
      return t("Blue");
    case "green":
      return t("Green");
    case "yellow":
      return t("Yellow");
    case "red":
      return t("Red");
    case "purple":
      return t("Purple");
    default:
      return t("Gray");
  }
}

/** A name and an email address, typed in. Both are kept on this device by the people provider. */
export function openAddPerson(opts: { name?: string; email?: string; onAdd: (person: Person) => void; onClose?: () => void }) {
  const name = h("input", { type: "text", id: "chip-person-name", value: opts.name ?? "", autocomplete: "off", spellcheck: "false" });
  const email = h("input", { type: "email", id: "chip-person-email", value: opts.email ?? "", autocomplete: "off", spellcheck: "false" });
  const error = h("p", { class: "sub", role: "alert", style: "color:var(--danger);min-height:1.2em" });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const add = h("button", { class: "btn primary", type: "button" }, t("Add"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Add a person"), style: "width:min(var(--dialog-s),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Add a person")), h("p", { class: "sub" }, t("They are kept on this device, so you can find them with @ next time."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "chip-person-name" }, t("Name")), name), h("div", { class: "field" }, h("label", { for: "chip-person-email" }, t("Email address")), email), error),
    h("div", { class: "dialog-actions" }, cancel, add),
  );
  const close = modal(dialog, opts.onClose);
  cancel.addEventListener("click", close);
  const go = () => {
    const n = name.value.trim();
    const e = email.value.trim();
    if (!n && !e) {
      error.textContent = t("Type a name or an email address.");
      name.focus();
      return;
    }
    if (e && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(e)) {
      error.textContent = t("That email address doesn't look right.");
      email.focus();
      return;
    }
    close();
    opts.onAdd({ name: n || e, ...(e ? { email: e } : {}) });
  };
  add.addEventListener("click", go);
  for (const i of [name, email]) {
    i.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        go();
      }
    });
  }
}

/** Make a drop-down chip or change one: a name, and options that each have a colour. */
export function openDropdownDialog(opts: { title?: string; options?: DropdownOption[]; edit?: boolean; onSave: (title: string, options: DropdownOption[]) => void; onClose?: () => void }) {
  const title = h("input", { type: "text", id: "chip-dd-title", value: opts.title ?? "", autocomplete: "off", spellcheck: "false", placeholder: t("Priority") });
  const rows = h("div", { class: "opt-rows" });
  const error = h("p", { class: "sub", role: "alert", style: "color:var(--danger);min-height:1.2em" });
  const addRow = (o: DropdownOption = { label: "", color: "gray" }) => {
    const n = rows.children.length + 1;
    const label = h("input", { type: "text", value: o.label, "aria-label": t("Option {n}", { n: String(n) }), autocomplete: "off", spellcheck: "false" });
    const color = h("select", { "aria-label": t("Colour of option {n}", { n: String(n) }) });
    for (const c of CHIP_COLORS) color.append(h("option", { value: c, ...(c === (o.color ?? "gray") ? { selected: true } : {}) }, colorName(c)));
    const remove = h("button", { class: "ctl-btn", type: "button", "aria-label": t("Remove option"), title: t("Remove option"), html: I.close });
    const row = h("div", { class: "opt-row" }, label, color, remove);
    remove.addEventListener("click", () => {
      row.remove();
      relabel();
    });
    label.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        if (row.nextElementSibling) (row.nextElementSibling.querySelector("input") as HTMLInputElement).focus();
        else {
          addRow();
          (rows.lastElementChild!.querySelector("input") as HTMLInputElement).focus();
        }
      }
    });
    rows.append(row);
  };
  const relabel = () => {
    Array.from(rows.children).forEach((row, i) => {
      row.querySelector("input")?.setAttribute("aria-label", t("Option {n}", { n: String(i + 1) }));
      row.querySelector("select")?.setAttribute("aria-label", t("Colour of option {n}", { n: String(i + 1) }));
    });
  };
  for (const o of opts.options?.length ? opts.options : [{ label: "", color: "gray" as ChipColor }, { label: "", color: "gray" as ChipColor }]) addRow(o);
  const more = h("button", { class: "btn ghost", type: "button" }, t("Add an option"));
  more.addEventListener("click", () => {
    addRow();
    (rows.lastElementChild!.querySelector("input") as HTMLInputElement).focus();
  });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const save = h("button", { class: "btn primary", type: "button" }, opts.edit ? t("Apply") : t("Insert"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": opts.edit ? t("Drop-down options") : t("Custom drop-down"), style: "width:min(var(--dialog-m),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, opts.edit ? t("Drop-down options") : t("Custom drop-down")), h("p", { class: "sub" }, t("Each option has a colour. Word shows the options as a drop-down list."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "chip-dd-title" }, t("Name")), title), h("div", { class: "field" }, h("span", { class: "menu-title" }, t("Options")), rows, more), error),
    h("div", { class: "dialog-actions" }, cancel, save),
  );
  const close = modal(dialog, opts.onClose);
  cancel.addEventListener("click", close);
  save.addEventListener("click", () => {
    const options: DropdownOption[] = [];
    for (const row of Array.from(rows.children)) {
      const label = (row.querySelector("input") as HTMLInputElement).value.trim();
      const color = (row.querySelector("select") as HTMLSelectElement).value as ChipColor;
      if (label) options.push({ label, color });
    }
    if (new Set(options.map((o) => o.label.toLowerCase())).size !== options.length) {
      error.textContent = t("Two options have the same name.");
      return;
    }
    if (options.length < 2) {
      error.textContent = t("Give at least two options.");
      return;
    }
    close();
    opts.onSave(title.value.trim() || t("Drop-down"), options);
  });
}
