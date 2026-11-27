// Where every page break falls, for a set of documents, on one production build: the proof that a change to the
// paginator moves nothing. Run it on the build before the change and after, then compare the two files.
//
//   node --experimental-strip-types perf/pagination-dump.mts <build dir> <out.json> [--corpus N] [--novel] [files...]
//
// For each document: the page count, every spacer the paginator inserted (the paragraph or row it is in, the text
// before it, its height), every block's top margin, the layouts Chrome ran while opening, and the open time.
import { Session } from "./lib/browser.mts";
import { serve } from "./lib/build.mts";
import { cached } from "./lib/docs.mts";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { basename, join } from "node:path";
import { homedir, tmpdir } from "node:os";

const [buildDir, outFile, ...rest] = process.argv.slice(2);
if (!buildDir || !outFile) throw new Error("usage: pagination-dump.mts <build dir> <out.json> [--corpus N] [--novel] [files...]");
const files: string[] = rest.filter((a) => a.endsWith(".docx"));
const corpusN = rest.includes("--corpus") ? Number(rest[rest.indexOf("--corpus") + 1]) : 0;
if (corpusN) {
  // The pinned sample of the corpus (untrusted data, opened only in the headless browser).
  const root = join(homedir(), ".cache", "docxtor-corpus");
  const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));
  for (const rel of (manifest.sample as string[]).slice(0, corpusN)) if (existsSync(join(root, rel))) files.push(join(root, rel));
}
const docs = join(tmpdir(), "docxtor-pagination-dump");
mkdirSync(docs, { recursive: true });
const names: string[] = [];
for (const f of files) {
  const n = basename(f).replace(/[^\w.-]/g, "_");
  copyFileSync(f, join(docs, n));
  names.push(n);
}
if (rest.includes("--novel")) {
  const c = await cached("war-and-peace", join(homedir(), ".cache", "docxtor-perf", "docs"));
  copyFileSync(c.file, join(docs, basename(c.file)));
  names.push(basename(c.file));
}

const app = await serve(buildDir, docs);
const out: Record<string, unknown> = {};
for (const n of names) {
  const s = await Session.launch(app.url);
  try {
    await s.load(app.url + "/");
    const m0 = await s.metrics();
    const t = Date.now();
    await s.openBytes(`${app.url}/__perf/${n}`, n);
    // The editor lays out again a moment after opening (fields, fonts): wait until the pages stop changing.
    let last = "";
    for (let i = 0; i < 40; i++) {
      await new Promise((r) => setTimeout(r, 400));
      const sig = await s.page.evaluate(() => `${document.querySelectorAll(".flow .pgsp").length}/${document.querySelectorAll(".sheet").length}`);
      if (sig === last) break;
      last = sig;
    }
    const ms = Date.now() - t;
    const m1 = await s.metrics();
    const dump = await s.page.evaluate(() => {
      const flow = document.querySelector(".flow") as HTMLElement;
      const textBefore = (block: Element, sp: Element) => {
        const r = document.createRange();
        r.setStart(block, 0);
        r.setEndBefore(sp);
        return r.toString().length;
      };
      const spacers = Array.from(flow.querySelectorAll<HTMLElement>(".pgsp")).map((sp) => {
        const block = sp.parentElement?.closest("[data-p], tr, table") ?? flow;
        const id = (block as HTMLElement).dataset?.p ?? block.tagName;
        return `${id}@${textBefore(block, sp)}:${sp.tagName}:${sp.style.height || sp.style.top || ""}`;
      });
      const margins = Array.from(flow.querySelectorAll<HTMLElement>("[data-p], .ftw, .cols"))
        .filter((e) => e.style.marginTop)
        .map((e) => `${e.dataset.p ?? e.className}:${e.style.marginTop}`);
      return { pages: document.querySelectorAll(".sheet").length, spacers, margins };
    });
    out[n] = { ...dump, layouts: m1.LayoutCount - m0.LayoutCount, layoutMs: Math.round((m1.LayoutDuration - m0.LayoutDuration) * 1000), ms };
    console.log(`${n}: ${dump.pages} pages, ${dump.spacers.length} spacers, ${m1.LayoutCount - m0.LayoutCount} layouts, ${ms} ms`);
  } catch (e) {
    out[n] = { error: String(e).slice(0, 300) };
    console.log(`${n}: error ${String(e).slice(0, 200)}`);
  } finally {
    await s.close();
  }
}
await app.close();
writeFileSync(outFile, JSON.stringify(out, null, 1));
