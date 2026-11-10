//! The floating panel view: channel rows, header controls, reply dock, live HUD footer, core
//! event pump and the `T3P_SELFTEST` drivers.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::{
    AnyView, App, AppContext, Context, Div, ElementId, Entity, Focusable, FontWeight, MouseButton,
    SharedString, Stateful, Window, div, prelude::*, px, rgb, rgba,
};
use serde_json::{Value, json};
use t3_panel_core::{ChannelView, Core, CoreEvent, LiveStats, Receiver, ThreadView};

use crate::input::{EditSource, InputEvent, TextInput};
use crate::mac::{self, FrontApp};

const PUMP_INTERVAL: Duration = Duration::from_millis(150);
const STATS_INTERVAL: Duration = Duration::from_secs(1);
const SELFTEST_TIMEOUT: Duration = Duration::from_secs(60);
const SELFTEST_QUIT_DELAY: Duration = Duration::from_secs(1);
const SELFTEST_REPLY_QUIT_DELAY: Duration = Duration::from_secs(3);
const SELFTEST_DEFAULT_STOP_AFTER: Duration = Duration::from_millis(8000);
/// Wispr pastes into the focused field itself; only fill from its History DB if nothing came.
const TRANSCRIPT_FILL_DELAY: Duration = Duration::from_millis(600);
const DOCK_CLOSE_DELAY: Duration = Duration::from_millis(1200);
/// Delay before recording which app ended up frontmost after an activate / restore.
const FOCUS_CHECK_DELAY: Duration = Duration::from_millis(500);

const TEXT: u32 = 0xe8e8ed;
const TEXT_DIM: u32 = 0x9a9aa2;
const BLUE: u32 = 0x0a84ff;
const GREEN: u32 = 0x30d158;
const AMBER: u32 = 0xffb340;
const GRAY: u32 = 0x8e8e93;
const RED: u32 = 0xff6961;

struct Row {
    id: SharedString,
    title: SharedString,
    status_color: u32,
    project: String,
    provider: String,
    /// Unix seconds parsed from `updated_at`, `None` if T3 sent something unparseable.
    updated_unix: Option<i64>,
}

impl Row {
    fn new(thread: &ThreadView) -> Row {
        Row {
            id: thread.id.clone().into(),
            title: thread.title.clone().into(),
            status_color: status_color(&thread.status),
            project: thread.project.clone(),
            provider: thread.provider.clone(),
            updated_unix: parse_rfc3339_unix(&thread.updated_at),
        }
    }
}

/// `T3P_SELFTEST` value.
#[derive(Clone, Debug)]
pub enum SelfTestMode {
    Off,
    /// `1`: listen to the first completed thread, wait for its audio to finish, mute all, quit.
    Listen,
    /// `reply:<thread_id>`: the Cevapla flow, then the Gönder path after `stop_after`.
    Reply { thread_id: String, stop_after: Duration, nosend: bool },
}

impl SelfTestMode {
    pub fn from_env() -> SelfTestMode {
        let Ok(value) = std::env::var("T3P_SELFTEST") else {
            return SelfTestMode::Off;
        };
        if value == "1" {
            return SelfTestMode::Listen;
        }
        let Some(thread_id) = value.strip_prefix("reply:").map(str::trim).filter(|t| !t.is_empty()) else {
            return SelfTestMode::Off;
        };
        let stop_after = std::env::var("T3P_SELFTEST_STOP_AFTER_MS")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .map_or(SELFTEST_DEFAULT_STOP_AFTER, Duration::from_millis);
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
        }
    }
}

enum SelfTest {
    Off,
    WaitingForThreads { deadline: Instant },
    Listening { thread_id: String, deadline: Instant },
    /// Dock open; take the Gönder path at `stop_at`.
    Replying { stop_at: Instant, deadline: Instant },
    /// Gönder taken; waiting for the send (or the no-send log) to finish.
    ReplySubmitted { deadline: Instant },
    Quitting { at: Instant },
}

#[derive(Clone, Debug, PartialEq)]
enum ReplyPhase {
    /// Wispr hands-free dictation was requested and has not delivered a transcript yet.
    Dictating,
    /// No dictation running; the user edits/types. `note` explains why (failure, transcript in).
    Editing { note: Option<SharedString> },
    /// Gönder pressed while dictating: `dictation_stop` sent, send as soon as text exists.
    WaitingText,
    Sending,
    Sent { queued: bool },
}

impl ReplyPhase {
    fn name(&self) -> &'static str {
        match self {
            ReplyPhase::Dictating => "dictating",
            ReplyPhase::Editing { .. } => "editing",
            ReplyPhase::WaitingText => "waiting_text",
            ReplyPhase::Sending => "sending",
            ReplyPhase::Sent { .. } => "sent",
        }
    }

    fn dictation_active(&self) -> bool {
        matches!(self, ReplyPhase::Dictating | ReplyPhase::WaitingText)
    }
}

struct ReplyDock {
    thread_id: String,
    title: SharedString,
    phase: ReplyPhase,
    /// Wispr History transcript waiting for the fill-or-compare decision.
    transcript: Option<(String, Instant)>,
    /// Every source that edited the text, in first-seen order (`ui.selftest_reply.source`).
    sources: Vec<EditSource>,
    /// App to re-activate when the dock closes (`None` if we were already frontmost).
    prev_app: Option<FrontApp>,
    opened_at: Instant,
    close_at: Option<Instant>,
}

impl ReplyDock {
    fn chip(&self) -> (SharedString, u32) {
        match &self.phase {
            ReplyPhase::Dictating => ("🎙 Wispr dinliyor — bitince Caps Lock ya da Gönder".into(), BLUE),
            ReplyPhase::Editing { note: Some(note) } => (note.clone(), AMBER),
            ReplyPhase::Editing { note: None } => ("✎ yaz ya da düzenle · Enter gönderir".into(), TEXT_DIM),
            ReplyPhase::WaitingText => ("⏳ metin bekleniyor".into(), AMBER),
            ReplyPhase::Sending => ("⏳ gönderiliyor".into(), AMBER),
            ReplyPhase::Sent { queued: false } => ("✓ gönderildi".into(), GREEN),
            ReplyPhase::Sent { queued: true } => ("✓ gönderildi · sıraya alındı".into(), GREEN),
        }
    }

    fn source_label(&self) -> String {
        let names: Vec<&str> = self.sources.iter().map(|s| s.as_str()).collect();
        names.join("+")
    }
}

/// A row-body click remembered to detect the second click of a double-click.
struct BodyClick {
    thread_id: SharedString,
    at: Instant,
}

pub struct Panel {
    core: Core,
    process_start: Instant,
    ready_logged: bool,
    rows: Vec<Row>,
    channels: HashMap<String, ChannelView>,
    stats: LiveStats,
    stats_at: Instant,
    last_error: Option<SharedString>,
    selected: Option<SharedString>,
    last_body_click: Option<BodyClick>,
    double_click_interval: Duration,
    input: Entity<TextInput>,
    reply: Option<ReplyDock>,
    /// Pending `ui.focus {action: "check"}` (records which app actually ended up frontmost).
    focus_check: Option<(Instant, &'static str)>,
    selftest_mode: SelfTestMode,
    selftest: SelfTest,
}

impl Panel {
    pub fn new(
        core: Core,
        events: Receiver<CoreEvent>,
        process_start: Instant,
        selftest_mode: SelfTestMode,
        window: &mut Window,
        cx: &mut Context<Panel>,
    ) -> Panel {
        let now = Instant::now();
        let input = cx.new(|cx| TextInput::new("yaz, yapıştır ya da Wispr ile dikte et…", cx));
        cx.subscribe_in(&input, window, Panel::on_input_event).detach();
        let selftest = match selftest_mode {
            SelfTestMode::Off => SelfTest::Off,
            _ => SelfTest::WaitingForThreads { deadline: now + SELFTEST_TIMEOUT },
        };
        let mut panel = Panel {
            stats: core.live_stats(),
            channels: core.channels(),
            core,
            process_start,
            ready_logged: false,
            rows: Vec::new(),
            stats_at: now,
            last_error: None,
            selected: None,
            last_body_click: None,
            double_click_interval: mac::double_click_interval(),
            input,
            reply: None,
            focus_check: None,
            selftest_mode,
            selftest,
        };
        panel.refresh_rows();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(PUMP_INTERVAL).await;
                if this.update_in(cx, |panel, window, cx| panel.pump(&events, window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        panel
    }

    fn refresh_rows(&mut self) -> Vec<ThreadView> {
        let threads = self.core.threads();
        self.rows.clear();
        self.rows.extend(threads.iter().map(Row::new));
        threads
    }

    /// Drains core events, refreshes cached state, advances the reply dock and the self-test;
    /// re-renders on change.
    fn pump(&mut self, events: &Receiver<CoreEvent>, window: &mut Window, cx: &mut Context<Panel>) {
        let mut changed = false;
        let mut threads_updated = false;
        let mut channels_updated = false;
        for event in events.try_iter() {
            changed = true;
            match event {
                CoreEvent::ThreadsUpdated => threads_updated = true,
                CoreEvent::ChannelsUpdated => channels_updated = true,
                CoreEvent::SpeakingStarted { .. } => {
                    self.stats = self.core.live_stats();
                    channels_updated = true;
                }
                CoreEvent::SpeakingFinished { thread_id } => {
                    self.stats = self.core.live_stats();
                    channels_updated = true;
                    if matches!(&self.selftest, SelfTest::Listening { thread_id: t, .. } if *t == thread_id) {
                        self.core.log_ui("ui.selftest_mute_all", json!({"thread": thread_id}));
                        self.core.mute_all();
                        self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_QUIT_DELAY };
                    }
                }
                CoreEvent::DictationTranscript { thread_id, text } => self.on_transcript(thread_id, text),
                CoreEvent::DictationFailed { thread_id, error } => self.on_dictation_failed(&thread_id, error),
                CoreEvent::ReplySent { thread_id, ok, queued, detail } => {
                    self.on_reply_sent(&thread_id, ok, queued, detail)
                }
                CoreEvent::Error(message) => self.last_error = Some(message.into()),
                CoreEvent::ThemeChanged => {}
            }
        }
        if threads_updated {
            let threads = self.refresh_rows();
            if matches!(self.selftest, SelfTest::WaitingForThreads { .. }) && !threads.is_empty() {
                self.start_selftest(&threads, window, cx);
            }
        }
        let now = Instant::now();
        if now.duration_since(self.stats_at) >= STATS_INTERVAL {
            self.stats = self.core.live_stats();
            self.stats_at = now;
            channels_updated = true;
            changed = true;
        }
        if channels_updated {
            self.channels = self.core.channels();
        }
        changed |= self.advance_reply(now, cx);
        if let Some((at, phase)) = self.focus_check
            && now >= at
        {
            self.focus_check = None;
            self.core.log_ui(
                "ui.focus",
                json!({
                    "action": "check", "phase": phase,
                    "frontmost": mac::frontmost_app().map(|a| a.to_json()),
                    "panel_key": mac::is_key_window(window),
                }),
            );
        }
        if self.advance_selftest(now, cx) {
            return;
        }
        if changed {
            cx.notify();
        }
    }

    // ── reply dock ──────────────────────────────────────────────────────────────────────────

    /// "Cevapla": open (or retarget) the dock, make the panel key, focus the input and start
    /// Wispr dictation for `thread_id`.
    fn open_reply(&mut self, thread_id: &str, source: &str, window: &mut Window, cx: &mut Context<Panel>) {
        let title = self
            .rows
            .iter()
            .find(|r| r.id.as_ref() == thread_id)
            .map_or_else(|| SharedString::from(thread_id.to_owned()), |r| r.title.clone());
        let mut retarget_from = None;
        let prev_app = match self.reply.take() {
            Some(dock) if dock.thread_id == thread_id => {
                // Same row again: just bring the focus back.
                self.core.log_ui("ui.reply_open", json!({"thread": thread_id, "source": source, "refocus": true}));
                self.reply = Some(dock);
                self.focus_input(window, cx);
                return;
            }
            Some(dock) => {
                if dock.phase.dictation_active() {
                    self.core.dictation_cancel();
                }
                retarget_from = Some(dock.thread_id);
                dock.prev_app
            }
            None => mac::frontmost_app().filter(|a| !a.is_self()),
        };
        self.core.log_ui(
            "ui.reply_open",
            json!({
                "thread": thread_id, "title": title.as_ref(), "source": source,
                "retarget_from": retarget_from, "prev_app": prev_app.as_ref().map(FrontApp::to_json),
            }),
        );
        if retarget_from.is_none() {
            self.core.log_ui(
                "ui.focus",
                json!({"action": "activate", "prev_app": prev_app.as_ref().map(FrontApp::to_json)}),
            );
        }
        let now = Instant::now();
        self.reply = Some(ReplyDock {
            thread_id: thread_id.to_owned(),
            title,
            phase: ReplyPhase::Dictating,
            transcript: None,
            sources: Vec::new(),
            prev_app,
            opened_at: now,
            close_at: None,
        });
        self.focus_input(window, cx);
        self.focus_check = Some((now + FOCUS_CHECK_DELAY, "opened"));
        self.core.dictation_start(thread_id);
        cx.notify();
    }

    fn focus_input(&self, window: &mut Window, cx: &mut Context<Panel>) {
        cx.activate(true);
        window.activate_window();
        let handle = self.input.read(cx).focus_handle(cx);
        window.focus(&handle);
    }

    fn on_input_event(
        &mut self,
        input: &Entity<TextInput>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Panel>,
    ) {
        match event {
            InputEvent::Edited(source) => {
                let chars = input.read(cx).text().chars().count();
                let Some(dock) = self.reply.as_mut() else {
                    return;
                };
                if !dock.sources.contains(source) {
                    dock.sources.push(*source);
                }
                self.core.log_ui(
                    "ui.reply_text",
                    json!({"thread": dock.thread_id, "chars": chars, "source": source.as_str(), "phase": dock.phase.name()}),
                );
                cx.notify();
            }
            InputEvent::Submit => self.submit_reply("enter", cx),
            InputEvent::Cancel => self.cancel_reply("escape", cx),
        }
    }

    /// Gönder / Enter.
    fn submit_reply(&mut self, source: &str, cx: &mut Context<Panel>) {
        let chars = self.input.read(cx).text().trim().chars().count();
        let Some(dock) = self.reply.as_mut() else {
            return;
        };
        self.core.log_ui(
            "ui.reply_submit",
            json!({"thread": dock.thread_id, "source": source, "phase": dock.phase.name(), "chars": chars}),
        );
        let send = match dock.phase {
            ReplyPhase::Dictating => {
                self.core.dictation_stop();
                dock.phase = ReplyPhase::WaitingText;
                chars > 0
            }
            ReplyPhase::Editing { .. } | ReplyPhase::WaitingText => chars > 0,
            ReplyPhase::Sending | ReplyPhase::Sent { .. } => false,
        };
        if send {
            self.send_reply(cx);
        }
        if matches!(self.selftest, SelfTest::Replying { .. }) {
            self.selftest = SelfTest::ReplySubmitted { deadline: self.selftest_deadline() };
        }
        cx.notify();
    }

    fn send_reply(&mut self, cx: &mut Context<Panel>) {
        let text = self.input.read(cx).text().trim().to_owned();
        let Some(dock) = self.reply.as_mut() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let nosend = matches!(self.selftest_mode, SelfTestMode::Reply { nosend: true, .. });
        self.core.log_ui(
            "ui.reply_send",
            json!({
                "thread": dock.thread_id, "chars": text.chars().count(), "source": dock.source_label(),
                "nosend": nosend, "open_ms": ms(dock.opened_at.elapsed()),
            }),
        );
        if nosend {
            self.core.log_ui(
                "ui.selftest_reply",
                json!({"thread": dock.thread_id, "text": text, "source": dock.source_label()}),
            );
            dock.phase = ReplyPhase::Sent { queued: false };
            dock.close_at = Some(Instant::now() + DOCK_CLOSE_DELAY);
            self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_REPLY_QUIT_DELAY };
        } else {
            dock.phase = ReplyPhase::Sending;
            self.core.send_reply(&dock.thread_id, &text);
        }
    }

    /// İptal / Esc.
    fn cancel_reply(&mut self, source: &str, cx: &mut Context<Panel>) {
        let chars = self.input.read(cx).text().chars().count();
        let Some(dock) = self.reply.as_ref() else {
            return;
        };
        if dock.phase.dictation_active() {
            self.core.dictation_cancel();
        }
        self.core.log_ui(
            "ui.reply_cancel",
            json!({"thread": dock.thread_id, "source": source, "phase": dock.phase.name(), "chars": chars}),
        );
        self.close_reply(cx);
    }

    /// Hides the dock, clears the input and gives the keyboard back to the previous app.
    fn close_reply(&mut self, cx: &mut Context<Panel>) {
        let Some(dock) = self.reply.take() else {
            return;
        };
        self.input.update(cx, |input, cx| input.clear(cx));
        let restored = dock.prev_app.as_ref().map(|app| mac::activate_app(app.pid));
        self.core.log_ui(
            "ui.focus",
            json!({
                "action": "restore", "thread": dock.thread_id,
                "prev_app": dock.prev_app.as_ref().map(FrontApp::to_json), "ok": restored,
            }),
        );
        self.focus_check = Some((Instant::now() + FOCUS_CHECK_DELAY, "restored"));
        cx.notify();
    }

    fn on_transcript(&mut self, thread_id: String, text: String) {
        let Some(dock) = self.reply.as_mut().filter(|d| d.thread_id == thread_id) else {
            return;
        };
        if dock.phase == ReplyPhase::Dictating {
            dock.phase = ReplyPhase::Editing { note: Some("✓ Wispr metni geldi · Enter gönderir".into()) };
        }
        dock.transcript = Some((text, Instant::now() + TRANSCRIPT_FILL_DELAY));
    }

    fn on_dictation_failed(&mut self, thread_id: &str, error: String) {
        let Some(dock) = self.reply.as_mut().filter(|d| d.thread_id == thread_id) else {
            self.last_error = Some(error.into());
            return;
        };
        if dock.phase.dictation_active() {
            dock.phase = ReplyPhase::Editing { note: Some(format!("⚠ Wispr: {error} — yazabilirsin").into()) };
        }
    }

    fn on_reply_sent(&mut self, thread_id: &str, ok: bool, queued: bool, detail: String) {
        let Some(dock) = self.reply.as_mut().filter(|d| d.thread_id == thread_id) else {
            if !ok {
                self.last_error = Some(format!("cevap gönderilemedi: {detail}").into());
            }
            return;
        };
        self.core.log_ui(
            "ui.reply_sent",
            json!({"thread": thread_id, "ok": ok, "queued": queued, "detail": detail, "open_ms": ms(dock.opened_at.elapsed())}),
        );
        if ok {
            dock.phase = ReplyPhase::Sent { queued };
            dock.close_at = Some(Instant::now() + DOCK_CLOSE_DELAY);
        } else {
            // Keep the dock (and the dictated text) so the user can retry with Gönder.
            dock.phase = ReplyPhase::Editing { note: Some(format!("✗ gönderilemedi: {detail}").into()) };
        }
        if matches!(self.selftest, SelfTest::ReplySubmitted { .. }) {
            self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_REPLY_QUIT_DELAY };
        }
    }

    /// Transcript fill-or-compare, auto-send after Gönder, delayed close. Returns true on change.
    fn advance_reply(&mut self, now: Instant, cx: &mut Context<Panel>) -> bool {
        let Some(dock) = self.reply.as_mut() else {
            return false;
        };
        let mut changed = false;
        if dock.transcript.as_ref().is_some_and(|(_, due)| now >= *due)
            && let Some((transcript, _)) = dock.transcript.take()
        {
            let current = self.input.read(cx).text().trim().to_owned();
            if current.is_empty() {
                // Recorded now: an auto-send below runs before the Edited event is delivered.
                if !dock.sources.contains(&EditSource::History) {
                    dock.sources.push(EditSource::History);
                }
                self.input.update(cx, |input, cx| input.set_text(transcript.trim(), EditSource::History, cx));
            } else {
                self.core.log_ui(
                    "ui.reply_compare",
                    json!({
                        "thread": dock.thread_id, "equal": current == transcript.trim(),
                        "input_chars": current.chars().count(), "transcript_chars": transcript.trim().chars().count(),
                    }),
                );
            }
            changed = true;
        }
        let Some(dock) = self.reply.as_ref() else {
            return changed;
        };
        if dock.phase == ReplyPhase::WaitingText && !self.input.read(cx).text().trim().is_empty() {
            self.send_reply(cx);
            changed = true;
        }
        if self.reply.as_ref().and_then(|d| d.close_at).is_some_and(|at| now >= at) {
            self.close_reply(cx);
            changed = true;
        }
        changed
    }

    // ── self-tests ──────────────────────────────────────────────────────────────────────────

    fn selftest_deadline(&self) -> Instant {
        match self.selftest {
            SelfTest::WaitingForThreads { deadline }
            | SelfTest::Listening { deadline, .. }
            | SelfTest::Replying { deadline, .. }
            | SelfTest::ReplySubmitted { deadline } => deadline,
            SelfTest::Off | SelfTest::Quitting { .. } => Instant::now() + SELFTEST_TIMEOUT,
        }
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
                self.open_reply(&thread_id, "selftest", window, cx);
            }
        }
    }

    /// Returns true if the app is quitting.
    fn advance_selftest(&mut self, now: Instant, cx: &mut Context<Panel>) -> bool {
        match &self.selftest {
            SelfTest::Off => false,
            SelfTest::Quitting { at } => {
                if now >= *at {
                    self.quit("selftest_done", cx);
                    return true;
                }
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
                self.core.log_ui("ui.selftest_timeout", json!({"phase": phase, "timeout_s": SELFTEST_TIMEOUT.as_secs()}));
                self.quit("selftest_timeout", cx);
                true
            }
            _ => false,
        }
    }

    fn quit(&mut self, reason: &str, cx: &mut Context<Panel>) {
        if self.reply.is_some() {
            self.cancel_reply("quit", cx);
        }
        self.core.log_ui("ui.quit", json!({"reason": reason, "uptime_ms": ms(self.process_start.elapsed())}));
        cx.quit();
    }

    // ── rendering ───────────────────────────────────────────────────────────────────────────

    fn render_header(&self, cx: &mut Context<Panel>) -> Div {
        let any_listening = self.channels.values().any(|c| c.listening);
        let mute_all = div()
            .id("mute_all")
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .h(px(24.))
            .px_2()
            .rounded_md()
            .text_size(px(11.5))
            .cursor_pointer()
            .when(!any_listening, |this| this.opacity(0.4))
            .hover(|this| this.bg(rgba(0xffffff1f)))
            .active(|this| this.bg(rgba(0xffffff33)))
            .tooltip(tip("tüm kanalları sustur"))
            .child("🔇")
            .child("hepsini sustur")
            .on_click(cx.listener(|panel, _, _, _| {
                panel.last_body_click = None;
                panel.core.log_ui("ui.click", json!({"action": "mute_all"}));
                panel.core.mute_all();
            }));
        let quit = icon_button("quit", "✕").tooltip(tip("kapat")).on_click(cx.listener(|panel, _, _, cx| {
            panel.core.log_ui("ui.click", json!({"action": "quit"}));
            panel.quit("user", cx);
        }));
        let core = self.core.clone();
        let handle = div()
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .pl_3()
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(13.))
            .child("T3 Panel")
            .on_mouse_down(MouseButton::Left, move |event, window, _| {
                let dragging = mac::drag_window(window);
                core.log_ui(
                    "ui.drag",
                    json!({"x": f32::from(event.position.x), "y": f32::from(event.position.y), "native_drag": dragging}),
                );
            });
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(34.))
            .pr_2()
            .gap_1()
            .border_b_1()
            .border_color(rgba(0xffffff14))
            .child(handle)
            .child(mute_all)
            .child(quit)
    }

    fn render_row(
        &self,
        ix: usize,
        row: &Row,
        ch: Option<&ChannelView>,
        now_unix: i64,
        cx: &mut Context<Panel>,
    ) -> Stateful<Div> {
        let listening = ch.is_some_and(|c| c.listening);
        let speaking = ch.is_some_and(|c| c.speaking);
        let selected = self.selected.as_ref() == Some(&row.id);
        let mut meta = String::with_capacity(row.project.len() + row.provider.len() + 16);
        meta.push_str(&row.project);
        meta.push_str(" · ");
        push_relative_time(&mut meta, row.updated_unix, now_unix);
        if !row.provider.is_empty() {
            meta.push_str(" · ");
            meta.push_str(&row.provider);
        }

        let mut title_line = div()
            .flex()
            .items_center()
            .gap_1p5()
            .min_w(px(0.))
            .child(div().flex_1().min_w(px(0.)).text_size(px(13.)).truncate().child(row.title.clone()));
        if let Some(c) = ch {
            if c.waiting_for_user {
                title_line = title_line.child(chip("seni bekliyor", AMBER, 0xffb34033));
            }
            if !c.listening && c.unheard > 0 {
                title_line = title_line.child(chip(format!("{} yeni", c.unheard), BLUE, 0x0a84ff33));
            }
        }

        let body_id = row.id.clone();
        let body = div()
            .id(("body", ix))
            .flex()
            .flex_1()
            .min_w(px(0.))
            .items_center()
            .gap_2()
            .tooltip(tip("çift tık: T3'te aç"))
            .child(div().flex_none().size(px(8.)).rounded_full().bg(rgb(row.status_color)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .child(title_line)
                    .child(div().text_size(px(11.)).text_color(rgb(TEXT_DIM)).truncate().child(meta)),
            )
            .on_click(cx.listener(move |panel, _, _, cx| panel.on_body_click(&body_id, cx)));

        let listen_id = row.id.clone();
        let listen = icon_button(("listen", ix), if listening { "🔊" } else { "🔈" })
            .when(!listening, |this| this.opacity(0.45))
            .when(listening && !speaking, |this| this.bg(rgba(0x0a84ff26)))
            .when(speaking, |this| this.bg(rgba(0x0a84ff8c)).border_1().border_color(rgb(BLUE)))
            .tooltip(tip(if listening { "dinliyorsun · tıkla: sustur" } else { "sessiz · tıkla: dinle" }))
            .on_click(cx.listener(move |panel, _, _, _| {
                panel.last_body_click = None;
                panel.core.log_ui(
                    "ui.click",
                    json!({"thread": listen_id.as_ref(), "action": if listening { "mute" } else { "listen" }}),
                );
                panel.core.set_listening(&listen_id, !listening);
            }));

        let reply_id = row.id.clone();
        let replying_here = self.reply.as_ref().is_some_and(|d| d.thread_id == row.id.as_ref());
        let reply = div()
            .id(("reply", ix))
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .h(px(24.))
            .px_1p5()
            .rounded_md()
            .text_size(px(11.5))
            .cursor_pointer()
            .when(replying_here, |this| this.bg(rgba(0x0a84ff38)))
            .hover(|this| this.bg(rgba(0xffffff1f)))
            .active(|this| this.bg(rgba(0xffffff33)))
            .child("💬")
            .child("Cevapla")
            .on_click(cx.listener(move |panel, _, window, cx| {
                panel.last_body_click = None;
                panel.core.log_ui("ui.click", json!({"thread": reply_id.as_ref(), "action": "reply"}));
                panel.open_reply(&reply_id, "click", window, cx);
            }));

        div()
            .id(("row", ix))
            .flex()
            .flex_col()
            .px_2()
            .py_1p5()
            .rounded_md()
            .when(speaking, |this| this.bg(rgba(0x0a84ff38)))
            .when(!speaking && selected, |this| this.bg(rgba(0xffffff12)))
            .when(!speaking, |this| this.hover(|this| this.bg(rgba(0xffffff0d))))
            .child(div().flex().items_center().gap_1().child(body).child(listen).child(reply))
            .when_some(ch.filter(|c| c.listening && (c.speaking || c.paused || c.queued > 0)), |this, c| {
                this.child(self.render_now_line(ix, &row.id, c, cx))
            })
    }

    /// Third row line of a listening channel with something current: what it says + controls.
    fn render_now_line(&self, ix: usize, id: &SharedString, c: &ChannelView, cx: &mut Context<Panel>) -> Div {
        let text: SharedString = match &c.now_text {
            Some(t) => t.clone().into(),
            None if c.queued > 0 => "sırada bekleyen var".into(),
            None => "…".into(),
        };
        let pause_id = id.clone();
        let pause = icon_button(("pause", ix), if c.paused { "▶" } else { "⏸" })
            .tooltip(tip(if c.paused { "devam et" } else { "duraklat" }))
            .on_click(cx.listener(move |panel, _, _, _| {
                panel.last_body_click = None;
                panel.core.log_ui("ui.click", json!({"thread": pause_id.as_ref(), "action": "toggle_pause"}));
                panel.core.toggle_pause(&pause_id);
            }));
        let skip_id = id.clone();
        let skip = icon_button(("skip", ix), "⏭").tooltip(tip("okey, geç")).on_click(cx.listener(move |panel, _, _, _| {
            panel.last_body_click = None;
            panel.core.log_ui("ui.click", json!({"thread": skip_id.as_ref(), "action": "skip"}));
            panel.core.skip(&skip_id);
        }));
        let live_id = id.clone();
        let live = (c.queued > 0).then(|| {
            div()
                .id(("live", ix))
                .flex_none()
                .px_1p5()
                .py_0p5()
                .rounded_md()
                .text_size(px(10.5))
                .text_color(rgb(BLUE))
                .bg(rgba(0x0a84ff26))
                .cursor_pointer()
                .hover(|this| this.bg(rgba(0x0a84ff4d)))
                .child(format!("{} sırada · canlıya geç", c.queued))
                .on_click(cx.listener(move |panel, _, _, _| {
                    panel.last_body_click = None;
                    panel.core.log_ui("ui.click", json!({"thread": live_id.as_ref(), "action": "jump_to_live"}));
                    panel.core.jump_to_live(&live_id);
                }))
        });
        div()
            .flex()
            .items_center()
            .gap_1()
            .pl(px(16.))
            .pt_0p5()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_size(px(11.))
                    .text_color(rgb(if c.paused { TEXT_DIM } else { TEXT }))
                    .truncate()
                    .child(text),
            )
            .child(pause)
            .child(skip)
            .children(live)
    }

    /// Single click selects; a second click on the same row body within the system
    /// double-click interval opens the thread in T3. Clicks on row buttons reset the pair.
    fn on_body_click(&mut self, thread_id: &SharedString, cx: &mut Context<Panel>) {
        let now = Instant::now();
        let is_double = self
            .last_body_click
            .as_ref()
            .is_some_and(|c| c.thread_id == *thread_id && now.duration_since(c.at) <= self.double_click_interval);
        self.selected = Some(thread_id.clone());
        if is_double {
            self.last_body_click = None;
            self.core.log_ui("ui.dblclick", json!({"thread": thread_id.as_ref()}));
            self.core.open_thread(thread_id);
        } else {
            self.last_body_click = Some(BodyClick { thread_id: thread_id.clone(), at: now });
        }
        cx.notify();
    }

    fn render_reply_dock(&self, dock: &ReplyDock, cx: &mut Context<Panel>) -> Div {
        let (chip_text, chip_color) = dock.chip();
        let busy = matches!(dock.phase, ReplyPhase::Sending | ReplyPhase::Sent { .. });
        let send = dock_button("send", "Gönder", true, busy).on_click(cx.listener(|panel, _, _, cx| {
            panel.submit_reply("button", cx);
        }));
        let cancel = dock_button("cancel", "İptal", false, false).on_click(cx.listener(|panel, _, _, cx| {
            panel.cancel_reply("button", cx);
        }));
        div()
            .flex()
            .flex_none()
            .flex_col()
            .gap_1p5()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(rgba(0xffffff14))
            .bg(rgba(0x0a84ff14))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(rgb(TEXT_DIM))
                    .truncate()
                    .child(format!("→ {}", dock.title)),
            )
            .child(self.input.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(px(11.))
                            .text_color(rgb(chip_color))
                            .line_clamp(2)
                            .child(chip_text),
                    )
                    .child(send)
                    .child(cancel),
            )
    }

    fn render_footer(&self, cx: &mut Context<Panel>) -> Stateful<Div> {
        let s = &self.stats;
        let line1 = format!(
            "RSS {:.0}MB · CPU {:.0}% · t3 {} · live {} · tts {} · RTF {}",
            s.rss_mb,
            s.cpu_pct,
            fmt_ms(s.last_t3_poll_ms),
            fmt_ms(s.last_live_poll_ms),
            fmt_ms(s.last_tts_ms),
            s.last_rtf.map_or_else(|| "–".to_owned(), |r| format!("{r:.2}")),
        );
        let line2 = format!(
            "click→audio {} · msg→audio {} · dict {} · 🔊 {} · q {}",
            fmt_ms(s.last_click_to_audio_ms),
            fmt_ms(s.last_msg_to_audio_ms),
            fmt_ms(s.last_dictation_ms),
            s.listening,
            s.queue_len,
        );
        let run_dir: SharedString = format!("{} (tıkla: Finder)", s.run_dir.display()).into();
        div()
            .id("footer")
            .flex()
            .flex_none()
            .flex_col()
            .px_3()
            .py_1p5()
            .border_t_1()
            .border_color(rgba(0xffffff14))
            .text_size(px(10.5))
            .text_color(rgb(TEXT_DIM))
            .cursor_pointer()
            .hover(|this| this.text_color(rgb(TEXT)))
            .tooltip(move |_, cx| cx.new(|_| Tip(run_dir.clone())).into())
            .child(div().truncate().child(line1))
            .child(div().truncate().child(line2))
            .on_click(cx.listener(|panel, _, _, _| {
                let run_dir = panel.core.live_stats().run_dir;
                panel.core.log_ui("ui.click", json!({"action": "open_run_dir", "path": run_dir.display().to_string()}));
                open_in_finder(run_dir);
            }))
    }
}

impl Render for Panel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.ready_logged {
            self.ready_logged = true;
            self.core.log_ui(
                "ui.ready",
                json!({"startup_ms": ms(self.process_start.elapsed()), "rows": self.rows.len()}),
            );
        }
        let now_unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);

        let rows: Vec<Stateful<Div>> = self
            .rows
            .iter()
            .enumerate()
            .map(|(ix, row)| self.render_row(ix, row, self.channels.get(row.id.as_ref()), now_unix, cx))
            .collect();
        let list = div()
            .id("threads")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p_1()
            .overflow_y_scroll()
            .when(self.rows.is_empty(), |this| {
                this.child(div().p_3().text_size(px(12.)).text_color(rgb(TEXT_DIM)).child("No recent threads yet…"))
            })
            .children(rows);
        let dock = self.reply.as_ref().map(|dock| self.render_reply_dock(dock, cx));

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_xl()
            .border_1()
            .border_color(rgba(0xffffff1f))
            .bg(rgba(0x141418f2))
            .text_color(rgb(TEXT))
            .text_size(px(12.))
            .child(self.render_header(cx))
            .child(list)
            .when_some(self.last_error.clone(), |this, err| {
                this.child(div().flex_none().px_3().py_1().text_size(px(10.5)).text_color(rgb(RED)).truncate().child(err))
            })
            .children(dock)
            .child(self.render_footer(cx))
    }
}

/// Tooltip bubble.
struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(rgba(0xffffff26))
            .bg(rgb(0x2a2a30))
            .text_size(px(11.))
            .text_color(rgb(TEXT))
            .child(self.0.clone())
    }
}

fn tip(text: &'static str) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_, cx| cx.new(|_| Tip(text.into())).into()
}

fn icon_button(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded_md()
        .text_size(px(12.))
        .cursor_pointer()
        .hover(|this| this.bg(rgba(0xffffff1f)))
        .active(|this| this.bg(rgba(0xffffff33)))
        .child(label)
}

fn dock_button(id: &'static str, label: &'static str, primary: bool, disabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px_2p5()
        .py_0p5()
        .rounded_md()
        .text_size(px(12.))
        .cursor_pointer()
        .when(primary, |this| this.bg(rgb(BLUE)).text_color(rgb(0xffffff)))
        .when(!primary, |this| this.bg(rgba(0xffffff1a)))
        .when(disabled, |this| this.opacity(0.5))
        .hover(|this| this.opacity(0.85))
        .child(label)
}

fn chip(label: impl Into<SharedString>, color: u32, bg: u32) -> Div {
    div()
        .flex_none()
        .px_1p5()
        .rounded_md()
        .text_size(px(10.5))
        .text_color(rgb(color))
        .bg(rgba(bg))
        .child(label.into())
}

fn status_color(status: &str) -> u32 {
    match status {
        "running" => BLUE,
        "completed" | "idle" => GREEN,
        "interrupted" | "cancelled" => AMBER,
        _ => GRAY,
    }
}

fn open_in_finder(path: std::path::PathBuf) {
    std::thread::spawn(move || {
        // `open` returns as soon as Finder has the request; waiting just reaps the child.
        let _ = std::process::Command::new("/usr/bin/open").arg(path).status();
    });
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn fmt_ms(v: Option<f64>) -> String {
    v.map_or_else(|| "–".to_owned(), |v| format!("{v:.0}ms"))
}

fn push_relative_time(out: &mut String, updated_unix: Option<i64>, now_unix: i64) {
    use std::fmt::Write;
    let Some(updated) = updated_unix else {
        out.push('?');
        return;
    };
    let secs = (now_unix - updated).max(0);
    let _ = match secs {
        0..60 => write!(out, "{secs}s ago"),
        60..3600 => write!(out, "{}m ago", secs / 60),
        3600..86400 => write!(out, "{}h ago", secs / 3600),
        _ => write!(out, "{}d ago", secs / 86400),
    };
}

/// Parses `YYYY-MM-DDTHH:MM:SS[.fff](Z|±HH:MM)` into Unix seconds.
fn parse_rfc3339_unix(s: &str) -> Option<i64> {
    let num = |start: usize, len: usize| -> Option<i64> { s.get(start..start + len)?.parse().ok() };
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    let bytes = s.as_bytes();
    let mut i = 19;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    let offset = match *bytes.get(i)? {
        b'Z' | b'z' => 0,
        sign @ (b'+' | b'-') => {
            let offset = num(i + 1, 2)? * 3600 + num(i + 4, 2)? * 60;
            if sign == b'+' { offset } else { -offset }
        }
        _ => return None,
    };
    Some(days_from_civil(year, month, day) * 86400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
