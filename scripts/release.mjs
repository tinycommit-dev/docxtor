// Small helpers for cutting a release (docs/deploy.md, "Cutting a release"); the release workflow calls them.
//   node scripts/release.mjs version            prints the version (the "version" in the root package.json) and fails if the others differ
//   node scripts/release.mjs check-tag v0.2.0   fails unless the tag is "v" plus that version
//   node scripts/release.mjs notes 0.2.0        prints the CHANGELOG.md section "## [0.2.0]" (the release notes)
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (f) => readFileSync(join(root, f), "utf8");

/** The version in every place that carries one. */
export function versions() {
  const cargo = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(read("Cargo.toml"))?.[1];
  return {
    "package.json": JSON.parse(read("package.json")).version,
    "web/package.json": JSON.parse(read("web/package.json")).version,
    "Cargo.toml [workspace.package]": cargo,
  };
}

/** The release version, or an error naming the places that disagree. */
export function releaseVersion() {
  const all = versions();
  const main = all["package.json"];
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(main ?? "")) throw new Error(`package.json: "${main}" is not a version like 1.2.3`);
  const off = Object.entries(all).filter(([, v]) => v !== main);
  if (off.length) throw new Error(`The version is ${main} in package.json, but ${off.map(([f, v]) => `${v} in ${f}`).join(", ")}.`);
  return main;
}

/** The text under `## [version]` in a changelog, up to the next `## `. Throws when there is none, or it is empty. */
export function changelogSection(changelog, version) {
  const lines = changelog.split("\n");
  const esc = version.replace(/[.+*?^${}()|[\]\\]/g, "\\$&");
  const start = lines.findIndex((l) => new RegExp(`^## \\[${esc}\\]( |$)`).test(l));
  if (start < 0) throw new Error(`CHANGELOG.md has no section "## [${version}]". Write it before tagging.`);
  let end = lines.findIndex((l, i) => i > start && /^## /.test(l));
  if (end < 0) end = lines.length;
  const body = lines.slice(start + 1, end).join("\n").trim();
  if (!body) throw new Error(`The CHANGELOG.md section for ${version} is empty.`);
  return body + "\n";
}

export function checkTag(tag, version) {
  if (tag !== `v${version}`) throw new Error(`The tag is ${tag}, but the version in package.json is ${version} (the tag must be v${version}).`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [cmd, arg] = process.argv.slice(2);
  try {
    if (cmd === "version") console.log(releaseVersion());
    else if (cmd === "check-tag") checkTag(arg, releaseVersion());
    else if (cmd === "notes") process.stdout.write(changelogSection(read("CHANGELOG.md"), arg ?? releaseVersion()));
    else throw new Error("usage: release.mjs version | check-tag <tag> | notes <version>");
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}
