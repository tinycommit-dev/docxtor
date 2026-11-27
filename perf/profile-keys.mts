// Profile typing in a big document on a production build: where the page's own work per key goes.
//
//   [FAR=<pages>] BUILD=<dir> node --experimental-strip-types perf/profile-keys.mts [doc] [keys] [--layouts]
//
// `doc` is a file in ~/.cache/docxtor-perf/docs (default: War and Peace), `keys` how many keys (default 30), each given its own
// frame. Prints Chrome's counters (script, layout and style time, layouts and style recalcs per key) and the hottest
// functions, self and inclusive. With `--layouts`, a trace is recorded too and each forced layout or style recalculation is
// listed with the JavaScript stack that forced it, grouped by stack: the way to find who reads the layout too early.
// Build the app first: `npm run wasm`, then `viteBuild(dir)` from perf/lib/build.mts (or any `vite build --outDir <dir>` in web/).
import { Session, sleep } from "./lib/browser.mts";
import { serve } from "./lib/build.mts";
import { homedir } from "node:os";

const args = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const LAYOUTS = process.argv.includes("--layouts");
const HARNESS = process.argv.includes("--harness");
import { readdirSync } from "node:fs";
// A short name ("war-and-peace", "proust") finds the cached file; a name that matches nothing stops here instead of waiting forever for the open.
const docs = readdirSync(`${homedir()}/.cache/docxtor-perf/docs`).filter((f) => f.endsWith(".docx"));
const want = args[0] ?? "war-and-peace";
const doc = docs.find((f) => f === want) ?? docs.find((f) => f.startsWith(want + "-"));
if (!doc) {
  console.error(`No document "${want}" in ~/.cache/docxtor-perf/docs (there: ${docs.join(", ")}). Run npm run perf once to fetch them.`);
  process.exit(1);
}
const KEYS = Number(args[1] ?? 30);
const app = await serve(process.env.BUILD!, `${homedir()}/.cache/docxtor-perf/docs`);
const s = await Session.launch(app.url);
// FAR=n: leave pages far from the view out of the layout, n pages either side (the test switch `__DOCXTOR_FAR_PAGES__`).
if (process.env.FAR !== undefined) await s.context.addInitScript((n) => ((globalThis as any).__DOCXTOR_FAR_PAGES__ = n), Number(process.env.FAR));
await s.load(app.url + "/");
await s.openBytes(`${app.url}/__perf/${doc}`, doc);
if (HARNESS) {
  // Open as the performance harness does: a key typed at the start of the document, then wait until the memory settles.
  await s.firstKeyAccepted();
  await s.settled();
}
if (!HARNESS) await sleep(4000);
await s.caretAt(Number(process.env.CARET ?? 0.5));
if (!HARNESS) await sleep(1500);

async function startTrace(): Promise<() => Promise<any[]>> {
  const events: any[] = [];
  const b = s.bcdp;
  b.on("Tracing.dataCollected" as never, ((e: { value: any[] }) => events.push(...e.value)) as never);
  await b.send("Tracing.start", { traceConfig: { recordMode: "recordUntilFull", includedCategories: ["-*", "devtools.timeline", "disabled-by-default-devtools.timeline", "disabled-by-default-devtools.timeline.stack", "disabled-by-default-devtools.timeline.invalidationTracking"] }, transferMode: "ReportEvents" } as never);
  return async () => {
    const done = new Promise((ok) => b.once("Tracing.tracingComplete" as never, ok as never));
    await b.send("Tracing.end");
    await done;
    return events;
  };
}

/** Each Layout and UpdateLayoutTree the page ran, with the JavaScript frames that forced it, grouped by stack. */
async function reportLayouts(all: any[]) {
  // Only the keys: from the first keydown to just after the last keyup (what the page does after the last key, a pause later, is not per key).
  const keyed = all.filter((e) => e.name === "EventDispatch" && e.args?.data?.type === "keydown");
  const ups = all.filter((e) => e.name === "EventDispatch" && e.args?.data?.type === "keyup");
  const from = Math.min(...keyed.map((e) => e.ts));
  const to = Math.max(...ups.map((e) => e.ts)) + 40_000;
  const events = all.filter((e) => e.ts >= from && e.ts <= to);
  const groups = new Map<string, { n: number; ms: number; kind: string }>();
  let total = 0;
  for (const e of events) {
    if (e.ph !== "X" || (e.name !== "Layout" && e.name !== "UpdateLayoutTree")) continue;
    const st: any[] = e.args?.beginData?.stackTrace ?? [];
    if (st.some((f) => f.functionName === "layoutNow")) continue;
    const frames = st.slice(0, 6).map((f) => `${f.functionName || "(anon)"} ${String(f.url).split("/").pop()}:${f.lineNumber + 1}:${f.columnNumber}`);
    const k = `${e.name}\n    ` + (frames.join("\n    ") || "(no JS: the browser's own frame work)");
    const g = groups.get(k) ?? { n: 0, ms: 0, kind: e.name };
    g.n++;
    g.ms += (e.dur ?? 0) / 1000;
    groups.set(k, g);
    total++;
  }
  // What made them necessary: the writes that invalidated style or layout, by the stack that did it.
  const inv = new Map<string, number>();
  for (const e of events) {
    if (!/InvalidationTracking$|^ScheduleStyleRecalculation$/.test(e.name)) continue;
    const d = e.args?.data ?? {};
    const st: any[] = d.stackTrace ?? [];
    const frames = st.slice(0, 4).map((f) => `${f.functionName || "(anon)"} ${String(f.url).split("/").pop()}:${f.lineNumber + 1}`);
    if (st.some((f) => f.functionName === "layoutNow" || f.functionName === "layout")) continue; // the pagination a pause after the keys: not per key (tracing makes keys slow enough for it to run between them)
    const k = `${e.name} ${d.reason ?? ""} on ${d.nodeName ?? ""}${d.selectorPart ? " (" + d.selectorPart + ")" : ""}\n    ` + (frames.join("\n    ") || "(no JS)");
    inv.set(k, (inv.get(k) ?? 0) + 1);
  }
  console.log(`\n=== what invalidated style or layout, by stack (per key)`);
  for (const [k, n] of [...inv].sort((a, b) => b[1] - a[1]).slice(0, 40)) console.log(`${(n / KEYS).toFixed(2)} per key: ${k}`);
  console.log(`\n=== ${total} layouts and style recalculations in the trace, by the stack that forced them (per key: n / ${KEYS})`);
  for (const [k, g] of [...groups].sort((a, b) => b[1].ms - a[1].ms)) console.log(`${(g.n / KEYS).toFixed(2)} per key, ${(g.ms / KEYS).toFixed(2)} ms per key: ${k}`);
}

async function profiled(label: string, act: () => Promise<void>) {
  const m0 = await s.metrics();
  await s.cdp.send("Profiler.enable");
  await s.cdp.send("Profiler.setSamplingInterval", { interval: 200 });
  await s.cdp.send("Profiler.start");
  const t = Date.now();
  const trace = LAYOUTS ? await startTrace() : null;
  await act();
  if (trace) await reportLayouts(await trace());
  const ms = Date.now() - t;
  const { profile } = (await s.cdp.send("Profiler.stop")) as any;
  const m1 = await s.metrics();
  const d = (k: string) => +((m1[k] - m0[k]) * 1000).toFixed(0);
  console.log(`\n=== ${label}: ${ms} ms wall; per key own work ${((d("ScriptDuration") + d("LayoutDuration") + d("RecalcStyleDuration")) / KEYS).toFixed(2)} ms, ${((m1.LayoutCount - m0.LayoutCount) / KEYS).toFixed(2)} layouts, ${((m1.RecalcStyleCount - m0.RecalcStyleCount) / KEYS).toFixed(2)} style recalcs; script ${d("ScriptDuration")} ms, layout ${d("LayoutDuration")} ms (${m1.LayoutCount - m0.LayoutCount} layouts), style ${d("RecalcStyleDuration")} ms`);
  const byId = new Map<number, any>(profile.nodes.map((n: any) => [n.id, n]));
  const parent = new Map<number, number>();
  for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const self = new Map<number, number>();
  for (let i = 0; i < profile.samples.length; i++) self.set(profile.samples[i], (self.get(profile.samples[i]) ?? 0) + (profile.timeDeltas[i] ?? 0) / 1000);
  const key = (n: any) => `${n.callFrame.functionName || "(anon)"} ${n.callFrame.url.split("/").pop()}:${n.callFrame.lineNumber + 1}:${n.callFrame.columnNumber}`;
  const selfBy = new Map<string, number>();
  const incl = new Map<string, number>();
  for (const [id, ms] of self) {
    const n = byId.get(id);
    selfBy.set(key(n), (selfBy.get(key(n)) ?? 0) + ms);
    const seen = new Set<string>();
    for (let cur: number | undefined = id; cur !== undefined; cur = parent.get(cur)) {
      const k = key(byId.get(cur));
      if (seen.has(k)) continue;
      seen.add(k);
      incl.set(k, (incl.get(k) ?? 0) + ms);
    }
  }
  const top = (m: Map<string, number>, n: number) => [...m].sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, v]) => `${Math.round(v).toString().padStart(7)} ms  ${k}`).join("\n");
  console.log("SELF\n" + top(selfBy, 40));
  // Who calls the browser's own functions that cost time here: the nearest JavaScript function above each, by time.
  const callers = new Map<string, Map<string, number>>();
  for (const [id, ms] of self) {
    const n = byId.get(id);
    if (n.callFrame.url) continue;
    const fn = n.callFrame.functionName;
    if (!/^(querySelectorAll|querySelector|remove|getBoundingClientRect|getClientRects|replaceWith|replaceChildren|append|closest|contains|getSelection|setBaseAndExtent)$/.test(fn)) continue;
    let up = parent.get(id);
    while (up !== undefined && !byId.get(up).callFrame.url) up = parent.get(up);
    const who = up === undefined ? "(top)" : key(byId.get(up));
    const m = callers.get(fn) ?? new Map<string, number>();
    m.set(who, (m.get(who) ?? 0) + ms);
    callers.set(fn, m);
  }
  console.log("CALLERS of the browser's own functions");
  for (const [fn, m] of callers) console.log(`${fn}\n` + [...m].sort((a, b) => b[1] - a[1]).slice(0, 4).map(([k, v]) => `    ${Math.round(v).toString().padStart(5)} ms  ${k}`).join("\n"));
  console.log("INCLUSIVE\n" + top(incl, 90));
}

if (LAYOUTS) {
  // Tracing makes each key slow enough for the editor's pause (120 ms after the last key) to pass between keys, which paginates the document and
  // redraws everything on the spot: not what a person typing at speed gets. Hold that one timer back until the keys are done.
  await s.page.evaluate(() => {
    const real = window.setTimeout.bind(window);
    (window as any).setTimeout = (fn: TimerHandler, ms?: number, ...a: unknown[]) => real(fn, ms === 120 ? 30_000 : ms, ...a);
  });
}
await profiled(`${KEYS} keystrokes, one frame each`, async () => {
  for (let i = 0; i < KEYS; i++) {
    await s.page.keyboard.press("abcdefghij"[i % 10]);
    await s.page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(null)))));
  }
});
await s.close();
await app.close();
