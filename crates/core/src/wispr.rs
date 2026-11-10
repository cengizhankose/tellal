//! Wispr Flow dictation bridge: hands-free start/stop through Wispr's URL scheme
//! (`/usr/bin/open -g wispr-flow://…`, nothing activated) and a read-only watch of Wispr's
//! History DB (through `sqlite3 -readonly`) for the transcript row of that dictation.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use serde_json::{Value, json};

use crate::CoreEvent;
use crate::t3::{argv_json, run_cmd, sql_str, sqlite_json};
use crate::telemetry::{Telemetry, lock, ms, now_unix_ms, parse_rfc3339_ms, rfc3339_from_ms, round2};

const OPEN: &str = "/usr/bin/open";
const START_URL: &str = "wispr-flow://start-hands-free";
const STOP_URL: &str = "wispr-flow://stop-hands-free";
/// History poll period while a dictation is active.
const POLL: Duration = Duration::from_millis(250);
/// A dictation without a final History row this long after its start fails.
const TIMEOUT_MS: i64 = 5 * 60 * 1000;

/// The one dictation in progress.
struct Session {
    /// Distinguishes sessions so a finishing watcher never clears a newer one.
    id: u64,
    thread: String,
    /// Set by `stop()`; stays `None` when the user ended dictation in Wispr itself.
    stop_ms: Option<i64>,
    /// Set by `cancel()`: the transcript is suppressed and a new dictation may start.
    cancelled: bool,
}

pub(crate) struct Wispr {
    tel: Arc<Telemetry>,
    db: PathBuf,
    events: Sender<CoreEvent>,
    session: Arc<Mutex<Option<Session>>>,
    next_id: AtomicU64,
}

impl Wispr {
    pub(crate) fn new(tel: Arc<Telemetry>, db: PathBuf, events: Sender<CoreEvent>) -> Wispr {
        Wispr { tel, db, events, session: Arc::new(Mutex::new(None)), next_id: AtomicU64::new(1) }
    }

    /// Starts hands-free dictation for a reply to `thread_id` and watches History for its row
    /// (→ `DictationTranscript` / `DictationFailed`). One dictation at a time.
    pub(crate) fn start(&self, thread_id: &str) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        // Noted before `open` runs, so Wispr's row is always newer.
        let start_ms = now_unix_ms();
        let active = {
            let mut cur = lock(&self.session);
            let active = cur.as_ref().filter(|s| !s.cancelled).map(|s| s.thread.clone());
            if active.is_none() {
                *cur = Some(Session { id, thread: thread_id.to_owned(), stop_ms: None, cancelled: false });
            }
            active
        };
        match active {
            Some(t) if t == thread_id => {
                let out = json!({"ignored": "dictation already active for this thread"});
                self.tel.event("reply.dictation_start", None, true, open_input(thread_id, START_URL), out, None);
                return;
            }
            Some(t) => {
                let error = format!("another dictation is active ({t})");
                self.fail_start(thread_id, error);
                return;
            }
            None => {}
        }
        let watch = Watch {
            tel: Arc::clone(&self.tel),
            db: self.db.clone(),
            events: self.events.clone(),
            session: Arc::clone(&self.session),
            id,
            thread: thread_id.to_owned(),
            start_ms,
        };
        if let Err(e) = std::thread::Builder::new().name("t3p-wispr".into()).spawn(move || watch.run()) {
            {
                let mut cur = lock(&self.session);
                if cur.as_ref().is_some_and(|s| s.id == id) {
                    *cur = None;
                }
            }
            self.fail_start(thread_id, format!("spawn t3p-wispr: {e}"));
        }
    }

    /// Ends hands-free dictation; the watcher then delivers the transcript.
    pub(crate) fn stop(&self) {
        self.end("reply.dictation_stop", false);
    }

    /// Ends hands-free dictation and suppresses its transcript.
    pub(crate) fn cancel(&self) {
        self.end("reply.dictation_cancel", true);
    }

    fn fail_start(&self, thread_id: &str, error: String) {
        self.tel.event("reply.dictation_start", None, false, open_input(thread_id, START_URL), Value::Null, Some(&error));
        let _ = self.events.send(CoreEvent::DictationFailed { thread_id: thread_id.to_owned(), error });
    }

    /// Marks the active session stopped (first stop time wins) or cancelled, then opens the stop
    /// URL on a short-lived thread (callers run on the UI thread).
    fn end(&self, kind: &'static str, cancel: bool) {
        let thread = {
            let mut cur = lock(&self.session);
            cur.as_mut().filter(|s| !s.cancelled).map(|s| {
                if cancel {
                    s.cancelled = true;
                } else {
                    s.stop_ms.get_or_insert_with(now_unix_ms);
                }
                s.thread.clone()
            })
        };
        let Some(thread) = thread else {
            self.tel.event(kind, None, false, json!({"url": STOP_URL}), Value::Null, Some("no active dictation"));
            return;
        };
        let tel = Arc::clone(&self.tel);
        let input_thread = thread.clone();
        if let Err(e) = std::thread::Builder::new().name("t3p-wispr-open".into()).spawn(move || {
            log_open(&tel, kind, &thread, STOP_URL);
        }) {
            let err = format!("spawn t3p-wispr-open: {e}");
            self.tel.event(kind, None, false, open_input(&input_thread, STOP_URL), Value::Null, Some(&err));
        }
    }
}

/// Watcher of one session: opens the start URL, then polls History until a final row, a
/// cancel, or [`TIMEOUT_MS`].
struct Watch {
    tel: Arc<Telemetry>,
    db: PathBuf,
    events: Sender<CoreEvent>,
    session: Arc<Mutex<Option<Session>>>,
    id: u64,
    thread: String,
    start_ms: i64,
}

/// What a finished watch saw.
#[derive(Default)]
struct Outcome {
    polls: u64,
    query_errors: u64,
    last_query_error: Option<String>,
    /// The final row, or the newest in-flight one when none became final.
    row: Option<Value>,
    row_ms: Option<i64>,
    /// When the row was first seen final (unix ms).
    seen_ms: Option<i64>,
}

impl Watch {
    fn run(self) {
        let mut o = Outcome::default();
        if let Some(err) = log_open(&self.tel, "reply.dictation_start", &self.thread, START_URL) {
            self.finish(o, Err(format!("wispr start failed: {err}")));
            return;
        }
        let sql = format!(
            "select transcriptEntityId, asrText, formattedText, timestamp, status, app, e2eLatency, duration, \
             numWords, language from History where timestamp > {} order by timestamp desc limit 1;",
            sql_str(&wispr_ts(self.start_ms))
        );
        // Polls are 4/s, so only their counts reach the final `wispr.history` event.
        let result = loop {
            if self.cancelled() {
                break Err("cancelled".to_owned());
            }
            if now_unix_ms() - self.start_ms > TIMEOUT_MS {
                break Err("wispr: no transcript within 5 min".to_owned());
            }
            o.polls += 1;
            match sqlite_json::<Value>(&self.db, &sql).1 {
                Err(e) => {
                    o.query_errors += 1;
                    o.last_query_error = Some(format!("{e:#}"));
                }
                Ok((_, rows)) => {
                    if let Some(row) = rows.into_iter().next() {
                        match row["timestamp"].as_str().and_then(wispr_ts_ms) {
                            None => {
                                o.query_errors += 1;
                                o.last_query_error = Some(format!("unparseable History.timestamp {}", row["timestamp"]));
                            }
                            // The SQL text comparison already filters; this guards a format surprise.
                            Some(row_ms) if row_ms > self.start_ms => {
                                let status = row["status"].as_str().unwrap_or_default();
                                // In flight: Wispr updates the row in place.
                                let decided = (!status.is_empty() && status != "processing")
                                    .then(|| transcript(&row).ok_or_else(|| format!("wispr: {status} (no text)")));
                                o.row_ms = Some(row_ms);
                                o.row = Some(row);
                                if let Some(r) = decided {
                                    o.seen_ms = Some(now_unix_ms());
                                    break r;
                                }
                            }
                            Some(_) => {}
                        }
                    }
                }
            }
            std::thread::sleep(POLL);
        };
        self.finish(o, result);
    }

    /// True once this session was cancelled or replaced.
    fn cancelled(&self) -> bool {
        lock(&self.session).as_ref().is_none_or(|s| s.id != self.id || s.cancelled)
    }

    /// Ends the session (unless a newer one replaced it), delivers the result unless cancelled,
    /// logs `wispr.history` and updates `last_dictation_ms`.
    fn finish(self, o: Outcome, result: Result<String, String>) {
        let (stop_ms, cancelled) = {
            let mut cur = lock(&self.session);
            let mine = cur.as_ref().filter(|s| s.id == self.id).map(|s| (s.stop_ms, s.cancelled));
            if mine.is_some() {
                *cur = None;
            }
            mine.unwrap_or((None, true))
        };
        if !cancelled {
            let thread_id = self.thread.clone();
            let ev = match &result {
                Ok(text) => CoreEvent::DictationTranscript { thread_id, text: text.clone() },
                Err(error) => CoreEvent::DictationFailed { thread_id, error: error.clone() },
            };
            let _ = self.events.send(ev);
        }
        let delivered = !cancelled && result.is_ok();
        let latency_from_stop = o.seen_ms.zip(stop_ms).map(|(seen, stop)| seen - stop);
        let null = Value::Null;
        let row = o.row.as_ref().unwrap_or(&null);
        if delivered {
            // Wispr's own stop estimate: row timestamp + recording duration.
            let wispr_stop = || {
                let (seen, row_ms) = o.seen_ms.zip(o.row_ms)?;
                let d = row["duration"].as_f64().filter(|d| *d >= 0.0)?;
                let v = seen as f64 - (row_ms as f64 + d * 1000.0);
                (v >= 0.0).then_some(v)
            };
            if let Some(v) = latency_from_stop.map(|l| l as f64).or_else(wispr_stop) {
                self.tel.update_stats(|s| s.last_dictation_ms = Some(round2(v)));
            }
        }
        let input = json!({
            "thread": self.thread, "db": self.db.display().to_string(), "start": rfc3339_from_ms(self.start_ms),
            "polls": o.polls, "query_errors": o.query_errors, "cancelled": cancelled,
        });
        let out = json!({
            "transcript_id": row["transcriptEntityId"], "text": result.as_ref().ok(),
            "asr_text": row["asrText"], "formatted_text": row["formattedText"], "timestamp": row["timestamp"],
            "status": row["status"], "app": row["app"], "e2eLatency": row["e2eLatency"],
            "duration": row["duration"], "numWords": row["numWords"], "language": row["language"],
            "latency_from_stop_ms": latency_from_stop,
            "latency_from_start_ms": o.seen_ms.map(|s| s - self.start_ms),
            "row_ts_from_start_ms": o.row_ms.map(|r| r - self.start_ms),
            "last_query_error": o.last_query_error,
        });
        let err = match (&result, cancelled) {
            (_, true) => Some("cancelled"),
            (Err(e), false) => Some(e.as_str()),
            (Ok(_), false) => None,
        };
        let dur = (now_unix_ms() - self.start_ms) as f64;
        self.tel.event("wispr.history", Some(dur), delivered, input, out, err);
    }
}

fn open_argv(url: &str) -> [&str; 3] {
    [OPEN, "-g", url]
}

fn open_input(thread: &str, url: &str) -> Value {
    json!({"thread": thread, "argv": argv_json(&open_argv(url)), "url": url})
}

/// Runs `/usr/bin/open -g <url>` and logs it as `kind`; returns the error, if any.
fn log_open(tel: &Telemetry, kind: &str, thread: &str, url: &str) -> Option<String> {
    let t = Instant::now();
    let (out, err) = match run_cmd(&open_argv(url), None) {
        Ok(o) => {
            let err = (!o.success()).then(|| o.failure());
            (o.telemetry(), err)
        }
        Err(e) => (Value::Null, Some(format!("spawn: {e}"))),
    };
    tel.event(kind, Some(ms(t.elapsed())), err.is_none(), open_input(thread, url), out, err.as_deref());
    err
}

/// The transcript of a final row: `formattedText`, else `asrText` (trimmed, non-empty).
fn transcript(row: &Value) -> Option<String> {
    ["formattedText", "asrText"]
        .into_iter()
        .filter_map(|k| row[k].as_str())
        .map(str::trim)
        .find(|t| !t.is_empty())
        .map(str::to_owned)
}

/// Unix ms → Wispr's History timestamp text `YYYY-MM-DD HH:MM:SS.mmm +00:00` (UTC).
fn wispr_ts(unix_ms: i64) -> String {
    let r = rfc3339_from_ms(unix_ms);
    format!("{} {} +00:00", &r[..10], &r[11..23])
}

/// Wispr's `YYYY-MM-DD HH:MM:SS.mmm +00:00` → unix ms.
fn wispr_ts_ms(s: &str) -> Option<i64> {
    let mut t = s.replace(" +", "+").replace(" -", "-");
    if t.as_bytes().get(10) == Some(&b' ') {
        t.replace_range(10..11, "T");
    }
    parse_rfc3339_ms(&t)
}
