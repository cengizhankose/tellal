//! `TELLAL_SELFTEST` drivers: `1` (listen to one thread, then mute and quit), `reply:<id>` (the
//! Cevapla flow), `shots` (walks the UI states for screenshots without synthetic clicks; each
//! state logs `ui.selftest_state` with the window number for `screencapture -l`; the
//! computer-use states wait for the demo fixture's sessions) and `demo` (a choreographed
//! walkthrough of the demo fixture for the launch video, including computer use and a new task;
//! every beat logs `ui.selftest_action` and, once settled, `ui.selftest_state`), `clip:<name>`
//! (one feature per run for the README clips, on the same beat machinery; see [`Clip`]) and
//! `hero` (the real-T3 hero clip: types `TELLAL_SELFTEST_TASK` into the new-task dock, Gönder →
//! the real `start_task`, then stays on the new thread until its run ended and its narration
//! went quiet). Permission grants are pressed only in demo mode (`Config.demo`), never against
//! the real cua-driver.

use std::time::{Duration, Instant};

use gpui::{App, Context, Window};
use serde_json::{Value, json};
use tellal_core::{ThreadView, VoiceState};

use super::{Panel, Retry, ms};
use crate::input::EditSource;
use crate::mac;
use crate::theme::Theme;

const SELFTEST_TIMEOUT: Duration = Duration::from_secs(60);
const SELFTEST_QUIT_DELAY: Duration = Duration::from_secs(1);
const SELFTEST_DEFAULT_STOP_AFTER: Duration = Duration::from_millis(8000);
const SHOTS_DEFAULT_STEP: Duration = Duration::from_millis(2500);
/// A state is logged this long after it was entered (its height animation has settled).
const STATE_SETTLE: Duration = Duration::from_millis(600);
const DEMO_TIMEOUT: Duration = Duration::from_secs(120);
/// The dictated text stays on screen this long before Gönder.
const DEMO_READ_TEXT: Duration = Duration::from_millis(2000);
/// Pause this far into the agent's spoken acknowledgement.
const DEMO_PAUSE_INTO_ACK: Duration = Duration::from_millis(1600);
/// Quiet time after the acknowledgement before the metrics drawer opens.
const DEMO_AFTER_ACK: Duration = Duration::from_millis(800);

/// `shots` walk, in order. `permissions` and `computer` need the matching computer-use state
/// (the demo fixture provides it) and are skipped after `SHOT_WAIT` without it. The `voice_*`
/// states walk the row's download (demo: the fixture's voice states; a `download_fails` voice
/// fails once, then the retry succeeds) and are skipped at once when no voice row can appear.
const SHOTS: [&str; 13] = [
    "full",
    "voice_downloading",
    "voice_failed",
    "voice_ready",
    "permissions",
    "selected",
    "dock",
    "drawer",
    "computer",
    "compact",
    "empty_search",
    "error",
    "new_task",
];
/// How long a `shots` state waits for its computer-use precondition (the demo fixture's first
/// session starts 46 s after launch).
const SHOT_WAIT: Duration = Duration::from_secs(45);
/// Demo dictation (~2.5 s) plus the History fill: the `new_task` shot stays up this much longer.
const SHOT_DICTATION: Duration = Duration::from_millis(3500);

/// `hero`: the run may last this long (the agent's work plus its narration).
const HERO_TIMEOUT: Duration = Duration::from_secs(600);
/// The empty list stays on screen this long before `+`.
const HERO_INTRO: Duration = Duration::from_millis(2500);
/// The dock's slide-in before typing starts.
const HERO_DOCK_OPEN: Duration = Duration::from_millis(700);
/// One typed character every this long.
const HERO_TYPE_EVERY: Duration = Duration::from_millis(35);
/// `t3 thread start` must have answered by then (else the run quits).
const HERO_START_TIMEOUT: Duration = Duration::from_secs(60);
/// After the run ended, its channel stays quiet this long before the panel quits.
const HERO_QUIET: Duration = Duration::from_millis(3000);

/// `clip:listen`: the catch-up line plus two live steps, then the clip ends.
const CLIP_LISTEN_LINES: usize = 3;
/// A `Quiet` beat waits this long (after its predecessor's hold) for speech to start before it
/// counts silence.
const CLIP_SPEECH_GRACE: Duration = Duration::from_millis(1000);
/// Silence that ends a clip's narration.
const CLIP_QUIET: Duration = Duration::from_millis(800);

/// `demo` walkthrough and `clip:*` beats. Each beat runs once its gate opens (see
/// `Panel::demo_gate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Beat {
    /// The list has appeared.
    List,
    /// Select the clip's thread, else the first running thread.
    Select,
    /// Unmute it: catch-up line, then live narration of its steps.
    Listen,
    /// A thread started waiting for the user (the attention card slid in).
    Attention,
    /// The attention card's jump: select the waiting thread.
    Jump,
    /// Its Cevapla: dock opens, dictation starts.
    Reply,
    /// The dictated text has been on screen for `DEMO_READ_TEXT`: Gönder.
    Send,
    /// The agent's acknowledgement started speaking.
    Ack,
    Pause,
    Resume,
    /// Compact mode while the acknowledgement is still playing (the player is all that shows).
    Compact,
    Expand,
    /// The acknowledgement has finished: the metrics drawer.
    Drawer,
    DrawerClose,
    /// Press `İzin ver` on the computer-use permission row (demo: granted ~1.5 s later).
    Permissions,
    /// An agent session started: the computer-use banner is up.
    Computer,
    /// `Durdur`, once.
    ComputerStop,
    /// `+`: the dock in new-task mode, dictation starts.
    NewTask,
    /// The dictated task has been on screen for `DEMO_READ_TEXT`: Gönder → `start_task`.
    TaskSend,
    /// `TaskStarted` arrived and the new thread is selected (and narrated).
    TaskSelected,
    /// Turn the clip's thread on without selecting it (catch-up line, then live narration).
    Tune,
    /// The listened thread has finished speaking this many utterances.
    Heard(usize),
    /// The listened thread's run ended (its status left `running`).
    Finished,
    /// Speech happened and every channel has been quiet for `CLIP_QUIET`.
    Quiet,
    /// Asks the recorder (`tools/record-demo.sh`) to switch its private copy of T3's settings
    /// to the light appearance.
    AskLight,
    /// The panel followed: its theme is light.
    Light,
    /// Asks the recorder to switch back to dark.
    AskDark,
    /// The panel's theme is dark again.
    Dark,
    Done,
}

const BEATS: [Beat; 22] = [
    Beat::List,
    Beat::Permissions,
    Beat::Select,
    Beat::Listen,
    Beat::Attention,
    Beat::Jump,
    Beat::Reply,
    Beat::Send,
    Beat::Ack,
    Beat::Pause,
    Beat::Resume,
    Beat::Compact,
    Beat::Expand,
    Beat::Drawer,
    Beat::DrawerClose,
    Beat::Computer,
    Beat::ComputerStop,
    Beat::NewTask,
    Beat::TaskSend,
    Beat::TaskSelected,
    // The new thread's spoken acknowledgement.
    Beat::Heard(1),
    Beat::Done,
];

/// `clip:<name>[:<thread>]`: one feature per run, recorded for the README clips
/// (`tools/record-demo.sh`, `tools/build-media.py`). `<thread>` is the thread the clip selects or
/// tunes in to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clip {
    /// Select a running thread, unmute it: catch-up line, then live steps.
    Listen,
    /// Listen to a thread until its run ends and its 🔊 summary has been spoken.
    Summary,
    /// A tuned-in thread starts waiting: the attention card, then its jump.
    Attention,
    /// Attention card → Cevapla → dictation → Gönder → the spoken acknowledgement.
    Reply,
    /// `+` → dictation → Gönder → the new thread is selected and narrated.
    NewTask,
    /// An agent session's banner, then `Durdur`.
    Computer,
    /// T3's appearance switches to light and back; the panel follows.
    Theme,
}

impl Clip {
    const ALL: [Clip; 7] =
        [Clip::Listen, Clip::Summary, Clip::Attention, Clip::Reply, Clip::NewTask, Clip::Computer, Clip::Theme];

    fn name(self) -> &'static str {
        match self {
            Clip::Listen => "listen",
            Clip::Summary => "summary",
            Clip::Attention => "attention",
            Clip::Reply => "reply",
            Clip::NewTask => "new-task",
            Clip::Computer => "computer",
            Clip::Theme => "theme",
        }
    }

    /// Every clip starts by granting the demo's computer-use permissions, so the permission row
    /// is gone (and its toast faded) before the story starts.
    fn beats(self) -> &'static [Beat] {
        match self {
            Clip::Listen => {
                &[Beat::List, Beat::Permissions, Beat::Select, Beat::Listen, Beat::Heard(CLIP_LISTEN_LINES), Beat::Done]
            }
            Clip::Summary => {
                &[Beat::List, Beat::Permissions, Beat::Select, Beat::Listen, Beat::Finished, Beat::Quiet, Beat::Done]
            }
            Clip::Attention => {
                &[Beat::List, Beat::Permissions, Beat::Tune, Beat::Attention, Beat::Jump, Beat::Quiet, Beat::Done]
            }
            Clip::Reply => &[
                Beat::List,
                Beat::Permissions,
                Beat::Attention,
                Beat::Jump,
                Beat::Reply,
                Beat::Send,
                Beat::Ack,
                Beat::Quiet,
                Beat::Done,
            ],
            Clip::NewTask => &[
                Beat::List,
                Beat::Permissions,
                Beat::NewTask,
                Beat::TaskSend,
                Beat::TaskSelected,
                // The new thread's spoken acknowledgement.
                Beat::Heard(1),
                Beat::Done,
            ],
            Clip::Computer => &[
                Beat::List,
                Beat::Permissions,
                Beat::Tune,
                Beat::Computer,
                Beat::ComputerStop,
                Beat::Quiet,
                Beat::Done,
            ],
            Clip::Theme => &[
                Beat::List,
                Beat::Permissions,
                Beat::Select,
                Beat::Tune,
                Beat::AskLight,
                Beat::Light,
                Beat::AskDark,
                Beat::Dark,
                Beat::Quiet,
                Beat::Done,
            ],
        }
    }
}

impl Beat {
    fn name(self) -> &'static str {
        match self {
            Beat::List => "list",
            Beat::Select => "select",
            Beat::Listen => "listen",
            Beat::Attention => "attention",
            Beat::Jump => "attention_jump",
            Beat::Reply => "reply",
            Beat::Send => "send",
            Beat::Ack => "ack",
            Beat::Pause => "pause",
            Beat::Resume => "resume",
            Beat::Drawer => "drawer",
            Beat::DrawerClose => "drawer_close",
            Beat::Compact => "compact",
            Beat::Expand => "expand",
            Beat::Permissions => "permissions",
            Beat::Computer => "computer",
            Beat::ComputerStop => "computer_stop",
            Beat::NewTask => "new_task",
            Beat::TaskSend => "task_send",
            Beat::TaskSelected => "task_selected",
            Beat::Done => "done",
            Beat::Tune => "tune",
            Beat::Heard(_) => "heard",
            Beat::Finished => "finished",
            Beat::Quiet => "quiet",
            Beat::AskLight => "t3_light",
            Beat::Light => "light",
            Beat::AskDark => "t3_dark",
            Beat::Dark => "dark",
        }
    }

    /// Minimum time the panel stays in the state this beat entered before the next beat. Clips
    /// start sooner, wait out the permission toast and hold the computer-use banner longer.
    fn hold(self, clip: bool) -> Duration {
        match self {
            Beat::List if clip => Duration::from_millis(1000),
            Beat::List => Duration::from_millis(3000),
            Beat::Select => Duration::from_millis(1500),
            // The catch-up line and the first narrated steps.
            Beat::Listen => Duration::from_millis(6000),
            Beat::Attention => Duration::from_millis(2500),
            Beat::Jump => Duration::from_millis(1500),
            Beat::Ack => DEMO_PAUSE_INTO_ACK,
            Beat::Pause => Duration::from_millis(1800),
            Beat::Resume => Duration::from_millis(800),
            Beat::Compact => Duration::from_millis(3000),
            Beat::Expand => Duration::from_millis(1200),
            Beat::Drawer => Duration::from_millis(3000),
            Beat::DrawerClose => Duration::from_millis(1500),
            Beat::Permissions if clip => Duration::from_millis(4200),
            Beat::Permissions => Duration::from_millis(2500),
            Beat::Computer if clip => Duration::from_millis(4500),
            Beat::Computer => Duration::from_millis(3500),
            Beat::ComputerStop => Duration::from_millis(2500),
            // The new thread's first narrated line.
            Beat::TaskSelected => Duration::from_millis(4000),
            Beat::Tune => Duration::from_millis(2000),
            Beat::Light => Duration::from_millis(3000),
            Beat::Dark => Duration::from_millis(2000),
            Beat::Reply
            | Beat::Send
            | Beat::NewTask
            | Beat::TaskSend
            | Beat::Done
            | Beat::Heard(_)
            | Beat::Finished
            | Beat::Quiet
            | Beat::AskLight
            | Beat::AskDark => Duration::ZERO,
        }
    }
}

/// Progress of the `demo` walkthrough or a clip.
pub(super) struct DemoRun {
    /// `BEATS` or the clip's beats.
    beats: &'static [Beat],
    /// A `clip:*` run (`Beat::hold`).
    clip: bool,
    /// The clip's thread (`clip:<name>:<thread>`).
    target: Option<String>,
    /// Index into `beats` of the next beat.
    beat: usize,
    /// The next beat may run from here on.
    at: Instant,
    /// The narrated thread.
    listen: Option<String>,
    /// Utterances `listen` has finished speaking.
    heard: usize,
    /// A channel was heard speaking while a `Quiet` beat waited.
    spoke: bool,
    /// The waiting thread that gets the reply.
    reply_to: Option<String>,
    /// When the dictated text appeared in the dock.
    filled_at: Option<Instant>,
    /// When the reply thread's channel (`Drawer`) or every channel (`Quiet`) went quiet.
    quiet_at: Option<Instant>,
    deadline: Instant,
}

/// `hero` steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeroStep {
    /// The (empty) list is up; `+` at `HeroRun.at`.
    Intro,
    /// Typing the task into the dock, one character at a time.
    Typing,
    /// Typed; Gönder at `HeroRun.at`.
    Send,
    /// Gönder taken; waiting for `TaskStarted` and the new thread to be listed and selected.
    Starting,
    /// The new thread is selected and narrated; waiting for its run to end and go quiet.
    Running,
}

/// Progress of the `hero` run.
pub(super) struct HeroRun {
    step: HeroStep,
    at: Instant,
    /// Characters of the task typed so far.
    typed: usize,
    sent_at: Option<Instant>,
    thread: Option<String>,
    /// The thread has been seen `running` (its end is a status change away from it).
    seen_running: bool,
    quiet_at: Option<Instant>,
    deadline: Instant,
}

/// `TELLAL_SELFTEST` value.
#[derive(Clone, Debug)]
pub enum SelfTestMode {
    Off,
    /// `1`: listen to the first completed thread, wait for its audio to finish, mute all, quit.
    Listen,
    /// `reply:<thread_id>`: the Cevapla flow, then the Gönder path after `stop_after`.
    Reply {
        thread_id: String,
        stop_after: Duration,
        nosend: bool,
    },
    /// `shots`: one UI state every `step` (`TELLAL_SELFTEST_STEP_MS`, default 2500).
    Shots {
        step: Duration,
    },
    /// `demo`: the launch-video walkthrough (`BEATS`) on the demo fixture (`TELLAL_DEMO`).
    Demo,
    /// `clip:<name>[:<thread>]`: one feature's beats on the demo fixture.
    Clip {
        clip: Clip,
        thread: Option<String>,
    },
    /// `hero` (+ `TELLAL_SELFTEST_TASK`): one real task started from the new-task dock.
    Hero {
        task: String,
    },
}

impl SelfTestMode {
    pub fn from_env() -> SelfTestMode {
        let Ok(value) = std::env::var("TELLAL_SELFTEST") else {
            return SelfTestMode::Off;
        };
        let env_ms = |key: &str| std::env::var(key).ok().and_then(|v| v.trim().parse().ok()).map(Duration::from_millis);
        match value.trim() {
            "1" => return SelfTestMode::Listen,
            "shots" => {
                return SelfTestMode::Shots { step: env_ms("TELLAL_SELFTEST_STEP_MS").unwrap_or(SHOTS_DEFAULT_STEP) };
            }
            "demo" => return SelfTestMode::Demo,
            "hero" => {
                return std::env::var("TELLAL_SELFTEST_TASK")
                    .ok()
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .map_or(SelfTestMode::Off, |task| SelfTestMode::Hero { task });
            }
            _ => {}
        }
        if let Some(spec) = value.trim().strip_prefix("clip:") {
            let (name, thread) = spec.split_once(':').unwrap_or((spec, ""));
            let thread = Some(thread.trim().to_owned()).filter(|t| !t.is_empty());
            return Clip::ALL
                .into_iter()
                .find(|c| c.name() == name.trim())
                .map_or(SelfTestMode::Off, |clip| SelfTestMode::Clip { clip, thread });
        }
        let Some(thread_id) = value.strip_prefix("reply:").map(str::trim).filter(|t| !t.is_empty()) else {
            return SelfTestMode::Off;
        };
        let stop_after = env_ms("TELLAL_SELFTEST_STOP_AFTER_MS").unwrap_or(SELFTEST_DEFAULT_STOP_AFTER);
        let nosend = std::env::var_os("TELLAL_SELFTEST_NOSEND").is_some_and(|v| v == "1");
        SelfTestMode::Reply { thread_id: thread_id.to_owned(), stop_after, nosend }
    }

    pub fn to_json(&self) -> Value {
        match self {
            SelfTestMode::Off => Value::Null,
            SelfTestMode::Listen => json!({"mode": "listen"}),
            SelfTestMode::Reply { thread_id, stop_after, nosend } => json!({
                "mode": "reply", "thread": thread_id,
                "stop_after_ms": stop_after.as_millis() as u64, "nosend": nosend,
            }),
            SelfTestMode::Shots { step } => {
                json!({"mode": "shots", "step_ms": step.as_millis() as u64})
            }
            SelfTestMode::Demo => json!({"mode": "demo"}),
            SelfTestMode::Clip { clip, thread } => {
                json!({"mode": "clip", "clip": clip.name(), "thread": thread})
            }
            SelfTestMode::Hero { task } => json!({"mode": "hero", "task": task}),
        }
    }

    /// The walkthrough runs while the user keeps working elsewhere: it never activates the
    /// panel (nor re-activates another app), so it cannot take the keyboard.
    pub(super) fn leaves_focus(&self) -> bool {
        matches!(
            self,
            SelfTestMode::Demo | SelfTestMode::Clip { .. } | SelfTestMode::Shots { .. } | SelfTestMode::Hero { .. }
        )
    }
}

pub(super) enum SelfTest {
    Off,
    WaitingForThreads {
        deadline: Instant,
    },
    Listening {
        thread_id: String,
        deadline: Instant,
    },
    /// Dock open; take the Gönder path at `stop_at`.
    Replying {
        stop_at: Instant,
        deadline: Instant,
    },
    /// Gönder taken; waiting for the send (or the no-send log) to finish.
    ReplySubmitted {
        deadline: Instant,
    },
    /// Showing `SHOTS[step]` until `next_at`; `waiting` = since when the state at `step` waits
    /// for its precondition.
    Shots {
        step: usize,
        next_at: Instant,
        waiting: Option<Instant>,
    },
    Demo(DemoRun),
    Hero(HeroRun),
    Quitting {
        at: Instant,
    },
}

impl SelfTest {
    pub(super) fn new(mode: &SelfTestMode, now: Instant) -> SelfTest {
        match mode {
            SelfTestMode::Off => SelfTest::Off,
            // Starts on an empty list (a fresh project): no need to wait for threads.
            SelfTestMode::Hero { .. } => SelfTest::Hero(HeroRun {
                step: HeroStep::Intro,
                at: now + HERO_INTRO,
                typed: 0,
                sent_at: None,
                thread: None,
                seen_running: false,
                quiet_at: None,
                deadline: now + HERO_TIMEOUT,
            }),
            _ => SelfTest::WaitingForThreads { deadline: now + SELFTEST_TIMEOUT },
        }
    }
}

impl Panel {
    pub(super) fn selftest_deadline(&self) -> Instant {
        match &self.selftest {
            SelfTest::WaitingForThreads { deadline }
            | SelfTest::Listening { deadline, .. }
            | SelfTest::Replying { deadline, .. }
            | SelfTest::ReplySubmitted { deadline } => *deadline,
            SelfTest::Demo(run) => run.deadline,
            SelfTest::Hero(run) => run.deadline,
            SelfTest::Off | SelfTest::Shots { .. } | SelfTest::Quitting { .. } => Instant::now() + SELFTEST_TIMEOUT,
        }
    }

    pub(super) fn selftest_threads_ready(&mut self, window: &mut Window, cx: &mut Context<Panel>) {
        if !matches!(self.selftest, SelfTest::WaitingForThreads { .. }) || self.threads.is_empty() {
            return;
        }
        let threads = self.threads.clone();
        self.start_selftest(&threads, window, cx);
    }

    fn start_selftest(&mut self, threads: &[ThreadView], window: &mut Window, cx: &mut Context<Panel>) {
        let deadline = self.selftest_deadline();
        match self.selftest_mode.clone() {
            SelfTestMode::Off | SelfTestMode::Hero { .. } => {}
            SelfTestMode::Listen => {
                let chosen = threads.iter().find(|t| t.status == "completed").unwrap_or(&threads[0]);
                self.core.log_ui(
                    "ui.selftest",
                    json!({
                        "mode": "listen", "threads": threads.len(), "thread": chosen.id,
                        "status": chosen.status, "title": chosen.title,
                    }),
                );
                self.core.log_ui("ui.click", json!({"thread": chosen.id, "action": "listen", "source": "selftest"}));
                self.core.set_listening(&chosen.id, true);
                self.selftest = SelfTest::Listening { thread_id: chosen.id.clone(), deadline };
            }
            SelfTestMode::Reply { thread_id, stop_after, nosend } => {
                let listed = threads.iter().any(|t| t.id == thread_id);
                self.core.log_ui(
                    "ui.selftest",
                    json!({
                        "mode": "reply", "threads": threads.len(), "thread": thread_id, "listed": listed,
                        "stop_after_ms": stop_after.as_millis() as u64, "nosend": nosend,
                        "frontmost_before": mac::frontmost_app().map(|a| a.to_json()),
                    }),
                );
                self.selftest = SelfTest::Replying { stop_at: Instant::now() + stop_after, deadline };
                self.open_reply(&thread_id, "selftest", true, window, cx);
            }
            SelfTestMode::Shots { .. } => {
                self.core.log_ui("ui.selftest", json!({"mode": "shots", "threads": threads.len(), "states": SHOTS}));
                self.selftest = SelfTest::Shots { step: 0, next_at: Instant::now(), waiting: None };
            }
            SelfTestMode::Demo => self.start_demo(&BEATS, None, None, threads.len()),
            SelfTestMode::Clip { clip, thread } => self.start_demo(clip.beats(), Some(clip), thread, threads.len()),
        }
    }

    /// Recorded runs: the user's pointer (the window sits where a live panel would) must not
    /// hover, click or show tooltips in the take. AppKit stops sending mouse events to the
    /// window, and gpui forgets the position it last saw (or read at launch).
    pub(super) fn ignore_pointer(&self) {
        if let Some(native) = self.native {
            native.ignore_mouse();
            native.forget_pointer();
        }
    }

    fn start_demo(&mut self, beats: &'static [Beat], clip: Option<Clip>, target: Option<String>, threads: usize) {
        let names: Vec<&str> = beats.iter().map(|b| b.name()).collect();
        let mode = if clip.is_some() { "clip" } else { "demo" };
        self.core.log_ui(
            "ui.selftest",
            json!({
                "mode": mode, "clip": clip.map(Clip::name), "thread": target, "threads": threads, "beats": names,
            }),
        );
        self.ignore_pointer();
        let now = Instant::now();
        self.selftest = SelfTest::Demo(DemoRun {
            beats,
            clip: clip.is_some(),
            target,
            beat: 0,
            at: now,
            listen: None,
            heard: 0,
            spoke: false,
            reply_to: None,
            filled_at: None,
            quiet_at: None,
            deadline: now + DEMO_TIMEOUT,
        });
    }

    pub(super) fn selftest_speaking_started(&mut self, thread_id: &str, cx: &App) {
        if matches!(&self.selftest, SelfTest::Listening { thread_id: t, .. } if t == thread_id) {
            self.log_selftest_state("now_playing", 0, cx);
        }
    }

    pub(super) fn selftest_speaking_finished(&mut self, thread_id: &str) {
        if let SelfTest::Demo(run) = &mut self.selftest {
            if run.listen.as_deref() == Some(thread_id) {
                run.heard += 1;
            }
            return;
        }
        if matches!(&self.selftest, SelfTest::Listening { thread_id: t, .. } if t == thread_id) {
            self.core.log_ui("ui.selftest_mute_all", json!({"thread": thread_id}));
            self.core.mute_all();
            self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_QUIT_DELAY };
        }
    }

    fn log_selftest_state(&self, state: &str, step: usize, cx: &App) {
        self.core.log_ui(
            "ui.selftest_state",
            json!({
                "state": state, "step": step, "window_number": self.native.map(|n| n.number()),
                "rows": self.model.row_ids.len(), "selected": self.selected, "compact": self.compact,
                "drawer": self.drawer_open, "search": self.search.query, "dock": self.reply.is_some(),
                "error": self.error.is_some(), "target_height": self.content_height(),
                "new_task": self.reply.as_ref().is_some_and(|d| d.is_new_task()),
                "dock_text_chars": self.input.read(cx).text().chars().count(),
                "computer_sessions": self.computer.view.sessions.len(),
                "permission_row": self.computer.needs_permission(),
                "lang": self.s.lang.code(),
                "voice_row": self.voice_row().map(|v| json!({"id": v.id, "state": format!("{:?}", v.state)})),
                "uptime_ms": ms(self.process_start.elapsed()),
            }),
        );
    }

    /// `ui.selftest_state` once the state just entered has settled.
    fn log_state_settled(&self, state: &'static str, step: usize, cx: &mut Context<Panel>) {
        self.log_state_after(state, step, STATE_SETTLE, cx);
    }

    fn log_state_after(&self, state: &'static str, step: usize, delay: Duration, cx: &mut Context<Panel>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |panel, cx| panel.log_selftest_state(state, step, cx));
        })
        .detach();
    }

    /// Whether the computer-use or voice state a `shots` state shows is there (demo fixture).
    fn shot_ready(&self, name: &str) -> bool {
        let voice = self.voice_row().map(|v| &v.state);
        match name {
            // Each computer-use state on its own: the permission row before any session, the
            // banner once granted (an agent cannot act without permissions) and the toast gone.
            "permissions" => self.computer.needs_permission() && !self.computer.active(),
            "computer" => self.computer.active() && !self.computer.needs_permission() && self.toast.is_none(),
            "voice_downloading" => matches!(voice, Some(VoiceState::Downloading { done_bytes, .. }) if *done_bytes > 0),
            "voice_failed" => matches!(voice, Some(VoiceState::Failed(_))),
            "voice_ready" => voice.is_none(),
            _ => true,
        }
    }

    /// A `shots` state whose precondition can no longer come (skipped without waiting).
    fn shot_hopeless(&self, name: &str) -> bool {
        let voice = self.voice_row().map(|v| &v.state);
        match name {
            "voice_downloading" | "voice_failed" | "voice_ready" if self.voices.is_empty() || !self.demo => true,
            "voice_downloading" => voice.is_none(),
            // A download that ended ready never fails.
            "voice_failed" => voice.is_none(),
            _ => false,
        }
    }

    /// Once, when a `shots` state starts waiting: demo mode grants the permissions for
    /// `computer` and presses the voice row's Download / Retry.
    fn prepare_shot(&mut self, name: &str, cx: &mut Context<Panel>) {
        if name == "computer" && self.demo && self.computer.needs_permission() {
            self.grant_computer_permissions("selftest", cx);
        }
        let idle = matches!(self.voice_row().map(|v| &v.state), Some(VoiceState::Missing | VoiceState::Failed(_)));
        if matches!(name, "voice_downloading" | "voice_ready") && self.demo && idle {
            self.download_needed_voice("selftest", cx);
        }
    }

    /// Whether `beat` may run now (beyond its predecessor's hold).
    fn demo_gate(&self, beat: Beat, run: &mut DemoRun, now: Instant, cx: &mut Context<Panel>) -> bool {
        let reply_channel = run.reply_to.as_deref().and_then(|id| self.channel(id));
        match beat {
            Beat::Attention => !self.waiting().is_empty(),
            Beat::Send | Beat::TaskSend => {
                if run.filled_at.is_none() && !self.input.read(cx).text().trim().is_empty() {
                    run.filled_at = Some(now);
                    self.log_state_settled("dictated", run.beat, cx);
                }
                run.filled_at.is_some_and(|at| now >= at + DEMO_READ_TEXT)
            }
            Beat::Ack => reply_channel.is_some_and(|c| c.speaking),
            Beat::Computer => self.computer.active(),
            // The dock closed on `TaskStarted` and the new thread was listed and selected.
            Beat::TaskSelected => self.reply.is_none() && self.pending_task.is_none(),
            Beat::Drawer => {
                if reply_channel.is_some_and(|c| c.speaking || c.paused || c.queued > 0) {
                    run.quiet_at = None;
                    return false;
                }
                now >= *run.quiet_at.get_or_insert(now) + DEMO_AFTER_ACK
            }
            Beat::Heard(lines) => run.heard >= lines,
            Beat::Finished => {
                run.listen.as_deref().and_then(|id| self.thread(id)).is_some_and(|t| t.status != "running")
            }
            Beat::Quiet => {
                if self.channels.values().any(|c| c.speaking || c.paused || c.queued > 0) {
                    run.spoke = true;
                    run.quiet_at = None;
                    return false;
                }
                if !run.spoke && now < run.at + CLIP_SPEECH_GRACE {
                    return false;
                }
                now >= *run.quiet_at.get_or_insert(now) + CLIP_QUIET
            }
            Beat::Light => !cx.global::<Theme>().view().dark,
            Beat::Dark => cx.global::<Theme>().view().dark,
            _ => true,
        }
    }

    /// Runs the next `demo` beat if its gate is open.
    fn advance_demo(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Panel>) {
        let SelfTest::Demo(mut run) = std::mem::replace(&mut self.selftest, SelfTest::Off) else {
            return;
        };
        let beat = run.beats[run.beat];
        if !self.demo_gate(beat, &mut run, now, cx) {
            self.selftest = SelfTest::Demo(run);
            return;
        }
        let thread = match beat {
            Beat::List | Beat::Attention | Beat::Ack | Beat::Quiet | Beat::AskLight | Beat::AskDark => None,
            Beat::Light | Beat::Dark | Beat::Heard(_) | Beat::Finished => run.listen.clone(),
            Beat::Select => {
                let waiting = |id: &str| self.channel(id).is_some_and(|c| c.waiting_for_user);
                run.heard = 0;
                run.listen = match &run.target {
                    Some(id) => Some(id.clone()),
                    None => {
                        let running = self.threads.iter().find(|t| t.status == "running" && !waiting(&t.id));
                        running.or(self.threads.first()).map(|t| t.id.clone())
                    }
                };
                self.select(run.listen.clone(), "selftest");
                run.listen.clone()
            }
            Beat::Listen | Beat::Tune => {
                // `Tune` turns the clip's thread on without selecting it.
                if beat == Beat::Tune && run.target.is_some() {
                    run.heard = 0;
                    run.listen = run.target.clone();
                }
                if let Some(id) = &run.listen {
                    self.toggle_listen(id, "selftest");
                }
                run.listen.clone()
            }
            Beat::Jump => {
                self.jump_to_waiting("selftest", cx);
                run.reply_to = self.selected.clone();
                run.reply_to.clone()
            }
            Beat::Reply => {
                if let Some(id) = run.reply_to.clone() {
                    self.log_action("selftest", "reply", Some(&id), json!({}));
                    self.open_reply(&id, "selftest", true, window, cx);
                }
                run.reply_to.clone()
            }
            Beat::Send => {
                self.submit_reply("selftest", cx);
                run.reply_to.clone()
            }
            Beat::Pause | Beat::Resume => {
                if let Some(id) = &run.reply_to {
                    self.player_action("selftest", "toggle_pause", id);
                }
                run.reply_to.clone()
            }
            Beat::Drawer => {
                self.set_drawer(true, "selftest", cx);
                None
            }
            Beat::DrawerClose => {
                self.set_drawer(false, "selftest", cx);
                None
            }
            Beat::Compact => {
                self.set_compact(true, "selftest", cx);
                None
            }
            Beat::Expand => {
                self.set_compact(false, "selftest", cx);
                None
            }
            Beat::Permissions => {
                // Only the demo's fake daemon is asked; never the real permission flow.
                if self.demo {
                    self.grant_computer_permissions("selftest", cx);
                }
                None
            }
            Beat::Computer => None,
            Beat::ComputerStop => {
                self.stop_computer("selftest", cx);
                None
            }
            Beat::NewTask => {
                run.filled_at = None;
                self.open_new_task("selftest", window, cx);
                None
            }
            Beat::TaskSend => {
                self.submit_reply("selftest", cx);
                None
            }
            Beat::TaskSelected => {
                // Selected and already listened to: its narration counts for `Heard`.
                run.heard = 0;
                run.listen = self.selected.clone();
                run.listen.clone()
            }
            Beat::Done => {
                self.mute_all("selftest");
                None
            }
        };
        self.core.log_ui(
            "ui.selftest_action",
            json!({
                "beat": beat.name(), "step": run.beat, "thread": thread,
                "uptime_ms": ms(now.duration_since(self.process_start)),
            }),
        );
        self.log_state_settled(beat.name(), run.beat, cx);
        run.beat += 1;
        run.at = now + beat.hold(run.clip);
        self.selftest = match run.beats.get(run.beat) {
            Some(_) => SelfTest::Demo(run),
            None => SelfTest::Quitting { at: now + SELFTEST_QUIT_DELAY },
        };
        cx.notify();
    }

    /// Puts the panel into `SHOTS[step]` from a clean state.
    fn enter_shot(&mut self, step: usize, window: &mut Window, cx: &mut Context<Panel>) {
        if self.reply.is_some() {
            self.cancel_reply("selftest", cx);
        }
        self.close_search("selftest", window, cx);
        self.set_drawer(false, "selftest", cx);
        self.set_compact(false, "selftest", cx);
        self.error = None;
        let first = self.model.row_ids.first().cloned();
        match SHOTS[step] {
            "full" | "permissions" | "computer" | "voice_downloading" | "voice_failed" => self.select(None, "selftest"),
            // The row is gone; the drawer lists every voice.
            "voice_ready" => {
                self.select(None, "selftest");
                self.set_drawer(true, "selftest", cx);
            }
            "new_task" => {
                self.open_new_task("selftest", window, cx);
                // Demo mode dictates (fixture text, no Wispr); otherwise type the sample.
                if !self.demo {
                    let text = self.s.sample_task;
                    self.input.update(cx, |input, cx| input.set_text(text, EditSource::Typed, cx));
                }
            }
            // Hover is simulated by selection: a selected row shows the same actions.
            "selected" => self.select(first, "selftest"),
            "dock" => {
                if let Some(id) = first {
                    self.select(Some(id.clone()), "selftest");
                    self.open_reply(&id, "selftest", false, window, cx);
                    let text = self.s.sample_reply;
                    self.input.update(cx, |input, cx| input.set_text(text, EditSource::Typed, cx));
                }
            }
            "drawer" => self.set_drawer(true, "selftest", cx),
            "compact" => self.set_compact(true, "selftest", cx),
            "empty_search" => {
                self.open_search("selftest", window, cx);
                let text = self.s.sample_no_match;
                self.search.input.update(cx, |input, cx| input.set_text(text, EditSource::Typed, cx));
            }
            "error" => self.show_error(self.s.sample_error.into(), self.model.row_ids.first().cloned().map(Retry::Open)),
            _ => {}
        }
        cx.notify();
    }

    /// Returns true if the app is quitting.
    pub(super) fn advance_selftest(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Panel>) -> bool {
        match &self.selftest {
            SelfTest::Off => false,
            SelfTest::Quitting { at } => {
                if now >= *at {
                    self.quit("selftest_done", cx);
                    return true;
                }
                false
            }
            SelfTest::Shots { step, next_at, waiting } if now >= *next_at => {
                let (step, next_at, waiting) = (*step, *next_at, *waiting);
                if step == SHOTS.len() {
                    self.enter_shot(0, window, cx);
                    self.selftest = SelfTest::Quitting { at: now + SELFTEST_QUIT_DELAY };
                    return false;
                }
                let SelfTestMode::Shots { step: every } = self.selftest_mode else {
                    return false;
                };
                let name = SHOTS[step];
                if self.shot_hopeless(name) || !self.shot_ready(name) {
                    let since = waiting.unwrap_or_else(|| {
                        self.prepare_shot(name, cx);
                        now
                    });
                    if now.duration_since(since) < SHOT_WAIT && !self.shot_hopeless(name) {
                        self.selftest = SelfTest::Shots { step, next_at, waiting: Some(since) };
                        return false;
                    }
                    self.core.log_ui(
                        "ui.selftest_state",
                        json!({"state": name, "step": step, "skipped": "precondition not met", "waited_ms": ms(now.duration_since(since))}),
                    );
                    self.selftest = SelfTest::Shots { step: step + 1, next_at: now, waiting: None };
                    return false;
                }
                self.enter_shot(step, window, cx);
                let dictates = name == "new_task" && self.demo;
                let hold = if dictates { every + SHOT_DICTATION } else { every };
                self.selftest = SelfTest::Shots { step: step + 1, next_at: now + hold, waiting: None };
                self.log_state_after(name, step, if dictates { SHOT_DICTATION } else { STATE_SETTLE }, cx);
                false
            }
            SelfTest::Replying { stop_at, .. } if now >= *stop_at => {
                self.core.log_ui("ui.selftest_stop", json!({"after_ms": ms(now.duration_since(self.process_start))}));
                self.submit_reply("selftest", cx);
                false
            }
            SelfTest::Demo(run) if now >= run.deadline => {
                let beat = run.beats[run.beat].name();
                self.core.log_ui(
                    "ui.selftest_timeout",
                    json!({"phase": "demo", "beat": beat, "timeout_s": DEMO_TIMEOUT.as_secs()}),
                );
                self.quit("selftest_timeout", cx);
                true
            }
            SelfTest::Demo(run) if now >= run.at => {
                self.advance_demo(now, window, cx);
                false
            }
            SelfTest::Hero(_) => self.advance_hero(now, window, cx),
            SelfTest::WaitingForThreads { deadline }
            | SelfTest::Listening { deadline, .. }
            | SelfTest::Replying { deadline, .. }
            | SelfTest::ReplySubmitted { deadline }
                if now >= *deadline =>
            {
                let phase = match &self.selftest {
                    SelfTest::WaitingForThreads { .. } => "waiting_for_threads",
                    SelfTest::Listening { .. } => "listening",
                    SelfTest::Replying { .. } => "replying",
                    _ => "reply_submitted",
                };
                self.core
                    .log_ui("ui.selftest_timeout", json!({"phase": phase, "timeout_s": SELFTEST_TIMEOUT.as_secs()}));
                self.quit("selftest_timeout", cx);
                true
            }
            _ => false,
        }
    }

    /// One `hero` tick; returns true if the app is quitting. Every step logs `ui.selftest_action`
    /// (`beat` = the step entered) and, once settled, `ui.selftest_state`.
    fn advance_hero(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Panel>) -> bool {
        let SelfTestMode::Hero { task } = &self.selftest_mode else {
            return false;
        };
        let task = task.clone();
        let SelfTest::Hero(mut run) = std::mem::replace(&mut self.selftest, SelfTest::Off) else {
            return false;
        };
        if now >= run.deadline || run.sent_at.is_some_and(|at| run.thread.is_none() && now >= at + HERO_START_TIMEOUT) {
            let timeout = if run.thread.is_none() && run.sent_at.is_some() { HERO_START_TIMEOUT } else { HERO_TIMEOUT };
            self.core.log_ui(
                "ui.selftest_timeout",
                json!({"phase": "hero", "step": format!("{:?}", run.step), "timeout_s": timeout.as_secs()}),
            );
            self.quit("selftest_timeout", cx);
            return true;
        }
        let entered = match run.step {
            // The first poll normally lands within the intro; never wait on it for long.
            HeroStep::Intro if now >= run.at && (self.loaded || now >= run.at + HERO_INTRO) => {
                // The window sits where the user's own panel is: their pointer must not land in it.
                self.ignore_pointer();
                self.core.log_ui("ui.selftest", json!({"mode": "hero", "threads": self.threads.len(), "task": task}));
                self.open_new_task("selftest", window, cx);
                run.step = HeroStep::Typing;
                run.at = now + HERO_DOCK_OPEN;
                Some("new_task")
            }
            HeroStep::Typing if now >= run.at => {
                let total = task.chars().count();
                let typed = (now.duration_since(run.at).as_millis() / HERO_TYPE_EVERY.as_millis()) as usize + 1;
                let typed = typed.min(total);
                if typed > run.typed {
                    run.typed = typed;
                    let end = task.char_indices().nth(typed).map_or(task.len(), |(i, _)| i);
                    self.input.update(cx, |input, cx| input.set_text(&task[..end], EditSource::Typed, cx));
                }
                if typed == total {
                    run.step = HeroStep::Send;
                    run.at = now + DEMO_READ_TEXT;
                    Some("typed")
                } else {
                    None
                }
            }
            HeroStep::Send if now >= run.at => {
                self.submit_reply("selftest", cx);
                run.sent_at = Some(now);
                run.step = HeroStep::Starting;
                Some("task_send")
            }
            // `TaskStarted` closed the dock and the new thread is listed and selected.
            HeroStep::Starting if self.reply.is_none() && self.pending_task.is_none() && self.selected.is_some() => {
                run.thread = self.selected.clone();
                run.step = HeroStep::Running;
                Some("task_selected")
            }
            HeroStep::Running => {
                let id = run.thread.as_deref().unwrap_or_default();
                let running = self.thread(id).is_some_and(|t| t.status == "running");
                run.seen_running |= running;
                let busy = self.channel(id).is_some_and(|c| c.speaking || c.paused || c.queued > 0);
                if !run.seen_running || running || busy {
                    run.quiet_at = None;
                    None
                } else if now >= *run.quiet_at.get_or_insert(now) + HERO_QUIET {
                    self.mute_all("selftest");
                    self.core.log_ui(
                        "ui.selftest_action",
                        json!({"beat": "done", "thread": run.thread, "uptime_ms": ms(now.duration_since(self.process_start))}),
                    );
                    self.selftest = SelfTest::Quitting { at: now + SELFTEST_QUIT_DELAY };
                    cx.notify();
                    return false;
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(beat) = entered {
            self.core.log_ui(
                "ui.selftest_action",
                json!({"beat": beat, "thread": run.thread, "uptime_ms": ms(now.duration_since(self.process_start))}),
            );
            self.log_state_settled(beat, 0, cx);
            cx.notify();
        }
        self.selftest = SelfTest::Hero(run);
        false
    }
}
