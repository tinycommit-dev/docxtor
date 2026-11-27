#!/usr/bin/env node
// A scripted mock of the Anthropic Messages API and OpenAI-compatible chat
// completions (both streaming), for exercising Docxtor's agent loop without an API key.
//
//   node tools/mock-ai.mjs            # listens on http://localhost:8787
//
// Point Docxtor's AI settings at it: provider "Custom (OpenAI-compatible)" with base URL
// http://localhost:8787/v1, or set the Anthropic base URL to http://localhost:8787.
import { createServer } from "node:http";

const PORT = Number(process.env.PORT ?? 8787);

/** The scripted agent: decides the next step from how many tool results it has seen. */
function plan(toolResults) {
  switch (toolResults) {
    case 0:
      return { text: "Let me find the churn figure first.", tool: { name: "search", input: { query: "Churn" } } };
    case 1:
      return { text: "", tool: { name: "replace_text", input: { find: "Churn down to 2.1%", replace: "Churn fell to 1.9%" } } };
    case 2:
      return { text: "", tool: { name: "insert_content", input: { markdown: "## Next steps\n\n- Ship **v1** of the editor\n- Publish the fidelity dashboard" } } };
    default:
      return { text: "Done — I updated the churn figure and added a **Next steps** section at the end." };
  }
}

const cors = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "*",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

function sse(res) {
  res.writeHead(200, { ...cors, "Content-Type": "text/event-stream", "Cache-Control": "no-cache" });
  return (event, data) => res.write(`${event ? `event: ${event}\n` : ""}data: ${typeof data === "string" ? data : JSON.stringify(data)}\n\n`);
}

const chunks = (s, n = 12) => s.match(new RegExp(`[\\s\\S]{1,${n}}`, "g")) ?? [];

function anthropic(body, res) {
  const results = body.messages.flatMap((m) => (Array.isArray(m.content) ? m.content : [])).filter((c) => c.type === "tool_result").length;
  const step = plan(results);
  const send = sse(res);
  send("message_start", { type: "message_start", message: { id: `msg_${Date.now()}`, type: "message", role: "assistant", model: body.model, content: [], stop_reason: null, stop_sequence: null, usage: { input_tokens: 100, output_tokens: 0 } } });
  let index = 0;
  if (step.text) {
    send("content_block_start", { type: "content_block_start", index, content_block: { type: "text", text: "" } });
    for (const c of chunks(step.text)) send("content_block_delta", { type: "content_block_delta", index, delta: { type: "text_delta", text: c } });
    send("content_block_stop", { type: "content_block_stop", index });
    index++;
  }
  if (step.tool) {
    send("content_block_start", { type: "content_block_start", index, content_block: { type: "tool_use", id: `toolu_${results}_${Date.now()}`, name: step.tool.name, input: {} } });
    for (const c of chunks(JSON.stringify(step.tool.input), 9)) send("content_block_delta", { type: "content_block_delta", index, delta: { type: "input_json_delta", partial_json: c } });
    send("content_block_stop", { type: "content_block_stop", index });
  }
  send("message_delta", { type: "message_delta", delta: { stop_reason: step.tool ? "tool_use" : "end_turn", stop_sequence: null }, usage: { output_tokens: 42 } });
  send("message_stop", { type: "message_stop" });
  res.end();
}

function openai(body, res) {
  const results = body.messages.filter((m) => m.role === "tool").length;
  const step = plan(results);
  const send = sse(res);
  const base = { id: `chatcmpl_${Date.now()}`, object: "chat.completion.chunk", model: body.model };
  for (const c of chunks(step.text)) send("", { ...base, choices: [{ index: 0, delta: { content: c } }] });
  if (step.tool) {
    const args = JSON.stringify(step.tool.input);
    send("", { ...base, choices: [{ index: 0, delta: { tool_calls: [{ index: 0, id: `call_${results}`, type: "function", function: { name: step.tool.name, arguments: "" } }] } }] });
    for (const c of chunks(args, 9)) send("", { ...base, choices: [{ index: 0, delta: { tool_calls: [{ index: 0, function: { arguments: c } }] } }] });
  }
  send("", { ...base, choices: [{ index: 0, delta: {}, finish_reason: step.tool ? "tool_calls" : "stop" }] });
  send("", "[DONE]");
  res.end();
}

createServer((req, res) => {
  if (req.method === "OPTIONS") {
    res.writeHead(204, cors);
    return res.end();
  }
  let raw = "";
  req.on("data", (c) => (raw += c));
  req.on("end", () => {
    let body;
    try {
      body = JSON.parse(raw || "{}");
    } catch {
      res.writeHead(400, cors);
      return res.end("bad json");
    }
    const tools = (body.tools ?? []).map((t) => t.name ?? t.function?.name);
    console.log(`${req.url} model=${body.model} messages=${body.messages?.length} tools=${tools.length}`);
    const path = new URL(req.url, "http://x").pathname;
    if (path.endsWith("/v1/messages")) return anthropic(body, res);
    if (path.endsWith("/chat/completions")) return openai(body, res);
    res.writeHead(404, cors);
    res.end("not found");
  });
}).listen(PORT, () => console.log(`mock AI listening on http://localhost:${PORT}`));
