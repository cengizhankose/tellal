//! Tellal core: T3 thread source (or the demo fixture), live message tail, per-thread speech
//! channels, local TTS (a Turkish and an English voice, downloaded on demand), Wispr Flow
//! dictation bridge and telemetry.
//!
//! PUBLIC API CONTRACT (the GPUI app depends on exactly these items; keep signatures stable).
//! The modules below are public for the `tellal-bench` / `tellal-report` tools.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

pub use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use serde_json::{Value, json};

mod computer;
mod demo;
mod ema;
mod english;
pub mod extract;
mod lang;
mod lexicon;
mod live;
mod speech;
mod supertonic;
pub mod t3;
pub mod telemetry;
pub mod theme;
mod voices;
mod wispr;
pub mod tts {
    //! TTS engine and audio helpers (used by `tellal-bench`).
    pub use crate::speech::{Synth, TtsEngine, open_output, samples_source, write_wav};
}

use telemetry::Telemetry;

/// Runtime configuration. `Config::from_env()` fills defaults and applies `TELLAL_*` overrides.
#[derive(Clone, Debug)]
pub struct Config {
    /// T3 CLI launcher. Default `~/.t3/bin/t3`. Env `TELLAL_T3_BIN`.
    pub t3_bin: PathBuf,
    /// Project ids or paths to watch. `None` = discover from T3's state DB (read-only).
    /// Env `TELLAL_PROJECTS` (comma separated).
    pub projects: Option<Vec<String>>,
    /// Thread list poll interval. Default 5s. Env `TELLAL_POLL_MS`.
    pub poll_interval: Duration,
    /// Root of the voice models, one directory per voice (`ema-lightning-onnx`,
    /// `supertonic-2-en`). Default `~/Library/Application Support/tellal/models`. Env
    /// `TELLAL_MODELS_DIR`.
    pub models_dir: PathBuf,
    /// ONNX Runtime intra-op threads per voice. Default 2. Env `TELLAL_TTS_THREADS`.
    pub tts_threads: usize,
    /// User pronunciation lexicon for the Turkish voice (`written<TAB>spoken` per line), added to
    /// the built-in one. Default `~/Library/Application Support/tellal/lexicon.tsv`. Env
    /// `TELLAL_LEXICON`.
    pub lexicon_path: PathBuf,
    /// The same for the English voice. Default `~/Library/Application Support/tellal/lexicon-en.tsv`.
    /// Env `TELLAL_LEXICON_EN`.
    pub lexicon_en_path: PathBuf,
    /// Root for per-run telemetry dirs. Default `~/Library/Logs/tellal`. Env `TELLAL_LOG_ROOT`.
    pub log_root: PathBuf,
    /// Save every synthesized utterance as WAV in the run dir. Default true. Env `TELLAL_SAVE_AUDIO=0` disables.
    pub save_audio: bool,
    /// Settled threads updated within this window are still listed. Default 6h.
    pub recent_window: Duration,
    /// Max rows returned by `Core::threads`. Default 20.
    pub max_threads: usize,
    /// Live-tail interval (new assistant messages + waiting requests of listed threads, one
    /// read-only query). Default 1s. Env `TELLAL_LIVE_POLL_MS`.
    pub live_poll_interval: Duration,
    /// Prepended to every reply and new task sent from the panel so the agent speaks through its
    /// skill: `"$sesli "` (Turkish, `sesli` skill) or `"$aloud "` (English, `aloud` skill) by
    /// [`Config::lang`]. Env `TELLAL_REPLY_PREFIX` overrides (empty string disables).
    pub reply_prefix: String,
    /// Wispr Flow History DB (read-only). Default
    /// `~/Library/Application Support/Wispr Flow/flow.sqlite`. Env `TELLAL_WISPR_DB`.
    pub wispr_db: PathBuf,
    /// T3 Code desktop app bundle whose built-in theme palettes are read. `None` = the most
    /// recently updated of `/Applications` / `~/Applications` `T3 Code (Nightly).app` /
    /// `T3 Code.app`. Env `TELLAL_T3_APP`.
    pub t3_app: Option<PathBuf>,
    /// The T3 desktop app's Electron userData dir (its `Local Storage` holds the selected theme;
    /// read-only). Default `~/Library/Application Support/t3code-v2`. Env `TELLAL_T3_APP_DATA`.
    pub t3_app_data: PathBuf,
    /// Demo fixture JSON (see `demo/fixture.json`). `Some` = threads, messages, a timed script,
    /// dictation and replies come from the fixture; no `t3` CLI, T3 state DB or Wispr Flow
    /// access. Env `TELLAL_DEMO`.
    pub demo: Option<PathBuf>,
    /// Playback volume of every utterance, 0.0–1.0 (clamped; default 1.0). Synthesis, timing and
    /// saved WAVs are unaffected. Env `TELLAL_OUTPUT_VOLUME`.
    pub output_volume: f32,
    /// cua-driver binary (computer use). Default
    /// `/Applications/CuaDriver.app/Contents/MacOS/cua-driver`. Env `TELLAL_CUA_DRIVER`. Missing
    /// binary = computer use is simply unavailable (`ComputerView.installed == false`).
    pub cua_driver: PathBuf,
    /// T3 project (id or path) for tasks started from the panel ([`Core::start_task`]). `None` =
    /// T3's "No project" (workspace `~/.t3/scratch`). Env `TELLAL_TASK_PROJECT`.
    pub task_project: Option<String>,
    /// Language of the panel's UI copy and of the lines the core speaks itself (catch-up and
    /// status lines). Env `TELLAL_LANG` = `tr` | `en`; unset = macOS's preferred language.
    pub lang: Lang,
    /// Which voice speaks. Env `TELLAL_VOICE` = `auto` (default: per utterance, by the
    /// utterance's language) | `tr` | `en`.
    pub voice: VoicePref,
}

/// UI and speech language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Tr,
    En,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tr" => Some(Lang::Tr),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Lang::Tr => "tr",
            Lang::En => "en",
        }
    }
    /// macOS's preferred language (`tr*` → `Tr`, anything else → `En`), read from
    /// CoreFoundation (`CFLocaleCopyPreferredLanguages`; no subprocess). `Config::from_env`
    /// calls it once, only when `TELLAL_LANG` is unset.
    pub fn system() -> Lang {
        lang::from_tag(lang::preferred_language().as_deref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoicePref {
    /// Pick the voice per utterance from the utterance's language.
    Auto,
    Only(Lang),
}

impl VoicePref {
    pub fn parse(s: &str) -> Option<VoicePref> {
        if s.trim().eq_ignore_ascii_case("auto") { Some(VoicePref::Auto) } else { Lang::parse(s).map(VoicePref::Only) }
    }
}

/// One installable voice model.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceView {
    /// Stable id, e.g. `tr-ema-lightning`.
    pub id: String,
    pub lang: Lang,
    /// Display name, e.g. `EMA Lightning`.
    pub name: String,
    /// Download size.
    pub download_mb: u32,
    pub state: VoiceState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VoiceState {
    Ready,
    Missing,
    Downloading { done_bytes: u64, total_bytes: u64 },
    Failed(String),
}

impl Config {
    pub fn from_env() -> Config {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let lang = var("TELLAL_LANG").and_then(|v| Lang::parse(&v)).unwrap_or_else(Lang::system);
        let support = home.join("Library/Application Support/tellal");
        Config {
            t3_bin: var("TELLAL_T3_BIN").map_or_else(|| home.join(".t3/bin/t3"), PathBuf::from),
            projects: var("TELLAL_PROJECTS").map(|v| {
                v.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_owned).collect()
            }),
            poll_interval: var("TELLAL_POLL_MS")
                .and_then(|v| v.trim().parse().ok())
                .map_or(Duration::from_secs(5), Duration::from_millis),
            models_dir: var("TELLAL_MODELS_DIR").map_or_else(|| support.join("models"), PathBuf::from),
            tts_threads: var("TELLAL_TTS_THREADS").and_then(|v| v.trim().parse().ok()).filter(|&n| n > 0).unwrap_or(2),
            lexicon_path: var("TELLAL_LEXICON").map_or_else(|| support.join("lexicon.tsv"), PathBuf::from),
            lexicon_en_path: var("TELLAL_LEXICON_EN").map_or_else(|| support.join("lexicon-en.tsv"), PathBuf::from),
            log_root: var("TELLAL_LOG_ROOT").map_or_else(|| home.join("Library/Logs/tellal"), PathBuf::from),
            save_audio: var("TELLAL_SAVE_AUDIO").is_none_or(|v| v.trim() != "0"),
            recent_window: Duration::from_secs(6 * 3600),
            max_threads: 20,
            live_poll_interval: var("TELLAL_LIVE_POLL_MS")
                .and_then(|v| v.trim().parse().ok())
                .map_or(Duration::from_secs(1), Duration::from_millis),
            reply_prefix: std::env::var("TELLAL_REPLY_PREFIX").unwrap_or_else(|_| {
                match lang {
                    Lang::Tr => "$sesli ",
                    Lang::En => "$aloud ",
                }
                .to_owned()
            }),
            wispr_db: var("TELLAL_WISPR_DB")
                .map_or_else(|| home.join("Library/Application Support/Wispr Flow/flow.sqlite"), PathBuf::from),
            t3_app: var("TELLAL_T3_APP").map(PathBuf::from),
            t3_app_data: var("TELLAL_T3_APP_DATA")
                .map_or_else(|| home.join("Library/Application Support/t3code-v2"), PathBuf::from),
            demo: var("TELLAL_DEMO").map(PathBuf::from),
            output_volume: var("TELLAL_OUTPUT_VOLUME")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .map_or(1.0, |v| v.clamp(0.0, 1.0)),
            cua_driver: var("TELLAL_CUA_DRIVER")
                .map_or_else(|| PathBuf::from("/Applications/CuaDriver.app/Contents/MacOS/cua-driver"), PathBuf::from),
            task_project: var("TELLAL_TASK_PROJECT"),
            lang,
            voice: var("TELLAL_VOICE").and_then(|v| VoicePref::parse(&v)).unwrap_or(VoicePref::Auto),
        }
    }

    /// The config as logged in `app.start`.
    pub fn to_json(&self) -> Value {
        json!({
            "t3_bin": self.t3_bin, "projects": self.projects,
            "poll_interval_ms": self.poll_interval.as_millis() as u64, "models_dir": self.models_dir,
            "tts_threads": self.tts_threads, "lexicon_path": self.lexicon_path, "lexicon_en_path": self.lexicon_en_path,
            "log_root": self.log_root, "save_audio": self.save_audio,
            "recent_window_s": self.recent_window.as_secs(), "max_threads": self.max_threads,
            "live_poll_interval_ms": self.live_poll_interval.as_millis() as u64,
            "reply_prefix": self.reply_prefix, "wispr_db": self.wispr_db,
            "t3_app": self.t3_app, "t3_app_data": self.t3_app_data,
            "demo": self.demo, "output_volume": self.output_volume,
            "cua_driver": self.cua_driver, "task_project": self.task_project,
            "lang": self.lang.code(), "voice": format!("{:?}", self.voice),
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
            name: "Tellal".into(),
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

/// Dictation target id for a new task (not a reply): pass it to [`Core::dictation_start`];
/// `CoreEvent::DictationTranscript` / `DictationFailed` carry it back as `thread_id`.
pub const NEW_TASK: &str = "new-task";

/// Computer use through the local cua-driver daemon. Agents (any MCP client: omp in T3, Claude
/// Code, Codex) drive the screen through it; the panel only watches and can stop them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputerView {
    /// `Config.cua_driver` exists.
    pub installed: bool,
    pub version: Option<String>,
    pub daemon_running: bool,
    /// Accessibility + Screen Recording granted to CuaDriver; `None` = unknown (no daemon).
    pub permissions_ok: Option<bool>,
    /// `standard` | `bounded` | `unrestricted`, as the daemon reports it.
    pub permission_mode: Option<String>,
    /// Live agent sessions: an agent is connected and may act on the screen.
    pub sessions: Vec<ComputerSession>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerSession {
    pub id: String,
    /// Agent/client label as cua-driver reports it, if any.
    pub client: Option<String>,
    /// RFC3339.
    pub started_at: Option<String>,
    /// App the agent last acted on, if reported.
    pub app: Option<String>,
    /// One-line label of the last action (e.g. `click "Kaydet"`), if reported.
    pub last_action: Option<String>,
    /// RFC3339 of the last action, if reported.
    pub last_action_at: Option<String>,
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
    /// [`Core::computer`] changed (install/daemon/permission state, a session started, acted or
    /// ended).
    ComputerUpdated,
    /// [`Core::start_task`] finished: `thread_id` is the new thread (already listening) when `ok`.
    TaskStarted { thread_id: Option<String>, ok: bool, detail: String },
    /// [`Core::voices`] changed (a download started, progressed, finished or failed).
    VoicesUpdated,
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
    computer: Arc<RwLock<ComputerView>>,
    /// T3 project id → display name, for every project the poller knows (T3 source only).
    project_names: Arc<RwLock<HashMap<String, String>>>,
    lang: Lang,
    voices: voices::Voices,
}

/// Where threads, messages, dictation and replies come from.
enum Source {
    /// T3 (`t3` CLI + read-only state DB), Wispr Flow and cua-driver. `poll` wakes the T3
    /// poller for an immediate tick.
    T3 { wispr: wispr::Wispr, t3_bin: PathBuf, computer: computer::Computer, poll: Sender<()>, task_project: Option<String> },
    /// `Config.demo` fixture; no subprocess at all.
    Demo(demo::Demo),
}

impl Core {
    /// Creates the run dir, starts the poller, live tail, cua-driver watcher, TTS worker and
    /// metrics sampler (in demo mode the fixture driver replaces the poller, live tail, Wispr
    /// bridge and cua-driver watcher), and downloads the voice for `Config.lang` when it is
    /// missing (real mode).
    pub fn start(config: Config) -> anyhow::Result<(Core, Receiver<CoreEvent>)> {
        let start_ms = telemetry::now_unix_ms();
        let lang = config.lang;
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
        let computer_view = Arc::new(RwLock::new(ComputerView::default()));
        let project_names = Arc::new(RwLock::new(HashMap::new()));
        let demo = match &config.demo {
            Some(path) => Some(demo::Demo::load(
                tel.clone(),
                &config,
                path,
                threads.clone(),
                computer_view.clone(),
                tx.clone(),
            )?),
            None => None,
        };
        let (messages, labels): (speech::MessageSource, _) = match &demo {
            Some((d, _)) => (d.message_source(), d.labels()),
            None => (Arc::new(t3::latest_messages), HashMap::new()),
        };
        let demo_voices = demo.as_ref().map(|(d, _)| d.voices());
        let voices =
            voices::Voices::new(tel.clone(), tx.clone(), config.models_dir.clone(), &voices::VOICES, demo_voices.as_deref())?;
        let speech =
            speech::Speech::start(tel.clone(), &config, tx.clone(), threads.clone(), messages, labels, voices.clone())?;
        voices.start(lang);
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
                let task_project = config.task_project.clone();
                let computer =
                    computer::Computer::spawn(tel.clone(), config.cua_driver.clone(), computer_view.clone(), tx.clone())?;
                let (poll, poll_wake) = crossbeam_channel::unbounded();
                let on_status = speech.clone();
                let names = project_names.clone();
                t3::spawn_poller(tel.clone(), config, threads.clone(), names, tx.clone(), poll_wake, move |thread, to| {
                    status_changed(&on_status, thread, to);
                })?;
                Source::T3 { wispr, t3_bin, computer, poll, task_project }
            }
        };
        let inner = Inner {
            tel,
            threads,
            theme,
            speech,
            source,
            events: tx,
            reply_prefix,
            computer: computer_view,
            project_names,
            lang,
            voices,
        };
        Ok((Core { inner: Arc::new(inner) }, rx))
    }
    /// Latest snapshot of threads (never blocks on I/O). Order mirrors T3's sidebar and is
    /// stable: activity (`updated_at`) never moves a row. Pinned threads first, in T3's pin order
    /// (`pinOrderKey` ascending; the newest pin is on top); then active threads, newest
    /// `created_at` first; then settled or snoozed threads, newest `created_at` first.
    pub fn threads(&self) -> Vec<ThreadView> {
        self.inner.threads.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    /// Display name of the T3 project `id` (any project in T3's state DB, listed or not; never
    /// blocks on I/O). `None` until the first poll, for unknown ids, and in demo mode.
    pub fn project_name(&self, id: &str) -> Option<String> {
        self.inner.project_names.read().unwrap_or_else(|p| p.into_inner()).get(id).cloned()
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
                self.inner.speech.listen_from(thread_id, telemetry::now_unix_ms(), "reply");
                let (ok, queued, detail) = d.send_reply(thread_id, &payload);
                let _ = self.inner.events.send(CoreEvent::ReplySent { thread_id: thread_id.to_owned(), ok, queued, detail });
                return;
            }
        };
        let core = self.clone();
        let thread_id = thread_id.to_owned();
        let spawned = std::thread::Builder::new()
            .name("tellal-reply".into())
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
        inner.speech.listen_from(thread, telemetry::now_unix_ms(), "reply");
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
        let spawned = std::thread::Builder::new().name("tellal-open".into()).spawn(move || {
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
    /// Computer-use state (never blocks on I/O). Real mode: the `tellal-computer` watcher of the
    /// cua-driver daemon (see `computer.rs` for sources and cadence); demo mode: the fixture's
    /// `computer` block.
    pub fn computer(&self) -> ComputerView {
        self.inner.computer.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    /// UI/speech language ([`Config::lang`]).
    pub fn lang(&self) -> Lang {
        self.inner.lang
    }
    /// Every voice Tellal knows, installed or not, Turkish first (never blocks on I/O): registry
    /// + on-disk state (scanned at start and after each download) + live download progress.
    pub fn voices(&self) -> Vec<VoiceView> {
        self.inner.voices.views()
    }
    /// Downloads and installs voice `id` in the background (HTTPS from its pinned Hugging Face
    /// revision, SHA-256 per file, atomic install); progress → `VoicesUpdated` (≤ ~4/s), logged
    /// as `voice.download`. Ignored while it is Ready or downloading. Demo mode: simulated, no
    /// network (see `voices.rs`).
    pub fn download_voice(&self, id: &str) {
        self.inner.voices.download(id, "user");
    }
    /// Emergency stop: revoke every live cua-driver session (`cua-driver revoke --all`) so no
    /// agent can act on the screen until it reconnects. Non-blocking; → `ComputerUpdated`.
    /// Demo mode: every live scripted session ends at once.
    pub fn computer_stop_all(&self) {
        match &self.inner.source {
            Source::T3 { computer, .. } => computer.stop_all(),
            Source::Demo(d) => d.computer_stop_all(),
        }
    }
    /// Opens CuaDriver's macOS permission flow (`cua-driver permissions grant`: Accessibility,
    /// Screen Recording). Non-blocking; the user approves the system dialogs; → `ComputerUpdated`.
    /// Demo mode: nothing runs; permissions turn granted ~1.5 s later.
    pub fn computer_grant_permissions(&self) {
        match &self.inner.source {
            Source::T3 { computer, .. } => computer.grant_permissions(),
            Source::Demo(d) => d.computer_grant_permissions(),
        }
    }
    /// Starts a new T3 thread with `text` (prefixed with `Config.reply_prefix`) in
    /// `Config.task_project`, using the provider/model of the most recently created thread
    /// ([`task_model`]: listed threads first, else any thread in T3's state DB, else none: T3's
    /// default for the project), and listens to it from its first message. Non-blocking;
    /// → `TaskStarted`.
    ///
    /// Runs `t3 thread start --json [--provider <p> --model <m>] --title <first line of text>
    /// <project> -` (stdin = the prefixed text; the title keeps the prefix out of T3's sidebar).
    /// `Config.task_project == None` resolves T3's "No project" (the state DB project whose
    /// workspace root is `~/.t3/scratch`). Logged as `task.start`. On success the new thread
    /// listens from the send time (no catch-up line, like `send_reply`) and the poller lists it
    /// at once. Demo mode: the fixture `new_task` block scripts the new thread.
    pub fn start_task(&self, text: &str) {
        let inner = &*self.inner;
        let payload = format!("{}{text}", inner.reply_prefix);
        let send_ms = telemetry::now_unix_ms();
        if text.trim().is_empty() {
            let err = "empty task text";
            inner.tel.event("task.start", None, false, json!({"text": text}), Value::Null, Some(err));
            let _ = inner.events.send(CoreEvent::TaskStarted { thread_id: None, ok: false, detail: err.to_owned() });
            return;
        }
        let (t3_bin, task_project, poll) = match &inner.source {
            Source::T3 { t3_bin, task_project, poll, .. } => (t3_bin.clone(), task_project.clone(), poll.clone()),
            Source::Demo(d) => {
                d.start_task(text, &payload, send_ms);
                return;
            }
        };
        let core = self.clone();
        let text = text.to_owned();
        let spawned = std::thread::Builder::new()
            .name("tellal-task".into())
            .spawn(move || core.task(&t3_bin, task_project, &poll, &text, &payload, send_ms));
        if let Err(e) = spawned {
            let _ = inner.events.send(CoreEvent::Error(format!("task thread: {e}")));
        }
    }

    fn task(&self, t3_bin: &Path, task_project: Option<String>, poll: &Sender<()>, text: &str, payload: &str, send_ms: i64) {
        let inner = &*self.inner;
        let listed = latest_model(&inner.threads.read().unwrap_or_else(|p| p.into_inner()));
        let (latest, model_source, model_error) = task_model(listed, t3::latest_model_any);
        let title = task_title(text);
        let mut input = json!({"text": text, "text_with_prefix": payload, "title": title,
                               "model_from_thread": latest.as_ref().map(|l| &l.0),
                               "model_source": model_source, "model_error": model_error});
        let project = match task_project {
            Some(p) => Ok((p, "config")),
            None => t3::scratch_project().map(|p| (p, "scratch")),
        };
        let (thread_id, detail) = match project {
            Err(e) => {
                let err = format!("task project: {e:#}");
                inner.tel.event("task.start", None, false, input, Value::Null, Some(&err));
                (None, err)
            }
            Ok((project, project_source)) => {
                let model = latest.as_ref().map(|(_, provider, model)| (provider.as_str(), model.as_str()));
                let argv = task_argv(t3_bin, model, &title, &project);
                input["argv"] = t3::argv_json(&argv);
                input["project"] = json!(project);
                input["project_source"] = json!(project_source);
                match t3::run_cmd(&argv, Some(payload.as_bytes())) {
                    Ok(o) => {
                        let stdout = String::from_utf8_lossy(&o.stdout).trim().to_owned();
                        let parsed: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
                        let thread_id = parsed["threadId"].as_str().filter(|_| o.success()).map(str::to_owned);
                        let err = match (&thread_id, o.success()) {
                            (_, false) => Some(o.failure()),
                            (None, true) => Some(format!("no threadId in output: {stdout}")),
                            (Some(_), true) => None,
                        };
                        let mut out = o.telemetry();
                        out["exit"] = json!(o.exit_code);
                        out["json"] = parsed;
                        out["stdout"] = json!(stdout);
                        out["stderr"] = json!(o.stderr.trim());
                        out["thread"] = json!(thread_id);
                        inner.tel.event("task.start", Some(o.wall_ms), err.is_none(), input, out, err.as_deref());
                        (thread_id, err.unwrap_or(stdout))
                    }
                    Err(e) => {
                        let err = format!("spawn: {e}");
                        inner.tel.event("task.start", None, false, input, Value::Null, Some(&err));
                        (None, err)
                    }
                }
            }
        };
        if let Some(id) = &thread_id {
            inner.speech.listen_from(id, send_ms, "task");
            let _ = poll.send(());
        }
        let ok = thread_id.is_some();
        let _ = inner.events.send(CoreEvent::TaskStarted { thread_id, ok, detail });
    }
    /// Lets the app write `ui.*` events into the same events.jsonl.
    pub fn log_ui(&self, kind: &str, input: serde_json::Value) {
        let kind = if kind.starts_with("ui.") { kind.to_owned() } else { format!("ui.{kind}") };
        self.inner.tel.event(&kind, None, true, input, Value::Null, None);
    }
}

/// Downloads voices `ids` into `cfg.models_dir` exactly as [`Core::download_voice`] does (same
/// pinned files, checks and `voice.download` events in a run dir under `cfg.log_root`), blocking
/// until none is downloading; `on_update` sees every `VoicesUpdated`. For `tellal-bench
/// --download`, without starting the panel's sources.
pub fn download_voices(cfg: &Config, ids: &[String], mut on_update: impl FnMut(&[VoiceView])) -> anyhow::Result<Vec<VoiceView>> {
    let tel = Telemetry::create(&cfg.log_root)?;
    let (tx, rx) = crossbeam_channel::unbounded();
    let voices = voices::Voices::new(tel, tx, cfg.models_dir.clone(), &voices::VOICES, None)?;
    for id in ids {
        voices.download(id, "bench");
    }
    while voices.views().iter().any(|v| matches!(v.state, VoiceState::Downloading { .. })) {
        if let Ok(CoreEvent::VoicesUpdated) = rx.recv_timeout(Duration::from_secs(1)) {
            on_update(&voices.views());
        }
    }
    Ok(voices.views())
}

/// Side effect of a logged `state.change` (T3 poller and demo source): a listening channel hears
/// that its thread was stopped.
fn status_changed(speech: &speech::Speech, thread: &str, to: &str) {
    if matches!(to, "interrupted" | "cancelled") {
        speech.notice(thread, extract::Notice::Stopped);
    }
}

/// `(thread id, provider, model)` of the most recently created thread that has both.
fn latest_model(threads: &[ThreadView]) -> Option<(String, String, String)> {
    threads
        .iter()
        .filter(|t| !t.provider.is_empty() && !t.model.is_empty())
        .max_by_key(|t| telemetry::parse_rfc3339_ms(&t.created_at).unwrap_or(i64::MIN))
        .map(|t| (t.id.clone(), t.provider.clone(), t.model.clone()))
}

/// The model for a new task and where it came from: the newest listed thread's (`listed`), else
/// the newest thread's in T3's state DB, any project (`state_db`, read only when nothing listed
/// has one), else none (`none`: `t3` picks the project's default). A failed DB read is returned
/// for the log and counts as none.
fn task_model(
    listed: Option<(String, String, String)>,
    state_db: impl FnOnce() -> anyhow::Result<Option<(String, String, String)>>,
) -> (Option<(String, String, String)>, &'static str, Option<String>) {
    if listed.is_some() {
        return (listed, "listed", None);
    }
    match state_db() {
        Ok(Some(m)) => (Some(m), "state_db", None),
        Ok(None) => (None, "none", None),
        Err(e) => (None, "none", Some(format!("{e:#}"))),
    }
}

/// Thread title for a task: the text's first non-empty line, at most 80 characters.
fn task_title(text: &str) -> String {
    const MAX: usize = 80;
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    if line.chars().count() <= MAX {
        return line.to_owned();
    }
    let cut: String = line.chars().take(MAX - 1).collect();
    format!("{}…", cut.trim_end())
}

/// `t3 thread start --json [--provider <p> --model <m>] --title <title> <project> -`; without a
/// model to copy, T3 picks the project's default model.
fn task_argv(t3_bin: &Path, model: Option<(&str, &str)>, title: &str, project: &str) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec![t3_bin.into()];
    argv.extend(["thread", "start", "--json"].map(OsString::from));
    if let Some((provider, model)) = model {
        argv.extend(["--provider", provider, "--model", model].map(OsString::from));
    }
    argv.extend(["--title", title, project, "-"].map(OsString::from));
    argv
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

#[cfg(test)]
mod tests {
    use super::*;

    fn view(id: &str, created: &str, provider: &str, model: &str) -> ThreadView {
        ThreadView {
            id: id.into(),
            project: String::new(),
            project_id: String::new(),
            project_icon: None,
            project_pinned: false,
            title: String::new(),
            status: "idle".into(),
            settled: false,
            snoozed: false,
            pinned: false,
            created_at: created.into(),
            updated_at: created.into(),
            running_since: None,
            link: String::new(),
            provider: provider.into(),
            model: model.into(),
        }
    }

    #[test]
    fn task_arguments() {
        let threads = [
            view("old", "2026-11-11T08:00:00.000Z", "codex", "gpt-5"),
            view("newest-no-model", "2026-11-11T12:00:00.000Z", "omp", ""),
            view("new", "2026-11-11T10:00:00.000Z", "omp", "anthropic/claude-opus"),
        ];
        assert_eq!(latest_model(&threads), Some(("new".into(), "omp".into(), "anthropic/claude-opus".into())));
        assert_eq!(latest_model(&threads[1..2]), None);

        // Pick order: listed > T3's state DB (any project) > none.
        let m = |id: &str| (id.to_owned(), "omp".to_owned(), "m/x".to_owned());
        let db_unused = || -> anyhow::Result<_> { panic!("the state DB is read only when nothing listed has a model") };
        assert_eq!(task_model(Some(m("listed")), db_unused), (Some(m("listed")), "listed", None));
        assert_eq!(task_model(None, || Ok(Some(m("db")))), (Some(m("db")), "state_db", None));
        assert_eq!(task_model(None, || Ok(None)), (None, "none", None));
        let (model, source, err) = task_model(None, || Err(anyhow::anyhow!("sqlite3 failed")));
        assert_eq!((model, source, err.as_deref()), (None, "none", Some("sqlite3 failed")));

        assert_eq!(task_title("\n  Ekran görüntülerini klasörle  \nikinci satır"), "Ekran görüntülerini klasörle");
        let long = task_title(&"ş".repeat(100));
        assert_eq!(long.chars().count(), 80);
        assert!(long.ends_with('…'));

        let utf8 = |argv: Vec<OsString>| -> Vec<String> {
            argv.into_iter().map(|a| a.into_string().expect("utf8")).collect()
        };
        assert_eq!(
            utf8(task_argv(Path::new("/t3"), Some(("omp", "m/x")), "Başlık", "proj-1")),
            ["/t3", "thread", "start", "--json", "--provider", "omp", "--model", "m/x", "--title", "Başlık", "proj-1", "-"]
        );
        // Nothing listed to copy (empty project, fresh install): T3's default model.
        assert_eq!(
            utf8(task_argv(Path::new("/t3"), None, "Başlık", "proj-1")),
            ["/t3", "thread", "start", "--json", "--title", "Başlık", "proj-1", "-"]
        );
    }
}
