//! `T3P_SELFTEST` drivers: `1` (listen to one thread, then mute and quit), `reply:<id>` (the
//! Cevapla flow), `shots` (walks the UI states for screenshots without synthetic clicks; each
//! state logs `ui.selftest_state` with the window number for `screencapture -l`) and `demo` (a
//! choreographed walkthrough of the demo fixture for the launch video; every beat logs
//! `ui.selftest_action` and, once settled, `ui.selftest_state`).

use std::time::{Duration, Instant};

use gpui::{Context, Window};
use serde_json::{Value, json};
use t3_panel_core::ThreadView;

use super::{Panel, Retry, ms};
use crate::input::EditSource;
use crate::mac;

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

/// `shots` walk, in order.
const SHOTS: [&str; 7] = ["full", "selected", "dock", "drawer", "compact", "empty_search", "error"];

/// `demo` walkthrough, in order. Each beat runs once its gate opens (see `Panel::demo_gate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Beat {
    /// The list has appeared.
    List,
    /// Select the first running thread.
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
    Done,
}

const BEATS: [Beat; 15] = [
    Beat::List,
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
    Beat::Done,
];

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
            Beat::Done => "done",
        }
    }

    /// Minimum time the panel stays in the state this beat entered before the next beat.
    fn hold(self) -> Duration {
        match self {
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
            Beat::Reply | Beat::Send | Beat::Done => Duration::ZERO,
        }
    }
}

/// Progress of the `demo` walkthrough.
pub(super) struct DemoRun {
    /// Index into `BEATS` of the next beat.
    beat: usize,
    /// The next beat may run from here on.
    at: Instant,
    /// The narrated running thread.
    listen: Option<String>,
    /// The waiting thread that gets the reply.
    reply_to: Option<String>,
    /// When the dictated text appeared in the dock.
    filled_at: Option<Instant>,
    /// When the reply thread's channel went quiet after the acknowledgement resumed.
    quiet_at: Option<Instant>,
    deadline: Instant,
}

/// `T3P_SELFTEST` value.
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
    /// `shots`: one UI state every `step` (`T3P_SELFTEST_STEP_MS`, default 2500).
    Shots {
        step: Duration,
    },
    /// `demo`: the launch-video walkthrough (`BEATS`) on the demo fixture (`T3P_DEMO`).
    Demo,
}

impl SelfTestMode {
    pub fn from_env() -> SelfTestMode {
        let Ok(value) = std::env::var("T3P_SELFTEST") else {
            return SelfTestMode::Off;
        };
        let env_ms = |key: &str| std::env::var(key).ok().and_then(|v| v.trim().parse().ok()).map(Duration::from_millis);
        match value.trim() {
            "1" => return SelfTestMode::Listen,
            "shots" => {
                return SelfTestMode::Shots { step: env_ms("T3P_SELFTEST_STEP_MS").unwrap_or(SHOTS_DEFAULT_STEP) };
            }
            "demo" => return SelfTestMode::Demo,
            _ => {}
        }
        let Some(thread_id) = value.strip_prefix("reply:").map(str::trim).filter(|t| !t.is_empty()) else {
            return SelfTestMode::Off;
        };
        let stop_after = env_ms("T3P_SELFTEST_STOP_AFTER_MS").unwrap_or(SELFTEST_DEFAULT_STOP_AFTER);
        let nosend = std::env::var_os("T3P_SELFTEST_NOSEND").is_some_and(|v| v == "1");
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
            SelfTestMode::Shots { step } => json!({"mode": "shots", "step_ms": step.as_millis() as u64}),
            SelfTestMode::Demo => json!({"mode": "demo"}),
        }
    }

    /// The walkthrough runs while the user keeps working elsewhere: it never activates the
    /// panel (nor re-activates another app), so it cannot take the keyboard.
    pub(super) fn leaves_focus(&self) -> bool {
        matches!(self, SelfTestMode::Demo)
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
    /// Showing `SHOTS[step]` until `next_at`.
    Shots {
        step: usize,
        next_at: Instant,
    },
    Demo(DemoRun),
    Quitting {
        at: Instant,
    },
}

impl SelfTest {
    pub(super) fn new(mode: &SelfTestMode, now: Instant) -> SelfTest {
        match mode {
            SelfTestMode::Off => SelfTest::Off,
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
            SelfTestMode::Off => {}
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
                self.selftest = SelfTest::Shots { step: 0, next_at: Instant::now() };
            }
            SelfTestMode::Demo => {
                let beats: Vec<&str> = BEATS.iter().map(|b| b.name()).collect();
                self.core.log_ui("ui.selftest", json!({"mode": "demo", "threads": threads.len(), "beats": beats}));
                let now = Instant::now();
                self.selftest = SelfTest::Demo(DemoRun {
                    beat: 0,
                    at: now,
                    listen: None,
                    reply_to: None,
                    filled_at: None,
                    quiet_at: None,
                    deadline: now + DEMO_TIMEOUT,
                });
            }
        }
    }

    pub(super) fn selftest_speaking_started(&mut self, thread_id: &str) {
        if matches!(&self.selftest, SelfTest::Listening { thread_id: t, .. } if t == thread_id) {
            self.log_selftest_state("now_playing", 0);
        }
    }

    pub(super) fn selftest_speaking_finished(&mut self, thread_id: &str) {
        if matches!(&self.selftest, SelfTest::Listening { thread_id: t, .. } if t == thread_id) {
            self.core.log_ui("ui.selftest_mute_all", json!({"thread": thread_id}));
            self.core.mute_all();
            self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_QUIT_DELAY };
        }
    }

    fn log_selftest_state(&self, state: &str, step: usize) {
        self.core.log_ui(
            "ui.selftest_state",
            json!({
                "state": state, "step": step, "window_number": self.native.map(|n| n.number()),
                "rows": self.model.row_ids.len(), "selected": self.selected, "compact": self.compact,
                "drawer": self.drawer_open, "search": self.search.query, "dock": self.reply.is_some(),
                "error": self.error.is_some(), "target_height": self.content_height(),
                "uptime_ms": ms(self.process_start.elapsed()),
            }),
        );
    }

    /// `ui.selftest_state` once the state just entered has settled.
    fn log_state_settled(&self, state: &'static str, step: usize, cx: &mut Context<Panel>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(STATE_SETTLE).await;
            let _ = this.update(cx, |panel, _| panel.log_selftest_state(state, step));
        })
        .detach();
    }

    /// Whether `beat` may run now (beyond its predecessor's hold).
    fn demo_gate(&self, beat: Beat, run: &mut DemoRun, now: Instant, cx: &mut Context<Panel>) -> bool {
        let reply_channel = run.reply_to.as_deref().and_then(|id| self.channel(id));
        match beat {
            Beat::Attention => !self.waiting().is_empty(),
            Beat::Send => {
                if run.filled_at.is_none() && !self.input.read(cx).text().trim().is_empty() {
                    run.filled_at = Some(now);
                    self.log_state_settled("dictated", run.beat, cx);
                }
                run.filled_at.is_some_and(|at| now >= at + DEMO_READ_TEXT)
            }
            Beat::Ack => reply_channel.is_some_and(|c| c.speaking),
            Beat::Drawer => {
                if reply_channel.is_some_and(|c| c.speaking || c.paused || c.queued > 0) {
                    run.quiet_at = None;
                    return false;
                }
                now >= *run.quiet_at.get_or_insert(now) + DEMO_AFTER_ACK
            }
            _ => true,
        }
    }

    /// Runs the next `demo` beat if its gate is open.
    fn advance_demo(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Panel>) {
        let SelfTest::Demo(mut run) = std::mem::replace(&mut self.selftest, SelfTest::Off) else {
            return;
        };
        let beat = BEATS[run.beat];
        if !self.demo_gate(beat, &mut run, now, cx) {
            self.selftest = SelfTest::Demo(run);
            return;
        }
        let thread = match beat {
            Beat::List | Beat::Attention | Beat::Ack => None,
            Beat::Select => {
                let waiting = |id: &str| self.channel(id).is_some_and(|c| c.waiting_for_user);
                let running = self.threads.iter().find(|t| t.status == "running" && !waiting(&t.id));
                run.listen = running.or(self.threads.first()).map(|t| t.id.clone());
                self.select(run.listen.clone(), "selftest");
                run.listen.clone()
            }
            Beat::Listen => {
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
        run.at = now + beat.hold();
        self.selftest = match BEATS.get(run.beat) {
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
            "full" => self.select(None, "selftest"),
            // Hover is simulated by selection: a selected row shows the same actions.
            "selected" => self.select(first, "selftest"),
            "dock" => {
                if let Some(id) = first {
                    self.select(Some(id.clone()), "selftest");
                    self.open_reply(&id, "selftest", false, window, cx);
                    self.input.update(cx, |input, cx| {
                        input.set_text("Testleri çalıştır, sonra özetle.", EditSource::Typed, cx)
                    });
                }
            }
            "drawer" => self.set_drawer(true, "selftest", cx),
            "compact" => self.set_compact(true, "selftest", cx),
            "empty_search" => {
                self.open_search("selftest", window, cx);
                self.search.input.update(cx, |input, cx| input.set_text("zzzz-yok", EditSource::Typed, cx));
            }
            "error" => self.show_error(
                "T3 komutu yanıt vermedi (selftest örneği)".into(),
                self.model.row_ids.first().cloned().map(Retry::Open),
            ),
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
            SelfTest::Shots { step, next_at } if now >= *next_at => {
                let step = *step;
                if step == SHOTS.len() {
                    self.enter_shot(0, window, cx);
                    self.selftest = SelfTest::Quitting { at: now + SELFTEST_QUIT_DELAY };
                    return false;
                }
                let SelfTestMode::Shots { step: every } = self.selftest_mode else {
                    return false;
                };
                self.enter_shot(step, window, cx);
                self.selftest = SelfTest::Shots { step: step + 1, next_at: now + every };
                self.log_state_settled(SHOTS[step], step, cx);
                false
            }
            SelfTest::Replying { stop_at, .. } if now >= *stop_at => {
                self.core.log_ui("ui.selftest_stop", json!({"after_ms": ms(now.duration_since(self.process_start))}));
                self.submit_reply("selftest", cx);
                false
            }
            SelfTest::Demo(run) if now >= run.deadline => {
                let beat = BEATS[run.beat].name();
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
}
