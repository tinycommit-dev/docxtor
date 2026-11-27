import { defineConfig, type Plugin } from "vite";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, dirname, join, resolve } from "node:path";

// The spelling dictionaries (wooorm's Hunspell packages) hide their files behind `exports`, so the app
// reaches them through aliases that point at the package folders (see src/spelling.ts).
const require = createRequire(import.meta.url);
const dictionaryDir = (pkg: string) => dirname(require.resolve(pkg));

/** Dev-only: POST /__dev/save?name=x.docx writes the body to ../fixtures/out for inspection. */
function devSave(): Plugin {
  return {
    name: "docxtor-dev-save",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__dev/save", (req, res) => {
        const name = basename(new URL(req.url ?? "", "http://x").searchParams.get("name") ?? "out.docx");
        const chunks: Buffer[] = [];
        req.on("data", (c: Buffer) => chunks.push(c));
        req.on("end", () => {
          const dir = resolve(__dirname, "../fixtures/out");
          mkdirSync(dir, { recursive: true });
          writeFileSync(resolve(dir, name), Buffer.concat(chunks));
          res.end("ok");
        });
      });
    },
  };
}

/** Every file under a folder, as paths relative to it. */
function filesUnder(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    return statSync(full).isDirectory() ? filesUnder(full).map((f) => `${name}/${f}`) : [name];
  });
}

/** Faces for scripts most documents never use are kept as they are first needed rather than up front. */
const LATE_FONT = /-(cyrillic|greek|hebrew|arabic|thai|devanagari|vietnamese)(-ext)?-/;

/**
 * Makes the app work offline: writes `sw.js` (from offline/sw.js) into the build with the list of every file the app needs
 * (its pages, scripts, wasm, spelling dictionaries, the bundled fonts, the icons and the manifest). The page registers it
 * (src/offline.ts). The list is the build's own output, so it is never out of step with the files it names.
 */
function offline(): Plugin {
  return {
    name: "docxtor-offline",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const emitted = Object.keys(bundle).filter((f) => !f.endsWith(".map") && !f.endsWith(".woff") && !LATE_FONT.test(f));
      // Files from public/ are copied after the bundle is made, so they are listed from the folder.
      const pub = filesUnder(resolve(__dirname, "public"));
      // The page itself is written after this runs; it is named here because it is always there.
      const list = [...new Set(["./", "index.html", ...emitted, ...pub])].sort();
      const hash = createHash("sha1");
      for (const f of list) hash.update(f);
      for (const f of pub) hash.update(readFileSync(resolve(__dirname, "public", f)));
      const index = bundle["index.html"];
      if (index && "source" in index) hash.update(String(index.source));
      const version = `${Date.now().toString(36).padStart(9, "0")}-${hash.digest("hex").slice(0, 10)}`;
      const sw = readFileSync(resolve(__dirname, "offline/sw.js"), "utf8").replace("__VERSION__", version).replace("__PRECACHE__", JSON.stringify(list, null, 1));
      this.emitFile({ type: "asset", fileName: "sw.js", source: sw });
    },
  };
}

/** Dev only: serves a built copy (`vite build --outDir`) under /__built/ so the offline tests can run the real service worker on the one test port. */
function serveBuilt(): Plugin {
  return {
    name: "docxtor-serve-built",
    apply: "serve",
    configureServer(server) {
      const dir = process.env.DOCXTOR_BUILT_DIR;
      if (!dir) return;
      const types: Record<string, string> = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".woff2": "font/woff2", ".woff": "font/woff", ".webmanifest": "application/manifest+json", ".aff": "text/plain", ".dic": "text/plain" };
      server.middlewares.use("/__built", (req, res) => {
        const path = decodeURIComponent((req.url ?? "/").split("?")[0]);
        const file = resolve(dir, "." + (path.endsWith("/") ? `${path}index.html` : path));
        if (!file.startsWith(resolve(dir)) || !existsSync(file) || !statSync(file).isFile()) {
          res.statusCode = 404;
          return res.end("not found");
        }
        const ext = file.slice(file.lastIndexOf("."));
        res.setHeader("content-type", types[ext] ?? "application/octet-stream");
        res.setHeader("service-worker-allowed", "/__built/");
        res.end(readFileSync(file));
      });
    },
  };
}

/**
 * Writes the list of npm packages that end up in the build (their scripts, styles, fonts and dictionaries) to
 * node_modules/.cache/docxtor-notices/bundled-packages.json, for `npm run notices`, which credits each with its licence.
 * The list comes from the build's own modules and assets, so it is never out of step with what ships.
 */
function bundledPackages(): Plugin {
  const found = new Set<string>();
  const note = (id: string) => {
    const clean = id.split("?")[0].replace(/\\/g, "/");
    const at = clean.lastIndexOf("/node_modules/");
    if (at < 0) return;
    const parts = clean.slice(at + "/node_modules/".length).split("/");
    found.add(parts[0].startsWith("@") ? `${parts[0]}/${parts[1]}` : parts[0]);
  };
  return {
    name: "docxtor-bundled-packages",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      for (const file of Object.values(bundle)) {
        if (file.type === "chunk") {
          for (const id of file.moduleIds ?? Object.keys(file.modules ?? {})) note(id);
        } else {
          for (const id of (file as { originalFileNames?: string[] }).originalFileNames ?? []) note(resolve(__dirname, id));
        }
      }
    },
    closeBundle() {
      const root = resolve(__dirname, "..");
      const packages = [...found].sort().map((name) => {
        const info = JSON.parse(readFileSync(resolve(root, "node_modules", name, "package.json"), "utf8"));
        return { name, version: info.version as string };
      });
      const dir = resolve(root, "node_modules/.cache/docxtor-notices");
      mkdirSync(dir, { recursive: true });
      writeFileSync(resolve(dir, "bundled-packages.json"), JSON.stringify(packages, null, 2) + "\n");
    },
  };
}

export default defineConfig({
  base: "./",
  plugins: [devSave(), offline(), serveBuilt(), bundledPackages()],
  // Hunspell dictionaries are data files: the app fetches them, they are not code.
  assetsInclude: ["**/*.aff", "**/*.dic"],
  resolve: {
    alias: {
      "@dictionaries/en-us": dictionaryDir("dictionary-en"),
      "@dictionaries/en-gb": dictionaryDir("dictionary-en-gb"),
      "@dictionaries/es": dictionaryDir("dictionary-es"),
      "@dictionaries/fr": dictionaryDir("dictionary-fr"),
      "@dictionaries/de": dictionaryDir("dictionary-de"),
    },
  },
  build: { target: "es2022", chunkSizeWarningLimit: 2000 },
  // Dev only: lets the dev server read ../fixtures for manual testing, and answer to a name ending in .test (e2e/launch.spec.ts opens
  // the app at http://docxtor.test, which is not a secure address, to see what Docxtor says there).
  // Two folders the browser tests write pages into, which the dev server serves but must not watch: `dist-test`, a production build
  // (e2e/offline.spec.ts, served under /__built/), and `examples/_docs`, the documentation's examples as pages (e2e/docs-examples.spec.ts).
  // Vite reloads every open page when an .html file under its root changes, so each browser test running at that moment lost its page
  // (a flake in every browser, most often in Firefox and Safari, whose pages were slower to settle).
  server: { port: 5173, fs: { allow: [".."] }, allowedHosts: [".test"], watch: { ignored: ["**/dist-test/**", "**/examples/_docs/**"] } },
});
