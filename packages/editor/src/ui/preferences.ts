// Preferences: what the editor changes as you type. Smart quotes, capital letters, links, Markdown
// shortcuts and a list of symbols to substitute (a table of "Replace" and "With" pairs).
import { msg, t, type SettingDefinition } from "@docxtor/extensions";
import { cleanException, DEFAULT_REPLACEMENTS, type AutoformatSettings, type Substitution } from "../editor/autoformat";
import { PASTE_CHOICES, type PasteChoice } from "../editor/paste-flow";
import { h, modal, svg } from "./dom";
import { I } from "./icons";
import { settingFields } from "./setting-fields";

type Flag = "smartQuotes" | "capitalize" | "links" | "markdown" | "substitutions";

const CHECKS: { key: Flag; label: string; help: string }[] = [
  { key: "smartQuotes", label: msg("Use smart quotes"), help: msg("Straight quotes become curly ones that match the document's language.") },
  { key: "capitalize", label: msg("Automatically capitalize words"), help: msg("The first word of a sentence, and the word “i”.") },
  { key: "links", label: msg("Automatically detect links"), help: msg("A web address or e-mail address becomes a link when you press Space or Enter after it.") },
  { key: "markdown", label: msg("Use Markdown shortcuts"), help: msg("Type # and a space for a heading, - for a bulleted list, **bold** or --- and Enter for a line.") },
  { key: "substitutions", label: msg("Use substitutions"), help: msg("Replace the text you type with a symbol, using the list below.") },
];

/**
 * The Preferences dialog. `save` gets the new settings when the person applies them; `onClose`
 * runs however the dialog ends (to give the document its focus back).
 */
export type UnitChoice = "auto" | "in" | "cm";

export function openPreferences(opts: {
  settings: AutoformatSettings;
  save: (settings: AutoformatSettings) => void;
  onClose?: () => void;
  paste?: { mode: PasteChoice; save: (mode: PasteChoice) => void };
  /** The measuring unit of dialogs and rulers: automatic follows the language (centimetres outside the United States). */
  units?: { mode: UnitChoice; save: (mode: UnitChoice) => void };
  /** Settings extensions contribute (`ctx.settings`), listed under their groups and applied with the rest. */
  extra?: readonly SettingDefinition[];
  /** Font families the font settings offer. */
  fonts?: () => string[];
  /** Opens the dialog that changes keyboard shortcuts. */
  shortcuts?: () => void;
}) {
  const flags: Record<Flag, boolean> = {
    smartQuotes: opts.settings.smartQuotes,
    capitalize: opts.settings.capitalize,
    links: opts.settings.links,
    markdown: opts.settings.markdown,
    substitutions: opts.settings.substitutions,
  };
  let rows: Substitution[] = opts.settings.replacements.map((r) => ({ ...r }));

  const boxes = new Map<Flag, HTMLInputElement>();
  const checks = CHECKS.map((c) => {
    const box = h("input", { type: "checkbox", id: `pref-${c.key}`, checked: flags[c.key] });
    box.addEventListener("change", () => {
      flags[c.key] = box.checked;
      dim();
    });
    boxes.set(c.key, box);
    return h("div", { class: "pref-check" }, h("label", { class: "check", for: `pref-${c.key}` }, box, h("span", {}, t(c.label))), h("p", { class: "pref-help" }, t(c.help)));
  });

  // What a plain Paste does with formatting.
  let pasteMode: PasteChoice = opts.paste?.mode ?? "keep";
  const pasteSelect = h("select", { id: "pref-paste" }, ...PASTE_CHOICES.map((c) => h("option", { value: c.id, selected: c.id === pasteMode }, c.label())));
  pasteSelect.addEventListener("change", () => (pasteMode = pasteSelect.value as PasteChoice));
  const pasteField = opts.paste
    ? h("div", { class: "pref-check" }, h("div", { class: "field" }, h("label", { for: "pref-paste" }, t("When you paste")), pasteSelect), h("p", { class: "pref-help" }, t("Paste special and the Paste options button next to a paste let you choose each time.")))
    : null;
  // The measuring unit.
  let unitMode: UnitChoice = opts.units?.mode ?? "auto";
  const unitSelect = h(
    "select",
    { id: "pref-units" },
    ...(
      [
        ["auto", t("Automatic (by language)")],
        ["in", t("Inches")],
        ["cm", t("Centimeters")],
      ] as [UnitChoice, string][]
    ).map(([v, label]) => h("option", { value: v, selected: v === unitMode }, label)),
  );
  unitSelect.addEventListener("change", () => (unitMode = unitSelect.value as UnitChoice));
  const unitField = opts.units
    ? h("div", { class: "pref-check" }, h("div", { class: "field" }, h("label", { for: "pref-units" }, t("Measurement units")), unitSelect), h("p", { class: "pref-help" }, t("Page setup, columns, indents and the rulers show lengths in this unit.")))
    : null;
  // Abbreviations that end in a period without ending a sentence (the capital letter after them is left alone).
  const exceptionsBox = h("textarea", { id: "pref-exceptions", rows: 3, spellcheck: "false", autocomplete: "off", autocapitalize: "off" });
  exceptionsBox.value = opts.settings.exceptions.map((x) => `${x}.`).join("\n");
  const exceptionsField = h(
    "div",
    { class: "pref-check pref-exceptions" },
    h("div", { class: "field" }, h("label", { for: "pref-exceptions" }, t("Don't capitalize after these abbreviations")), exceptionsBox),
    h("p", { class: "pref-help" }, t("One on each line, such as dept. or approx. Mr., Dr., etc., e.g. and i.e. are already known.")),
  );
  const extra = settingFields(opts.extra ?? [], opts.fonts ?? (() => []));
  const shortcutsField = opts.shortcuts
    ? h(
        "div",
        { class: "pref-check" },
        h("div", { class: "field" }, h("label", {}, t("Keyboard shortcuts")), h("div", {}, h("button", { class: "btn", type: "button", onclick: () => opts.shortcuts?.() }, t("Change shortcuts…")))),
        h("p", { class: "pref-help" }, t("Give any command a shortcut of your own, or take one away.")),
      )
    : null;
  const list = h("div", { class: "pref-subs", role: "group", "aria-label": t("Substitutions") });
  const add = h("button", { class: "btn", type: "button" }, t("Add substitution"));
  const restore = h("button", { class: "btn ghost", type: "button" }, t("Restore defaults"));

  const input = (value: string, label: string, set: (v: string) => void) => {
    const el = h("input", { type: "text", value, "aria-label": label, spellcheck: "false", autocomplete: "off", autocapitalize: "off" });
    el.addEventListener("input", () => set(el.value));
    return el;
  };
  function render(focusLast = false) {
    list.replaceChildren(
      h("div", { class: "pref-sub head", "aria-hidden": "true" }, h("span", {}, t("Replace")), h("span", {}, t("With")), h("span", {})),
      ...rows.map((r) => {
        const remove = h("button", { class: "btn ghost pref-remove", type: "button", "aria-label": t("Remove {text}", { text: r.find || t("this substitution") }), title: t("Remove") }, svg(I.close));
        remove.addEventListener("click", () => {
          rows = rows.filter((x) => x !== r);
          render();
        });
        return h("div", { class: "pref-sub" }, input(r.find, t("Replace"), (v) => (r.find = v)), input(r.with, t("With"), (v) => (r.with = v)), remove);
      }),
    );
    if (focusLast) {
      list.querySelector<HTMLInputElement>(".pref-sub:last-child input")?.focus();
      list.scrollTop = list.scrollHeight;
    }
    dim();
  }
  const dim = () => {
    exceptionsBox.disabled = !flags.capitalize;
    const off = !flags.substitutions;
    list.classList.toggle("off", off);
    list.querySelectorAll("input,button").forEach((el) => ((el as HTMLInputElement | HTMLButtonElement).disabled = off));
    add.disabled = off;
    restore.disabled = off;
  };
  add.addEventListener("click", () => {
    rows = [...rows, { find: "", with: "" }];
    render(true);
  });
  restore.addEventListener("click", () => {
    rows = DEFAULT_REPLACEMENTS.map((r) => ({ ...r }));
    render();
  });

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog pref-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Preferences") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Preferences")), h("p", { class: "sub" }, t("These change the text as you type, in every document."))),
    h("div", { class: "dialog-body" }, pasteField, unitField, extra.el, shortcutsField, ...checks, exceptionsField, h("div", { class: "pref-table" }, list, h("div", { class: "pref-actions" }, add, restore))),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog, opts.onClose);
  render();
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    // A pair needs something to find; the last of two with the same text wins.
    const seen = new Map<string, Substitution>();
    for (const r of rows) if (r.find !== "") seen.set(r.find, { find: r.find, with: r.with });
    const exceptions = [...new Set(exceptionsBox.value.split(/\r?\n|,/).map(cleanException).filter(Boolean))];
    opts.save({ ...flags, replacements: [...seen.values()], exceptions });
    opts.paste?.save(pasteMode);
    opts.units?.save(unitMode);
    void extra.apply().finally(close);
  });
}
