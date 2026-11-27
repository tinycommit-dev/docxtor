// The Extensions dialog: every extension (built-in ones included), what it adds, and a switch.
import { currentLocale, h, modal, t, tn, type ExtensionHost, type ExtensionInfo, type Registry } from "@docxtor/editor";

/** "12 commands, 3 exporters": what an enabled extension currently contributes. */
function contributions(host: ExtensionHost, id: string): string {
  const kinds: [Registry<unknown>, (n: number) => string][] = [
    [host.commands as Registry<unknown>, (n) => tn(n, "{n} command", "{n} commands")],
    [host.toolbar as Registry<unknown>, (n) => tn(n, "{n} toolbar button", "{n} toolbar buttons")],
    [host.menus as Registry<unknown>, (n) => tn(n, "{n} menu item", "{n} menu items")],
    [host.panels as Registry<unknown>, (n) => tn(n, "{n} panel", "{n} panels")],
    [host.exporters as Registry<unknown>, (n) => tn(n, "{n} export format", "{n} export formats")],
    [host.importers as Registry<unknown>, (n) => tn(n, "{n} import format", "{n} import formats")],
    [host.templates as Registry<unknown>, (n) => tn(n, "{n} template", "{n} templates")],
    [host.themes as Registry<unknown>, (n) => tn(n, "{n} theme", "{n} themes")],
    [host.aiProviders as Registry<unknown>, (n) => tn(n, "{n} AI provider", "{n} AI providers")],
    [host.aiTools as Registry<unknown>, (n) => tn(n, "{n} AI tool", "{n} AI tools")],
    [host.storage as Registry<unknown>, (n) => tn(n, "{n} storage location", "{n} storage locations")],
    [host.spellingDictionaries as Registry<unknown>, (n) => tn(n, "{n} spelling dictionary source", "{n} spelling dictionary sources")],
    [host.grammarCheckers as Registry<unknown>, (n) => tn(n, "{n} grammar checker", "{n} grammar checkers")],
  ];
  const parts: string[] = [];
  for (const [reg, label] of kinds) {
    const n = reg.all().filter((e) => e.extension === id).length;
    if (n) parts.push(label(n));
  }
  return new Intl.ListFormat(currentLocale() === "en-XA" ? "en" : currentLocale(), { type: "conjunction" }).format(parts);
}

/** Text as it is searched: lower case, without accents (búsqueda finds busqueda). */
const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

export function openExtensions(host: ExtensionHost, toggle: (id: string, on: boolean) => Promise<void>) {
  const list = h("div", { class: "ext-list" });
  const search = h("input", { type: "search", class: "kb-search ext-search", "aria-label": t("Search extensions"), placeholder: t("Search extensions"), autocomplete: "off", spellcheck: "false" });
  const note = h("p", { class: "kb-note ext-note", role: "status" });
  search.addEventListener("input", () => render());
  const close = modal(
    h(
      "div",
      { class: "dialog ext-dialog", role: "dialog", "aria-modal": "true", "aria-labelledby": "ext-title" },
      h(
        "div",
        { class: "dialog-head" },
        h("h2", { id: "ext-title" }, t("Extensions")),
        h("p", { class: "sub" }, t("Docxtor's own features are built as extensions, on the same API anyone can use. Turn off the ones you don't need.")),
        search,
        note,
      ),
      h("div", { class: "dialog-body" }, list),
      h("div", { class: "dialog-actions" }, h("button", { class: "btn primary", type: "button", onclick: () => close() }, t("Done"))),
    ),
    () => sub.dispose(),
  );

  const row = (x: ExtensionInfo) => {
    const what = x.enabled ? contributions(host, x.id) : "";
    const sw = x.required
      ? h("span", { class: "ext-fixed" }, t("Always on"))
      : h("button", { class: "switch", type: "button", role: "switch", "data-ext": x.id, "aria-checked": String(x.enabled), "aria-label": x.enabled ? t("{name}: on", { name: t(x.name) }) : t("{name}: off", { name: t(x.name) }) });
    if (!x.required) sw.addEventListener("click", () => void toggle(x.id, !x.enabled));
    return h(
      "div",
      { class: `ext-row${x.enabled ? "" : " off"}` },
      h(
        "div",
        { class: "ext-text" },
        h("div", { class: "ext-name" }, t(x.name), x.builtin ? h("span", { class: "ext-tag" }, t("Built in")) : null, x.version ? h("span", { class: "ext-ver" }, x.version) : null),
        x.description ? h("p", { class: "ext-desc" }, t(x.description)) : null,
        x.privacy ? h("p", { class: "ext-desc ext-privacy" }, t(x.privacy)) : null,
        x.error ? h("p", { class: "ext-error" }, t("Couldn't start: {reason}", { reason: x.error })) : what ? h("p", { class: "ext-adds" }, t("Adds {what}", { what })) : null,
      ),
      sw,
    );
  };
  const render = () => {
    const all = host.list();
    // Others' extensions first: they're the ones people come here to manage.
    const sorted = [...all.filter((x) => !x.builtin), ...all.filter((x) => x.builtin)];
    // Every word searched for is in its name, its description, what it adds, or its id.
    const words = fold(search.value).split(/\s+/).filter(Boolean);
    const shown = words.length
      ? sorted.filter((x) => {
          const text = fold([t(x.name), x.description ? t(x.description) : "", x.enabled ? contributions(host, x.id) : "", x.id].join(" "));
          return words.every((w) => text.includes(w));
        })
      : sorted;
    note.textContent = words.length ? (shown.length ? tn(shown.length, "{n} extension", "{n} extensions") : t("No extension matches.")) : tn(all.length, "{n} extension", "{n} extensions");
    const focused = (document.activeElement as HTMLElement | null)?.dataset.ext;
    list.replaceChildren(...shown.map(row));
    if (focused) list.querySelector<HTMLElement>(`.switch[data-ext="${CSS.escape(focused)}"]`)?.focus();
  };
  const sub = host.onChange(render);
  render();
}
