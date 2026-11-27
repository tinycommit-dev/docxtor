// The self-host archive: dist/docxtor-web-<version>.tar.gz, the web app as it ships, with what a person needs to serve it.
//   npm run package:web                 build (scripts/build-web.mjs: reuses the shipped engine when it is newer than its sources) and pack
//   npm run package:web -- --no-build   pack the web/dist already there
// The version is the "version" in the root package.json (the one place; scripts/release.mjs checks the others agree).
// Inside, one folder docxtor-web-<version>/:
//   site/            the built files to serve (index.html, assets/, sw.js, the engine, the dictionaries, licenses.txt ...)
//   _headers         the one source of the headers (Cloudflare Pages reads this as it is)
//   nginx.conf       the same rules as nginx locations, Caddyfile as a Caddy snippet (generated from _headers by scripts/headers.mjs)
//   licenses.txt, THIRD_PARTY_NOTICES.md, LICENSE-APACHE, LICENSE-MIT
//   README.md        how to serve it, and what breaks without HTTPS
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { buildWeb } from "./build-web.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export const readme = (version) => `# Docxtor ${version}: the web app, to serve yourself

These are the files of the Docxtor web app (https://docxtor.tinycommit.dev) exactly as it ships. Nothing runs on a server: documents stay
in each person's browser. Any web server that can send files with the right headers will do. The documentation is at
https://docxtor.tinycommit.dev/docs/ (the hosted copy has it; this archive does not, to stay small).

## What is here

- \`site/\`: the files to serve. Put them in the root of a site (\`/\` must open \`index.html\`).
- \`_headers\`: the security headers and caching rules, in Cloudflare Pages' format. It is the one source of the next two files.
- \`nginx.conf\`: the same rules as nginx \`location\` blocks. \`Caddyfile\`: the same rules as a Caddy snippet. (Both also carry the rules for \`/docs/\`, the documentation site; with no such folder in \`site/\` they do nothing.)
- \`licenses.txt\`, \`THIRD_PARTY_NOTICES.md\`, \`LICENSE-APACHE\`, \`LICENSE-MIT\`: the licences of Docxtor and of everything in it.

## Serve it

Serve \`site/\` with these headers, and over HTTPS (or on \`localhost\`).

nginx, inside a \`server { }\` with \`root /srv/docxtor;\` (the contents of \`site/\`):

    include /etc/nginx/docxtor-headers.conf;      # this archive's nginx.conf
    gzip on;
    gzip_types application/wasm text/css text/javascript application/javascript application/json image/svg+xml;

Caddy, with this archive's \`Caddyfile\` next to yours (\`import docxtor-headers\` then reads it):

    import Caddyfile                              # at the top of your Caddyfile: the snippet
    example.com {
        root * /srv/docxtor
        import docxtor-headers
        encode zstd gzip
        file_server
    }

The engine (\`assets/*.wasm\`) must be sent as \`application/wasm\`; nginx and Caddy do that already. Any other server must send the
headers in \`_headers\`: without the Content-Security-Policy the app still works, but loses the protection it was built with.

## Without HTTPS

Browsers switch several features off on a page that is not served over HTTPS (\`localhost\` counts as secure; an address like
http://192.168.1.20:8080 does not). Docxtor still opens and edits documents there, but:

- the browser may not keep your documents (storage is not made persistent), so clearing site data or low disk space can lose them;
- the offline copy (the service worker) is not installed, so the app needs the network on every visit;
- version history is not recorded (it needs the browser's cryptography, which is also HTTPS only);
- the clipboard buttons and opening and saving files in place are unavailable.

Docxtor says so on the page when it finds itself on such an address. Put a TLS proxy (Caddy does it by itself) or a certificate in
front of it to switch them on.

Docker image, if you would rather run a container: \`docker run -p 8080:8080 ghcr.io/tinycommit-dev/docxtor-web:${version}\`.
`;

/** Packs a built folder into <out>/docxtor-web-<version>.tar.gz and returns its path. `dist` is the built app (web/dist). */
export function makeArchive({ dist = join(root, "web/dist"), out = join(root, "dist"), version, repo = root }) {
  if (!existsSync(join(dist, "index.html"))) throw new Error(`No built app in ${dist}.`);
  const name = `docxtor-web-${version}`;
  const stage = join(out, `.stage-${name}`);
  rmSync(stage, { recursive: true, force: true });
  const dir = join(stage, name);
  mkdirSync(dir, { recursive: true });
  // The documentation site (web/dist/docs, added for the hosted deploy by scripts/add-docs.mjs) is not part of the archive: it stays small.
  cpSync(dist, join(dir, "site"), { recursive: true, filter: (src) => src !== join(dist, "docs") });
  rmSync(join(dir, "site/_headers"), { force: true }); // deploy.mjs copies it into web/dist for Cloudflare; here it sits beside site/
  const copy = (from, to = from) => cpSync(join(repo, from), join(dir, to));
  copy("deploy/_headers", "_headers");
  copy("deploy/nginx.conf", "nginx.conf");
  copy("deploy/Caddyfile", "Caddyfile");
  copy("THIRD_PARTY_NOTICES.md");
  copy("LICENSE-APACHE");
  copy("LICENSE-MIT");
  cpSync(join(dist, "licenses.txt"), join(dir, "licenses.txt"));
  writeFileSync(join(dir, "README.md"), readme(version));
  mkdirSync(out, { recursive: true });
  const file = join(out, `${name}.tar.gz`);
  rmSync(file, { force: true });
  // No Apple metadata files, no owner names in the archive.
  const gnu = /GNU tar/.test(execFileSync("tar", ["--version"], { encoding: "utf8" }));
  execFileSync("tar", ["-czf", file, ...(gnu ? ["--owner=0", "--group=0", "--numeric-owner"] : ["--uid", "0", "--gid", "0", "--no-mac-metadata"]), "-C", stage, name], {
    env: { ...process.env, COPYFILE_DISABLE: "1" },
  });
  rmSync(stage, { recursive: true, force: true });
  return file;
}

export const rootVersion = () => JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const built = process.argv.includes("--no-build") ? null : buildWeb();
  const file = makeArchive({ version: rootVersion() });
  console.log(`${file}${built ? ` (shipped engine ${built.engine})` : ""}`);
}
