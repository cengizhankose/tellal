//! Status line and the metrics drawer (DESIGN §5): the v0.2 HUD numbers in a grid plus 60-second
//! RSS and CPU sparklines from the 1 s `live_stats()` samples (no extra polling).

use std::collections::VecDeque;

use gpui::{Context, FontWeight, Rgba, SharedString, div, prelude::*, px};
use serde_json::json;
use tellal_core::LiveStats;

use super::{Panel, STATUS_H};
use crate::icons::{Icon, icon};
use crate::theme::{Tokens, TypeScale, alpha};
use crate::widgets::{ButtonKind, tabular, text_button, tip};

const SPARK_LEN: usize = 60;

#[derive(Default)]
pub(super) struct Sparks {
    rss: VecDeque<f32>,
    cpu: VecDeque<f32>,
}

impl Sparks {
    pub(super) fn push(&mut self, s: &LiveStats) {
        for (series, v) in [(&mut self.rss, s.rss_mb as f32), (&mut self.cpu, s.cpu_pct as f32)] {
            if series.len() == SPARK_LEN {
                series.pop_front();
            }
            series.push_back(v);
        }
    }
}

fn fmt_ms(v: Option<f64>) -> String {
    v.map_or_else(|| "–".to_owned(), |v| format!("{v:.0} ms"))
}

/// 60 one-second bars, scaled to the window's max (G1-23: length encodes the value).
fn sparkline(series: &VecDeque<f32>, color: Rgba, t: &Tokens) -> impl IntoElement {
    let max = series.iter().copied().fold(0f32, f32::max).max(1.);
    let pad = SPARK_LEN - series.len();
    div()
        .flex()
        .items_end()
        .gap(px(1.))
        .w_full()
        .h(px(28.))
        .px_1()
        .rounded(px(4.))
        .bg(alpha(t.text_muted, 0.06))
        .children((0..pad).map(|_| div().flex_1().min_w(px(0.)).h(px(1.)).bg(alpha(t.text_muted, 0.2))))
        .children(
            series
                .iter()
                .map(|v| div().flex_1().min_w(px(0.)).h(px((v / max * 26.).max(1.))).rounded(px(1.)).bg(color)),
        )
}

impl Panel {
    pub(super) fn render_status_line(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> impl IntoElement {
        let listening = self.channels.values().filter(|c| c.listening).count();
        let l = self.s;
        let (dot, dot_tip) = if self.error.is_some() {
            (t.danger, l.conn_error_tip)
        } else if self.loaded && self.stats.t3_connected {
            (t.success, l.conn_ok_tip)
        } else {
            (t.warning, l.conn_connecting_tip)
        };
        let mute = (listening > 0 && self.player_channel().is_none()).then(|| {
            text_button("status_mute", Some(Icon::VolumeX), l.mute, ButtonKind::Ghost, t, ty.badge + 0.5)
                .h(px(24.))
                .tooltip(tip(l.mute_tip))
                .on_click(cx.listener(|panel, _, _, cx| {
                    panel.mute_all("click");
                    cx.notify();
                }))
        });
        let toggle = div()
            .id("metrics_toggle")
            .flex()
            .items_center()
            .gap_1()
            .h(px(24.))
            .px_1p5()
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(t.surface_hover))
            .tooltip(tip(if self.drawer_open { l.metrics_close } else { l.metrics_open }))
            .child(l.metrics)
            .child(icon(if self.drawer_open { Icon::ChevronDown } else { Icon::ChevronRight }, 12., t.text_muted))
            .on_click(cx.listener(|panel, _, _, cx| panel.set_drawer(!panel.drawer_open, "click", cx)));
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(STATUS_H))
            .px_3()
            .border_t_1()
            .border_color(t.border)
            .text_size(px(ty.secondary))
            .text_color(t.text_muted)
            .child(div().id("conn").size(px(7.)).rounded_full().bg(dot).tooltip(tip(dot_tip)))
            .child(tabular(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(l.status_counts(self.threads.len(), listening)),
            ))
            .children(mute)
            .child(toggle)
    }

    pub(super) fn render_drawer(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> impl IntoElement {
        let s = &self.stats;
        let l = self.s;
        let cells: [(&str, String); 10] = [
            ("RSS", format!("{:.0} MB", s.rss_mb)),
            ("CPU", format!("{:.1} %", s.cpu_pct)),
            (l.cell_t3_poll, fmt_ms(s.last_t3_poll_ms)),
            (l.cell_live_poll, fmt_ms(s.last_live_poll_ms)),
            ("TTS", fmt_ms(s.last_tts_ms)),
            ("RTF", s.last_rtf.map_or_else(|| "–".to_owned(), |r| format!("{r:.2}"))),
            (l.cell_click_audio, fmt_ms(s.last_click_to_audio_ms)),
            (l.cell_msg_audio, fmt_ms(s.last_msg_to_audio_ms)),
            (l.cell_dictation, fmt_ms(s.last_dictation_ms)),
            (l.cell_queue, l.queue_cell(s.queue_len, s.listening)),
        ];
        let cell = |(label, value): (&str, String)| {
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap_2()
                .h(px(18.))
                .child(div().text_color(t.text_muted).child(SharedString::from(label.to_owned())))
                .child(tabular(div().font_weight(FontWeight::MEDIUM).child(value)))
        };
        let spark = |label: &str, value: String, series: &std::collections::VecDeque<f32>, color: Rgba| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(div().text_color(t.text_muted).child(SharedString::from(label.to_owned())))
                        .child(tabular(div().child(value))),
                )
                .child(sparkline(series, color, t))
        };
        let run_dir = s.run_dir.display().to_string();
        let hotkey = self.hotkeys.toggle.as_ref().map_or_else(|| l.shortcut_off.to_owned(), |h| l.shortcut(&h.spec));
        let stop_key = self.hotkeys.stop.as_ref().map_or_else(|| l.off.to_owned(), |h| h.spec.clone());
        let mut computer_cells = self.computer.cells(l);
        computer_cells.push((l.cell_stop, stop_key));
        let computer = div()
            .id("computer_status")
            .flex()
            .flex_col()
            .tooltip(tip(l.computer_tip))
            .child(div().h(px(18.)).font_weight(FontWeight::MEDIUM).child(l.computer_title))
            .child(div().grid().grid_cols(2).gap_x_4().children(computer_cells.into_iter().map(cell)));
        div()
            .flex()
            .flex_none()
            .flex_col()
            .gap_2()
            .h(px(self.drawer_height()))
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(t.border)
            .bg(t.surface)
            .text_size(px(ty.badge + 0.5))
            .child(div().grid().grid_cols(2).gap_x_4().children(cells.into_iter().map(cell)))
            .child(computer)
            .children(self.render_voices_section(t, ty, cx))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(div().flex_1().min_w(px(0.)).child(spark(
                        l.rss_60,
                        format!("{:.0} MB", s.rss_mb),
                        &self.sparks.rss,
                        t.info,
                    )))
                    .child(div().flex_1().min_w(px(0.)).child(spark(
                        l.cpu_60,
                        format!("{:.1} %", s.cpu_pct),
                        &self.sparks.cpu,
                        t.accent,
                    ))),
            )
            .child(
                div()
                    .id("run_dir")
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .h(px(20.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(t.text_muted)
                    .hover(|s| s.text_color(t.text))
                    .tooltip(tip(l.run_dir_tip(&run_dir)))
                    .child(icon(Icon::Folder, 12., t.text_muted))
                    .child(div().flex_1().min_w(px(0.)).truncate().child(run_dir))
                    .child(div().flex_none().child(hotkey))
                    .on_click(cx.listener(|panel, _, _, _| {
                        let run_dir = panel.stats.run_dir.clone();
                        panel.log_action("click", "open_run_dir", None, json!({"path": run_dir.display().to_string()}));
                        open_in_finder(run_dir);
                    })),
            )
    }
}

pub(super) fn open_in_finder(path: std::path::PathBuf) {
    std::thread::spawn(move || {
        // `open` returns as soon as Finder has the request; waiting just reaps the child.
        let _ = std::process::Command::new("/usr/bin/open").arg(path).status();
    });
}
