// The lines of Preferences that extensions contribute (`ctx.settings.register`): a toggle, a choice, a number, a text or a
// font. They sit under a heading for each group, and take effect when the person applies the dialog.
import type { SettingDefinition } from "@docxtor/extensions";
import { h } from "./dom";

export interface SettingFields {
  el: HTMLElement;
  /** Hand every value that changed to its setting. */
  apply(): Promise<void>;
}

let counter = 0;

export function settingFields(defs: readonly SettingDefinition[], fonts: () => string[]): SettingFields {
  const pending = new Map<SettingDefinition, unknown>();
  const groups = new Map<string, SettingDefinition[]>();
  for (const d of defs) {
    const g = d.group ?? "";
    groups.set(g, [...(groups.get(g) ?? []), d]);
  }
  const el = h("div", { class: "pref-settings" });
  const list = h("datalist", { id: `pref-fonts-${++counter}` });
  const listId = list.id;
  let listed = false;
  const fillFonts = () => {
    if (listed) return;
    listed = true;
    for (const f of fonts()) list.append(h("option", { value: f }));
  };

  for (const [group, items] of groups) {
    if (group) el.append(h("h3", { class: "pref-heading" }, group));
    for (const d of items) {
      const id = `pref-x-${++counter}`;
      const now = d.get();
      let control: HTMLElement;
      let inline = false;
      if (d.kind === "toggle") {
        const box = h("input", { type: "checkbox", id, checked: !!now });
        box.addEventListener("change", () => pending.set(d, box.checked));
        el.append(h("div", { class: "pref-check" }, h("label", { class: "check", for: id }, box, h("span", {}, d.label)), d.help ? h("p", { class: "pref-help" }, d.help) : null));
        continue;
      }
      if (d.kind === "choice") {
        const sel = h("select", { id }, ...(d.choices ?? []).map((c) => h("option", { value: c.value, selected: c.value === String(now) }, c.label)));
        sel.addEventListener("change", () => pending.set(d, sel.value));
        control = sel;
      } else if (d.kind === "number") {
        const input = h("input", { type: "number", id, value: String(now ?? ""), min: d.min, max: d.max, step: d.step ?? 1, class: "pref-number" });
        input.addEventListener("input", () => {
          const v = input.valueAsNumber;
          if (Number.isFinite(v)) pending.set(d, Math.min(d.max ?? Infinity, Math.max(d.min ?? -Infinity, v)));
        });
        control = d.unit ? h("span", { class: "pref-with-unit" }, input, h("span", { class: "pref-unit" }, d.unit)) : input;
        inline = true;
      } else {
        const input = h("input", { type: "text", id, value: String(now ?? ""), autocomplete: "off", spellcheck: "false", list: d.kind === "font" ? listId : undefined });
        if (d.kind === "font") input.addEventListener("focus", fillFonts, { once: true });
        input.addEventListener("input", () => pending.set(d, input.value.trim()));
        control = input;
      }
      el.append(h("div", { class: `pref-check${inline ? " inline" : ""}` }, h("div", { class: "field" }, h("label", { for: id }, d.label), control), d.help ? h("p", { class: "pref-help" }, d.help) : null));
    }
  }
  el.append(list);
  return {
    el,
    async apply() {
      for (const [d, v] of pending) await d.set(v);
      pending.clear();
    },
  };
}
