//! `T3P_SELFTEST` drivers: `1` (listen to one thread, then mute and quit), `reply:<id>` (the
//! Cevapla flow) and `shots` (walks the UI states for screenshots without synthetic clicks;
//! each state logs `ui.selftest_state` with the window number for `screencapture -l`).

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

/// `shots` walk, in order.
const SHOTS: [&str; 7] = ["full", "selected", "dock", "drawer", "compact", "empty_search", "error"];

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
        }
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
        match self.selftest {
            SelfTest::WaitingForThreads { deadline }
            | SelfTest::Listening { deadline, .. }
            | SelfTest::Replying { deadline, .. }
            | SelfTest::ReplySubmitted { deadline } => deadline,
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
                // Logged after the state's height animation has settled.
                self.selftest = SelfTest::Shots { step: step + 1, next_at: now + every };
                let state = SHOTS[step];
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(600)).await;
                    let _ = this.update(cx, |panel, _| panel.log_selftest_state(state, step));
                })
                .detach();
                false
            }
            SelfTest::Replying { stop_at, .. } if now >= *stop_at => {
                self.core.log_ui("ui.selftest_stop", json!({"after_ms": ms(now.duration_since(self.process_start))}));
                self.submit_reply("selftest", cx);
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
