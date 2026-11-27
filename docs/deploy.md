# Putting Docxtor online

The free public version is the web app as static files on Cloudflare Pages. Nothing runs on a
server: documents stay in each person's browser. The same files are also released as an archive and a Docker image, for anyone who
wants to host their own copy.

## What a release contains

A release is a version tag (`v0.1.0`) on GitHub. The release workflow (`.github/workflows/release.yml`) makes all of this from it:

- the hosted app at https://docxtor.tinycommit.dev (Cloudflare Pages, project `docxtor`), with this documentation at https://docxtor.tinycommit.dev/docs/ on the same deploy;
- the GitHub release, with these files and `SHA256SUMS` over all of them (each also has GitHub's build provenance, checked with `gh attestation verify <file> --repo tinycommit-dev/docxtor`):
  - `docxtor-<version>-<target>` and `docxtor-mcp-<version>-<target>`, the command-line tool and the MCP server, for macOS (Apple silicon `aarch64-apple-darwin`, Intel `x86_64-apple-darwin`), Linux (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`: static programs that run on any distribution) and Windows (`x86_64-pc-windows-msvc`, a .zip). Each archive has its program, the licences and the Rust crates' notices for that program, and a README line;
  - `docxtor-web-<version>.tar.gz`, the web app for self-hosting (below);
  - the CHANGELOG section for that version as the release notes;
- the Docker image `ghcr.io/tinycommit-dev/docxtor-web`, tagged with the version and `latest`, for amd64 and arm64.

No npm packages are published yet; they come with a later release.

One build: the hosted app, the archive and the image are made by the same function (`scripts/build-web.mjs`: the engine as `npm run wasm:ship` builds it, 12.6 MB raw, 4.1 MB gzipped, 2.7 MB brotli, then `npm run build`). The image builds inside its own build stage with that function, so it is the same build, not the same bytes (the service worker's version stamp carries the build time).

## Cutting a release

1. Bump the version in `package.json` (the one place), and in `Cargo.toml` (`[workspace.package]`) and `web/package.json`, which must say the same: `npm test` fails when the three differ. Run `cargo update --workspace` so `Cargo.lock` follows, and `npm install --package-lock-only` for `package-lock.json`.
2. In `CHANGELOG.md`, rename what is under `## [Unreleased]` to `## [<version>] - <date>` (a new empty `Unreleased` above it). That section is the release notes: the workflow stops when it is missing or empty.
3. Commit, push, and wait for CI on `main` to pass.
4. Tag it and push the tag (the block below).
5. Watch the workflow. The order: the gates (all of `ci.yml`), then the builds, the image (pushed, run read-only, tested with the production test and scanned), the Cloudflare preview (tested the same way), the GitHub release, and only then the live site and the image's `latest` tag. A failed step stops what comes after it; nothing is rolled back, so re-run the failed job (or delete the release and tag and start again).

*Sketch, not run by a test: pushes a tag, which starts a release.*
```bash
git tag v0.1.0 && git push origin v0.1.0
```

To try the whole workflow without publishing anything, run it by hand from the Actions tab ("Run workflow", dry run on): it builds and tests everything and uploads nothing. A dry run needs no secrets.

To put the hosted app up without a release, run it by hand with dry run off and site only on: the web build, the Cloudflare preview and its production test, then the live site, in about six minutes. It does not run the checks again: it waits for CI on the same commit and stops if CI did not pass. Nothing else is published (no GitHub release, no image, no programs).

`node scripts/release.mjs version` prints the version the three files agree on, `node scripts/release.mjs notes <version>` prints the notes.

## Once: GitHub and Cloudflare

On GitHub (repository `tinycommit-dev/docxtor`):

- Settings, Actions, General: workflow permissions can stay read-only; the release workflow asks for what each job needs.
- Secrets (Settings, Secrets and variables, Actions): `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`. Nothing else: the image and the release use the workflow's own token.
- Optional: an environment named `production` (Settings, Environments) with a required reviewer makes the live deploy wait for a person's click after everything else has passed.
- After the first push of the image, make the package public (the package's settings on github.com, "Change visibility"), so `docker pull` works without signing in.

On Cloudflare (a free account):

1. Create the Pages project, once, by hand: `npx wrangler login`, then `npx wrangler pages project create docxtor --production-branch main`. The name is `docxtor` (`PAGES_PROJECT` in the workflow; `DOCXTOR_PAGES_PROJECT` for `npm run deploy`).
2. The custom domain: the project's Custom domains, add `docxtor.tinycommit.dev`. If `tinycommit.dev` is on Cloudflare's DNS, it adds the record itself; otherwise add the CNAME to `docxtor.pages.dev` where the DNS lives.
3. The API token (My Profile, API Tokens, Create Token, custom): permission Account, Cloudflare Pages, Edit, on that account only. The account id is on the account's home page. These two are the two secrets above.
4. Preview deployments of other branches are on by default; the workflow deploys the release to a branch named `release-<version>` first, and the production test must pass there before the live deploy.

## Deploying by hand

Only for a quick fix without a release; a release does this from CI.

*Sketch, not run by a test: uploads to a Cloudflare account.*
```bash
npm run deploy -- --preview
```

builds the app as it ships (reusing the shipped engine when it is newer than its sources; otherwise `wasm:ship`, about 4 minutes) and puts it at a preview address of its own. Try it there, then:

*Sketch, not run by a test: uploads to a Cloudflare account.*
```bash
npm run deploy
```

puts the same build on the live address. `-- --no-build` uploads the build already in `web/dist`.

## What goes up

- `web/dist`: the app, the engine (12.6 MB; 2.7 MB compressed with brotli, 4.1 MB with gzip), the spelling dictionaries, the fonts, the offline service worker.
- `web/dist/docs`: the documentation site (`npm run docs:add`, which `npm run deploy` and the release workflow run after the app is built): the Markdown of `docs/` and the root files `CONTRIBUTING.md`, `PRIVACY.md` and `SECURITY.md`, built by VitePress (`npm run docs:build` builds it alone, into `docs/.vitepress/dist`, and fails on a dead link). It is about 2 MB, has its own Content-Security-Policy and no script from outside; the self-host archive and the Docker image do not carry it.
- `deploy/_headers` becomes `web/dist/_headers`: the Content-Security-Policy (scripts only from the site itself, plus the engine's
  WebAssembly; styles and fonts also from Google Fonts; pictures from anywhere; connections to any https address and to this computer, for
  the AI providers, Ollama, LM Studio, LanguageTool and Zotero), and caching (built files for a year, the page and the service worker
  looked at again on every visit).

## The headers, one source

`deploy/_headers` is the one source of the security headers and caching rules. `scripts/headers.mjs` turns it into the same rules for the two
servers people self-host with: `deploy/nginx.conf` (location blocks) and `deploy/Caddyfile` (a snippet). Both are generated and committed; edit
`_headers`, then:

*Sketch, not run by a test: rewrites two files in deploy/.*
```bash
npm run headers
```

`npm test` runs `scripts/headers.test.mjs`, which reads both files back and asks them what they send for real paths (the answer must be
Cloudflare's), and fails when a file no longer matches what `_headers` makes. The nginx file is also passed to `nginx -t` inside the image's
nginx (`scripts/nginx-config.test.mjs`, when Docker is running). The Caddyfile is read back by the test's own parser: no test runs Caddy itself.

Cloudflare adds every matching rule's headers and joins two values of one name with a comma, so a rule that wants to replace a header says `! Name` first (the docs' rule for `/docs/*` does that for the Content-Security-Policy, and `/docs/assets/*` for Cache-Control). The generator, `scripts/serve-dist.mjs` and the test all follow that rule. The docs' policy allows its two small inline scripts (the theme switch and the Mac key labels, which VitePress writes into every page) by their hash instead of `'unsafe-inline'`; `scripts/docs-site.test.mjs` hashes the scripts of the built pages and fails when `deploy/_headers` lists other hashes (after a VitePress update, copy the new ones from the failure and run `npm run headers`).

## Self-hosting with the archive

Download `docxtor-web-<version>.tar.gz` from the release and unpack it. It holds `site/` (the files to serve), `_headers`, `nginx.conf`, `Caddyfile`,
the licences and a README with the same steps as here.

nginx, in a `server { }` that serves `site/` from its `root` (the headers file is `nginx.conf` from the archive):

*Sketch, not run by a test: a fragment of an nginx configuration; the same rules are checked by nginx -t in scripts/nginx-config.test.mjs.*
```nginx
root /srv/docxtor;
include /etc/nginx/docxtor-headers.conf;
gzip on;
gzip_types application/wasm text/css text/javascript application/json image/svg+xml;
```

Caddy, with the archive's `Caddyfile` imported at the top of yours:

*Sketch, not run by a test: a Caddyfile; Caddy is not run by any test here.*
```caddyfile
import Caddyfile

docxtor.example.com {
    root * /srv/docxtor
    import docxtor-headers
    encode zstd gzip
    file_server
}
```

Serve it over HTTPS (Caddy does that by itself). Docxtor still opens and edits documents without it, but the browser then switches off
what needs a secure page: the offline copy (the service worker), version history, the clipboard buttons, opening and saving files in
place, and asking the browser to keep your documents when the disk runs low. `localhost` counts as secure; an address like
http://192.168.1.20:8080 does not. Docxtor says so once on the page when it finds itself there.

## Self-hosting with Docker

*Sketch, not run by a test: pulls an image from a registry.*
```bash
docker run -p 8080:8080 ghcr.io/tinycommit-dev/docxtor-web:0.1.0
```

serves the app at http://localhost:8080/. The image is the app on the official unprivileged nginx (user 101, port 8080), pinned by digest, for amd64 and arm64:

- Read-only works, and is the way to run it: nothing is written except nginx's process file and temporary files in `/tmp` (the block below).
- `/healthz` answers `200` with `ok` (plain text, never cached), and the image has a `HEALTHCHECK` on it (`docker ps` shows `healthy`).
- The headers are `deploy/nginx.conf`, the same rules Cloudflare sends. The engine goes out as `application/wasm`, compressed with gzip (nginx's official build has no brotli: the engine is 4.1 MB over the wire).
- The licences are in the image (`/usr/share/doc/docxtor/`) and served at `/licenses.txt`.
- Put a TLS proxy in front (Caddy, Traefik, nginx, a cloud load balancer) so the page is HTTPS; the proxy can pass everything through and
  must not strip the security headers (the second block below, with Caddy).

*Sketch, not run by a test: needs the image; the same flags are run by the release workflow's image job.*
```bash
docker run -d --name docxtor --read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges -p 8080:8080 ghcr.io/tinycommit-dev/docxtor-web:0.1.0
```

*Sketch, not run by a test: a Caddyfile; Caddy is not run by any test here.*
```caddyfile
docxtor.example.com {
    reverse_proxy docxtor:8080
}
```

Building the image yourself (from the repository root: the build stage compiles the engine, so the first build takes about ten minutes):

*Sketch, not run by a test: a long Docker build.*
```bash
docker build -f deploy/Dockerfile --build-arg VERSION=0.1.0 -t docxtor-web .
```

## Checking a build before it goes up

*Sketch, not run by a test: a full production build, run by hand before each release.*
```bash
npm run build
```

*Sketch, not run by a test: serves the production build until stopped; run by hand before each release.*
```bash
npm run preview:dist
```

serves `web/dist` at http://localhost:5190/ with the same headers Cloudflare sends. The browser test of it:

*Sketch, not run by a test: needs the production build; run by hand before each release.*
```bash
DOCXTOR_DIST=1 npx playwright test e2e/production.spec.ts
```

(opens a real file, edits and saves it, installs the service worker, and opens again with the network off; any resource the policy blocks
fails it). The same test runs against a copy that is already running (the Docker container, a Cloudflare preview address) with
`DOCXTOR_DIST_URL`:

*Sketch, not run by a test: needs a running copy of the app.*
```bash
DOCXTOR_DIST_URL=http://localhost:8080/ npx playwright test e2e/production.spec.ts
```

The archive is made by `npm run package:web` (into `dist/`), and `scripts/package-web.test.mjs` unpacks one and checks what is in it.
