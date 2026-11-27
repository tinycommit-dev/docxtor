// A proxy for Docxtor's Custom AI provider. The provider POSTs to <base URL>/chat/completions, so set
// its base URL to http://localhost:8788/v1. The key stays here; the page never sees it.
//   OPENAI_API_KEY=sk-... node node-proxy.mjs
import { createServer } from "node:http";
import { pipeline, Readable } from "node:stream";
import { pathToFileURL } from "node:url";

export function createProxy({ key, upstream = "https://api.openai.com/v1/chat/completions", origin = "*", maxBytes = 2_000_000, model }) {
  const cors = { "access-control-allow-origin": origin, "access-control-allow-headers": "content-type, authorization", "access-control-allow-methods": "POST, OPTIONS", vary: "origin" };
  const reply = (res, status, text) => res.writeHead(status, { ...cors, "content-type": "text/plain" }).end(text);
  return createServer(async (req, res) => {
    if (new URL(req.url, "http://x").pathname !== "/v1/chat/completions") return reply(res, 404, "Not found");
    if (req.method === "OPTIONS") return reply(res, 204, ""); // the browser's check before a cross-origin POST
    if (req.method !== "POST") return reply(res, 405, "POST only");
    // Your sign-in goes here: read a session cookie or token from req.headers and refuse (401) when it is
    // missing. Your rate limit goes here too: count requests per user and refuse (429) past your limit.
    const tooLarge = () => (reply(res, 413, "Too large"), req.resume()); // answer at once, throw the rest of the upload away
    if (Number(req.headers["content-length"] ?? 0) > maxBytes) return tooLarge();
    const parts = [];
    let size = 0;
    for await (const part of req) {
      size += part.length;
      if (size > maxBytes) return tooLarge();
      parts.push(part);
    }
    let body = Buffer.concat(parts);
    if (model) {
      try { body = JSON.stringify({ ...JSON.parse(body), model }); } catch { return reply(res, 400, "Not JSON"); }
    }
    const stop = new AbortController();
    res.on("close", () => stop.abort()); // the page went away: stop paying for the answer
    try {
      // Only the key from the environment is sent. Whatever Authorization header the page sent is dropped.
      const up = await fetch(upstream, { method: "POST", headers: { "content-type": "application/json", authorization: `Bearer ${key}` }, body, signal: stop.signal });
      res.writeHead(up.status, { ...cors, "content-type": up.headers.get("content-type") ?? "text/event-stream", "cache-control": "no-store" });
      pipeline(Readable.fromWeb(up.body), res, () => {}); // pass the stream through as it arrives; a page that left is no error
    } catch {
      if (!res.headersSent) reply(res, 502, "The model did not answer");
    }
  });
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const env = process.env;
  if (!env.OPENAI_API_KEY) throw new Error("Set OPENAI_API_KEY");
  createProxy({ key: env.OPENAI_API_KEY, upstream: env.UPSTREAM_URL, origin: env.ALLOWED_ORIGIN, maxBytes: Number(env.MAX_BYTES ?? 2_000_000), model: env.MODEL }).listen(Number(env.PORT ?? 8788));
}
