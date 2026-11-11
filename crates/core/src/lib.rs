//! t3-panel core: T3 thread source (or the demo fixture), live message tail, per-thread speech
//! channels, local TTS, Wispr Flow dictation bridge and telemetry.
//!
//! PUBLIC API CONTRACT (the GPUI app depends on exactly these items; keep signatures stable).
//! The modules below are public for the `t3p-bench` / `t3p-report` tools.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

pub use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use serde_json::{Value, json};

mod demo;
mod ema;
pub mod extract;
mod lexicon;
mod live;
mod speech;
pub mod t3;
pub mod telemetry;
pub mod theme;
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
    /// EMA Lightning ONNX model directory (`meta.json`, `text.onnx`, `sound.onnx`,
    /// `decoder.onnx`). Default `~/Library/Application Support/t3-panel/models/ema-lightning-onnx`.
    /// Env `T3P_MODEL_DIR`.
    pub model_dir: PathBuf,
    /// ONNX Runtime intra-op threads for the voice. Default 2. Env `T3P_TTS_THREADS`.
    pub tts_threads: usize,
    /// User pronunciation lexicon (`written<TAB>spoken` per line), added to the built-in one.
    /// Default `~/Library/Application Support/t3-panel/lexicon.tsv`. Env `T3P_LEXICON`.
    pub lexicon_path: PathBuf,
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
    /// T3 Code desktop app bundle whose built-in theme palettes are read. `None` = the most
    /// recently updated of `/Applications` / `~/Applications` `T3 Code (Nightly).app` /
    /// `T3 Code.app`. Env `T3P_T3_APP`.
    pub t3_app: Option<PathBuf>,
    /// The T3 desktop app's Electron userData dir (its `Local Storage` holds the selected theme;
    /// read-only). Default `~/Library/Application Support/t3code-v2`. Env `T3P_T3_APP_DATA`.
    pub t3_app_data: PathBuf,
    /// Demo fixture JSON (see `demo/fixture.json`). `Some` = threads, messages, a timed script,
    /// dictation and replies come from the fixture; no `t3` CLI, T3 state DB or Wispr Flow
    /// access. Env `T3P_DEMO`.
    pub demo: Option<PathBuf>,
    /// Playback volume of every utterance, 0.0–1.0 (clamped; default 1.0). Synthesis, timing and
    /// saved WAVs are unaffected. Env `T3P_OUTPUT_VOLUME`.
    pub output_volume: f32,
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
                || home.join("Library/Application Support/t3-panel/models/ema-lightning-onnx"),
                PathBuf::from,
            ),
            tts_threads: var("T3P_TTS_THREADS").and_then(|v| v.trim().parse().ok()).filter(|&n| n > 0).unwrap_or(2),
            lexicon_path: var("T3P_LEXICON")
                .map_or_else(|| home.join("Library/Application Support/t3-panel/lexicon.tsv"), PathBuf::from),
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
            t3_app: var("T3P_T3_APP").map(PathBuf::from),
            t3_app_data: var("T3P_T3_APP_DATA")
                .map_or_else(|| home.join("Library/Application Support/t3code-v2"), PathBuf::from),
            demo: var("T3P_DEMO").map(PathBuf::from),
            output_volume: var("T3P_OUTPUT_VOLUME")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .map_or(1.0, |v| v.clamp(0.0, 1.0)),
        }
    }

    /// The config as logged in `app.start`.
    pub fn to_json(&self) -> Value {
        json!({
            "t3_bin": self.t3_bin, "projects": self.projects,
            "poll_interval_ms": self.poll_interval.as_millis() as u64, "model_dir": self.model_dir,
            "tts_threads": self.tts_threads, "lexicon_path": self.lexicon_path,
            "log_root": self.log_root, "save_audio": self.save_audio,
            "recent_window_s": self.recent_window.as_secs(), "max_threads": self.max_threads,
            "live_poll_interval_ms": self.live_poll_interval.as_millis() as u64,
            "reply_prefix": self.reply_prefix, "wispr_db": self.wispr_db,
            "t3_app": self.t3_app, "t3_app_data": self.t3_app_data,
            "demo": self.demo, "output_volume": self.output_volume,
        })
    }
}

/// One row in the panel. Order: see [`Core::threads`].
#[derive(Clone, Debug)]
pub struct ThreadView {
    pub id: String,
    /// Project display name (T3 project title, else workspace dir name, else id).
    pub project: String,
    /// T3 project id (stable grouping key; `project` is only the display name).
    pub project_id: String,
    /// The project's icon as set in T3 (`projection_projects.project_icon_json`), if any.
    pub project_icon: Option<ProjectIcon>,
    /// The project is pinned in T3's sidebar.
    pub project_pinned: bool,
    pub title: String,
    /// Raw T3 status. Observed values: running, idle, completed, interrupted, cancelled.
    pub status: String,
    pub settled: bool,
    pub snoozed: bool,
    /// The thread is pinned in T3's sidebar.
    pub pinned: bool,
    /// RFC3339 from T3.
    pub created_at: String,
    /// RFC3339 from T3.
    pub updated_at: String,
    /// RFC3339 start of the thread's current run (T3's `startedAt`, else `requestedAt`) while a
    /// run is active; what T3's "Working 1h 9m" label counts from. `None` when no run is active.
    pub running_since: Option<String>,
    /// `https://app.t3.codes/threads/<env>/<thread>`; opening it focuses the thread in T3.
    pub link: String,
    /// T3 provider instance id (e.g. `omp`, `omp-firstmate`, `claude`, `codex`).
    pub provider: String,
    pub model: String,
}

/// A T3 project icon, e.g. `{"kind":"lucide","name":"cooking-pot","color":"red"}`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectIcon {
    /// `lucide` (icon from lucide.dev by `name`) or another kind T3 adds later.
    pub kind: String,
    pub name: String,
    /// T3 color name (`red`, `gray`, …) as stored; `None` = default.
    pub color: Option<String>,
}

/// The panel's colors, resolved from the theme selected in the T3 desktop app (falls back to a
/// built-in dark palette). Colors are `0xRRGGBB`.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeView {
    /// Where the palette came from: `t3:<theme-id>` (the T3 app's selected built-in or custom
    /// theme; `t3:system|light|dark` = T3's neutral palette), `t3-env:<theme-id>` (an environment
    /// theme from `~/.t3/userdata/themes`), or `default`.
    pub source: String,
    pub name: String,
    pub dark: bool,
    /// Window background (T3's sidebar background: the panel mirrors T3's sidebar).
    pub canvas: u32,
    /// Raised surfaces (cards, docks, popovers).
    pub surface: u32,
    /// Row hover / pressed background.
    pub surface_hover: u32,
    pub border: u32,
    /// Primary text (≥ 7:1 on canvas when the palette allows, never < 4.5:1).
    pub text: u32,
    /// Secondary text (≥ 4.5:1 on canvas).
    pub text_muted: u32,
    /// Progress, focus and selection marks (≥ 3:1 on canvas).
    pub accent: u32,
    /// Text/icon color on an `accent` fill (≥ 4.5:1).
    pub accent_text: u32,
    pub success: u32,
    pub warning: u32,
    pub danger: u32,
    pub info: u32,
    /// T3's interface font family (`""` = system font) and size in px.
    pub font_family: String,
    pub font_size: f32,
}

impl Default for ThemeView {
    fn default() -> ThemeView {
        ThemeView {
            source: "default".into(),
            name: "T3 Panel".into(),
            dark: true,
            canvas: 0x16_1618,
            surface: 0x1f_1f22,
            surface_hover: 0x27_272b,
            border: 0x33_3338,
            text: 0xed_edef,
            text_muted: 0xa1_a1aa,
            accent: 0x5b_8def,
            accent_text: 0xff_ffff,
            success: 0x3f_b950,
            warning: 0xd2_9922,
            danger: 0xf8_5149,
            info: 0x58_a6ff,
            font_family: String::new(),
            font_size: 14.0,
        }
    }
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
    /// Playback position and length of the current utterance (ms), for a progress bar. Live on
    /// every `Core::channels()` call (no event per tick: poll while `speaking`); frozen while
    /// paused; the total grows until the utterance is fully synthesized.
    pub now_elapsed_ms: Option<u64>,
    pub now_total_ms: Option<u64>,
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
    /// The voice (EMA model + lexicon) loaded on the speech thread; until then nothing can be
    /// spoken.
    pub voice_ready: bool,
    /// The last T3 poll listed every project without a failure (`t3.poll` ok); false before the
    /// first poll and after a failed one.
    pub t3_connected: bool,
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
    /// [`Core::theme`] changed (the user picked another theme or appearance in T3).
    ThemeChanged,
}

/// Cheap to clone; `Send + Sync`. All work runs on core-owned background threads.
#[derive(Clone)]
pub struct Core {
    inner: Arc<Inner>,
}

struct Inner {
    tel: Arc<Telemetry>,
    threads: Arc<RwLock<Vec<ThreadView>>>,
    theme: Arc<RwLock<ThemeView>>,
    speech: speech::Speech,
    source: Source,
    events: Sender<CoreEvent>,
    reply_prefix: String,
}

/// Where threads, messages, dictation and replies come from.
enum Source {
    /// T3 (`t3` CLI + read-only state DB) and Wispr Flow.
    T3 { wispr: wispr::Wispr, t3_bin: PathBuf },
    /// `Config.demo` fixture; no subprocess at all.
    Demo(demo::Demo),
}

impl Core {
    /// Creates the run dir, starts the poller, live tail, TTS worker and metrics sampler (in demo
    /// mode the fixture driver replaces the poller, live tail and Wispr bridge).
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
        let reply_prefix = config.reply_prefix.clone();
        let demo = match &config.demo {
            Some(path) => Some(demo::Demo::load(tel.clone(), &config, path, threads.clone(), tx.clone())?),
            None => None,
        };
        let (messages, labels): (speech::MessageSource, _) = match &demo {
            Some((d, _)) => (d.message_source(), d.labels()),
            None => (Arc::new(t3::latest_messages), HashMap::new()),
        };
        let speech = speech::Speech::start(tel.clone(), &config, tx.clone(), threads.clone(), messages, labels)?;
        let theme = match &demo {
            Some((d, _)) if d.default_theme() => Arc::new(RwLock::new(ThemeView::default())),
            _ => theme::start(tel.clone(), &config, tx.clone())?,
        };
        let source = match demo {
            Some((d, jobs)) => {
                d.run(jobs, speech.clone())?;
                Source::Demo(d)
            }
            None => {
                live::spawn(tel.clone(), config.live_poll_interval, start_ms, threads.clone(), speech.clone())?;
                let wispr = wispr::Wispr::new(tel.clone(), config.wispr_db.clone(), tx.clone());
                let t3_bin = config.t3_bin.clone();
                let on_status = speech.clone();
                t3::spawn_poller(tel.clone(), config, threads.clone(), tx.clone(), move |thread, to| {
                    status_changed(&on_status, thread, to);
                })?;
                Source::T3 { wispr, t3_bin }
            }
        };
        let inner = Inner { tel, threads, theme, speech, source, events: tx, reply_prefix };
        Ok((Core { inner: Arc::new(inner) }, rx))
    }
    /// Latest snapshot of threads (never blocks on I/O). Order mirrors T3's sidebar and is
    /// stable: activity (`updated_at`) never moves a row. Pinned threads first, in T3's pin order
    /// (`pinOrderKey` ascending; the newest pin is on top); then active threads, newest
    /// `created_at` first; then settled or snoozed threads, newest `created_at` first.
    pub fn threads(&self) -> Vec<ThreadView> {
        self.inner.threads.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    /// The palette resolved from T3's selected theme (never blocks on I/O). `ThemeChanged`
    /// fires when it changes.
    pub fn theme(&self) -> ThemeView {
        self.inner.theme.read().unwrap_or_else(|p| p.into_inner()).clone()
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
    /// Demo mode: the thread's fixture `reply.dictation` arrives after ~2.5 s (no Wispr).
    pub fn dictation_start(&self, thread_id: &str) {
        match &self.inner.source {
            Source::T3 { wispr, .. } => wispr.start(thread_id),
            Source::Demo(d) => d.dictation_start(thread_id),
        }
    }
    /// Ends hands-free dictation (`wispr-flow://stop-hands-free`); the transcript follows.
    pub fn dictation_stop(&self) {
        match &self.inner.source {
            Source::T3 { wispr, .. } => wispr.stop(),
            Source::Demo(d) => d.dictation_stop(),
        }
    }
    /// Ends hands-free dictation and ignores its transcript.
    pub fn dictation_cancel(&self) {
        match &self.inner.source {
            Source::T3 { wispr, .. } => wispr.cancel(),
            Source::Demo(d) => d.dictation_cancel(),
        }
    }
    /// Non-blocking `t3 thread send <id> - --json` (text prefixed with `Config.reply_prefix`).
    /// Right before sending, the channel starts listening without a catch-up line, from the send
    /// time on, so the agent's acknowledgement is the first thing heard. → `ReplySent`.
    /// Demo mode: nothing is sent; the fixture `reply.ack` arrives ~1.5 s later as a new
    /// assistant message.
    pub fn send_reply(&self, thread_id: &str, text: &str) {
        let payload = format!("{}{text}", self.inner.reply_prefix);
        let t3_bin = match &self.inner.source {
            Source::T3 { t3_bin, .. } => t3_bin.clone(),
            Source::Demo(d) => {
                self.inner.speech.listen_from(thread_id, telemetry::now_unix_ms());
                let (ok, queued, detail) = d.send_reply(thread_id, &payload);
                let _ = self.inner.events.send(CoreEvent::ReplySent { thread_id: thread_id.to_owned(), ok, queued, detail });
                return;
            }
        };
        let core = self.clone();
        let thread_id = thread_id.to_owned();
        let spawned = std::thread::Builder::new()
            .name("t3p-reply".into())
            .spawn(move || core.reply(&t3_bin, &thread_id, &payload));
        if let Err(e) = spawned {
            let _ = self.inner.events.send(CoreEvent::Error(format!("reply thread: {e}")));
        }
    }

    fn reply(&self, t3_bin: &Path, thread: &str, payload: &str) {
        let inner = &*self.inner;
        let status = {
            let threads = inner.threads.read().unwrap_or_else(|p| p.into_inner());
            threads.iter().find(|t| t.id == thread).map(|t| t.status.clone()).unwrap_or_default()
        };
        let queued = status == "running";
        inner.speech.listen_from(thread, telemetry::now_unix_ms());
        let argv: [&OsStr; 6] =
            [t3_bin.as_os_str(), "thread".as_ref(), "send".as_ref(), thread.as_ref(), "-".as_ref(), "--json".as_ref()];
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

    /// Opens the thread inside the T3 desktop app (`t3code://threads/<env>/<thread>`). Demo
    /// mode only logs `thread.open`.
    pub fn open_thread(&self, thread_id: &str) {
        if let Source::Demo(_) = &self.inner.source {
            let input = json!({"thread": thread_id, "demo": true});
            self.inner.tel.event("thread.open", None, true, input, json!({"skipped": "demo mode"}), None);
            return;
        }
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

/// Side effect of a logged `state.change` (T3 poller and demo source): a listening channel hears
/// that its thread was stopped.
fn status_changed(speech: &speech::Speech, thread: &str, to: &str) {
    if matches!(to, "interrupted" | "cancelled") {
        speech.notice(thread, "stopped", "Durduruldu.");
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
