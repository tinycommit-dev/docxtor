// Paste this whole file into the DevTools console of an editor tab (Docxtor or Google Docs), press Enter, click into the
// document, type about 50 characters at your normal pace, then run:  typingReport()   (and typingStop() when done).
//
// It measures what the browser says about each key you type, the same way on every editor:
//   input delay   from the moment the key reached the browser until the page's first handler ran
//   processing    from the first handler starting to the last handler finishing (Event Timing's processingStart..processingEnd,
//                 and, exact for every key, our own listeners on the way down and back up)
// It does not measure the time until the character is drawn (Event Timing's `duration` does, rounded to 8 ms, and only
// for events over 16 ms), so it never says "Google Docs paints slower"; it says how long the page's own code held up the main thread.
//
// Google Docs takes typing in a hidden iframe (docs-texteventtarget-iframe), and events fire in the frame that receives them,
// so a listener on the top window never sees a key. This script therefore installs itself in the page and in every
// same-origin iframe, now and as new ones appear (checked every second), each with that frame's own PerformanceObserver
// and event listeners. A cross-origin frame cannot be reached; it is reported as skipped.
(() => {
  if (window.__typingProbe) window.__typingProbe.stop();
  const frames = new Map(); // frame window -> { name, keys, observer, off }
  const rows = []; // one per key: { frame, delay, proc, timing }
  const skipped = new Set();
  const names = ["keydown", "beforeinput", "input"];

  function attach(win, label) {
    if (frames.has(win)) return;
    const f = { label, events: 0, off: [], observer: null, timing: [] };
    frames.set(win, f);
    const open = new Map();
    for (const type of names) {
      const down = (e) => {
        const t0 = win.performance.now();
        const r = { type, delay: t0 - e.timeStamp, t0, end: t0, frame: label, at: Date.now() };
        open.set(e, r);
        rows.push(r);
        f.events++;
      };
      const up = (e) => {
        const r = open.get(e);
        if (r) {
          r.end = win.performance.now();
          open.delete(e);
        }
      };
      win.addEventListener(type, down, true);
      win.addEventListener(type, up, false);
      f.off.push(() => win.removeEventListener(type, down, true), () => win.removeEventListener(type, up, false));
    }
    try {
      // This frame's own observer: another frame's observer never sees these events.
      f.observer = new win.PerformanceObserver((list) => {
        for (const e of list.getEntries()) if (names.includes(e.name)) f.timing.push({ type: e.name, delay: e.processingStart - e.startTime, proc: e.processingEnd - e.processingStart, dur: e.duration });
      });
      f.observer.observe({ type: "event", durationThreshold: 16, buffered: false });
    } catch (err) {
      f.observer = null;
    }
  }

  function scan(win, label) {
    attach(win, label);
    let list = [];
    try {
      list = Array.from(win.document.querySelectorAll("iframe"));
    } catch (e) {}
    list.forEach((el, i) => {
      let w = null;
      try {
        w = el.contentWindow && el.contentWindow.document ? el.contentWindow : null; // throws when cross-origin
      } catch (e) {
        w = null;
      }
      const name = `${label} > ${el.className || el.id || el.name || "iframe" + i}`.slice(0, 80);
      if (w) scan(w, name);
      else if (!skipped.has(el)) skipped.add(el);
    });
  }

  scan(window, "top");
  const timer = setInterval(() => scan(window, "top"), 1000);

  const med = (a) => (a.length ? a.slice().sort((x, y) => x - y)[Math.floor(a.length / 2)] : NaN);
  const pct = (a, p) => (a.length ? a.slice().sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor((p / 100) * a.length))] : NaN);
  const fmt = (x) => (Number.isFinite(x) ? x.toFixed(2) : "-");

  window.typingReport = () => {
    const out = [];
    for (const [, f] of frames) {
      const mine = rows.filter((r) => r.frame === f.label);
      if (!mine.length && !f.timing.length) continue;
      const keys = mine.filter((r) => r.type === "keydown");
      // One key's processing is the sum over its keydown, beforeinput and input events.
      const perKey = [];
      let cur = 0;
      let started = false;
      for (const r of mine) {
        if (r.type === "keydown") {
          if (started) perKey.push(cur);
          cur = 0;
          started = true;
        }
        cur += r.end - r.t0;
      }
      if (started) perKey.push(cur);
      out.push({
        frame: f.label,
        keys: keys.length,
        "input delay median (ms)": fmt(med(keys.map((r) => r.delay))),
        "input delay p90 (ms)": fmt(pct(keys.map((r) => r.delay), 90)),
        "processing per key median (ms)": fmt(med(perKey)),
        "processing per key p90 (ms)": fmt(pct(perKey, 90)),
        "processing per key worst (ms)": fmt(Math.max(...perKey)),
        "Event Timing entries (events over 16 ms only)": f.timing.length,
        "Event Timing processing median (ms)": fmt(med(f.timing.map((t) => t.proc))),
        "Event Timing input delay median (ms)": fmt(med(f.timing.map((t) => t.delay))),
      });
    }
    if (!out.length) console.log("No keys seen yet. Click into the document and type; if this is Google Docs, the keys go to a hidden iframe: it is watched too.");
    else console.table(out);
    if (skipped.size) console.log(`${skipped.size} cross-origin iframe(s) could not be watched.`);
    console.log("Frames watched:", Array.from(frames.values()).map((f) => `${f.label} (${f.events} events)`).join("; "));
    return out;
  };
  window.typingStop = () => {
    clearInterval(timer);
    for (const [, f] of frames) {
      f.off.forEach((o) => o());
      if (f.observer) f.observer.disconnect();
    }
    frames.clear();
  };
  window.__typingProbe = { stop: window.typingStop };
  console.log("Typing probe on in " + frames.size + " frame(s). Type about 50 characters, then run typingReport().");
})();
