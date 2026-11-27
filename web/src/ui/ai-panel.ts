import { Agent } from "../ai/agent";
import { wordDiff } from "../ai/diff";
import { escapeHtml, h, I, msg, svg, t, tn, type AISettings, type Editor, type ExtensionHost } from "@docxtor/editor";
import { isConfigured, loadSettings, openSettings } from "./settings";

const SUGGESTIONS = [
  msg("Summarize this document"),
  msg("Fix spelling and grammar"),
  msg("Make the selection more concise"),
  msg("Add an executive summary at the top"),
  msg("Turn the selection into a table"),
  msg("Improve the headings"),
];

/** Minimal, safe Markdown for chat replies. */
function md(text: string): string {
  const lines = escapeHtml(text).split("\n");
  let html = "";
  let inList = false;
  for (const raw of lines) {
    const line = raw
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/(^|[^*])\*([^*\s][^*]*)\*/g, "$1<em>$2</em>");
    const li = line.match(/^\s*(?:[-*•]|\d+\.)\s+(.*)$/);
    if (li) {
      if (!inList) html += "<ul>";
      inList = true;
      html += `<li>${li[1]}</li>`;
      continue;
    }
    if (inList) {
      html += "</ul>";
      inList = false;
    }
    if (line.trim()) html += `<p>${line.replace(/^#+\s*/, "")}</p>`;
  }
  if (inList) html += "</ul>";
  return html;
}

function describe(name: string, input: Record<string, unknown>): string {
  const q = (v: unknown) => (typeof v === "string" ? `“${v.length > 40 ? v.slice(0, 40) + "…" : v}”` : "");
  switch (name) {
    case "read_document":
      return input.start ? t("from {id}", { id: String(input.start) }) : t("whole document");
    case "search":
      return q(input.query);
    case "replace_text":
      return t("{find} with {replace}", { find: q(input.find), replace: q(input.replace) });
    case "rewrite_paragraph":
    case "format_text":
    case "change_case":
    case "set_link":
      return String(input.id ?? "");
    case "insert_content":
      return input.after ? t("after {id}", { id: String(input.after) }) : input.before ? t("before {id}", { id: String(input.before) }) : t("at the end");
    case "delete_blocks":
    case "set_paragraph_style":
    case "format_paragraphs":
      return Array.isArray(input.ids) ? input.ids.join(", ") : "";
    default:
      return "";
  }
}

const LABELS: Record<string, string> = {
  read_document: msg("Read"),
  get_outline: msg("Outline"),
  search: msg("Search"),
  replace_text: msg("Replace"),
  rewrite_paragraph: msg("Rewrite"),
  insert_content: msg("Insert"),
  delete_blocks: msg("Delete"),
  format_text: msg("Format"),
  change_case: msg("Change case"),
  set_link: msg("Link"),
  set_paragraph_style: msg("Style"),
  format_paragraphs: msg("Layout"),
  edit_table: msg("Table"),
  list_styles: msg("Styles"),
  apply_ops: msg("Ops"),
  check_accessibility: msg("Accessibility"),
  set_document_properties: msg("Properties"),
  set_language: msg("Language"),
};

export class AIPanel {
  el: HTMLElement;
  private log!: HTMLElement;
  private input!: HTMLTextAreaElement;
  private sendBtn!: HTMLButtonElement;
  private chip!: HTMLButtonElement;
  private selChip!: HTMLElement;
  private agent: Agent;
  private settings: AISettings = loadSettings();
  private abort: AbortController | null = null;
  /** Settles the review card still waiting for Keep or Revert all, if any (see `review`). */
  private pendingReview: (() => void) | null = null;
  open = false;
  onToggle?: (open: boolean) => void;
  /** Version-history hooks around each AI run. */
  onRunStart?: (prompt: string) => Promise<void>;
  onRunEnd?: (prompt: string, changedParagraphs: number, model: string) => Promise<void>;

  constructor(
    private ed: Editor,
    private host: ExtensionHost,
  ) {
    this.agent = new Agent(ed, host);
    // Providers come and go with their extensions.
    host.aiProviders.onChange(() => this.applySettings(this.settings));
    this.el = h("aside", { class: "ai-panel", "aria-label": t("AI assistant") });
    this.build();
  }

  private title = h("div", { class: "t" }, h("i", {}), t("Assistant"));

  private build() {
    this.chip = h("button", { class: "model-chip", type: "button", title: t("AI settings") });
    this.chip.addEventListener("click", () => openSettings(this.host, (s) => this.applySettings(s)));
    const newChat = h("button", { class: "tb", type: "button", title: t("New chat") }, svg(I.refresh));
    newChat.addEventListener("click", () => this.reset());
    const close = h("button", { class: "tb", type: "button", title: t("Close") }, svg(I.close));
    close.addEventListener("click", () => this.toggle(false));
    this.log = h("div", { class: "ai-log", "aria-live": "polite" });
    this.input = h("textarea", { rows: 1, placeholder: t("Ask anything, or describe an edit…"), "aria-label": t("Message the assistant") });
    this.input.addEventListener("input", () => this.autosize());
    this.input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
        e.preventDefault();
        void this.send();
      }
    });
    this.sendBtn = h("button", { class: "send", type: "button", title: t("Send (Enter)"), "aria-label": t("Send") }, t("Send"));
    this.sendBtn.addEventListener("click", () => (this.abort ? this.stop() : void this.send()));
    this.selChip = h("span", { class: "sel-chip", style: "display:none" });
    const composer = h(
      "div",
      { class: "composer" },
      h("div", { class: "composer-box" }, this.input, h("div", { class: "composer-row" }, this.selChip, this.sendBtn)),
      h("div", { class: "composer-hint" }, h("span", {}, t("Edits apply live, and every change can be undone. Shift-Return adds a line."))),
    );
    this.el.append(
      h(
        "div",
        { class: "ai-inner" },
        h("div", { class: "ai-head" }, this.title, this.chip, newChat, close),
        this.log,
        composer,
      ),
    );
    this.applySettings(this.settings);
    this.empty();
  }

  private applySettings(s: AISettings) {
    this.settings = s;
    this.agent.reset();
    const ok = isConfigured(this.host, s);
    this.chip.innerHTML = "";
    this.chip.append(h("i", {}), ok ? s.model : t("Connect a model"));
    this.chip.classList.toggle("off", !ok);
    if (!this.log.querySelector(".entry")) this.empty();
  }

  private empty() {
    this.log.innerHTML = "";
    const ok = isConfigured(this.host, this.settings);
    const list = h("div", { class: "cmd-list" });
    SUGGESTIONS.forEach((source) => {
      const text = t(source);
      const c = h("button", { class: "cmd", type: "button" }, text);
      c.addEventListener("click", () => {
        this.input.value = text;
        void this.send();
      });
      list.append(c);
    });
    const connect = h("button", { class: "btn primary", type: "button" }, t("Connect a model"));
    connect.addEventListener("click", () => openSettings(this.host, (s) => this.applySettings(s)));
    this.log.append(
      h(
        "div",
        { class: "ai-empty" },
        h("h3", {}, ok ? t("Ask for an edit") : t("Bring your own model")),
        h(
          "p",
          {},
          ok
            ? t("It reads the whole document, makes its edits paragraph by paragraph, and shows each change so you can keep or undo it.")
            : t("Claude, OpenAI, Gemini, OpenRouter or a model on your own machine. Your key stays in this browser and goes only to the provider you pick."),
        ),
        ok ? list : connect,
      ),
    );
  }

  reset() {
    this.stop();
    // The cleared chat takes its review cards with it; what they changed stays, and undo still has it.
    this.pendingReview?.();
    this.agent.reset();
    this.empty();
  }

  toggle(open = !this.open) {
    this.open = open;
    this.el.classList.toggle("open", open);
    this.onToggle?.(open);
    if (open) setTimeout(() => this.input.focus(), 180);
  }

  /** Open with a prompt about the current selection. */
  askAbout(prompt?: string) {
    this.toggle(true);
    this.updateSelection();
    if (prompt) {
      this.input.value = prompt;
      void this.send();
    }
  }

  updateSelection() {
    const sel = this.ed.selectedText().trim();
    this.selChip.style.display = sel ? "" : "none";
    this.selChip.textContent = sel ? t("Selection: “{text}”", { text: `${sel.slice(0, 60)}${sel.length > 60 ? "…" : ""}` }) : "";
  }

  private autosize() {
    this.input.style.height = "auto";
    this.input.style.height = `${Math.min(180, this.input.scrollHeight)}px`;
  }

  private scroll() {
    this.log.scrollTop = this.log.scrollHeight;
  }

  stop() {
    this.abort?.abort();
  }

  private setBusy(busy: boolean) {
    this.sendBtn.innerHTML = "";
    this.sendBtn.append(busy ? t("Stop") : t("Send"));
    this.title.classList.toggle("busy", busy);
    this.sendBtn.classList.toggle("stop", busy);
    this.sendBtn.title = busy ? t("Stop") : t("Send");
  }

  async send() {
    const text = this.input.value.trim();
    if (!text || this.abort) return;
    // A document that can't be changed (open in another window, marked as final, a view that locks it) is not worked on: the engine would refuse every edit.
    if (this.ed.engine.access() === "view") {
      if (this.log.querySelector(".ai-empty")) this.log.innerHTML = "";
      this.log.append(h("div", { class: "err-line" }, t("This document is read-only, so the assistant can't change it. Ask again once it can be edited.")));
      this.scroll();
      return;
    }
    if (!isConfigured(this.host, this.settings)) {
      openSettings(this.host, (s) => {
        this.applySettings(s);
        if (isConfigured(this.host, s)) void this.send();
      });
      return;
    }
    if (this.log.querySelector(".ai-empty")) this.log.innerHTML = "";
    const selText = this.ed.selectedText().trim();
    this.log.append(
      h("div", { class: "entry user" }, h("span", { class: "who" }, t("You")), h("div", { class: "body" }, selText ? h("span", { class: "sel-ref" }, `On “${selText.slice(0, 80)}”`) : null, text)),
    );
    let steps = 0;
    this.input.value = "";
    this.autosize();
    await this.onRunStart?.(text).catch(() => undefined);
    // Asking again keeps what the last answer changed (undo still has it), so only one review holds undo history at a time.
    this.pendingReview?.();
    const raw = this.ed.engine.raw;
    const checkpoint = raw.checkpoint();
    // The undo history is trimmed to a memory budget; the steps of this run and its review must survive that, or
    // the changes could not be shown or reverted as one.
    raw.pinCheckpoint(checkpoint);
    const thinking = h("div", { class: "thinking" }, h("i", {}), h("span", {}, t("Thinking")));
    this.log.append(thinking);
    this.scroll();
    let bot: HTMLElement | null = null;
    let botText = "";
    this.abort = new AbortController();
    this.setBusy(true);
    try {
      await this.agent.run(
        this.settings,
        text,
        {
          onText: (d) => {
            if (!bot) {
              bot = h("div", { class: "body" });
              thinking.before(h("div", { class: "entry bot" }, h("span", { class: "who" }, t("AI")), bot));
            }
            botText += d;
            bot.innerHTML = md(botText);
            this.scroll();
          },
          onTool: (name, input, result, ok) => {
            bot = null;
            botText = "";
            steps++;
            const row = h(
              "div",
              { class: `tool-row ${ok ? "ok" : "bad"}`, role: "button", tabindex: "0", title: ok ? "Show result" : "Show error" },
              h("span", { class: "idx" }, String(steps).padStart(2, "0")),
              h("span", { class: "tname" }, LABELS[name] ? t(LABELS[name]) : name),
              h("span", { class: "tdetail" }, ok ? describe(name, input) : t("failed: {what}", { what: describe(name, input) })),
            );
            const out = h("div", { class: "tool-out", style: "display:none" }, result);
            row.addEventListener("click", () => (out.style.display = out.style.display ? "" : "none"));
            thinking.before(row, out);
            this.scroll();
          },
          onNote: (n) => {
            thinking.before(h("div", { class: "note-line" }, n));
          },
        },
        this.abort.signal,
      );
    } catch (e) {
      const err = e as Error;
      if (err.name !== "AbortError" && !this.abort?.signal.aborted) {
        thinking.before(h("div", { class: "err-line" }, friendlyError(err)));
      } else thinking.before(h("div", { class: "note-line" }, t("Stopped.")));
    } finally {
      thinking.remove();
      this.abort = null;
      this.setBusy(false);
      const changed = this.review(checkpoint);
      // The review card holds the pin until it is settled; with no card there is nothing to revert.
      if (!changed) raw.unpinCheckpoint(checkpoint);
      this.scroll();
      if (changed) void this.onRunEnd?.(text, changed, this.settings.model);
    }
  }

  /** Code-editor style review of what the agent changed. */
  private review(checkpoint: number): number {
    const raw = this.ed.engine.raw;
    const diff: [number, string | null, string | null][] = JSON.parse(raw.diffSince(checkpoint));
    if (!diff.length) return 0;
    this.ed.aiMarks = new Set(diff.filter((d) => d[2] !== null).map((d) => d[0]));
    this.ed.refresh({ keepScroll: true });
    this.ed.flow.querySelectorAll<HTMLElement>("[data-p]").forEach((p) => p.classList.toggle("ai-changed", this.ed.aiMarks.has(Number(p.dataset.p))));
    const list = h("div", { class: "review-list" });
    for (const [id, before, after] of diff.slice(0, 80)) {
      const item = h("div", { class: "review-item", role: "button", tabindex: "0" });
      if (before === null) item.innerHTML = `<span class="tag add">${escapeHtml(t("Added"))}</span><span><ins>${escapeHtml(after ?? "") || "¶"}</ins></span>`;
      else if (after === null) item.innerHTML = `<span class="tag del">${escapeHtml(t("Removed"))}</span><span><del>${escapeHtml(before) || "¶"}</del></span>`;
      else item.innerHTML = `<span class="tag mod">${escapeHtml(t("Edited"))}</span><span>${wordDiff(before, after)}</span>`;
      if (after !== null) item.addEventListener("click", () => this.ed.scrollToParagraph(id));
      list.append(item);
    }
    const keep = h("button", { class: "btn primary", type: "button" }, t("Keep"));
    const revert = h("button", { class: "btn", type: "button" }, svg(I.undo), t("Revert all"));
    const card = h(
      "div",
      { class: "review" },
      h("div", { class: "review-head" }, h("span", { class: "lbl-eng" }, t("Changes")), h("span", { class: "count", title: tn(diff.length, "{n} paragraph changed", "{n} paragraphs changed") }, `${diff.length} ¶`)),
      list,
      h("div", { class: "review-actions" }, revert, keep),
    );
    let released = false;
    const release = () => {
      if (released) return;
      released = true;
      if (this.pendingReview === keepOnNewRun) this.pendingReview = null;
      try {
        raw.unpinCheckpoint(checkpoint);
      } catch {
        // The document was closed meanwhile; its history went with it.
      }
    };
    const settle = (msg: string) => {
      release();
      this.ed.aiMarks.clear();
      this.ed.flow.querySelectorAll(".ai-changed").forEach((p) => p.classList.remove("ai-changed"));
      card.classList.add("settled");
      card.querySelector(".review-actions")!.replaceWith(h("div", { class: "review-actions" }, h("small", { style: "color:var(--muted)" }, msg)));
    };
    const keepOnNewRun = () => settle(t("Kept. Undo is still available."));
    this.pendingReview = keepOnNewRun;
    keep.addEventListener("click", keepOnNewRun);
    revert.addEventListener("click", () => {
      raw.revertTo(checkpoint);
      this.ed.refresh({ keepScroll: true });
      settle(t("Reverted."));
    });
    this.log.append(card);
    return diff.length;
  }
}

function friendlyError(e: Error): string {
  const m = e.message || String(e);
  if (/401|authentication|invalid x-api-key|incorrect api key/i.test(m)) return t("The provider rejected the API key. Check it in AI settings.");
  if (/Failed to fetch|NetworkError|CORS/i.test(m)) return t("Couldn't reach the provider. For local models, make sure the server is running and allows browser (CORS) requests.");
  if (/429|rate/i.test(m)) return t("The provider is limiting requests. Try again in a moment.");
  return m;
}
