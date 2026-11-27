// Model providers for the in-app assistant, contributed through the extension API by the
// built-in "AI providers" extension. Keys stay in the browser and are sent only to the provider
// the user configured.
// The SDK is loaded the first time the assistant talks to Anthropic, not on every visit.
import type Anthropic from "@anthropic-ai/sdk";
import { defineExtension, msg, t, type AIProvider, type AISettings, type Conversation, type StepResult, type ToolDefinition as ToolDef } from "@docxtor/editor";

export type Effort = "low" | "medium" | "high" | "xhigh" | "max";
const EFFORTS: Effort[] = ["low", "medium", "high", "xhigh", "max"];

export const DEFAULT_SETTINGS: AISettings = { provider: "anthropic", apiKey: "", baseUrl: "", model: "claude-opus-5-5", options: { effort: "high" } };

interface Compatible {
  id: string;
  label: string;
  baseUrl: string;
  keyHint?: string;
  note?: string;
  /** Works without an API key. */
  keyless?: boolean;
}

/** Providers that speak the OpenAI chat-completions protocol. */
const COMPATIBLE: Compatible[] = [
  { id: "openai", label: "OpenAI", baseUrl: "https://api.openai.com/v1", keyHint: "sk-…" },
  { id: "openrouter", label: msg("OpenRouter (any model)"), baseUrl: "https://openrouter.ai/api/v1", keyHint: "sk-or-…" },
  { id: "gemini", label: "Google Gemini", baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai", keyHint: msg("AI Studio key") },
  { id: "groq", label: "Groq", baseUrl: "https://api.groq.com/openai/v1", keyHint: "gsk_…" },
  { id: "mistral", label: "Mistral", baseUrl: "https://api.mistral.ai/v1", keyHint: msg("Mistral key") },
  {
    id: "ollama",
    label: msg("Ollama (local)"),
    baseUrl: "http://localhost:11434/v1",
    keyless: true,
    note: msg("Start Ollama with OLLAMA_ORIGINS=* so the browser may connect. Use a model with tool support."),
  },
  { id: "lmstudio", label: msg("LM Studio (local)"), baseUrl: "http://localhost:1234/v1", keyless: true, note: msg("Enable CORS in LM Studio's server settings.") },
  { id: "custom", label: msg("Custom (OpenAI-compatible)"), baseUrl: "", keyHint: msg("optional"), keyless: true },
];

/** The built-in providers: Anthropic through its SDK, and every OpenAI-compatible endpoint. */
export const aiProviders = defineExtension({
  id: "docxtor.ai-providers",
  name: msg("AI providers"),
  description: msg("Anthropic, OpenAI, OpenRouter, Gemini, Groq, Mistral, Ollama, LM Studio and any OpenAI-compatible endpoint."),
  activate(ctx) {
    ctx.ai.providers.register({
      id: "anthropic",
      label: t("Anthropic (Claude)"),
      models: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-2"],
      keyHint: "sk-ant-…  (console.anthropic.com)",
      options: [{ id: "effort", label: t("Effort"), choices: EFFORTS, default: "high", help: t("Higher effort thinks harder on complex edits; lower is faster and cheaper.") }],
      configured: (s) => !!s.apiKey && !!s.model,
      connect: (s, system, tools) => new AnthropicConversation(s, system, tools),
    });
    for (const c of COMPATIBLE) {
      ctx.ai.providers.register({
        id: c.id,
        label: t(c.label),
        models: [],
        keyHint: c.keyless && !c.keyHint ? undefined : t(c.keyHint ?? ""),
        baseUrl: c.baseUrl,
        note: c.note && t(c.note),
        configured: (s) => !!s.model && (c.id === "custom" ? !!s.baseUrl : c.keyless || !!s.apiKey),
        connect: (s, system, tools) => new OpenAIConversation(s, c, system, tools),
      } satisfies AIProvider);
    }
  },
});

/** Settings saved by earlier versions ({ preset, effort, … }) in today's shape. */
export function migrateSettings(raw: Record<string, unknown>): AISettings {
  if (typeof raw.provider === "string") return { ...DEFAULT_SETTINGS, ...(raw as Partial<AISettings>), options: { ...(raw.options as object) } };
  return {
    provider: String(raw.preset ?? DEFAULT_SETTINGS.provider),
    apiKey: String(raw.apiKey ?? ""),
    baseUrl: String(raw.baseUrl ?? ""),
    model: String(raw.model ?? ""),
    options: raw.effort ? { effort: raw.effort } : {},
  };
}

// ---------------------------------------------------------------------------
// Anthropic (official SDK, streaming manual tool loop)
// ---------------------------------------------------------------------------

/** Models that accept the server-side refusal fallback ("default" routing). */
const FALLBACK_MODELS = new Set(["claude-opus-5-5", "claude-sonnet-5-5", "claude-fable-5-2", "claude-fable-5-1"]);

class AnthropicConversation implements Conversation {
  private client: Promise<{ client: Anthropic; APIError: typeof Anthropic.APIError }> | undefined;
  private messages: Anthropic.Beta.Messages.BetaMessageParam[] = [];
  private tools: Anthropic.Beta.Messages.BetaTool[];

  constructor(private s: AISettings, private system: string, tools: ToolDef[]) {
    this.tools = tools.map((t) => ({
      name: t.name,
      description: t.description,
      input_schema: t.input_schema as Anthropic.Beta.Messages.BetaTool.InputSchema,
      eager_input_streaming: true,
    }));
  }

  private sdk() {
    // Bring-your-own-key: the key lives only in this browser and goes straight to Anthropic.
    // An optional base URL supports API gateways and proxies. The SDK sends the key as
    // `Authorization: Bearer`; it goes in `X-Api-Key` as before, which the API and every gateway
    // that worked with earlier versions of Docxtor read.
    if (this.client) return this.client;
    this.client = import("@anthropic-ai/sdk").then(({ default: SDK }) => ({
      client: new SDK({
        apiKey: this.s.apiKey,
        defaultHeaders: { "X-Api-Key": this.s.apiKey, Authorization: null },
        dangerouslyAllowBrowser: true,
        ...(this.s.baseUrl ? { baseURL: this.s.baseUrl } : {}),
      }),
      APIError: SDK.APIError,
    }));
    // A load that failed (offline, say) is tried again on the next step.
    this.client.catch(() => (this.client = undefined));
    return this.client;
  }

  addUser(text: string) {
    this.messages.push({ role: "user", content: [{ type: "text", text }] });
  }

  addToolResults(results: { id: string; content: string; isError?: boolean }[]) {
    this.messages.push({
      role: "user",
      content: results.map((r) => ({ type: "tool_result" as const, tool_use_id: r.id, content: r.content, ...(r.isError ? { is_error: true } : {}) })),
    });
  }

  async step(onText: (d: string) => void, signal: AbortSignal): Promise<StepResult> {
    const fallback = FALLBACK_MODELS.has(this.s.model);
    const { client, APIError } = await this.sdk();
    let message: Anthropic.Beta.Messages.BetaMessage | undefined;
    for (let attempt = 0; ; attempt++) {
      const stream = client.beta.messages.stream(
        {
          model: this.s.model,
          max_tokens: 64000,
          // Stable prefix (tools + guide) is cached; document context travels in user turns.
          system: [{ type: "text", text: this.system, cache_control: { type: "ephemeral" } }],
          tools: this.tools,
          messages: this.messages,
          output_config: { effort: EFFORTS.includes(this.s.options.effort as Effort) ? (this.s.options.effort as Effort) : "high" },
          ...(fallback ? { betas: ["server-side-fallback-2026-07-01"], fallbacks: "default" as const } : {}),
        },
        { signal },
      );
      stream.on("text", (d) => onText(d));
      try {
        message = await stream.finalMessage();
        break;
      } catch (err) {
        // Only a tool input that could not be parsed is retried; API errors surface to the user.
        if (err instanceof APIError || signal.aborted || attempt >= 2) throw err;
      }
    }
    const msg = message!;
    if (msg.stop_reason === "refusal") {
      this.messages.push({ role: "assistant", content: msg.content as Anthropic.Beta.Messages.BetaContentBlockParam[] });
      return { toolCalls: [], done: true, note: t("The model declined this request.") };
    }
    this.messages.push({ role: "assistant", content: msg.content as Anthropic.Beta.Messages.BetaContentBlockParam[] });
    if (msg.stop_reason === "pause_turn") return { toolCalls: [], done: false };
    const uses = msg.content.filter((b): b is Anthropic.Beta.Messages.BetaToolUseBlock => b.type === "tool_use");
    if (!uses.length) return { toolCalls: [], done: true };
    const truncated = msg.stop_reason === "max_tokens";
    return {
      done: false,
      toolCalls: uses.map((u) => {
        const input = u.input as Record<string, unknown>;
        const valid = input && typeof input === "object" && !Array.isArray(input);
        return {
          id: u.id,
          name: u.name,
          input: valid ? input : {},
          invalid: truncated ? "Tool input was cut off (max_tokens). Retry with a smaller edit." : valid ? undefined : "INVALID_JSON: tool input was not an object",
        };
      }),
    };
  }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible chat completions (OpenAI, OpenRouter, Gemini, Groq, Mistral, Ollama, ...)
// ---------------------------------------------------------------------------

interface OAIMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  tool_calls?: { id: string; type: "function"; function: { name: string; arguments: string } }[];
  tool_call_id?: string;
}

class OpenAIConversation implements Conversation {
  private messages: OAIMessage[];
  private tools: unknown[];

  constructor(
    private s: AISettings,
    private preset: Compatible,
    system: string,
    tools: ToolDef[],
  ) {
    this.messages = [{ role: "system", content: system }];
    this.tools = tools.map((t) => ({ type: "function", function: { name: t.name, description: t.description, parameters: t.input_schema } }));
  }

  addUser(text: string) {
    this.messages.push({ role: "user", content: text });
  }

  addToolResults(results: { id: string; content: string; isError?: boolean }[]) {
    for (const r of results) this.messages.push({ role: "tool", tool_call_id: r.id, content: r.isError ? `ERROR: ${r.content}` : r.content });
  }

  async step(onText: (d: string) => void, signal: AbortSignal): Promise<StepResult> {
    const base = (this.s.baseUrl || this.preset.baseUrl).replace(/\/+$/, "");
    if (!base) throw new Error(t("Set a base URL for this provider in AI settings."));
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (this.s.apiKey) headers.Authorization = `Bearer ${this.s.apiKey}`;
    if (this.preset.id === "openrouter") {
      headers["HTTP-Referer"] = location.origin;
      headers["X-Title"] = "Docxtor";
    }
    const res = await fetch(`${base}/chat/completions`, {
      method: "POST",
      headers,
      signal,
      body: JSON.stringify({ model: this.s.model, messages: this.messages, tools: this.tools, stream: true }),
    });
    if (!res.ok || !res.body) {
      const body = await res.text().catch(() => "");
      throw new Error(`${res.status} ${res.statusText}: ${body.slice(0, 400)}`);
    }
    let text = "";
    const calls: { id: string; name: string; args: string }[] = [];
    let finish = "";
    const reader = res.body.getReader();
    const dec = new TextDecoder();
    let buf = "";
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        if (!line.startsWith("data:")) continue;
        const data = line.slice(5).trim();
        if (data === "[DONE]") continue;
        let j: { choices?: { delta?: { content?: string; tool_calls?: { index?: number; id?: string; function?: { name?: string; arguments?: string } }[] }; finish_reason?: string }[] };
        try {
          j = JSON.parse(data);
        } catch {
          continue;
        }
        const ch = j.choices?.[0];
        if (!ch) continue;
        if (ch.delta?.content) {
          text += ch.delta.content;
          onText(ch.delta.content);
        }
        for (const tc of ch.delta?.tool_calls ?? []) {
          const i = tc.index ?? calls.length;
          calls[i] ??= { id: tc.id ?? `call_${i}_${Date.now()}`, name: "", args: "" };
          if (tc.id) calls[i].id = tc.id;
          if (tc.function?.name) calls[i].name += tc.function.name;
          if (tc.function?.arguments) calls[i].args += tc.function.arguments;
        }
        if (ch.finish_reason) finish = ch.finish_reason;
      }
    }
    const valid = calls.filter(Boolean);
    this.messages.push({
      role: "assistant",
      content: text || null,
      ...(valid.length ? { tool_calls: valid.map((c) => ({ id: c.id, type: "function" as const, function: { name: c.name, arguments: c.args || "{}" } })) } : {}),
    });
    if (!valid.length) return { toolCalls: [], done: true, note: finish === "length" ? t("The response was cut off at the model's length limit.") : undefined };
    return {
      done: false,
      toolCalls: valid.map((c) => {
        try {
          const input = JSON.parse(c.args || "{}");
          return { id: c.id, name: c.name, input };
        } catch {
          return { id: c.id, name: c.name, input: {}, invalid: `INVALID_JSON: ${c.args.slice(0, 200)}` };
        }
      }),
    };
  }
}
