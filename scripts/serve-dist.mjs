// Serves the production build (web/dist) the way Cloudflare Pages does: with the headers of deploy/_headers (every matching rule adds its
// headers, same names joined, `! Name` detaches), `/` and a folder as its index.html, a page without its .html ending (/docs/embedding for
// docs/embedding.html, and a redirect from the address with it), and a 404 for anything else. `npm run preview:dist` (after `npm run build`), then open http://localhost:5190/. Also used by e2e/production.spec.ts.
import { createReadStream, existsSync, readFileSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize, resolve } from "node:path";
import { mergeHeaders, parseHeaders, patternRegExp } from "./headers.mjs";

const root = resolve(process.argv[2] ?? "web/dist");
const port = Number(process.env.PORT ?? 5190);
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".woff2": "font/woff2", ".woff": "font/woff", ".webmanifest": "application/manifest+json", ".aff": "text/plain", ".dic": "text/plain", ".txt": "text/plain", ".xml": "application/xml", ".ico": "image/x-icon" };

const rules = parseHeaders(readFileSync(resolve("deploy/_headers"), "utf8")).map((r) => ({ re: patternRegExp(r.pattern), headers: r.headers }));
createServer((req, res) => {
  const url = new URL(req.url ?? "/", "http://x");
  const path = decodeURIComponent(url.pathname);
  let file = normalize(join(root, path));
  if (!file.startsWith(root)) file = "";
  // Cloudflare Pages: a folder answers with its index.html (and /docs asks for /docs/), a page is found without its .html, and an
  // address that names the .html is sent to the one without it (/index.html to /, /docs/embedding.html to /docs/embedding).
  if (path.endsWith(".html") && file && existsSync(file)) {
    const to = path.endsWith("/index.html") ? path.slice(0, -"index.html".length) : path.slice(0, -".html".length);
    res.writeHead(308, { location: `${to}${url.search}` });
    res.end();
    return;
  }
  if (file && existsSync(file) && statSync(file).isDirectory()) {
    if (!path.endsWith("/")) {
      res.writeHead(308, { location: `${path}/${url.search}` });
      res.end();
      return;
    }
    file = join(file, "index.html");
  } else if (file && !existsSync(file) && existsSync(`${file}.html`)) file = `${file}.html`;
  if (!file || !existsSync(file) || statSync(file).isDirectory()) {
    res.writeHead(404, { "content-type": "text/plain" });
    res.end("Not found");
    return;
  }
  for (const [k, v] of mergeHeaders(rules.filter((r) => r.re.test(path)).flatMap((r) => r.headers))) res.setHeader(k, v);
  res.setHeader("content-type", TYPES[extname(file)] ?? "application/octet-stream");
  createReadStream(file).pipe(res);
}).listen(port, () => console.log(`Docxtor's production build at http://localhost:${port}/ (headers from deploy/_headers)`));
