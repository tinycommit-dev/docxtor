//! Worker processes for harnesses that must survive the things they are made to find: a panic, a stack overflow, a
//! hang, a failed allocation. The same machinery as `tests/mutate_open.rs` (which keeps its own copy for now; it can
//! switch to this one), in a form two harnesses can share:
//!
//! - `Counting`: an allocator that counts live bytes and refuses a request past a limit (switched on only around the
//!   engine call). A binary declares `#[global_allocator] static ALLOC: proc::Counting = proc::Counting;`.
//! - `serve`: the worker side. The test binary runs itself again with an environment variable; the worker reads one
//!   case (a JSON line) at a time from stdin and runs it in a thread with a small stack and the memory limit.
//! - `Proc`: the supervisor side. It starts a worker, sends a case, watches the clock, and when the worker dies says
//!   from its last words why (stack overflow, out of memory) and in which stage (`stage` prints `@@stage <name>`).
//!
//! Included with `#[path = "common/proc.rs"] mod proc;`.

#![allow(dead_code)]

use serde_json::{json, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// The counting allocator
// ---------------------------------------------------------------------------

pub struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);
// Exact totals for the performance harness (tests/perf_counts.rs): how many allocations were made and how many bytes
// were asked for since the last `reset_totals`. They never affect the limit.
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);

/// Allocations and bytes requested since `reset_totals`, and the most bytes live at once above where it was called.
pub fn totals() -> (usize, usize, usize) {
    (ALLOCS.load(Relaxed), TOTAL.load(Relaxed), PEAK.load(Relaxed).saturating_sub(BASE.load(Relaxed)))
}
static BASE: AtomicUsize = AtomicUsize::new(0);
pub fn reset_totals() {
    ALLOCS.store(0, Relaxed);
    TOTAL.store(0, Relaxed);
    let live = LIVE.load(Relaxed);
    BASE.store(live, Relaxed);
    PEAK.store(live, Relaxed);
}
pub fn live_bytes() -> usize {
    LIVE.load(Relaxed)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        TOTAL.fetch_add(l.size(), Relaxed);
        let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
        if n > LIMIT.load(Relaxed) {
            LIVE.fetch_sub(l.size(), Relaxed);
            return std::ptr::null_mut();
        }
        PEAK.fetch_max(n, Relaxed);
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        TOTAL.fetch_add(l.size(), Relaxed);
        let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
        if n > LIMIT.load(Relaxed) {
            LIVE.fetch_sub(l.size(), Relaxed);
            return std::ptr::null_mut();
        }
        PEAK.fetch_max(n, Relaxed);
        System.alloc_zeroed(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        LIVE.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        TOTAL.fetch_add(new.saturating_sub(l.size()), Relaxed);
        if new > l.size() {
            let d = new - l.size();
            let n = LIVE.fetch_add(d, Relaxed) + d;
            if n > LIMIT.load(Relaxed) {
                LIVE.fetch_sub(d, Relaxed);
                return std::ptr::null_mut();
            }
            PEAK.fetch_max(n, Relaxed);
        } else {
            LIVE.fetch_sub(l.size() - new, Relaxed);
        }
        let q = System.realloc(p, l, new);
        if q.is_null() {
            if new > l.size() {
                LIVE.fetch_sub(new - l.size(), Relaxed);
            } else {
                LIVE.fetch_add(l.size() - new, Relaxed);
            }
        }
        q
    }
}

// ---------------------------------------------------------------------------
// The worker side
// ---------------------------------------------------------------------------

static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

pub fn say(tag: &str, v: &Value) {
    println!("@@{tag} {v}");
    let _ = std::io::stdout().flush();
}

/// Tell the supervisor which stage the case is in, so a crash can be placed.
pub fn stage(s: &str) {
    println!("@@stage {s}");
    let _ = std::io::stdout().flush();
}

/// The worker loop. Call it from a test that returns at once unless `env` is `1`. `handler` gets a case and returns
/// what to report; it runs in a thread with `stack` bytes of stack and at most `mem_mb` of memory.
pub fn serve(env: &str, stack: usize, mem_mb: usize, handler: fn(&Value) -> Value) {
    if std::env::var(env).as_deref() != Ok("1") {
        return;
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("docxtor-case") {
            let msg = info.payload().downcast_ref::<String>().cloned().or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            let at = info.location().map(|l| l.file().to_string()).unwrap_or_default();
            *LAST_PANIC.lock().unwrap() = format!("{} at {at}", msg.lines().next().unwrap_or(""));
        } else {
            prev(info);
        }
    }));
    println!("\n@@ready");
    let _ = std::io::stdout().flush();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let case: Value = serde_json::from_str(&line).expect("a case");
        one_case(&case, stack, mem_mb << 20, handler);
    }
}

fn one_case(case: &Value, stack: usize, mem: usize, handler: fn(&Value) -> Value) {
    println!("@@start");
    let _ = std::io::stdout().flush();
    let base = LIVE.load(Relaxed);
    PEAK.store(base, Relaxed);
    LIMIT.store(base.saturating_add(mem), Relaxed);
    let t0 = Instant::now();
    let c = case.clone();
    let h = std::thread::Builder::new().name("docxtor-case".into()).stack_size(stack).spawn(move || handler(&c)).unwrap();
    // The supervisor ends the worker at the time limit; here we only wait.
    let got = h.join();
    let ms = t0.elapsed().as_millis() as u64;
    let peak = PEAK.load(Relaxed).saturating_sub(base);
    LIMIT.store(usize::MAX, Relaxed);
    match got {
        Ok(v) => say("result", &json!({"value": v, "ms": ms, "peak_mb": peak >> 20})),
        Err(_) => {
            let p = std::mem::take(&mut *LAST_PANIC.lock().unwrap());
            say("result", &json!({"panic": p, "ms": ms}));
        }
    }
}

// ---------------------------------------------------------------------------
// The supervisor side
// ---------------------------------------------------------------------------

enum Msg {
    Line(String),
    Eof,
}

/// How a case ended.
#[derive(Debug, Clone)]
pub enum Ended {
    /// The handler's answer, with the time and the peak memory it took.
    Done { value: Value, ms: u64, peak_mb: u64 },
    /// A panic, with where it came from and the stage it was in.
    Panic { text: String, stage: String },
    /// No answer inside the time limit.
    Hang { ms: u64, stage: String },
    /// The process ended: `kind` is `stack`, `memory` or `abort`.
    Died { kind: &'static str, detail: String, stage: String },
}

pub struct Proc {
    /// The environment variable that makes the test binary a worker, and the name of the test that serves.
    env: &'static str,
    test: &'static str,
    mem_mb: usize,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    rx: Option<Receiver<Msg>>,
    err_path: std::path::PathBuf,
}

impl Proc {
    pub fn new(env: &'static str, test: &'static str, mem_mb: usize) -> Proc {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Relaxed);
        Proc { env, test, mem_mb, child: None, stdin: None, rx: None, err_path: std::env::temp_dir().join(format!("docxtor_{}_{}_{id}.err", env.to_lowercase(), std::process::id())) }
    }

    pub fn kill(&mut self) {
        self.stdin = None;
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.rx = None;
    }

    fn spawn(&mut self) {
        self.kill();
        let err = std::fs::File::create(&self.err_path).expect("the worker's error file");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", self.test, "--nocapture", "--test-threads=1"])
            .env(self.env, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(err)
            .spawn()
            .expect("start a worker");
        let out = child.stdout.take().unwrap();
        self.stdin = child.stdin.take();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                if tx.send(Msg::Line(line)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Msg::Eof);
        });
        self.child = Some(child);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Msg::Line(l)) if l == "@@ready" => break,
                Ok(Msg::Line(_)) => {}
                _ => panic!("the worker did not start; see {}", self.err_path.display()),
            }
        }
        self.rx = Some(rx);
    }

    fn stderr_text(&self) -> String {
        std::fs::read_to_string(&self.err_path).unwrap_or_default()
    }

    /// Run one case; start the worker again when it died or was ended.
    pub fn run(&mut self, case: &Value, limit: Duration) -> Ended {
        if self.child.is_none() {
            self.spawn();
        }
        let line = format!("{case}\n");
        if self.stdin.as_mut().is_none_or(|s| s.write_all(line.as_bytes()).and_then(|_| s.flush()).is_err()) {
            self.spawn();
            let s = self.stdin.as_mut().unwrap();
            s.write_all(line.as_bytes()).unwrap();
            let _ = s.flush();
        }
        let mut stage = "starting".to_string();
        let mut started: Option<Instant> = None;
        let start_deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let deadline = started.map(|s| s + limit).unwrap_or(start_deadline);
            let msg = self.rx.as_ref().unwrap().recv_timeout(deadline.saturating_duration_since(Instant::now()));
            match msg {
                Ok(Msg::Line(l)) => {
                    if l == "@@start" {
                        started = Some(Instant::now());
                        stage = "open".into();
                    } else if let Some(s) = l.strip_prefix("@@stage ") {
                        stage = s.to_string();
                    } else if let Some(j) = l.strip_prefix("@@result ") {
                        let r: Value = serde_json::from_str(j).unwrap_or(Value::Null);
                        if let Some(p) = r["panic"].as_str() {
                            return Ended::Panic { text: p.to_string(), stage };
                        }
                        return Ended::Done { value: r["value"].clone(), ms: r["ms"].as_u64().unwrap_or(0), peak_mb: r["peak_mb"].as_u64().unwrap_or(0) };
                    }
                }
                Ok(Msg::Eof) | Err(RecvTimeoutError::Disconnected) => return self.died(&stage),
                Err(RecvTimeoutError::Timeout) => {
                    self.kill();
                    return Ended::Hang { ms: limit.as_millis() as u64, stage };
                }
            }
        }
    }

    /// The worker ended by itself: say why from what it left on stderr.
    fn died(&mut self, stage: &str) -> Ended {
        let status = self.child.as_mut().and_then(|c| c.wait().ok());
        let err = self.stderr_text();
        self.kill();
        let tail: String = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(120).collect();
        let stage = stage.to_string();
        if err.contains("overflowed its stack") {
            Ended::Died { kind: "stack", detail: "stack overflow".into(), stage }
        } else if err.contains("memory allocation of") {
            Ended::Died { kind: "memory", detail: format!("memory past {} MB", self.mem_mb), stage }
        } else {
            #[cfg(unix)]
            let how = {
                use std::os::unix::process::ExitStatusExt;
                status.map(|s| s.signal().map(|n| format!("signal {n}")).unwrap_or_else(|| format!("{s}"))).unwrap_or_default()
            };
            #[cfg(not(unix))]
            let how = format!("{status:?}");
            Ended::Died { kind: "abort", detail: format!("the process ended ({how}): {tail}"), stage }
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_file(&self.err_path);
    }
}
