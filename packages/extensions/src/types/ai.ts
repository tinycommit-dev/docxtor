// AI providers and tools
import type { ToolDefinition } from "@docxtor/core";

export interface ToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
  /** Set when the call cannot be run (e.g. malformed arguments); sent back to the model as an error. */
  invalid?: string;
}

export interface StepResult {
  toolCalls: ToolCall[];
  /** The model finished its turn. */
  done: boolean;
  /** Something to tell the user ("The model declined this request."). */
  note?: string;
}

/** One chat with a model, driven by the host's agent loop. */
export interface Conversation {
  addUser(text: string): void;
  addToolResults(results: { id: string; content: string; isError?: boolean }[]): void;
  step(onText: (delta: string) => void, signal: AbortSignal): Promise<StepResult>;
}

export interface AISettings {
  provider: string;
  apiKey: string;
  baseUrl: string;
  model: string;
  /** Provider-specific extras (e.g. Anthropic's effort). */
  options: Record<string, unknown>;
}

export interface AIProvider {
  id: string;
  label: string;
  /** Model ids to offer; empty means the user types one. */
  models: string[];
  /** Placeholder for the key field, e.g. `sk-…`. Omit when no key is needed. */
  keyHint?: string;
  /** Default endpoint; when set, the settings show a base URL field. */
  baseUrl?: string;
  /** Extra help shown under the settings. */
  note?: string;
  /** Provider-specific choices shown in the settings, saved in `AISettings.options`. */
  options?: AIProviderOption[];
  /** Whether the settings are complete enough to connect. */
  configured(s: AISettings): boolean;
  connect(s: AISettings, system: string, tools: ToolDefinition[]): Conversation;
}

export interface AIProviderOption {
  id: string;
  label: string;
  choices: string[];
  default: string;
  help?: string;
}

export interface AITool extends ToolDefinition {
  /** Run the tool. Edit through `ctx.document`, so changes show live and can be reviewed and undone. */
  run(input: Record<string, unknown>): string | Promise<string>;
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

