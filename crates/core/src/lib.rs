//! t3-panel core: T3 thread source, live message tail, per-thread speech channels, local TTS,
//! Wispr Flow dictation bridge and telemetry.
//!
//! PUBLIC API CONTRACT (the GPUI app depends on exactly these items; keep signatures stable).
//! The modules below are public for the `t3p-bench` / `t3p-report` tools.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

pub use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use serde_json::{Value, json};

pub mod extract;
mod live;
mod speech;
pub mod t3;
pub mod telemetry;
mod wispr;
pub mod tts {
    //! TTS engine and audio helpers (used by `t3p-bench`).
    pub use crate::speech::{Synth, TtsEngine, open_output, samples_source, write_wav};
}

use telemetry::Telemetry;

/// Runtime configuration. `Config::from_env()` fills defaults and applies `T3P_*` overrides.
#[derive(Clone, Debug)]
pub struct Config {
    /// T3 CLI launcher. Default `~/.t3/bin/t3`. Env `T3P_T3_BIN`.
    pub t3_bin: PathBuf,
    /// Project ids or paths to watch. `None` = discover from T3's state DB (read-only).
    /// Env `T3P_PROJECTS` (comma separated).
    pub projects: Option<Vec<String>>,
    /// Thread list poll interval. Default 5s. Env `T3P_POLL_MS`.
    pub poll_interval: Duration,
    /// Piper/sherpa model directory. Default
    /// `~/Library/Application Support/t3-panel/models/vits-piper-tr_TR-dfki-medium`. Env `T3P_MODEL_DIR`.
    pub model_dir: PathBuf,
    /// Root for per-run telemetry dirs. Default `~/Library/Logs/t3-panel`. Env `T3P_LOG_ROOT`.
    pub log_root: PathBuf,
    /// Save every synthesized utterance as WAV in the run dir. Default true. Env `T3P_SAVE_AUDIO=0` disables.
    pub save_audio: bool,
    /// Settled threads updated within this window are still listed. Default 6h.
    pub recent_window: Duration,
    /// Max rows returned by `Core::threads`. Default 20.
    pub max_threads: usize,
    /// Live-tail interval (new assistant messages + waiting requests of listed threads, one
    /// read-only query). Default 1s. Env `T3P_LIVE_POLL_MS`.
    pub live_poll_interval: Duration,
    /// Prepended to every reply sent from the panel so the agent uses the `sesli` skill.
    /// Default `"$sesli "`. Env `T3P_REPLY_PREFIX` (empty string disables).
    pub reply_prefix: String,
    /// Wispr Flow History DB (read-only). Default
    /// `~/Library/Application Support/Wispr Flow/flow.sqlite`. Env `T3P_WISPR_DB`.
    pub wispr_db: PathBuf,
}

impl Config {
    pub fn from_env() -> Config {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Config {
            t3_bin: var("T3P_T3_BIN").map_or_else(|| home.join(".t3/bin/t3"), PathBuf::from),
            projects: var("T3P_PROJECTS").map(|v| {
                v.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_owned).collect()
            }),
            poll_interval: var("T3P_POLL_MS")
                .and_then(|v| v.trim().parse().ok())
                .map_or(Duration::from_secs(5), Duration::from_millis),
            model_dir: var("T3P_MODEL_DIR").map_or_else(
                || home.join("Library/Application Support/t3-panel/models/vits-piper-tr_TR-dfki-medium"),
                PathBuf::from,
            ),
            log_root: var("T3P_LOG_ROOT").map_or_else(|| home.join("Library/Logs/t3-panel"), PathBuf::from),
            save_audio: var("T3P_SAVE_AUDIO").is_none_or(|v| v.trim() != "0"),
            recent_window: Duration::from_secs(6 * 3600),
            max_threads: 20,
            live_poll_interval: var("T3P_LIVE_POLL_MS")
                .and_then(|v| v.trim().parse().ok())
                .map_or(Duration::from_secs(1), Duration::from_millis),
            reply_prefix: std::env::var("T3P_REPLY_PREFIX").unwrap_or_else(|_| "$sesli ".to_owned()),
            wispr_db: var("T3P_WISPR_DB")
                .map_or_else(|| home.join("Library/Application Support/Wispr Flow/flow.sqlite"), PathBuf::from),
        }
    }

    /// The config as logged in `app.start`.
    pub fn to_json(&self) -> Value {
        json!({
            "t3_bin": self.t3_bin, "projects": self.projects,
            "poll_interval_ms": self.poll_interval.as_millis() as u64, "model_dir": self.model_dir,
            "log_root": self.log_root, "save_audio": self.save_audio,
            "recent_window_s": self.recent_window.as_secs(), "max_threads": self.max_threads,
            "live_poll_interval_ms": self.live_poll_interval.as_millis() as u64,
            "reply_prefix": self.reply_prefix, "wispr_db": self.wispr_db,
        })
    }
}

/// One row in the panel. Sorted newest `updated_at` first, unsettled before settled.
#[derive(Clone, Debug)]
pub struct ThreadView {
    pub id: String,
    /// Project display name (T3 project title, else workspace dir name, else id).
    pub project: String,
    pub title: String,
    /// Raw T3 status. Observed values: running, idle, completed, interrupted, cancelled.
    pub status: String,
    pub settled: bool,
    /// RFC3339 from T3.
    pub updated_at: String,
    /// `https://app.t3.codes/threads/<env>/<thread>`; opening it focuses the thread in T3.
    pub link: String,
    pub provider: String,
    pub model: String,
}

/// Speech state of one thread ("channel").
///
/// Radio: a muted channel is never spoken; assistant messages that arrive while muted only bump
/// `unheard`. Unmuting (`Core::set_listening(id, true)`) speaks one catch-up line (the latest
/// `🔊` line, else the latest message's first sentence, else a status line) and from then on
/// speaks every new assistant message live.
/// Video: a paused channel holds its current utterance mid-sentence and keeps queueing new ones;
/// resuming continues exactly where it stopped. Nothing is dropped unless the user skips,
/// jumps to live or mutes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelView {
    pub listening: bool,
    pub paused: bool,
    /// This channel's utterance is the one audible right now.
    pub speaking: bool,
    /// Utterances waiting after the current one.
    pub queued: usize,
    /// Assistant messages that arrived while muted (reset on unmute).
    pub unheard: usize,
    /// Text of the current (playing or paused) utterance.
    pub now_text: Option<String>,
    /// The thread waits for the user (approval / question / secret request).
    pub waiting_for_user: bool,
}

/// Numbers for the panel's live HUD footer.
#[derive(Clone, Debug, Default)]
pub struct LiveStats {
    pub rss_mb: f64,
    pub cpu_pct: f64,
    pub last_t3_poll_ms: Option<f64>,
    /// Last live-tail query (new assistant messages / waiting requests) duration.
    pub last_live_poll_ms: Option<f64>,
    pub last_tts_ms: Option<f64>,
    pub last_rtf: Option<f64>,
    /// Unmute click → first audio of the catch-up line.
    pub last_click_to_audio_ms: Option<f64>,
    /// Assistant message `created_at` in T3 → its audio starts.
    pub last_msg_to_audio_ms: Option<f64>,
    /// `dictation_stop` (or Wispr's own stop) → transcript row seen in Wispr's History DB.
    pub last_dictation_ms: Option<f64>,
    /// Thread id currently audible.
    pub speaking: Option<String>,
    /// Utterances queued across all channels.
    pub queue_len: usize,
    /// Number of listening channels.
    pub listening: usize,
    /// This run's telemetry directory (events.jsonl, metrics.jsonl, audio/).
    pub run_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub enum CoreEvent {
    ThreadsUpdated,
    /// Any `ChannelView` changed (listening, paused, speaking, queue, unheard, waiting).
    ChannelsUpdated,
    SpeakingStarted { thread_id: String },
    SpeakingFinished { thread_id: String },
    /// Wispr Flow wrote a new History row after `dictation_start` (its formatted text).
    DictationTranscript { thread_id: String, text: String },
    DictationFailed { thread_id: String, error: String },
    /// `send_reply` finished. `queued` = the thread was busy, so T3 queued it as the next turn.
    ReplySent { thread_id: String, ok: bool, queued: bool, detail: String },
    Error(String),
}

/// Cheap to clone; `Send + Sync`. All work runs on core-owned background threads.
#[derive(Clone)]
pub struct Core {
    inner: Arc<Inner>,
}

struct Inner {
    tel: Arc<Telemetry>,
    threads: Arc<RwLock<Vec<ThreadView>>>,
    speech: speech::Speech,
    wispr: wispr::Wispr,
    events: Sender<CoreEvent>,
    t3_bin: PathBuf,
    reply_prefix: String,
}

impl Core {
    /// Creates the run dir, starts the poller, live tail, TTS worker and metrics sampler.
    pub fn start(config: Config) -> anyhow::Result<(Core, Receiver<CoreEvent>)> {
        let start_ms = telemetry::now_unix_ms();
        let tel = Telemetry::create(&config.log_root)?;
        tel.event(
            "app.start",
            None,
            true,
            json!({
                "config": config.to_json(), "version": env!("CARGO_PKG_VERSION"), "os": os_version(),
                "pid": std::process::id(), "exe": std::env::current_exe().ok(),
            }),
            json!({"run_dir": tel.run_dir()}),
            None,
        );
        tel.start_sampler()?;
        let (tx, rx) = crossbeam_channel::unbounded();
        let threads = Arc::new(RwLock::new(Vec::new()));
        let speech = speech::Speech::start(tel.clone(), &config, tx.clone(), threads.clone())?;
        live::spawn(tel.clone(), config.live_poll_interval, start_ms, threads.clone(), speech.clone())?;
        let wispr = wispr::Wispr::new(tel.clone(), config.wispr_db.clone(), tx.clone());
        let (t3_bin, reply_prefix) = (config.t3_bin.clone(), config.reply_prefix.clone());
        let on_stop = speech.clone();
        t3::spawn_poller(tel.clone(), config, threads.clone(), tx.clone(), move |thread, to| {
            if matches!(to, "interrupted" | "cancelled") {
                on_stop.notice(thread, "stopped", "Durduruldu.");
            }
        })?;
        let inner = Inner { tel, threads, speech, wispr, events: tx, t3_bin, reply_prefix };
        Ok((Core { inner: Arc::new(inner) }, rx))
    }
    /// Latest snapshot of threads (never blocks on I/O).
    pub fn threads(&self) -> Vec<ThreadView> {
        self.inner.threads.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    /// Radio switch. `true`: speak one catch-up line now (starts the click-to-audio clock), then
    /// every new assistant message live. `false`: drop this channel's current utterance and queue.
    pub fn set_listening(&self, thread_id: &str, on: bool) {
        self.inner.speech.set_listening(thread_id, on);
    }
    /// Video pause/resume of one channel; the paused utterance resumes mid-sentence.
    pub fn toggle_pause(&self, thread_id: &str) {
        self.inner.speech.toggle_pause(thread_id);
    }
    /// "Okey, geç": drop this channel's current utterance; the next queued one plays (unpaused).
    pub fn skip(&self, thread_id: &str) {
        self.inner.speech.skip(thread_id);
    }
    /// Drop this channel's current utterance and backlog; keep listening live.
    pub fn jump_to_live(&self, thread_id: &str) {
        self.inner.speech.jump_to_live(thread_id);
    }
    /// Mute every channel and silence playback.
    pub fn mute_all(&self) {
        self.inner.speech.mute_all();
    }
    /// Snapshot of every channel that is not the default (muted, idle, nothing unheard).
    pub fn channels(&self) -> HashMap<String, ChannelView> {
        self.inner.speech.channels()
    }
    /// Starts Wispr Flow hands-free dictation (`wispr-flow://start-hands-free`) for a reply to
    /// `thread_id` and watches Wispr's History DB for the transcript (→ `DictationTranscript`).
    pub fn dictation_start(&self, thread_id: &str) {
        self.inner.wispr.start(thread_id);
    }
    /// Ends hands-free dictation (`wispr-flow://stop-hands-free`); the transcript follows.
    pub fn dictation_stop(&self) {
        self.inner.wispr.stop();
    }
    /// Ends hands-free dictation and ignores its transcript.
    pub fn dictation_cancel(&self) {
        self.inner.wispr.cancel();
    }
    /// Non-blocking `t3 thread send <id> - --json` (text prefixed with `Config.reply_prefix`).
    /// Right before sending, the channel starts listening without a catch-up line, from the send
    /// time on, so the agent's acknowledgement is the first thing heard. → `ReplySent`.
    pub fn send_reply(&self, thread_id: &str, text: &str) {
        let core = self.clone();
        let thread_id = thread_id.to_owned();
        let payload = format!("{}{text}", self.inner.reply_prefix);
        let spawned = std::thread::Builder::new().name("t3p-reply".into()).spawn(move || core.reply(&thread_id, &payload));
        if let Err(e) = spawned {
            let _ = self.inner.events.send(CoreEvent::Error(format!("reply thread: {e}")));
        }
    }

    fn reply(&self, thread: &str, payload: &str) {
        let inner = &*self.inner;
        let status = {
            let threads = inner.threads.read().unwrap_or_else(|p| p.into_inner());
            threads.iter().find(|t| t.id == thread).map(|t| t.status.clone()).unwrap_or_default()
        };
        let queued = status == "running";
        inner.speech.listen_from(thread, telemetry::now_unix_ms());
        let argv: [&OsStr; 6] =
            [inner.t3_bin.as_os_str(), "thread".as_ref(), "send".as_ref(), thread.as_ref(), "-".as_ref(), "--json".as_ref()];
        let input = json!({"thread": thread, "argv": t3::argv_json(&argv), "text_with_prefix": payload,
                           "queued": queued, "status": status});
        let (ok, detail) = match t3::run_cmd(&argv, Some(payload.as_bytes())) {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).trim().to_owned();
                let mut out = o.telemetry();
                out["exit"] = json!(o.exit_code);
                out["json"] = serde_json::from_str(&stdout).unwrap_or(Value::Null);
                out["stdout"] = json!(stdout);
                out["stderr"] = json!(o.stderr.trim());
                let err = (!o.success()).then(|| o.failure());
                inner.tel.event("reply.send", Some(o.wall_ms), err.is_none(), input, out, err.as_deref());
                match err {
                    None => (true, stdout),
                    Some(e) => (false, e),
                }
            }
            Err(e) => {
                let err = format!("spawn: {e}");
                inner.tel.event("reply.send", None, false, input, Value::Null, Some(&err));
                (false, err)
            }
        };
        let _ = inner.events.send(CoreEvent::ReplySent { thread_id: thread.to_owned(), ok, queued, detail });
    }

    /// Opens the thread inside the T3 desktop app (`t3code://threads/<env>/<thread>`).
    pub fn open_thread(&self, thread_id: &str) {
        let link = {
            let threads = self.inner.threads.read().unwrap_or_else(|p| p.into_inner());
            threads.iter().find(|t| t.id == thread_id).map(|t| t.link.clone()).unwrap_or_default()
        };
        let Some(app_url) = app_url(&link) else {
            let err = format!("no T3 link known for thread {thread_id}");
            self.inner.tel.event("thread.open", None, false, json!({"thread": thread_id, "link": link}), Value::Null, Some(&err));
            let _ = self.inner.events.send(CoreEvent::Error(err));
            return;
        };
        let core = self.clone();
        let thread_id = thread_id.to_owned();
        let spawned = std::thread::Builder::new().name("t3p-open".into()).spawn(move || {
            let argv = ["/usr/bin/open", app_url.as_str()];
            let input = json!({"thread": thread_id, "link": link, "app_url": app_url, "argv": argv});
            if let Err(e) = t3::run_logged(&core.inner.tel, "thread.open", &argv, input, |_| Ok(((), Value::Null))) {
                let _ = core.inner.events.send(CoreEvent::Error(format!("open {app_url}: {e:#}")));
            }
        });
        if let Err(e) = spawned {
            let _ = self.inner.events.send(CoreEvent::Error(format!("open thread: {e}")));
        }
    }
    pub fn live_stats(&self) -> LiveStats {
        self.inner.tel.live_stats()
    }
    /// Lets the app write `ui.*` events into the same events.jsonl.
    pub fn log_ui(&self, kind: &str, input: serde_json::Value) {
        let kind = if kind.starts_with("ui.") { kind.to_owned() } else { format!("ui.{kind}") };
        self.inner.tel.event(&kind, None, true, input, Value::Null, None);
    }
}

/// `https://app.t3.codes/threads/<env>/<thread>` → `t3code://threads/<env>/<thread>` (the T3
/// desktop app's deep link, so `open` never lands in a browser).
fn app_url(link: &str) -> Option<String> {
    let path = link.split_once("/threads/")?.1;
    (!path.is_empty()).then(|| format!("t3code://threads/{path}"))
}

fn os_version() -> String {
    std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| std::env::consts::OS.to_owned(), |v| format!("macOS {}", v.trim()))
}
