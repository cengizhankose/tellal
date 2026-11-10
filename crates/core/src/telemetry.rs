//! Per-run telemetry: run dir + `latest` symlink, `events.jsonl` (one flushed JSON object per
//! line), `metrics.jsonl` (1 Hz sampler) and the aggregated [`LiveStats`] the panel's HUD reads.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::LiveStats;

/// Shared telemetry sink of one run. All methods are thread-safe.
pub struct Telemetry {
    run: String,
    run_dir: PathBuf,
    events: Mutex<EventLog>,
    stats: Mutex<LiveStats>,
    queue_len: AtomicUsize,
}

struct EventLog {
    seq: u64,
    out: BufWriter<File>,
}

impl Telemetry {
    /// Creates `<log_root>/<run-id>/` (run-id = `YYYYMMDD-HHMMSS-<pid>`, local time) and points
    /// `<log_root>/latest` at it.
    pub fn create(log_root: &Path) -> anyhow::Result<Arc<Telemetry>> {
        let run = format!("{}-{}", local_stamp(), std::process::id());
        let run_dir = log_root.join(&run);
        fs::create_dir_all(&run_dir)?;
        let latest = log_root.join("latest");
        match fs::remove_file(&latest) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        std::os::unix::fs::symlink(&run, &latest)?;
        let out = BufWriter::new(File::create(run_dir.join("events.jsonl"))?);
        Ok(Arc::new(Telemetry {
            stats: Mutex::new(LiveStats { run_dir: run_dir.clone(), ..LiveStats::default() }),
            run,
            run_dir,
            events: Mutex::new(EventLog { seq: 0, out }),
            queue_len: AtomicUsize::new(0),
        }))
    }

    pub fn run(&self) -> &str {
        &self.run
    }

    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    /// Appends one event and returns its `seq`. Output is flushed before returning.
    pub fn event(
        &self,
        kind: &str,
        dur_ms: Option<f64>,
        ok: bool,
        input: Value,
        output: Value,
        err: Option<&str>,
    ) -> u64 {
        let ts = rfc3339_now();
        let mut log = lock(&self.events);
        log.seq += 1;
        let seq = log.seq;
        let line = json!({
            "ts": ts, "run": self.run, "seq": seq, "kind": kind, "dur_ms": dur_ms,
            "ok": ok, "in": input, "out": output, "err": err,
        });
        // A failing log disk must never take the panel down; the event is dropped instead.
        let _ = serde_json::to_writer(&mut log.out, &line)
            .map_err(std::io::Error::from)
            .and_then(|()| log.out.write_all(b"\n"))
            .and_then(|()| log.out.flush());
        seq
    }

    /// Snapshot for the HUD; `rss_mb`/`cpu_pct` come from the 1 s sampler.
    pub fn live_stats(&self) -> LiveStats {
        let mut s = lock(&self.stats).clone();
        s.queue_len = self.queue_len.load(Ordering::Relaxed);
        s
    }

    /// Mutates the aggregated live stats (last poll/tts/rtf/click-to-audio, speaking).
    pub fn update_stats(&self, f: impl FnOnce(&mut LiveStats)) {
        f(&mut lock(&self.stats));
    }

    pub fn set_queue_len(&self, n: usize) {
        self.queue_len.store(n, Ordering::Relaxed);
    }

    /// Starts the 1 Hz metrics sampler (`metrics.jsonl`) that also refreshes `rss_mb`/`cpu_pct`.
    pub fn start_sampler(self: &Arc<Self>) -> anyhow::Result<()> {
        let mut out = BufWriter::new(File::create(self.run_dir.join("metrics.jsonl"))?);
        let tel = Arc::clone(self);
        std::thread::Builder::new().name("t3p-metrics".into()).spawn(move || {
            let mut sampler = ProcSampler::new();
            let start = Instant::now();
            let mut tick: u32 = 0;
            loop {
                let (rss_mb, cpu_pct) = sampler.sample();
                let speaking = {
                    let mut s = lock(&tel.stats);
                    s.rss_mb = rss_mb;
                    s.cpu_pct = cpu_pct;
                    s.speaking.is_some()
                };
                let row = json!({
                    "ts": rfc3339_now(), "run": tel.run, "rss_mb": round2(rss_mb),
                    "cpu_pct": round2(cpu_pct), "threads": own_thread_count(),
                    "queue_len": tel.queue_len.load(Ordering::Relaxed), "speaking": speaking,
                    "children_cpu_ms_total": round2(children_cpu_ms()),
                });
                let _ = serde_json::to_writer(&mut out, &row)
                    .map_err(std::io::Error::from)
                    .and_then(|()| out.write_all(b"\n"))
                    .and_then(|()| out.flush());
                tick += 1;
                let next = start + Duration::from_secs(u64::from(tick));
                std::thread::sleep(next.saturating_duration_since(Instant::now()));
            }
        })?;
        Ok(())
    }
}

/// CPU% and RSS of this process via sysinfo (CPU% is relative to one core, like `top`).
pub struct ProcSampler {
    /// `None` if sysinfo cannot initialise on this platform (samples then read 0).
    sys: Option<sysinfo::System>,
    pid: sysinfo::Pid,
}

impl Default for ProcSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcSampler {
    pub fn new() -> Self {
        ProcSampler { sys: sysinfo::System::new().ok(), pid: sysinfo::Pid::from_u32(std::process::id()) }
    }

    /// Returns `(rss_mb, cpu_pct)`. CPU% is measured since the previous call.
    pub fn sample(&mut self) -> (f64, f64) {
        let Some(sys) = &mut self.sys else { return (0.0, 0.0) };
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[self.pid]),
            false,
            sysinfo::ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );
        match sys.process(self.pid) {
            Some(p) => (p.memory() as f64 / 1_048_576.0, f64::from(p.cpu_usage())),
            None => (0.0, 0.0),
        }
    }
}

/// Current resident set size of this process in MiB.
pub fn current_rss_mb() -> f64 {
    ProcSampler::new().sample().0
}

/// Peak RSS of this process in MiB (`getrusage(RUSAGE_SELF).ru_maxrss`).
pub fn peak_rss_mb() -> f64 {
    maxrss_to_mb(rusage(libc::RUSAGE_SELF).ru_maxrss)
}

/// Total user+sys CPU of all reaped children (`getrusage(RUSAGE_CHILDREN)`), in ms.
pub fn children_cpu_ms() -> f64 {
    rusage_cpu_ms(&rusage(libc::RUSAGE_CHILDREN))
}

pub(crate) fn rusage_cpu_ms(ru: &libc::rusage) -> f64 {
    let tv = |t: libc::timeval| t.tv_sec as f64 * 1000.0 + t.tv_usec as f64 / 1000.0;
    tv(ru.ru_utime) + tv(ru.ru_stime)
}

/// `ru_maxrss` is bytes on macOS and KiB on Linux.
pub(crate) fn maxrss_to_mb(maxrss: libc::c_long) -> f64 {
    if cfg!(target_os = "macos") {
        maxrss as f64 / 1_048_576.0
    } else {
        maxrss as f64 / 1024.0
    }
}

fn rusage(who: libc::c_int) -> libc::rusage {
    // SAFETY: getrusage only writes into the provided, properly sized struct.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(who, &mut ru);
        ru
    }
}

#[cfg(target_os = "macos")]
fn own_thread_count() -> i32 {
    // SAFETY: proc_pidinfo writes at most `size` bytes into `info`.
    unsafe {
        let mut info: libc::proc_taskinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
        let n = libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTASKINFO,
            0,
            (&raw mut info).cast(),
            size,
        );
        if n == size { info.pti_threadnum } else { 0 }
    }
}

#[cfg(not(target_os = "macos"))]
fn own_thread_count() -> i32 {
    fs::read_dir("/proc/self/task").map(|d| d.count() as i32).unwrap_or(0)
}

pub fn ms(d: Duration) -> f64 {
    round2(d.as_secs_f64() * 1000.0)
}

pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` for now.
pub fn rfc3339_now() -> String {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs() as i64;
    let (y, mo, day) = civil_from_days(secs.div_euclid(86_400));
    let sod = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{mo:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        sod / 3600,
        sod / 60 % 60,
        sod % 60,
        d.subsec_millis()
    )
}

/// Parses `YYYY-MM-DDTHH:MM:SS[.fff](Z|±HH:MM)` into Unix milliseconds.
pub fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse().ok() };
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' {
        return None;
    }
    let days = days_from_civil(num(0..4)?, num(5..7)?, num(8..10)?);
    let mut ms = ((days * 24 + num(11..13)?) * 60 + num(14..16)?) * 60_000 + num(17..19)? * 1000;
    let mut i = 19;
    if b.get(i) == Some(&b'.') {
        let start = i + 1;
        i = start;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let frac = &s[start..i.min(start + 3)];
        ms += frac.parse::<i64>().ok()? * 10_i64.pow(3 - frac.len() as u32);
    }
    match b.get(i) {
        Some(b'Z') => {}
        Some(&sign @ (b'+' | b'-')) => {
            let off = (num(i + 1..i + 3)? * 60 + num(i + 4..i + 6)?) * 60_000;
            ms -= if sign == b'+' { off } else { -off };
        }
        _ => return None,
    }
    Some(ms)
}

pub fn now_unix_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn local_stamp() -> String {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as libc::time_t;
    // SAFETY: localtime_r fills the provided tm from a valid time_t.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

// Howard Hinnant's civil date algorithms.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
