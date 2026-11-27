// LanguageTool, as an app extension: grammar and style checking by a LanguageTool server, next to Docxtor's own
// grammar rules. It is its own extension, and off until someone turns it on, because it is the one place a
// document's text leaves the page: the paragraphs on screen are sent to the server in the settings
// (languagetool.org unless changed). A server you run yourself keeps the text at home.
//
// It plugs into `ctx.grammar` like any checker. The server is asked for a few paragraphs at a time: requests
// wait a moment for more paragraphs to join them, one is in flight at once, paragraphs already checked are
// remembered by their text, and a "too many requests" answer makes it back off.
import { timeLimit, defineExtension, h, modal, msg, t, type Disposable, type Extension, type ExtensionContext, type GrammarIssue, type GrammarParagraph } from "@docxtor/editor";

export const DEFAULT_SERVER = "https://api.languagetool.org";

/** How long requests wait for more paragraphs to join them, and the longest they wait in all. */
const DEBOUNCE_MS = 400;
const MAX_WAIT_MS = 1500;
/** LanguageTool's public server allows 20 requests a minute and 20,000 characters in one. */
const PUBLIC_GAP_MS = 3100;
const OWN_GAP_MS = 150;
const MAX_CHARS = 15000;
const REMEMBERED = 4000;
const ATTEMPTS = 3;

export interface Settings {
  server: string;
  username: string;
  apiKey: string;
  /** Also show LanguageTool's spelling findings (Docxtor has a spelling checker of its own, so they are left out by default). */
  spelling: boolean;
}

export const readSettings = (ctx: ExtensionContext): Settings => ({
  server: String(ctx.state.get("server", DEFAULT_SERVER) || DEFAULT_SERVER),
  username: String(ctx.state.get("username", "")),
  apiKey: String(ctx.state.get("apiKey", "")),
  spelling: !!ctx.state.get("spelling", false),
});

/** A server address, tidied: no spaces or trailing slash, and http or https. Null when it is not one. */
export function cleanServer(text: string): string | null {
  const v = text.trim().replace(/\/+$/, "").replace(/\/v2(\/check)?$/, "");
  try {
    const u = new URL(v);
    return u.protocol === "https:" || u.protocol === "http:" ? v : null;
  } catch {
    return null;
  }
}

/** The language code LanguageTool wants for a tag from the document (`en-US`, `es-MX`, `pt`). */
export function languageCode(tag: string): string {
  const [base, region = ""] = tag.replace(/_/g, "-").split("-");
  const lang = base.toLowerCase();
  const place = region.toUpperCase();
  if (lang === "en") return ["US", "GB", "AU", "CA", "NZ", "ZA"].includes(place) ? `en-${place}` : "en-US";
  if (lang === "de") return ["DE", "AT", "CH"].includes(place) ? `de-${place}` : "de-DE";
  if (lang === "pt") return place === "BR" ? "pt-BR" : "pt-PT";
  if (lang === "zh") return "zh-CN";
  return lang;
}

type Found = Omit<GrammarIssue, "paragraph">;

interface Wanted {
  key: string;
  text: string;
  lang: string;
  attempts: number;
  waiters: ((issues: Found[] | null) => void)[];
}

interface Match {
  message?: string;
  shortMessage?: string;
  offset: number;
  length: number;
  replacements?: { value?: string }[];
  rule?: { id?: string; description?: string; issueType?: string; category?: { id?: string; name?: string } };
}

/** What kind of finding a LanguageTool match is. */
function kindOf(m: Match): "grammar" | "style" | "spelling" {
  const type = m.rule?.issueType;
  if (type === "misspelling" || m.rule?.category?.id === "TYPOS") return "spelling";
  if (type === "style" || type === "register" || m.rule?.category?.id === "STYLE") return "style";
  return "grammar";
}

/** Asks a LanguageTool server, politely. */
export class LanguageToolChecker {
  private cache = new Map<string, Found[]>();
  private wanted = new Map<string, Wanted>();
  private timer = 0;
  private firstAt = 0;
  private waiting = 0;
  private busy = false;
  private last = 0;
  private retryAt = 0;
  private backoff = 0;
  private unsupported = new Set<string>();
  /** Requests sent, for tests. */
  requests = 0;

  constructor(private settings: () => Settings) {}

  async check({ paragraphs }: { paragraphs: GrammarParagraph[] }): Promise<GrammarIssue[]> {
    const out: GrammarIssue[] = [];
    const waits: Promise<void>[] = [];
    const spelling = this.settings().spelling;
    for (const p of paragraphs) {
      // An object (or a link or code the editor masked) becomes a space of the same length.
      const text = p.text.replace(/￼/g, " ");
      if (text.trim().length < 2) continue;
      const lang = languageCode(p.lang);
      if (this.unsupported.has(lang)) continue;
      const key = `${lang}\u0000${text}`;
      const take = (found: Found[] | null) => {
        for (const f of found ?? []) {
          if (f.kind === "spelling" && !spelling) continue;
          // Not where an object stood.
          if (p.text.slice(f.from, f.to).includes("￼")) continue;
          out.push({ ...f, paragraph: p.id });
        }
      };
      const cached = this.cache.get(key);
      if (cached) take(cached);
      else waits.push(this.want(key, text, lang).then(take));
    }
    await Promise.all(waits);
    return out;
  }

  private want(key: string, text: string, lang: string): Promise<Found[] | null> {
    return new Promise((resolve) => {
      let w = this.wanted.get(key);
      if (!w) this.wanted.set(key, (w = { key, text, lang, attempts: 0, waiters: [] }));
      w.waiters.push(resolve);
      this.schedule();
    });
  }

  /** Wait a little for more paragraphs, but never longer than MAX_WAIT_MS from the first. */
  private schedule() {
    const now = Date.now();
    if (!this.firstAt) this.firstAt = now;
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => {
      this.firstAt = 0;
      void this.pump();
    }, Math.max(0, Math.min(DEBOUNCE_MS, this.firstAt + MAX_WAIT_MS - now)));
  }

  private gap(): number {
    const { server } = this.settings();
    return cleanServer(server) === DEFAULT_SERVER ? PUBLIC_GAP_MS : OWN_GAP_MS;
  }

  /** Send the next batch: the newest paragraphs first (they are the ones on screen), one language per request. */
  private async pump() {
    if (this.busy || !this.wanted.size) return;
    const wait = Math.max(this.retryAt, this.last + this.gap()) - Date.now();
    if (wait > 0) {
      if (!this.waiting) {
        this.waiting = window.setTimeout(() => {
          this.waiting = 0;
          void this.pump();
        }, wait);
      }
      return;
    }
    const all = [...this.wanted.values()].reverse();
    const lang = all[0].lang;
    const batch: Wanted[] = [];
    let size = 0;
    for (const w of all) {
      if (w.lang !== lang) continue;
      // Longer than the server takes: nothing to be done for it.
      if (w.text.length > 19000) {
        this.wanted.delete(w.key);
        w.waiters.forEach((r) => r(null));
        continue;
      }
      if (batch.length && size + w.text.length + 2 > MAX_CHARS) break;
      batch.push(w);
      size += w.text.length + 2;
    }
    if (!batch.length) {
      if (this.wanted.size) void this.pump();
      return;
    }
    // The newest paragraphs were picked first; within the request they are in the order they were asked for.
    batch.reverse();
    for (const w of batch) this.wanted.delete(w.key);
    this.busy = true;
    let result: Map<string, Found[]> | "retry" | null = null;
    try {
      result = await this.request(batch, lang);
    } finally {
      this.busy = false;
      this.last = Date.now();
    }
    for (const w of batch) {
      if (result === "retry" && ++w.attempts < ATTEMPTS) {
        // Put it back with whoever is waiting for it.
        const again = this.wanted.get(w.key);
        if (again) again.waiters.push(...w.waiters);
        else this.wanted.set(w.key, w);
        continue;
      }
      const found = result instanceof Map ? (result.get(w.key) ?? []) : null;
      if (found) {
        if (this.cache.size > REMEMBERED) this.cache.clear();
        this.cache.set(w.key, found);
      }
      w.waiters.forEach((r) => r(found));
    }
    if (this.wanted.size) void this.pump();
  }

  /** One request. "retry" means try again later (the server asked us to slow down, or could not be reached). */
  private async request(batch: Wanted[], lang: string): Promise<Map<string, Found[]> | "retry" | null> {
    const s = this.settings();
    const server = cleanServer(s.server) ?? DEFAULT_SERVER;
    const text = batch.map((w) => w.text).join("\n\n");
    const body = new URLSearchParams({ text, language: lang });
    if (s.username && s.apiKey) {
      body.set("username", s.username);
      body.set("apiKey", s.apiKey);
    }
    let res: Response;
    this.requests++;
    try {
      res = await fetch(`${server}/v2/check`, { method: "POST", headers: { Accept: "application/json" }, body, signal: timeLimit(20000) });
    } catch {
      this.backoff = Math.min(Math.max(this.backoff * 2, 5000), 60000);
      this.retryAt = Date.now() + this.backoff;
      return "retry";
    }
    if (res.status === 429) {
      const after = Number(res.headers.get("Retry-After")) * 1000;
      this.backoff = Math.min(Math.max(this.backoff * 2, 2000, after || 0), 60000);
      this.retryAt = Date.now() + this.backoff;
      return "retry";
    }
    if (!res.ok) {
      // A language the server does not know is left alone for the rest of the session.
      if (res.status === 400 && /language/i.test(await res.text().catch(() => ""))) this.unsupported.add(lang);
      else {
        this.backoff = Math.min(Math.max(this.backoff * 2, 5000), 60000);
        this.retryAt = Date.now() + this.backoff;
        return res.status >= 500 ? "retry" : null;
      }
      return null;
    }
    this.backoff = 0;
    let json: { matches?: Match[] };
    try {
      json = (await res.json()) as { matches?: Match[] };
    } catch {
      return null;
    }
    // Where each paragraph starts in the text that was sent.
    const starts: number[] = [];
    let at = 0;
    for (const w of batch) {
      starts.push(at);
      at += w.text.length + 2;
    }
    const result = new Map<string, Found[]>(batch.map((w) => [w.key, []]));
    for (const m of json.matches ?? []) {
      if (typeof m.offset !== "number" || typeof m.length !== "number" || !m.length || !m.message) continue;
      let i = starts.length - 1;
      while (i > 0 && starts[i] > m.offset) i--;
      const w = batch[i];
      const from = m.offset - starts[i];
      if (from < 0 || from + m.length > w.text.length) continue;
      result.get(w.key)!.push({
        from,
        to: from + m.length,
        message: m.message,
        ...(m.shortMessage ? { shortMessage: m.shortMessage } : {}),
        replacements: (m.replacements ?? []).map((r) => r.value ?? "").filter(Boolean).slice(0, 5),
        rule: { id: m.rule?.id ?? "LANGUAGETOOL", ...(m.rule?.description ? { description: m.rule.description } : {}), ...(m.rule?.category?.name ? { category: m.rule.category.name } : {}) },
        kind: kindOf(m),
        source: "docxtor.languagetool",
      });
    }
    return result;
  }
}

/** The settings dialog (Tools, Spelling and grammar, LanguageTool settings…). It says plainly where the text goes. */
export function openLanguageToolSettings(ctx: ExtensionContext, applied: () => void) {
  const s = readSettings(ctx);
  const server = h("input", { id: "lt-server", type: "url", value: s.server, placeholder: DEFAULT_SERVER, spellcheck: "false", autocomplete: "off" });
  const username = h("input", { id: "lt-user", type: "text", value: s.username, spellcheck: "false", autocomplete: "off" });
  const key = h("input", { id: "lt-key", type: "password", value: s.apiKey, spellcheck: "false", autocomplete: "off" });
  const spelling = h("input", { type: "checkbox", id: "lt-spelling", checked: s.spelling });
  const where = h("p", { class: "lt-where" });
  const problem = h("div", { class: "help lt-problem", hidden: true }, t("Enter an address that starts with https:// or http://."));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const save = h("button", { class: "btn primary", type: "button" }, t("Save"));

  const paint = () => {
    const clean = cleanServer(server.value);
    problem.hidden = !server.value.trim() || !!clean;
    save.disabled = !clean && !!server.value.trim();
    const host = clean ? new URL(clean).host : new URL(DEFAULT_SERVER).host;
    where.textContent = t("The text of the paragraphs being checked is sent to {server}. Nothing is sent while this extension is off.", { server: host });
  };
  server.addEventListener("input", paint);
  paint();

  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("LanguageTool settings") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("LanguageTool settings")), h("p", { class: "sub" }, t("Checks grammar and style with a LanguageTool server, next to Docxtor's own grammar rules."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, where),
      h(
        "div",
        { class: "field" },
        h("label", { for: "lt-server" }, t("Server")),
        server,
        problem,
        h("div", { class: "help" }, t("Leave the default to use LanguageTool's public server, or enter the address of one you run yourself to keep your text private. A server you run must allow requests from this page (its allow-origin option).")),
      ),
      h("div", { class: "field" }, h("label", { for: "lt-user" }, t("Username (optional)")), username),
      h("div", { class: "field" }, h("label", { for: "lt-key" }, t("API key (optional)")), key, h("div", { class: "help" }, t("For LanguageTool Premium. They are sent to the same server, with the text."))),
      h("div", { class: "pref-check" }, h("label", { class: "check", for: "lt-spelling" }, spelling, h("span", {}, t("Also show LanguageTool's spelling suggestions"))), h("p", { class: "pref-help" }, t("Docxtor has a spelling checker of its own, so these are left out unless you ask for them."))),
    ),
    h("div", { class: "dialog-actions" }, cancel, save),
  );
  const close = modal(dialog, () => ctx.document.focus());
  server.focus();
  cancel.addEventListener("click", close);
  save.addEventListener("click", () => {
    ctx.state.set("server", cleanServer(server.value) ?? DEFAULT_SERVER);
    ctx.state.set("username", username.value.trim());
    ctx.state.set("apiKey", key.value.trim());
    ctx.state.set("spelling", spelling.checked);
    close();
    applied();
  });
}

export const languageTool: Extension = defineExtension({
  id: "docxtor.languagetool",
  name: msg("LanguageTool"),
  description: msg("Grammar and style checking by a LanguageTool server, next to Docxtor's own grammar rules."),
  privacy: msg("Sends the text of your document to the LanguageTool server named in its settings (languagetool.org unless you change it)."),
  enabledByDefault: false,
  activate(ctx) {
    let registered: Disposable | null = null;
    const install = () => {
      registered?.dispose();
      const checker = new LanguageToolChecker(() => readSettings(ctx));
      registered = ctx.grammar.register({ id: "docxtor.languagetool", label: "LanguageTool", languages: "*", check: (request) => checker.check(request) });
    };
    install();
    ctx.commands.register("tools.languageToolSettings", () => openLanguageToolSettings(ctx, install), { title: t("LanguageTool settings…"), category: t("Tools") });
    ctx.menus.register({ menu: "tools/spelling", group: "3", command: "tools.languageToolSettings" });
  },
});
