// Builds the Docxtor web app as it ships and puts it on Cloudflare Pages (docs/deploy.md).
//   npm run deploy               the live site (the production branch)
//   npm run deploy -- --preview  a preview address of its own, to try before it goes live
// The release workflow (.github/workflows/release.yml) deploys the same build itself with wrangler-action; this is the same by hand.
// Once before the first deploy: `npx wrangler login`. The project is "docxtor" unless DOCXTOR_PAGES_PROJECT names another.
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync } from "node:fs";
import { addDocs } from "./add-docs.mjs";
import { buildWeb } from "./build-web.mjs";

const WRANGLER = "wrangler@4.167.0";
const project = process.env.DOCXTOR_PAGES_PROJECT ?? "docxtor";
const preview = process.argv.includes("--preview");
const skipBuild = process.argv.includes("--no-build");
const run = (cmd, args) => execFileSync(cmd, args, { stdio: "inherit" });

if (!skipBuild) {
  // The same build as the self-host archive and the Docker image (scripts/build-web.mjs): the engine's shipped profile, wasm:ship.
  buildWeb();
}
if (!existsSync("web/dist/index.html")) throw new Error("No build in web/dist: run without --no-build.");
// The documentation site goes on the same deploy, at /docs/ (the archive and the image carry the app only).
addDocs();
copyFileSync("deploy/_headers", "web/dist/_headers");
const branch = preview ? `preview-${new Date().toISOString().slice(0, 10)}` : "main";
run("npx", ["--yes", WRANGLER, "pages", "deploy", "web/dist", "--project-name", project, "--branch", branch, "--commit-dirty=true"]);
