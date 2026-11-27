// Tests perf/console-typing.js on Docxtor: pasted into the console of a document tab, it must see the keys typed in the
// editor, and keys typed into a same-origin iframe added later (the way Google Docs takes typing).
//   node --experimental-strip-types perf/check-console-script.mts
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Session, sleep } from "./lib/browser.mts";
import { serve, viteBuild, WORK } from "./lib/build.mts";
import { PERF_DIR } from "./lib/gate.mts";

const build = viteBuild(join(WORK, "head"));
const app = await serve(build.dir);
const s = await Session.launch(app.url);
try {
  await s.load(app.url + "/");
  await s.newBlank();
  await s.firstKeyAccepted();
  await s.page.evaluate(readFileSync(join(PERF_DIR, "console-typing.js"), "utf8"));
  await s.caretAt(0);
  for (let i = 0; i < 30; i++) {
    await s.page.keyboard.press("a");
    await sleep(30);
  }
  // A hidden iframe that takes typing, added after the script was pasted.
  await s.page.evaluate(() => {
    const f = document.createElement("iframe");
    f.className = "docs-texteventtarget-iframe";
    f.srcdoc = "<body contenteditable style='height:40px'></body>";
    f.style.cssText = "position:fixed;left:0;top:0;width:100px;height:40px";
    document.body.append(f);
  });
  await sleep(1600);
  await s.page.evaluate(() => (document.querySelector("iframe.docs-texteventtarget-iframe") as HTMLIFrameElement).contentDocument!.body.focus());
  for (let i = 0; i < 20; i++) {
    await s.page.keyboard.press("b");
    await sleep(30);
  }
  const rows = (await s.page.evaluate(() => (window as any).typingReport())) as Record<string, string | number>[];
  console.log(JSON.stringify(rows, null, 1));
  const top = rows.find((r) => r.frame === "top");
  const frame = rows.find((r) => String(r.frame).includes("docs-texteventtarget-iframe"));
  const ok = top && Number(top.keys) >= 25 && frame && Number(frame.keys) >= 15;
  console.log(ok ? "OK: the script saw the editor's keys and the iframe's keys" : "FAILED: the script missed keys");
  if (!ok) process.exitCode = 1;
} finally {
  await s.close();
  await app.close();
}
