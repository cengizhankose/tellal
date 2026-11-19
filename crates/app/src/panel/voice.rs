//! Voices: the row above the status line while the voice the panel needs (the UI language's, or
//! `TELLAL_VOICE`'s) is not ready (download offer, progress, retry), and the metrics drawer's list
//! of every voice with its state. `Core::voices()` is re-read on `VoicesUpdated`.

use gpui::{AnyElement, Context, FontWeight, Rgba, SharedString, div, prelude::*, px};
use serde_json::{Value, json};
use tellal_core::{VoiceState, VoiceView};

use super::{DRAWER_H, Panel, VOICE_LINE_H, VOICE_ROW_H, VOICES_HEAD_H};
use crate::icons::{Icon, icon};
use crate::motion;
use crate::theme::{Tokens, TypeScale, alpha};
use crate::widgets::{ButtonKind, tabular, text_button, tip};

fn state_json(state: &VoiceState) -> Value {
    match state {
        VoiceState::Ready => json!("ready"),
        VoiceState::Missing => json!("missing"),
        VoiceState::Downloading { done_bytes, total_bytes } => {
            json!({"downloading": {"done_bytes": done_bytes, "total_bytes": total_bytes}})
        }
        VoiceState::Failed(error) => json!({"failed": error}),
    }
}

fn progress(done: u64, total: u64) -> f32 {
    if total == 0 { 0. } else { (done as f32 / total as f32).clamp(0., 1.) }
}

/// Thin progress track (`frac` of the width filled with `accent`).
fn track(frac: f32, t: &Tokens) -> gpui::Div {
    div()
        .h(px(3.))
        .rounded(px(1.5))
        .bg(alpha(t.text_muted, 0.25))
        .child(div().h_full().w(gpui::relative(frac)).rounded(px(1.5)).bg(t.accent))
}

impl Panel {
    /// The voice the row is about: the needed language's voice while it is not ready; if no
    /// voice speaks that language, the first voice while none at all is ready.
    pub(super) fn voice_row(&self) -> Option<&VoiceView> {
        if let Some(v) = self.voices.iter().find(|v| v.lang == self.voice_lang) {
            return (v.state != VoiceState::Ready).then_some(v);
        }
        if self.voices.iter().any(|v| v.state == VoiceState::Ready) { None } else { self.voices.first() }
    }

    pub(super) fn drawer_height(&self) -> f32 {
        if self.voices.is_empty() {
            DRAWER_H
        } else {
            DRAWER_H + VOICES_HEAD_H + VOICE_LINE_H * self.voices.len() as f32
        }
    }

    /// `VoicesUpdated`: re-reads `Core::voices()`; logs state changes; a toast once the needed
    /// voice became ready.
    pub(super) fn refresh_voices(&mut self) {
        let voices = self.core.voices();
        if voices == self.voices {
            return;
        }
        let needed_before = self.voice_row().map(|v| v.id.clone());
        let before = std::mem::replace(&mut self.voices, voices);
        let changed: Vec<Value> = self
            .voices
            .iter()
            .filter(|v| before.iter().find(|b| b.id == v.id).is_none_or(|b| b.state != v.state))
            .map(|v| json!({"id": v.id, "lang": v.lang.code(), "state": state_json(&v.state)}))
            .collect();
        // Download progress ticks are logged only when the state kind changes.
        let kind_changed = self.voices.iter().any(|v| {
            before
                .iter()
                .find(|b| b.id == v.id)
                .is_none_or(|b| std::mem::discriminant(&b.state) != std::mem::discriminant(&v.state))
        });
        if kind_changed || before.len() != self.voices.len() {
            self.core.log_ui("ui.voices", json!({"changed": changed, "row": self.voice_row().map(|v| v.id.clone())}));
        }
        if let Some(id) = needed_before
            && let Some(v) = self.voices.iter().find(|v| v.id == id && v.state == VoiceState::Ready)
        {
            let text = self.s.toast_voice_ready(v);
            self.show_toast(text);
        }
    }

    /// Download (or retry) voice `id`; ignored while it is ready or downloading.
    pub(super) fn download_voice(&mut self, id: &str, source: &str, cx: &mut Context<Panel>) {
        let Some(v) = self.voices.iter().find(|v| v.id == id) else {
            return;
        };
        let state = state_json(&v.state);
        if matches!(v.state, VoiceState::Ready | VoiceState::Downloading { .. }) {
            self.log_action(source, "voice_download", None, json!({"voice": id, "state": state, "ignored": true}));
            return;
        }
        self.log_action(source, "voice_download", None, json!({"voice": id, "before": state}));
        self.core.download_voice(id);
        cx.notify();
    }

    /// D: download the row's voice.
    pub(super) fn download_needed_voice(&mut self, source: &str, cx: &mut Context<Panel>) {
        match self.voice_row().map(|v| v.id.clone()) {
            Some(id) => self.download_voice(&id, source, cx),
            None => self.log_action(source, "voice_download", None, json!({"ignored": "no voice row"})),
        }
    }

    /// `English voice not installed · [Download voice (N MB)]`, the download's progress, or the
    /// failure with Retry.
    pub(super) fn render_voice_row(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> Option<AnyElement> {
        let v = self.voice_row()?;
        let s = self.s;
        let id = v.id.clone();
        let button = |label: String, kind: ButtonKind| {
            text_button("voice_download", Some(Icon::Download), label, kind, t, ty.badge + 0.5)
                .h(px(24.))
                .tooltip(tip(s.voice_tip(v)))
                .on_click(cx.listener(move |panel, _, _, cx| panel.download_voice(&id, "click", cx)))
        };
        let (lead, text, right, frac): (AnyElement, String, AnyElement, Option<f32>) = match &v.state {
            VoiceState::Ready => return None,
            VoiceState::Missing => (
                icon(Icon::Download, 14., t.info).into_any_element(),
                s.voice_missing_line(v),
                button(s.voice_download(v), ButtonKind::Secondary).into_any_element(),
                None,
            ),
            VoiceState::Downloading { done_bytes, total_bytes } => (
                icon(Icon::CircleDashed, 14., t.info).into_any_element(),
                s.voice_downloading_line(v, *done_bytes, *total_bytes),
                tabular(div().flex_none().text_color(t.text_muted).child(s.percent(*done_bytes, *total_bytes)))
                    .into_any_element(),
                Some(progress(*done_bytes, *total_bytes)),
            ),
            VoiceState::Failed(error) => (
                icon(Icon::TriangleAlert, 14., t.danger).into_any_element(),
                s.voice_failed_line(v, error),
                button(s.retry.to_owned(), ButtonKind::Ghost).into_any_element(),
                None,
            ),
        };
        let row = div()
            .id("voice_row")
            .flex()
            .flex_none()
            .flex_col()
            .justify_center()
            .h(px(VOICE_ROW_H))
            .px_3()
            .border_t_1()
            .border_color(t.border)
            .text_size(px(ty.secondary))
            .tooltip(tip(s.voice_tip(v)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(lead)
                    .child(div().flex_1().min_w(px(0.)).truncate().child(SharedString::from(text)))
                    .child(right),
            )
            .children(frac.map(|f| track(f, t).mt_1()));
        Some(motion::fade_in(row, "voice_row_in", motion::NORMAL, cx))
    }

    /// The drawer's `Sesler` / `Voices` section: one line per voice with its state.
    pub(super) fn render_voices_section(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> Option<AnyElement> {
        if self.voices.is_empty() {
            return None;
        }
        let s = self.s;
        let lines = self.voices.iter().map(|v| {
            let id = v.id.clone();
            let action = |label: String| {
                text_button(SharedString::from(format!("voice_{}", v.id)), None, label, ButtonKind::Ghost, t, ty.badge)
                    .h(px(22.))
                    .on_click(cx.listener(move |panel, _, _, cx| panel.download_voice(&id, "click", cx)))
                    .into_any_element()
            };
            let (color, right): (Rgba, AnyElement) = match &v.state {
                VoiceState::Ready => (t.success, div().text_color(t.success).child(s.voice_ready).into_any_element()),
                VoiceState::Missing => (t.text_muted, action(s.voice_download_short(v))),
                VoiceState::Downloading { done_bytes, total_bytes } => (
                    t.info,
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(track(progress(*done_bytes, *total_bytes), t).w(px(60.)))
                        .child(tabular(div().min_w(px(30.)).text_right().child(s.percent(*done_bytes, *total_bytes))))
                        .into_any_element(),
                ),
                VoiceState::Failed(_) => (t.danger, action(s.retry.to_owned())),
            };
            let (detail, state_tip) = match &v.state {
                VoiceState::Failed(error) => {
                    (format!("{} · {}", s.lang_name(v.lang), s.voice_failed), s.voice_failed_line(v, error))
                }
                VoiceState::Missing => (format!("{} · {}", s.lang_name(v.lang), s.voice_missing), s.voice_tip(v)),
                _ => (format!("{} · {} MB", s.lang_name(v.lang), v.download_mb), s.voice_tip(v)),
            };
            div()
                .id(SharedString::from(format!("voice_line_{}", v.id)))
                .flex()
                .items_center()
                .gap_2()
                .h(px(VOICE_LINE_H))
                .tooltip(tip(state_tip))
                .child(div().flex_none().size(px(6.)).rounded_full().bg(color))
                .child(div().flex_none().font_weight(FontWeight::MEDIUM).child(SharedString::from(v.name.clone())))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .text_color(t.text_muted)
                        .child(detail),
                )
                .child(right)
        });
        Some(
            div()
                .flex()
                .flex_col()
                .child(div().h(px(18.)).font_weight(FontWeight::MEDIUM).child(s.voices_title))
                .children(lines)
                .into_any_element(),
        )
    }
}
