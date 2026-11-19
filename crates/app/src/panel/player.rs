//! Now playing bar (DESIGN §5): label, current sentence, progress, queue chip and controls. The
//! only continuous motion in the panel (equalizer + progress), and only while audio plays.

use std::time::Duration;

use gpui::{Animation, AnimationExt, AnyElement, App, Context, FontWeight, Rgba, SharedString, div, prelude::*, px};
use serde_json::json;
use tellal_core::ChannelView;

use super::{PLAYER_H, Panel};
use crate::icons::{Icon, icon};
use crate::motion;
use crate::theme::{Tokens, TypeScale, alpha};
use crate::widgets::{icon_button, pill, tip};

const EQ_PERIOD: Duration = Duration::from_millis(900);

/// Three bars bouncing while audio plays; a static speaker under Reduce Motion.
pub(super) fn equalizer(color: Rgba, size: f32, cx: &App) -> AnyElement {
    if motion::reduced(cx) {
        return icon(Icon::Volume2, size, color).into_any_element();
    }
    let bar_w = (size / 5.).max(2.);
    div()
        .flex_none()
        .flex()
        .items_end()
        .justify_center()
        .gap(px(bar_w / 2.))
        .size(px(size))
        .with_animation("eq", Animation::new(EQ_PERIOD).repeat(), move |el, t| {
            el.children((0..3).map(|i| {
                let phase = std::f32::consts::TAU * (t + i as f32 * 0.29);
                let level = 0.35 + 0.65 * (0.5 + 0.5 * (phase * (1. + i as f32 * 0.5)).sin());
                div().w(px(bar_w)).h(px(size * level)).rounded(px(bar_w / 2.)).bg(color)
            }))
        })
        .into_any_element()
}

impl Panel {
    pub(super) fn render_player(
        &self,
        thread_id: &str,
        c: &ChannelView,
        t: &Tokens,
        ty: &TypeScale,
        cx: &mut Context<Panel>,
    ) -> AnyElement {
        let thread = self.thread(thread_id);
        let label: SharedString = match thread {
            Some(th) if !th.project.is_empty() => format!("{} · {}", th.project, th.title).into(),
            Some(th) => th.title.clone().into(),
            None => thread_id.to_owned().into(),
        };
        let text: SharedString = match &c.now_text {
            Some(text) => text.clone().into(),
            None if c.queued > 0 => self.s.queued_waiting.into(),
            None => "…".into(),
        };
        let audible = c.speaking && !c.paused;
        let lead: AnyElement = if audible {
            equalizer(t.accent, 14., cx)
        } else {
            icon(if c.paused { Icon::Pause } else { Icon::Volume1 }, 14., t.text_muted).into_any_element()
        };

        let id = thread_id.to_owned();
        let pause = icon_button("pause", if c.paused { Icon::Play } else { Icon::Pause }, t, c.paused)
            .tooltip(tip(if c.paused { self.s.resume_tip } else { self.s.pause_tip }))
            .on_click(cx.listener({
                let id = id.clone();
                move |panel, _, _, cx| {
                    panel.player_action("click", "toggle_pause", &id);
                    cx.notify();
                }
            }));
        let skip =
            icon_button("skip", Icon::SkipForward, t, false).tooltip(tip(self.s.skip_tip)).on_click(cx.listener({
                let id = id.clone();
                move |panel, _, _, cx| {
                    panel.player_action("click", "skip", &id);
                    cx.notify();
                }
            }));
        let live = (c.queued > 0).then(|| {
            let id = id.clone();
            icon_button("live", Icon::ChevronsRight, t, false).tooltip(tip(self.s.live_jump_tip)).on_click(cx.listener(
                move |panel, _, _, cx| {
                    panel.player_action("click", "jump_to_live", &id);
                    cx.notify();
                },
            ))
        });
        let mute = icon_button("mute_all", Icon::VolumeX, t, false).tooltip(tip(self.s.mute_all_tip)).on_click(
            cx.listener(|panel, _, _, cx| {
                panel.mute_all("click");
                cx.notify();
            }),
        );

        let bar = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_1p5()
            .h(px(PLAYER_H))
            .px_3()
            .pt_2p5()
            .pb_1p5()
            .border_t_1()
            .border_color(t.border)
            .bg(t.surface)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(18.))
                    .child(lead)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(ty.secondary))
                            .text_color(t.text_muted)
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    )
                    .when(c.queued > 0, |d| d.child(pill(self.s.queued(c.queued), t.accent, 0.16))),
            )
            .child(
                div()
                    .h(px(36.))
                    .line_clamp(2)
                    .text_size(px(ty.body))
                    .line_height(px(18.))
                    .text_color(if c.paused { t.text_muted } else { t.text })
                    .child(text),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .h(px(28.))
                    .child(self.render_progress(c, t))
                    .child(pause)
                    .child(skip)
                    .children(live)
                    .child(mute),
            );
        motion::slide_up(bar, "player_in", 12., cx)
    }

    /// Progress of the current utterance from the latest `channels()` snapshot (the pump
    /// refreshes it every 150 ms while a channel speaks; the total grows until synthesis ends).
    fn render_progress(&self, c: &ChannelView, t: &Tokens) -> AnyElement {
        let frac = match (c.now_elapsed_ms, c.now_total_ms) {
            (Some(elapsed), Some(total)) if total > 0 => (elapsed as f32 / total as f32).clamp(0., 1.),
            _ => 0.,
        };
        div()
            .flex_1()
            .h(px(4.))
            .mr_1()
            .rounded(px(2.))
            .bg(alpha(t.text_muted, 0.25))
            .child(div().h_full().w(gpui::relative(frac)).rounded(px(2.)).bg(t.accent))
            .into_any_element()
    }

    pub(super) fn player_action(&mut self, source: &str, action: &str, thread_id: &str) {
        let before = self.channel(thread_id).cloned().unwrap_or_default();
        self.log_action(
            source,
            action,
            Some(thread_id),
            json!({"before": {"paused": before.paused, "speaking": before.speaking, "queued": before.queued}}),
        );
        match action {
            "toggle_pause" => self.core.toggle_pause(thread_id),
            "skip" => self.core.skip(thread_id),
            "jump_to_live" => self.core.jump_to_live(thread_id),
            _ => {}
        }
    }

    pub(super) fn mute_all(&mut self, source: &str) {
        let listening = self.channels.values().filter(|c| c.listening).count();
        self.log_action(source, "mute_all", None, json!({"before_listening": listening}));
        self.core.mute_all();
    }
}
