// Packs one built program (the `docxtor` command or the `docxtor-mcp` server) for one platform, with the licence notices for that program.
//   node scripts/package-cli.mjs --bin docxtor --target aarch64-apple-darwin --version 0.1.0 [--binary target/<target>/release/docxtor] [--out dist]
// Writes <out>/<bin>-<version>-<target>.tar.gz (.zip for Windows) holding one folder with: the program, README.txt (what it is, one line to
// run it), LICENSE-APACHE, LICENSE-MIT and THIRD_PARTY_LICENSES.txt of that program's crate (crates/docxtor-cli or crates/docxtor-mcp: the
// Rust crates compiled into it, from `npm run notices`).
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export const PROGRAMS = {
  docxtor: { crate: "crates/docxtor-cli", about: "the command-line tool for .docx files: read, edit and check them, and run Docxtor's AI tools without a browser", try: "docxtor --help" },
  "docxtor-mcp": { crate: "crates/docxtor-mcp", about: "the MCP server that lets an AI assistant read and edit .docx files with Docxtor's tools", try: "add it to your AI assistant as an MCP server whose command is docxtor-mcp (it speaks MCP over standard input and output)" },
};

export function readmeFor(bin, version, target) {
  const p = PROGRAMS[bin];
  return `${bin} ${version} (${target}): ${p.about}.\n${bin === "docxtor" ? "Try it: " : "Use it: "}${p.try}\nLicences: LICENSE-APACHE and LICENSE-MIT (Docxtor), THIRD_PARTY_LICENSES.txt (the Rust crates in this program). Source: https://github.com/tinycommit-dev/docxtor\n`;
}

/** Packs `binary` as <out>/<bin>-<version>-<target>.(tar.gz|zip) and returns the archive's path. */
export function packageCli({ bin, target, version, binary, out = join(root, "dist"), repo = root }) {
  const p = PROGRAMS[bin];
  if (!p) throw new Error(`Unknown program ${bin}: one of ${Object.keys(PROGRAMS).join(", ")}.`);
  const windows = target.includes("windows");
  const exe = windows ? `${bin}.exe` : bin;
  binary ??= join(repo, "target", target, "release", exe);
  if (!existsSync(binary)) throw new Error(`No built program at ${binary}.`);
  const name = `${bin}-${version}-${target}`;
  const stage = join(out, `.stage-${name}`);
  rmSync(stage, { recursive: true, force: true });
  const dir = join(stage, name);
  mkdirSync(dir, { recursive: true });
  cpSync(binary, join(dir, exe));
  cpSync(join(repo, "LICENSE-APACHE"), join(dir, "LICENSE-APACHE"));
  cpSync(join(repo, "LICENSE-MIT"), join(dir, "LICENSE-MIT"));
  cpSync(join(repo, p.crate, "THIRD_PARTY_LICENSES.txt"), join(dir, "THIRD_PARTY_LICENSES.txt"));
  writeFileSync(join(dir, "README.txt"), readmeFor(bin, version, target));
  mkdirSync(out, { recursive: true });
  const file = join(out, `${name}.${windows ? "zip" : "tar.gz"}`);
  rmSync(file, { force: true });
  const env = { ...process.env, COPYFILE_DISABLE: "1" };
  // Windows' own tar (bsdtar) writes a zip with -a. Git's bash puts GNU tar first on the path, which reads "D:" in a path as a host.
  const winTar = process.platform === "win32" ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "tar";
  if (windows) execFileSync(winTar, ["-a", "-cf", file, "-C", stage, name], { env });
  else {
    const gnu = /GNU tar/.test(execFileSync("tar", ["--version"], { encoding: "utf8" }));
    execFileSync("tar", ["-czf", file, ...(gnu ? ["--owner=0", "--group=0", "--numeric-owner"] : ["--uid", "0", "--gid", "0", "--no-mac-metadata"]), "-C", stage, name], { env });
  }
  rmSync(stage, { recursive: true, force: true });
  return file;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = Object.fromEntries(process.argv.slice(2).flatMap((a, i, all) => (a.startsWith("--") ? [[a.slice(2), all[i + 1]]] : [])));
  if (!args.bin || !args.target || !args.version) {
    console.error("usage: package-cli.mjs --bin docxtor|docxtor-mcp --target <triple> --version <x.y.z> [--binary <path>] [--out <dir>]");
    process.exit(1);
  }
  console.log(packageCli({ bin: args.bin, target: args.target, version: args.version, binary: args.binary, out: args.out ? resolve(args.out) : undefined }));
}
