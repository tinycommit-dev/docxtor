// Injected into the page before the app runs (see perf/lib/browser.mts). Plain JavaScript, no imports.
// One listener per event type per measurement: the page's own handlers are never wrapped or duplicated.
(() => {
  if (window.__perf) return;
  globalThis.__DOCXTOR_PERF__ = { calls: {} };
  // The engine's raw exports, so a planted regression can be switched on from outside (see perf/plants.mts).
  window.__wasmExports = [];
  const note = (r) => {
    const i = (r && r.instance) || r;
    if (i && i.exports) window.__wasmExports.push(i.exports);
    return r;
  };
  const inst = WebAssembly.instantiate.bind(WebAssembly);
  WebAssembly.instantiate = (...a) => inst(...a).then(note);
  if (WebAssembly.instantiateStreaming) {
    const stream = WebAssembly.instantiateStreaming.bind(WebAssembly);
    WebAssembly.instantiateStreaming = (...a) => stream(...a).then(note);
  }
  const meter = globalThis.__DOCXTOR_PERF__;
  const P = (window.__perf = { events: [], timing: [], long: [], frames: [], added: 0, startup: { added: 0 } });
  const engineMs = () => (meter.calls.apply?.ms ?? 0) + (meter.calls.renderChanged?.ms ?? 0);

  // How much the DOM grew while the app started (the number of nodes ever added), to prove a change to startup took effect.
  try {
    new MutationObserver((list) => {
      for (const m of list) P.added += m.addedNodes.length;
    }).observe(document, { childList: true, subtree: true });
  } catch {}

  // Event Timing (the standard measure). Entries shorter than 16 ms are never delivered, so this only sees the slow events:
  // the count per key says how much it covers.
  try {
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) P.timing.push({ name: e.name, start: e.startTime, delay: e.processingStart - e.startTime, proc: e.processingEnd - e.processingStart, dur: e.duration });
    }).observe({ type: "event", durationThreshold: 16, buffered: true });
  } catch {}
  try {
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) P.long.push({ start: e.startTime, dur: e.duration });
    }).observe({ type: "longtask", buffered: true });
  } catch {}

  // Exact timing from our own capture-phase listeners (they run before the page's): the input delay is the time from the
  // event's own timestamp to our first look at it; the processing is up to a message posted from the listener, which
  // runs after the event's handlers have finished (and what they queued as microtasks).
  // (A first version ended the processing with a message posted from the listener; Chrome may run that after the next
  // frame, which put up to 16 ms of waiting into the number. A listener on the way back up runs right after the page's own.)
  const open = new Map();
  for (const type of ["keydown", "beforeinput", "input", "keyup"]) {
    window.addEventListener(
      type,
      (e) => {
        const t0 = performance.now();
        const r = { type, delay: t0 - e.timeStamp, t0, ts: e.timeStamp, end: t0 };
        open.set(e, r);
        P.events.push(r);
      },
      true,
    );
    window.addEventListener(type, (e) => {
      const r = open.get(e);
      if (r) {
        r.end = performance.now();
        open.delete(e);
      }
    });
  }
  // Resolves after the next frame has been produced (a rAF callback, then a task after it).
  P.settle = () =>
    new Promise((done) => {
      requestAnimationFrame((ts) => {
        const c = new MessageChannel();
        c.port1.onmessage = () => {
          P.frames.push({ ts, done: performance.now() });
          done(performance.now());
        };
        c.port2.postMessage(0);
      });
    });
  P.take = () => {
    const out = { events: P.events, timing: P.timing, long: P.long, frames: P.frames, engineMs: engineMs() };
    P.events = [];
    P.timing = [];
    P.long = [];
    P.frames = [];
    return out;
  };
  // The longest stall: the longest gap between two turns of a loop that does nothing but ask for its next turn. Whatever else runs
  // in the page (a pagination, a spelling pass) shows as a gap at least as long as it is.
  {
    const ch = new MessageChannel();
    let running = false;
    let max = 0;
    let last = 0;
    ch.port1.onmessage = () => {
      if (!running) return;
      const now = performance.now();
      max = Math.max(max, now - last);
      last = now;
      ch.port2.postMessage(0);
    };
    P.lagStart = () => {
      max = 0;
      last = performance.now();
      running = true;
      ch.port2.postMessage(0);
    };
    P.lagStop = () => {
      running = false;
      return max;
    };
  }
  // Reads of where things are on the page (rectangles and offsets), counted while asked to: switched on around a scenario so that the
  // wrappers cost nothing the rest of the time. A key whose work reads the rectangles of the whole document shows as a count that
  // grows with the length; leaving far blocks out of the layout hides them, which makes such a read return zeros.
  {
    const proto = [
      [Element.prototype, "getBoundingClientRect", "rects"],
      [Element.prototype, "getClientRects", "rects"],
      [Range.prototype, "getBoundingClientRect", "rects"],
      [Range.prototype, "getClientRects", "rects"],
    ];
    const getters = ["offsetTop", "offsetHeight", "offsetLeft", "offsetWidth"].map((n) => [HTMLElement.prototype, n]);
    let saved = null;
    let count = { rects: 0, offsets: 0 };
    // The top-level blocks of the text whose rectangles were read: a read of every block would make this grow with the document.
    let blocks = new Set();
    const noteBlock = (el) => {
      const e = el instanceof Element ? el : el && el.startContainer ? el.startContainer.parentElement : null;
      const b = e && e.closest ? e.closest(".flow > *") : null;
      if (b) blocks.add(b);
    };
    P.rectsStart = () => {
      if (saved) return;
      count = { rects: 0, offsets: 0 };
      blocks = new Set();
      saved = [];
      for (const [obj, name, kind] of proto) {
        const orig = obj[name];
        saved.push([obj, name, Object.getOwnPropertyDescriptor(obj, name)]);
        obj[name] = function (...a) {
          count[kind]++;
          noteBlock(this);
          return orig.apply(this, a);
        };
      }
      for (const [obj, name] of getters) {
        const d = Object.getOwnPropertyDescriptor(obj, name);
        if (!d || !d.get) continue;
        saved.push([obj, name, d]);
        Object.defineProperty(obj, name, {
          ...d,
          get() {
            count.offsets++;
            noteBlock(this);
            return d.get.call(this);
          },
        });
      }
    };
    P.rectsStop = () => {
      if (saved) for (const [obj, name, d] of saved) Object.defineProperty(obj, name, d);
      saved = null;
      return { ...count, blocks: blocks.size };
    };
  }
  P.engineMs = engineMs;
  P.meter = () => JSON.parse(JSON.stringify(meter.calls));
})();
