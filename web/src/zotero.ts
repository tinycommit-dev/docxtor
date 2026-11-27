// Zotero as a source provider: search a Zotero library from "Add a source" and add the items as sources. Two ways in:
//
// - Zotero 7 on this computer, through its local API at http://localhost:23119/api/. No key is needed, but the person
//   has to turn on "Allow other applications on this computer to communicate with Zotero" in Zotero's settings.
// - The Zotero web library, through https://api.zotero.org/ with a user ID and an API key from zotero.org/settings/keys.
//   The key is kept in this browser, with the other settings of this extension, and is sent only to api.zotero.org.
//
// Items come back as CSL-JSON, which Docxtor already reads (the same way it reads a DOI at doi.org), so the mapping to
// sources is the engine's own. It is an extension of its own: turn it off under Extensions, Manage extensions, and the
// app stops asking Zotero anything.
import { timeLimit, defineExtension, Engine, h, modal, msg, t, type Extension, type Source, type SourceResult } from "@docxtor/editor";

export type ZoteroMode = "local" | "web";

export interface ZoteroSettings {
  mode: ZoteroMode;
  userId: string;
  apiKey: string;
}

export const LOCAL_API = "http://localhost:23119/api";
export const WEB_API = "https://api.zotero.org";

/** The address that lists top-level items matching a search, as JSON with each item's CSL-JSON included. */
export function itemsUrl(s: ZoteroSettings, query: string): string {
  const base = s.mode === "local" ? `${LOCAL_API}/users/0` : `${WEB_API}/users/${encodeURIComponent(s.userId.trim())}`;
  const url = new URL(`${base}/items/top`);
  url.searchParams.set("format", "json");
  url.searchParams.set("include", "csljson");
  url.searchParams.set("limit", "25");
  url.searchParams.set("sort", "dateModified");
  url.searchParams.set("direction", "desc");
  if (query.trim()) url.searchParams.set("q", query.trim());
  return url.toString();
}

/** What Zotero says in its list: an item has a key, its own fields (`data`) and, asked for, its CSL-JSON. */
interface ZoteroItem {
  key?: string;
  data?: { itemType?: string };
  csljson?: Record<string, unknown>;
}

/** Items read as sources. Attachments and notes have nothing to cite, and an item Docxtor can't read is left out. */
export function itemsToResults(items: ZoteroItem[]): SourceResult[] {
  const out: SourceResult[] = [];
  for (const [i, item] of items.entries()) {
    if (!item.csljson || item.data?.itemType === "attachment" || item.data?.itemType === "note") continue;
    let source: Source | undefined;
    try {
      source = Engine.parseCslJson(JSON.stringify([item.csljson]))[0];
    } catch {
      continue;
    }
    if (source) out.push({ id: item.key ?? `item${i}`, source });
  }
  return out;
}

const NOT_REACHABLE = () =>
  t("Couldn't reach Zotero on this computer. Open Zotero, then in its settings under Advanced turn on “Allow other applications on this computer to communicate with Zotero”.");

/** Ask Zotero. The thrown message says what to do about it. */
export async function searchZotero(s: ZoteroSettings, query: string, signal: AbortSignal): Promise<SourceResult[]> {
  if (s.mode === "web" && (!s.userId.trim() || !s.apiKey.trim())) throw new Error(t("Enter your Zotero user ID and an API key in Zotero settings first."));
  let res: Response;
  try {
    res = await fetch(itemsUrl(s, query), { signal: timeLimit(15000, signal), headers: s.mode === "web" ? { "Zotero-API-Version": "3", "Zotero-API-Key": s.apiKey.trim() } : { "Zotero-API-Version": "3" } });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw e;
    throw new Error(s.mode === "local" ? NOT_REACHABLE() : t("Couldn't reach api.zotero.org. Check your connection and try again."));
  }
  if (s.mode === "local" && (res.status === 403 || res.status === 404)) throw new Error(NOT_REACHABLE());
  if (res.status === 403) throw new Error(t("Zotero didn't accept that API key. Check the key and the user ID in Zotero settings."));
  if (res.status === 404) throw new Error(t("Zotero has no library for that user ID. Check it in Zotero settings."));
  if (!res.ok) throw new Error(t("Zotero didn't answer (error {status}). Try again later.", { status: res.status }));
  let items: ZoteroItem[];
  try {
    items = await res.json();
    if (!Array.isArray(items)) throw new Error("not a list");
  } catch {
    throw new Error(t("Zotero's answer could not be read."));
  }
  return itemsToResults(items);
}

/** The settings dialog: which library, and for the web library a user ID and an API key. */
function openSettings(current: ZoteroSettings, save: (s: ZoteroSettings) => void): Promise<void> {
  return new Promise((resolve) => {
    const local = h("input", { type: "radio", name: "zotero-mode", id: "zotero-local", value: "local", checked: current.mode === "local" });
    const web = h("input", { type: "radio", name: "zotero-mode", id: "zotero-web", value: "web", checked: current.mode === "web" });
    const userId = h("input", { type: "text", id: "zotero-user", autocomplete: "off", spellcheck: "false", value: current.userId, placeholder: "1234567" });
    const apiKey = h("input", { type: "password", id: "zotero-key", autocomplete: "off", spellcheck: "false", value: current.apiKey });
    const webFields = h(
      "div",
      { class: "zotero-web" },
      h("div", { class: "field" }, h("label", { for: "zotero-user" }, t("User ID")), userId, h("span", { class: "help" }, t("The number on zotero.org/settings/keys, not your user name."))),
      h("div", { class: "field" }, h("label", { for: "zotero-key" }, t("API key")), apiKey, h("span", { class: "help" }, t("The key stays in this browser. It is sent only to api.zotero.org. Give it read access to your library."))),
    );
    const sync = () => (webFields.hidden = !web.checked);
    local.addEventListener("change", sync);
    web.addEventListener("change", sync);
    sync();
    const ok = h("button", { class: "btn primary", type: "submit" }, t("Save"));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const form = h(
      "form",
      { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Zotero settings"), style: "width:min(var(--dialog-m),100%)", novalidate: true },
      h("div", { class: "dialog-head" }, h("h2", {}, t("Zotero settings")), h("p", { class: "sub" }, t("Choose where Docxtor finds your Zotero library."))),
      h(
        "div",
        { class: "dialog-body" },
        h("div", { class: "field" }, h("label", { class: "radio", for: "zotero-local" }, local, h("span", {}, t("Zotero on this computer"))), h("span", { class: "help" }, t("Needs Zotero 7 running, with “Allow other applications on this computer to communicate with Zotero” turned on in its Advanced settings."))),
        h("div", { class: "field" }, h("label", { class: "radio", for: "zotero-web" }, web, h("span", {}, t("Zotero web library")))),
        webFields,
      ),
      h("div", { class: "dialog-actions" }, cancel, ok),
    );
    const close = modal(form, resolve);
    cancel.addEventListener("click", close);
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      save({ mode: web.checked ? "web" : "local", userId: userId.value.trim(), apiKey: apiKey.value.trim() });
      close();
    });
  });
}

export function zotero(): Extension {
  return defineExtension({
    id: "docxtor.zotero",
    name: msg("Zotero"),
    description: msg("Search your Zotero library and add items as sources. It asks Zotero on this computer, or api.zotero.org with the user ID and API key you enter. Turn this off to stop asking Zotero anything."),
    activate(ctx) {
      const read = (): ZoteroSettings => ({ mode: ctx.state.get<ZoteroMode>("mode", "local"), userId: ctx.state.get("userId", ""), apiKey: ctx.state.get("apiKey", "") });
      ctx.citations.sources.register({
        id: "zotero",
        label: t("From Zotero…"),
        title: t("Add from Zotero"),
        intro: t("Search your Zotero library, tick the items to add, and Docxtor adds them as sources."),
        inputLabel: t("Search your Zotero library"),
        placeholder: t("Title, author or year"),
        help: t("Docxtor asks Zotero as you type. Items you don't tick stay in Zotero."),
        note: t("Zotero is asked on this computer, or at api.zotero.org if you chose the web library. Your API key stays in this browser."),
        search: (query, signal) => searchZotero(read(), query, signal),
        settings: {
          label: t("Zotero settings…"),
          open: () =>
            openSettings(read(), (s) => {
              ctx.state.set("mode", s.mode);
              ctx.state.set("userId", s.userId);
              ctx.state.set("apiKey", s.apiKey);
            }),
        },
      });
    },
  });
}
