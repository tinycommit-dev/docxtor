// The document agent loop: the model reads and edits the live document through
// Docxtor's tools (the same ones the MCP server exposes) plus any tools extensions add,
// and every edit is applied to the editor as it happens.
import { systemGuide, toolDefinitions } from "@docxtor/core";
import { t, type AISettings, type Conversation, type Editor, type ExtensionHost, type ToolDefinition } from "@docxtor/editor";

export interface AgentEvents {
  onText(delta: string): void;
  onTool(name: string, input: Record<string, unknown>, result: string, ok: boolean): void;
  onNote(note: string): void;
}

const MAX_STEPS = 40;

export function systemPrompt(): string {
  return (
    systemGuide() +
    "\n\nYou are the assistant inside Docxtor, an open-source document editor. The user sees your edits applied live and can revert them. " +
    "Answer questions about the document directly when no edit is needed. Keep replies concise; use Markdown sparingly."
  );
}

export class Agent {
  private conv: Conversation | null = null;
  private key = "";

  constructor(
    private editor: Editor,
    private host: ExtensionHost,
  ) {}

  reset() {
    this.conv = null;
  }

  /** Docxtor's document tools, then extension tools (whose names don't clash with Docxtor's). */
  private tools(): ToolDefinition[] {
    const own = toolDefinitions();
    const names = new Set(own.map((t) => t.name));
    const extra = this.host.aiTools.list().filter((t) => !names.has(t.name));
    return [...own, ...extra.map(({ name, description, input_schema }) => ({ name, description, input_schema }))];
  }

  private conversation(s: AISettings): Conversation {
    const tools = this.tools();
    const key = JSON.stringify([s, tools.map((t) => t.name)]);
    if (!this.conv || key !== this.key) {
      const provider = this.host.aiProviders.list().find((p) => p.id === s.provider);
      if (!provider) throw new Error(t("The “{provider}” provider isn't available. Turn its extension on, or pick another provider in AI settings.", { provider: s.provider }));
      this.conv = provider.connect(s, systemPrompt(), tools);
      this.key = key;
    }
    return this.conv;
  }

  private context(): string {
    const doc = this.editor.engine.raw;
    const name = this.editor.engine.fileName;
    let ctx = `<document name="${name}">\n${doc.summary()}\n`;
    const script = doc.docscript(100000);
    if (script.length < 60000) ctx += `\nCurrent content (DocScript):\n${script}`;
    else ctx += `\nOutline:\n${doc.outline()}\nThe document is long: use read_document with \`start\` to read sections, or search.\n${doc.docscript(120)}`;
    ctx += "</document>";
    const r = this.editor.range();
    if (r && !this.editor.collapsed()) {
      ctx += `\n<selection>\n${doc.selectionContext(r[0].p, r[0].o, r[1].p, r[1].o)}</selection>`;
    } else if (r) {
      ctx += `\n<cursor paragraph="p${r[0].p}" offset="${r[0].o}"/>`;
    }
    return ctx;
  }

  private async call(name: string, input: Record<string, unknown>): Promise<string> {
    const ext = toolDefinitions().some((t) => t.name === name) ? null : this.host.aiTools.list().find((t) => t.name === name);
    if (ext) return String(await ext.run(input));
    return this.editor.runTool(name, input);
  }

  /** Run one user request to completion. */
  async run(settings: AISettings, request: string, ev: AgentEvents, signal: AbortSignal) {
    const conv = this.conversation(settings);
    conv.addUser(`${this.context()}\n\n${request}`);
    for (let step = 0; step < MAX_STEPS; step++) {
      if (signal.aborted) return;
      const r = await conv.step(ev.onText, signal);
      if (r.note) ev.onNote(r.note);
      if (!r.toolCalls.length) {
        if (r.done) return;
        continue; // paused turn: continue generating
      }
      const results: { id: string; content: string; isError?: boolean }[] = [];
      for (const call of r.toolCalls) {
        if (call.invalid) {
          results.push({ id: call.id, content: call.invalid, isError: true });
          ev.onTool(call.name, call.input, call.invalid, false);
          continue;
        }
        try {
          const out = await this.call(call.name, call.input);
          results.push({ id: call.id, content: out });
          ev.onTool(call.name, call.input, out, true);
        } catch (e) {
          const msg = String((e as Error).message ?? e);
          results.push({ id: call.id, content: msg, isError: true });
          ev.onTool(call.name, call.input, msg, false);
          // The engine refuses every edit of a read-only document: asking the model to try again would only repeat it.
          if (msg.startsWith("This document is read-only")) {
            ev.onNote(t("The document became read-only, so the assistant stopped."));
            this.editor.refresh({ keepScroll: true });
            return;
          }
        }
      }
      // Show the edits immediately.
      this.editor.refresh({ keepScroll: true });
      conv.addToolResults(results);
    }
    ev.onNote(t("Stopped after {n} steps.", { n: MAX_STEPS }));
  }
}
