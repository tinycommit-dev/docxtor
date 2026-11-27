// Tests for both example proxies with no network: the Worker's fetch is called with a Request and a stubbed
// upstream fetch; the Node proxy runs on a free port with its upstream pointed at a local stub server.
// Run: node --test examples/ai-proxy
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { test } from "node:test";
import { createProxy } from "./node-proxy.mjs";
import worker from "./worker.mjs";

const KEY = "server-secret-key";
const PATH = "/v1/chat/completions";
const chat = { model: "page-model", messages: [{ role: "user", content: "hi" }], stream: true };
const post = (body, headers = {}) => ({ method: "POST", headers: { "content-type": "application/json", ...headers }, body: typeof body === "string" ? body : JSON.stringify(body) });

/** A gate the upstream waits on between its two streamed chunks, so a test can see the first one arrive alone. */
const gate = () => {
  let open;
  const opened = new Promise((r) => (open = r));
  return { open, opened };
};

// Each harness gives: request(path, init), seen (what the upstream received), release() (lets the stream finish), close().
async function nodeHarness(options = {}) {
  const seen = [];
  const g = gate();
  const upstream = createServer(async (req, res) => {
    const chunks = [];
    for await (const c of req) chunks.push(c);
    seen.push({ headers: req.headers, body: Buffer.concat(chunks).toString() });
    res.writeHead(200, { "content-type": "text/event-stream" });
    res.write("data: one\n\n");
    await g.opened;
    res.end("data: two\n\ndata: [DONE]\n\n");
  }).listen(0);
  await new Promise((r) => upstream.once("listening", r));
  const proxy = createProxy({ key: KEY, upstream: `http://127.0.0.1:${upstream.address().port}/v1/chat/completions`, ...options }).listen(0);
  await new Promise((r) => proxy.once("listening", r));
  const base = `http://127.0.0.1:${proxy.address().port}`;
  return {
    request: (path, init) => fetch(base + path, init),
    seen,
    release: g.open,
    close: () => {
      g.open();
      proxy.closeAllConnections();
      upstream.closeAllConnections();
      proxy.close();
      upstream.close();
    },
  };
}

async function workerHarness(env = {}) {
  const seen = [];
  const g = gate();
  const realFetch = globalThis.fetch;
  globalThis.fetch = async (url, init) => {
    seen.push({ url: String(url), headers: Object.fromEntries(new Headers(init.headers)), body: String(init.body) });
    const enc = new TextEncoder();
    return new Response(
      new ReadableStream({
        async start(c) {
          c.enqueue(enc.encode("data: one\n\n"));
          await g.opened;
          c.enqueue(enc.encode("data: two\n\ndata: [DONE]\n\n"));
          c.close();
        },
      }),
      { status: 200, headers: { "content-type": "text/event-stream" } },
    );
  };
  return {
    request: (path, init) => worker.fetch(new Request(`https://proxy.test${path}`, init), { OPENAI_API_KEY: KEY, ...env }),
    seen,
    release: g.open,
    close: () => {
      g.open();
      globalThis.fetch = realFetch;
    },
  };
}

const kinds = {
  "Node proxy": { make: nodeHarness, limit: (n) => ({ maxBytes: n }), model: (m) => ({ model: m }) },
  "Cloudflare Worker": { make: workerHarness, limit: (n) => ({ MAX_BYTES: String(n) }), model: (m) => ({ MODEL: m }) },
};

for (const [name, k] of Object.entries(kinds)) {
  test(`${name}: the key reaches the upstream and never the page`, async () => {
    const h = await k.make();
    try {
      const res = await h.request(PATH, post(chat, { authorization: "Bearer page-supplied-key" }));
      h.release();
      const text = await res.text();
      assert.equal(res.status, 200);
      assert.match(text, /data: two/);
      assert.equal(h.seen.length, 1);
      assert.equal(h.seen[0].headers.authorization, `Bearer ${KEY}`);
      assert.ok(!JSON.stringify(h.seen[0]).includes("page-supplied-key"), "the page's own Authorization header is not forwarded");
      assert.deepEqual(JSON.parse(h.seen[0].body), chat);
      assert.ok(!text.includes(KEY) && ![...res.headers].join().includes(KEY), "nothing sent back holds the key");
    } finally {
      h.close();
    }
  });

  test(`${name}: any other path or method is refused and nothing is forwarded`, async () => {
    const h = await k.make();
    try {
      for (const path of ["/", "/v1/models", "/v1/embeddings", `${PATH}/extra`, "/v1/chat/completionsx", "//v1/chat/completions/..%2f"]) {
        const res = await h.request(path, post(chat));
        assert.equal(res.status, 404, path);
        await res.text();
      }
      for (const method of ["GET", "PUT", "DELETE", "PATCH"]) {
        const res = await h.request(PATH, { method, ...(method === "GET" ? {} : { body: "{}" }) });
        assert.equal(res.status, 405, method);
        await res.text();
      }
      assert.equal(h.seen.length, 0);
    } finally {
      h.close();
    }
  });

  test(`${name}: a body over the limit is refused with 413`, async () => {
    const h = await k.make(k.limit(1000));
    try {
      const big = await h.request(PATH, post({ ...chat, messages: [{ role: "user", content: "x".repeat(5000) }] }));
      assert.equal(big.status, 413);
      await big.text();
      assert.equal(h.seen.length, 0);
      const small = await h.request(PATH, post(chat));
      h.release();
      assert.equal(small.status, 200);
      await small.text();
    } finally {
      h.close();
    }
  });

  test(`${name}: a multi-megabyte body is refused with 413 (the answer arrives, the connection is not reset)`, async () => {
    const h = await k.make(k.limit(1000));
    try {
      const res = await h.request(PATH, post({ ...chat, messages: [{ role: "user", content: "x".repeat(6_000_000) }] }));
      assert.equal(res.status, 413);
      await res.text();
      assert.equal(h.seen.length, 0);
      const next = await h.request(PATH, post(chat)); // and the proxy is still serving
      h.release();
      assert.equal(next.status, 200);
      await next.text();
    } finally {
      h.close();
    }
  });

  test(`${name}: a page that leaves in the middle of an answer does not stop the proxy`, async () => {
    const h = await k.make();
    try {
      const res = await h.request(PATH, post(chat));
      const reader = res.body.getReader();
      await reader.read(); // the first chunk arrived
      await reader.cancel(); // the page went away
      h.release();
      await new Promise((done) => setTimeout(done, 100));
      const next = await h.request(PATH, post(chat)); // the proxy still answers
      h.release();
      assert.equal(next.status, 200);
      assert.match(await next.text(), /data: two/);
    } finally {
      h.close();
    }
  });

  test(`${name}: a streamed answer streams (the first chunk arrives before the upstream is done)`, async () => {
    const h = await k.make();
    try {
      const res = await h.request(PATH, post(chat));
      assert.equal(res.headers.get("content-type"), "text/event-stream");
      const reader = res.body.getReader();
      const first = await reader.read(); // the upstream is still holding its second chunk back
      assert.match(new TextDecoder().decode(first.value), /data: one/);
      h.release();
      let rest = "";
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        rest += new TextDecoder().decode(value);
      }
      assert.match(rest, /\[DONE\]/);
    } finally {
      h.close();
    }
  });

  test(`${name}: the browser's check before a cross-origin POST is answered, and a product can pin the model`, async () => {
    const h = await k.make(k.model("the-products-model"));
    try {
      const pre = await h.request(PATH, { method: "OPTIONS" });
      assert.equal(pre.status, 204);
      assert.ok(pre.headers.get("access-control-allow-origin"));
      assert.equal(h.seen.length, 0);
      const res = await h.request(PATH, post(chat));
      h.release();
      await res.text();
      assert.equal(JSON.parse(h.seen[0].body).model, "the-products-model");
      const bad = await h.request(PATH, post("not json"));
      assert.equal(bad.status, 400);
      await bad.text();
    } finally {
      h.close();
    }
  });
}
