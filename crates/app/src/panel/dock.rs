//! Reply dock (v0.2 behaviour, restyled): Wispr Flow hands-free dictation, focus/activate and
//! restore of the previous app, paste/typing, Wispr History fill-or-compare, auto-send after
//! Gönder, retarget to another thread, and every step logged. In new-task mode (target
//! [`NEW_TASK`]) the same flow ends in `Core::start_task` instead of a reply.

use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, Entity, Focusable, FontWeight, Rgba, SharedString, Window, div, prelude::*, px};
use serde_json::json;
use tellal_core::NEW_TASK;

use super::selftest::{SelfTest, SelfTestMode};
use super::{DOCK_H, FOCUS_CHECK_DELAY, Panel, ms};
use crate::icons::{Icon, icon};
use crate::input::{EditSource, InputEvent, TextInput};
use crate::mac::{self, FrontApp};
use crate::motion;
use crate::theme::{Tokens, TypeScale, alpha};
use crate::widgets::{ButtonKind, text_button, tip};

/// Wispr pastes into the focused field itself; only fill from its History DB if nothing came.
const TRANSCRIPT_FILL_DELAY: Duration = Duration::from_millis(600);
pub(super) const SELFTEST_REPLY_QUIT_DELAY: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReplyPhase {
    /// Wispr hands-free dictation was requested and has not delivered a transcript yet.
    Dictating,
    /// No dictation running; the user edits/types. `note` explains why (failure, transcript in).
    Editing {
        note: Option<(SharedString, Tone)>,
    },
    /// Gönder pressed while dictating: `dictation_stop` sent, send as soon as text exists.
    WaitingText,
    Sending,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Tone {
    Good,
    Bad,
}

impl ReplyPhase {
    fn name(&self) -> &'static str {
        match self {
            ReplyPhase::Dictating => "dictating",
            ReplyPhase::Editing { .. } => "editing",
            ReplyPhase::WaitingText => "waiting_text",
            ReplyPhase::Sending => "sending",
        }
    }

    pub(super) fn dictation_active(&self) -> bool {
        matches!(self, ReplyPhase::Dictating | ReplyPhase::WaitingText)
    }
}

pub(super) struct ReplyDock {
    /// The thread replied to, or [`NEW_TASK`] (new-task mode: the text starts a new thread).
    pub(super) thread_id: String,
    /// Thread title, or the task project's name in new-task mode.
    title: SharedString,
    phase: ReplyPhase,
    /// Wispr History transcript waiting for the fill-or-compare decision.
    transcript: Option<(String, Instant)>,
    /// Every source that edited the text, in first-seen order (`ui.selftest_reply.source`).
    sources: Vec<EditSource>,
    /// App to re-activate when the dock closes (`None` if we were already frontmost).
    prev_app: Option<FrontApp>,
    pub(super) opened_at: Instant,
    seq: usize,
}

impl ReplyDock {
    /// State chip: icon, text, colour.
    fn chip(&self, t: &Tokens) -> (Icon, SharedString, Rgba) {
        match &self.phase {
            ReplyPhase::Dictating => (Icon::Mic, "Wispr dinliyor · bitince Caps Lock ya da Gönder".into(), t.accent),
            ReplyPhase::Editing { note: Some((note, Tone::Good)) } => (Icon::Check, note.clone(), t.success),
            ReplyPhase::Editing { note: Some((note, Tone::Bad)) } => (Icon::TriangleAlert, note.clone(), t.danger),
            ReplyPhase::Editing { note: None } => {
                (Icon::MessageSquare, "Yaz ya da düzenle · Enter gönderir".into(), t.text_muted)
            }
            ReplyPhase::WaitingText => (Icon::Mic, "Wispr metni bekleniyor…".into(), t.warning),
            ReplyPhase::Sending if self.is_new_task() => (Icon::Send, "Görev başlatılıyor…".into(), t.text_muted),
            ReplyPhase::Sending => (Icon::Send, "Gönderiliyor…".into(), t.text_muted),
        }
    }

    fn source_label(&self) -> String {
        let names: Vec<&str> = self.sources.iter().map(|s| s.as_str()).collect();
        names.join("+")
    }

    pub(super) fn is_new_task(&self) -> bool {
        self.thread_id == NEW_TASK
    }

    /// `start_task` failed: keep the text, say why and how to retry (G2-5).
    pub(super) fn task_failed(&mut self, detail: &str) {
        self.phase = ReplyPhase::Editing {
            note: Some((format!("Görev başlatılamadı ({detail}) · Gönder ile tekrar dene").into(), Tone::Bad)),
        };
    }
}

impl Panel {
    /// "Cevapla": open (or retarget) the dock, make the panel key, focus the input and start
    /// Wispr dictation for `thread_id` ([`NEW_TASK`]: a new task). `live = false` is the
    /// dock-only self-test state: no dictation and no focus change. The `demo` and `shots`
    /// walkthroughs dictate without taking the focus.
    pub(super) fn open_reply(
        &mut self,
        thread_id: &str,
        source: &str,
        live: bool,
        window: &mut Window,
        cx: &mut Context<Panel>,
    ) {
        let new_task = thread_id == NEW_TASK;
        let title = match self.thread(thread_id) {
            _ if new_task => self.task_project_label(),
            Some(t) => SharedString::from(t.title.clone()),
            None => SharedString::from(thread_id.to_owned()),
        };
        if !new_task {
            self.mark_seen(thread_id);
        }
        let focus = live && !self.selftest_mode.leaves_focus();
        let mut retarget_from = None;
        let prev_app = match self.reply.take() {
            Some(dock) if dock.thread_id == thread_id => {
                // Same row again: just bring the focus back.
                self.core.log_ui("ui.reply_open", json!({"thread": thread_id, "source": source, "refocus": true}));
                self.reply = Some(dock);
                if focus {
                    self.focus_input(window, cx);
                }
                return;
            }
            Some(dock) => {
                if dock.phase.dictation_active() {
                    self.core.dictation_cancel();
                }
                retarget_from = Some(dock.thread_id);
                dock.prev_app
            }
            None if focus => mac::frontmost_app().filter(|a| !a.is_self()),
            None => None,
        };
        self.core.log_ui(
            "ui.reply_open",
            json!({
                "thread": thread_id, "title": title.as_ref(), "source": source, "dictation": live,
                "retarget_from": retarget_from, "prev_app": prev_app.as_ref().map(FrontApp::to_json),
            }),
        );
        if focus && retarget_from.is_none() {
            self.core.log_ui(
                "ui.focus",
                json!({"action": "activate", "prev_app": prev_app.as_ref().map(FrontApp::to_json)}),
            );
        }
        let now = Instant::now();
        let seq = self.next_seq();
        self.reply = Some(ReplyDock {
            thread_id: thread_id.to_owned(),
            title,
            phase: if live { ReplyPhase::Dictating } else { ReplyPhase::Editing { note: None } },
            transcript: None,
            sources: Vec::new(),
            prev_app,
            opened_at: now,
            seq,
        });
        if focus {
            self.focus_input(window, cx);
            self.focus_check = Some((now + FOCUS_CHECK_DELAY, "opened"));
        }
        if live {
            self.core.dictation_start(thread_id);
        }
        cx.notify();
    }

    fn focus_input(&self, window: &mut Window, cx: &mut Context<Panel>) {
        cx.activate(true);
        window.activate_window();
        let handle = self.input.read(cx).focus_handle(cx);
        window.focus(&handle);
    }

    pub(super) fn on_input_event(
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
    pub(super) fn submit_reply(&mut self, source: &str, cx: &mut Context<Panel>) {
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
            ReplyPhase::Sending => false,
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
            if dock.is_new_task() { "ui.task_send" } else { "ui.reply_send" },
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
            self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_REPLY_QUIT_DELAY };
            self.show_toast("Gönderildi");
            self.close_reply(cx);
        } else {
            dock.phase = ReplyPhase::Sending;
            if dock.is_new_task() {
                self.core.start_task(&text);
            } else {
                self.core.send_reply(&dock.thread_id, &text);
            }
        }
    }

    /// İptal / Esc.
    pub(super) fn cancel_reply(&mut self, source: &str, cx: &mut Context<Panel>) {
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
    pub(super) fn close_reply(&mut self, cx: &mut Context<Panel>) {
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

    pub(super) fn on_transcript(&mut self, thread_id: String, text: String) {
        let Some(dock) = self.reply.as_mut().filter(|d| d.thread_id == thread_id) else {
            return;
        };
        if dock.phase == ReplyPhase::Dictating {
            dock.phase =
                ReplyPhase::Editing { note: Some(("Wispr metni geldi · Enter gönderir".into(), Tone::Good)) };
        }
        dock.transcript = Some((text, Instant::now() + TRANSCRIPT_FILL_DELAY));
    }

    pub(super) fn on_dictation_failed(&mut self, thread_id: &str, error: String) {
        let Some(dock) = self.reply.as_mut().filter(|d| d.thread_id == thread_id) else {
            self.show_error(format!("Wispr dikte başlamadı: {error}").into(), None);
            return;
        };
        if dock.phase.dictation_active() {
            dock.phase = ReplyPhase::Editing {
                note: Some((format!("Wispr çalışmadı ({error}) · yazarak cevaplayabilirsin").into(), Tone::Bad)),
            };
        }
    }

    pub(super) fn on_reply_sent(
        &mut self,
        thread_id: &str,
        ok: bool,
        queued: bool,
        detail: String,
        cx: &mut Context<Panel>,
    ) {
        let Some(dock) = self.reply.as_mut().filter(|d| d.thread_id == thread_id) else {
            if !ok {
                self.show_error(format!("Cevap gönderilemedi: {detail}").into(), None);
            }
            return;
        };
        self.core.log_ui(
            "ui.reply_sent",
            json!({"thread": thread_id, "ok": ok, "queued": queued, "detail": detail, "open_ms": ms(dock.opened_at.elapsed())}),
        );
        if ok {
            self.show_toast(if queued { "Gönderildi · sıraya alındı" } else { "Gönderildi" });
            self.close_reply(cx);
        } else {
            // Keep the dock (and the dictated text) so the user can retry with Gönder (G2-5).
            dock.phase = ReplyPhase::Editing {
                note: Some((format!("Gönderilemedi ({detail}) · Gönder ile tekrar dene").into(), Tone::Bad)),
            };
        }
        if matches!(self.selftest, SelfTest::ReplySubmitted { .. }) {
            self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_REPLY_QUIT_DELAY };
        }
    }

    /// Transcript fill-or-compare and auto-send after Gönder. Returns true on change.
    pub(super) fn advance_reply(&mut self, now: Instant, cx: &mut Context<Panel>) -> bool {
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
        if self.reply.as_ref().is_some_and(|d| d.phase == ReplyPhase::WaitingText)
            && !self.input.read(cx).text().trim().is_empty()
        {
            self.send_reply(cx);
            changed = true;
        }
        changed
    }

    pub(super) fn render_reply_dock(
        &self,
        dock: &ReplyDock,
        t: &Tokens,
        ty: &TypeScale,
        cx: &mut Context<Panel>,
    ) -> AnyElement {
        let (chip_icon, chip_text, chip_color) = dock.chip(t);
        let sending = dock.phase == ReplyPhase::Sending;
        let send = text_button("send", Some(Icon::Send), "Gönder", ButtonKind::Primary, t, ty.secondary)
            .when(sending, |d| d.opacity(0.5))
            .tooltip(tip(if dock.is_new_task() {
                "görevi başlat (Enter) · dikte sürüyorsa önce bitirir"
            } else {
                "gönder (Enter) · dikte sürüyorsa önce bitirir"
            }))
            .on_click(cx.listener(|panel, _, _, cx| panel.submit_reply("button", cx)));
        let cancel = text_button("cancel", None, "İptal", ButtonKind::Secondary, t, ty.secondary)
            .tooltip(tip("vazgeç (Esc)"))
            .on_click(cx.listener(|panel, _, _, cx| panel.cancel_reply("button", cx)));
        let dock_el = div()
            .flex()
            .flex_none()
            .flex_col()
            .gap_2()
            .h(px(DOCK_H))
            .px_3()
            .py_2p5()
            .border_t_1()
            .border_color(t.border)
            .bg(t.surface)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .text_size(px(ty.secondary))
                    .text_color(t.text_muted)
                    .when(!dock.is_new_task(), |d| d.child(icon(Icon::MessageSquare, 13., t.text_muted)))
                    .child(if dock.is_new_task() { "→ Yeni görev ·" } else { "Cevap →" })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(dock.title.clone()),
                    ),
            )
            .child(self.input.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .h(px(24.))
                            .px_2()
                            .rounded(px(6.))
                            .bg(alpha(chip_color, 0.12))
                            .text_size(px(ty.badge + 0.5))
                            .text_color(chip_color)
                            .child(icon(chip_icon, 13., chip_color))
                            .child(div().flex_1().min_w(px(0.)).truncate().child(chip_text)),
                    )
                    .child(cancel)
                    .child(send),
            );
        motion::slide_up(dock_el, ("dock", dock.seq), 12., cx)
    }
}
