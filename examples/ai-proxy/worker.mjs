// A Cloudflare Worker for Docxtor's Custom AI provider. The provider POSTs to <base URL>/chat/completions,
// so set its base URL to https://your-worker.example/v1. The key is a Worker secret (OPENAI_API_KEY);
// the page never sees it.
export default {
  async fetch(req, env) {
    const cors = { "access-control-allow-origin": env.ALLOWED_ORIGIN ?? "*", "access-control-allow-headers": "content-type, authorization", "access-control-allow-methods": "POST, OPTIONS", vary: "origin" };
    const reply = (status, text) => new Response(text || null, { status, headers: { ...cors, "content-type": "text/plain" } });
    if (new URL(req.url).pathname !== "/v1/chat/completions") return reply(404, "Not found");
    if (req.method === "OPTIONS") return reply(204, ""); // the browser's check before a cross-origin POST
    if (req.method !== "POST") return reply(405, "POST only");
    // Your sign-in goes here: read a session cookie or token from req.headers and refuse (401) when it is
    // missing. Your rate limit goes here too (a Durable Object or the rate limiting binding), refusing with 429.
    const max = Number(env.MAX_BYTES ?? 2_000_000);
    if (Number(req.headers.get("content-length") ?? 0) > max) return reply(413, "Too large");
    const parts = [];
    let size = 0;
    for await (const part of req.body ?? []) {
      size += part.length;
      if (size > max) return reply(413, "Too large");
      parts.push(part);
    }
    let body = await new Blob(parts).text();
    if (env.MODEL) {
      try { body = JSON.stringify({ ...JSON.parse(body), model: env.MODEL }); } catch { return reply(400, "Not JSON"); }
    }
    // Only the secret is sent. Whatever Authorization header the page sent is dropped.
    const up = await fetch(env.UPSTREAM_URL ?? "https://api.openai.com/v1/chat/completions", {
      method: "POST",
      headers: { "content-type": "application/json", authorization: `Bearer ${env.OPENAI_API_KEY}` },
      body,
      signal: req.signal,
    });
    // Pass the stream through as it arrives.
    return new Response(up.body, { status: up.status, headers: { ...cors, "content-type": up.headers.get("content-type") ?? "text/event-stream", "cache-control": "no-store" } });
  },
};
