# Giving your users the AI

How to offer the assistant in your own product without putting a model key in the page, and how to test it without a key. Part of the [AI pages](index.md).

## Giving your users the AI without a key in the page

Anything in a web page can be read by the person who opens it, so a product that pays for the AI for its users must not put its own key in the page or in the settings it hands out. Keep the key on your server and point the Custom provider at an endpoint of yours. The page never sees the key.

<!-- text -->
```
the page (Custom provider)  -->  your endpoint (holds the key)  -->  the model provider
   no key, or none that matters       adds the key, forwards          streams the answer back
```

What the Custom provider sends (`OpenAIConversation` in `web/src/ai/providers.ts`): a `POST` to `<base URL>/chat/completions` with a JSON body `{ "model", "messages", "tools", "stream": true }` and `Content-Type: application/json`. It adds `Authorization: Bearer <key>` only when the person typed a key, so leave the key field empty. It reads the answer as a stream of server-sent events in the OpenAI chat completions format (`data: {...}` lines ending with `data: [DONE]`), and shows a non-2xx answer as its status and the first 400 characters of the body. The person (or your setup guide) sets Provider to Custom, the Base URL to your endpoint up to and including `/v1`, and the Model to a name your endpoint accepts. The app's Content-Security-Policy allows `https:` and `localhost` connections ([Putting Docxtor online](../deploy.md)), so an `https` endpoint works.

Two example proxies in [`examples/ai-proxy/`](../../examples/ai-proxy), each under 60 lines and written to the same rules:

- [`node-proxy.mjs`](../../examples/ai-proxy/node-proxy.mjs): plain `node:http` with no dependencies. `OPENAI_API_KEY=sk-... node examples/ai-proxy/node-proxy.mjs` listens on port 8788 (`PORT` changes it); the Base URL is `http://localhost:8788/v1`.
- [`worker.mjs`](../../examples/ai-proxy/worker.mjs): a Cloudflare Worker (a module with `export default { fetch }`). The key is a secret named `OPENAI_API_KEY` on the Worker; the Base URL is the Worker's address followed by `/v1`.

Both do the same things:

- They answer `POST /v1/chat/completions` and nothing else. Any other path is a 404 and any other method a 405. `OPTIONS` on that path is answered with the CORS headers a browser asks for before a cross-origin `POST`; the answer allows the origin in `ALLOWED_ORIGIN` (any origin if unset: set it to your app's address).
- They add `Authorization: Bearer <the key from the environment>` and drop whatever `Authorization` header the page sent, so the key reaches the model provider and never the page.
- They refuse a request body over `maxBytes` (2,000,000 bytes by default; `MAX_BYTES` sets it) with a 413 before anything is forwarded.
- They pass the answer through as it arrives, so the assistant shows text as the model writes it. If the page goes away, the Node proxy stops the request upstream.
- `UPSTREAM_URL` points them at another OpenAI-compatible provider (the default is `https://api.openai.com/v1/chat/completions`), and `MODEL` pins the model, so the page cannot ask for a dearer one.
- A comment in each marks where your own sign-in and your own rate limit go. They are not in the examples, and you need them: without them anyone who can reach the endpoint spends your money.

Both are tested without a network by `examples/ai-proxy/proxy.test.mjs` (run by `npm test`): the Worker's `fetch` is called with a `Request` and a stand-in for the upstream, and the Node proxy runs on a free port in front of a local stand-in server. The tests check that the key reaches the upstream and never the client, that another path or method is refused and nothing is forwarded, that a body over the limit gets a 413, and that the first chunk of a stream arrives while the upstream is still holding the rest. Deploying the Worker (`wrangler deploy`, `wrangler secret put OPENAI_API_KEY`) needs your Cloudflare account.

*Sketch, not run by a test: starts a server with a real key; the proxies are run by examples/ai-proxy/proxy.test.mjs.*
```bash
OPENAI_API_KEY=sk-... node examples/ai-proxy/node-proxy.mjs
```

## Testing without a key

<!-- example: ai-mock -->
```bash
node tools/mock-ai.mjs
```

Then set the provider to *Custom* with base URL `http://localhost:8787/v1` (or Anthropic with base URL `http://localhost:8787`). The mock scripts four steps over both APIs: a search, a replace, an insert, then an answer. `PORT=9000 node tools/mock-ai.mjs` listens on another port.
