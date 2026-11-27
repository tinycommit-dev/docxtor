import { DEFAULT_SETTINGS, migrateSettings } from "../ai/providers";
import { h, I, modal, svg, t, type AIProvider, type AISettings, type ExtensionHost } from "@docxtor/editor";
import { storage, store } from "../storage";

const KEY = "docxtor.ai.v2";
const OLD_KEY = "docxtor.ai.v1";

export function loadSettings(): AISettings {
  const saved = storage<Record<string, unknown> | null>(KEY, null) ?? storage<Record<string, unknown> | null>(OLD_KEY, null);
  return saved ? migrateSettings(saved) : { ...DEFAULT_SETTINGS, options: { ...DEFAULT_SETTINGS.options } };
}

export function providerOf(host: ExtensionHost, s: AISettings): AIProvider | undefined {
  return host.aiProviders.list().find((p) => p.id === s.provider);
}

export function isConfigured(host: ExtensionHost, s: AISettings): boolean {
  const p = providerOf(host, s);
  try {
    return !!p && p.configured(s);
  } catch {
    return false;
  }
}

/** The connection dialog. Providers come from extensions (the built-in ones and any others). */
export function openSettings(host: ExtensionHost, onSave: (s: AISettings) => void) {
  let s: AISettings = { ...loadSettings() };
  s.options = { ...s.options };
  const providers = host.aiProviders.list();
  if (!providers.length) {
    const close = modal(
      h(
        "div",
        { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("AI settings") },
        h("div", { class: "dialog-head" }, h("h2", {}, t("No AI providers")), h("p", { class: "sub" }, t("Turn on the AI providers extension, or add one, to connect a model."))),
        h("div", { class: "dialog-actions" }, h("button", { class: "btn primary", type: "button", onclick: () => close() }, t("OK"))),
      ),
    );
    return;
  }
  if (!providers.some((p) => p.id === s.provider)) s = { ...s, provider: providers[0].id, baseUrl: providers[0].baseUrl ?? "", model: providers[0].models[0] ?? "" };
  const body = h("div", {});
  const actions = h("div", { class: "dialog-actions" });
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("AI settings") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Connect a model")), h("p", { class: "sub" }, t("Any provider works. The model gets your document's full structure and a precise set of editing tools."))),
    h("div", { class: "dialog-body" }, body),
    actions,
  );
  const close = modal(dialog);

  const render = () => {
    const p = providers.find((x) => x.id === s.provider)!;
    body.innerHTML = "";
    const provider = h("select", { id: "ai-provider" });
    for (const pr of providers) provider.append(h("option", { value: pr.id, selected: pr.id === s.provider }, pr.label));
    provider.addEventListener("change", () => {
      const np = providers.find((x) => x.id === provider.value)!;
      s = { ...s, provider: np.id, baseUrl: np.baseUrl ?? "", model: np.models[0] ?? (np.id === s.provider ? s.model : ""), options: {} };
      render();
    });
    body.append(h("div", { class: "field" }, h("label", { for: "ai-provider" }, t("Provider")), provider));

    if (p.keyHint !== undefined) {
      const key = h("input", { id: "ai-key", type: "password", value: s.apiKey, placeholder: p.keyHint, autocomplete: "off", spellcheck: "false" });
      key.addEventListener("input", () => (s.apiKey = key.value.trim()));
      body.append(h("div", { class: "field" }, h("label", { for: "ai-key" }, t("API key")), key));
    }

    if (p.baseUrl !== undefined) {
      const base = h("input", { id: "ai-base", type: "url", value: s.baseUrl || p.baseUrl, placeholder: "https://…/v1" });
      base.addEventListener("input", () => (s.baseUrl = base.value.trim()));
      body.append(h("div", { class: "field" }, h("label", { for: "ai-base" }, t("Base URL")), base, p.note ? h("div", { class: "help" }, p.note) : null));
    } else if (p.note) body.append(h("div", { class: "field" }, h("div", { class: "help" }, p.note)));

    if (p.models.length) {
      const model = h("select", { id: "ai-model" });
      for (const m of p.models) model.append(h("option", { value: m, selected: m === s.model }, m));
      model.addEventListener("change", () => (s.model = model.value));
      if (!p.models.includes(s.model)) s.model = p.models[0];
      body.append(h("div", { class: "field" }, h("label", { for: "ai-model" }, t("Model")), model));
    } else {
      const model = h("input", { id: "ai-model", type: "text", value: s.model, placeholder: t("model id, e.g. the one your provider lists"), spellcheck: "false" });
      model.addEventListener("input", () => (s.model = model.value.trim()));
      body.append(
        h("div", { class: "field" }, h("label", { for: "ai-model" }, t("Model")), model, h("div", { class: "help" }, t("Pick a model that supports tool (function) calling."))),
      );
    }

    for (const opt of p.options ?? []) {
      if (!opt.choices.includes(String(s.options[opt.id]))) s.options[opt.id] = opt.default;
      const seg = h("div", { class: "seg", role: "radiogroup", "aria-label": opt.label });
      for (const c of opt.choices) {
        const b = h("button", { type: "button", role: "radio", "aria-checked": String(s.options[opt.id] === c), class: s.options[opt.id] === c ? "on" : "" }, c);
        b.addEventListener("click", () => {
          s.options[opt.id] = c;
          seg.querySelectorAll("button").forEach((x) => {
            x.classList.toggle("on", x === b);
            x.setAttribute("aria-checked", String(x === b));
          });
        });
        seg.append(b);
      }
      body.append(h("div", { class: "field" }, h("label", {}, opt.label), seg, opt.help ? h("div", { class: "help" }, opt.help) : null));
    }

    const privacy = h("div", { class: "privacy" }, svg(I.lock), h("span", {}, t("Your key is stored only in this browser and sent only to the provider you choose. Docxtor has no servers.")));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const save = h("button", { class: "btn primary", type: "button" }, t("Save"));
    cancel.addEventListener("click", close);
    save.addEventListener("click", () => {
      if (!s.baseUrl && p.baseUrl) s.baseUrl = p.baseUrl;
      store(KEY, s);
      close();
      onSave(s);
    });
    body.append(privacy);
    actions.replaceChildren(cancel, save);
  };
  render();
}
