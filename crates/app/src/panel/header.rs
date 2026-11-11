//! Header (identity + state pill + tools), error banner, search bar, attention card, compact idle
//! strip and the toast.

use gpui::{AnyElement, Context, FontWeight, MouseButton, SharedString, div, prelude::*, px};
use serde_json::json;

use super::{ATTENTION_H, BANNER_H, COMPACT_IDLE_H, ErrorBanner, HEADER_H, Panel, Retry, SEARCH_H, STATUS_H, Toast};
use crate::icons::{Icon, icon};
use crate::mac;
use crate::model::{Filter, Section};
use crate::motion;
use crate::theme::{Tokens, TypeScale, alpha};
use crate::widgets::{ButtonKind, icon_button, pill, text_button, tip};

/// Header state: a T3-style pill only when the panel is not live; `None` = live.
fn state_pill(panel: &Panel, t: &Tokens) -> Option<(&'static str, gpui::Rgba, &'static str)> {
    if panel.error.is_some() {
        Some(("hata", t.danger, "bir işlem başarısız oldu · ayrıntı aşağıda"))
    } else if !panel.loaded || !panel.stats.t3_connected {
        Some(("bağlanıyor", t.warning, "T3'ten thread listesi bekleniyor"))
    } else if !panel.stats.voice_ready {
        Some(("ses yükleniyor", t.info, "yerel ses modeli hazırlanıyor · dinleme birazdan başlar"))
    } else {
        None
    }
}

impl Panel {
    pub(super) fn render_header(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> impl IntoElement {
        let state = match state_pill(self, t) {
            Some((label, color, hint)) => div()
                .id("state")
                .flex_none()
                .flex()
                .items_center()
                .h(px(16.))
                .px(px(4.))
                .rounded(px(4.))
                .bg(alpha(color, 0.14))
                .text_size(px(ty.badge))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(label)
                .tooltip(tip(hint)),
            None => div()
                .id("state")
                .flex_none()
                .size(px(6.))
                .rounded_full()
                .bg(t.success)
                .tooltip(tip("canlı · T3'e bağlı, ses hazır")),
        };
        // While an agent may act on the screen: icon + `ekran` in danger (not colour alone).
        let screen = self.computer.active().then(|| {
            div()
                .id("screen")
                .flex_none()
                .flex()
                .items_center()
                .gap(px(3.))
                .h(px(16.))
                .px(px(4.))
                .rounded(px(4.))
                .bg(alpha(t.danger, 0.14))
                .text_size(px(ty.badge))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.danger)
                .child(icon(Icon::MousePointer2, 11., t.danger))
                .child("ekran")
                .tooltip(tip("bir ajan ekranını kullanıyor · Durdur ya da ⌘."))
        });
        let core = self.core.clone();
        let identity = div()
            .id("drag")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .items_center()
            .gap_2()
            .pl_3()
            // T3's wordmark treatment: bold "T3", muted "Panel", 14 px / 500.
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_1()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(div().font_weight(FontWeight::BOLD).child("T3"))
                    .child(div().text_color(t.text_muted).child("Panel")),
            )
            .child(state)
            .children(screen)
            .on_mouse_down(MouseButton::Left, move |event, window, _| {
                let dragging = mac::drag_window(window);
                core.log_ui(
                    "ui.drag",
                    json!({"x": f32::from(event.position.x), "y": f32::from(event.position.y), "native_drag": dragging}),
                );
            });
        let new_task = icon_button("new_task", Icon::Plus, t, self.reply.as_ref().is_some_and(|d| d.is_new_task()))
            .tooltip(tip("Yeni görev (N)"))
            .on_click(cx.listener(|panel, _, window, cx| panel.open_new_task("click", window, cx)));
        let search = icon_button("search", Icon::Search, t, self.search.open).tooltip(tip("ara (⌘K)")).on_click(
            cx.listener(|panel, _, window, cx| {
                if panel.search.open {
                    panel.close_search("click", window, cx);
                } else {
                    panel.open_search("click", window, cx);
                }
            }),
        );
        let drawer = icon_button("drawer", Icon::Gauge, t, self.drawer_open)
            .tooltip(tip(if self.drawer_open { "ölçümleri kapat" } else { "ölçümler" }))
            .on_click(cx.listener(|panel, _, _, cx| panel.set_drawer(!panel.drawer_open, "click", cx)));
        let compact = icon_button("compact", if self.compact { Icon::Maximize } else { Icon::Minimize }, t, false)
            .tooltip(tip(if self.compact { "tam görünüm (⌘⇧C)" } else { "yalnız oynatıcı (⌘⇧C)" }))
            .on_click(cx.listener(|panel, _, _, cx| panel.set_compact(!panel.compact, "click", cx)));
        let quit = icon_button("quit", Icon::X, t, false).tooltip(tip("paneli kapat (uygulamadan çıkar)")).on_click(
            cx.listener(|panel, _, _, cx| {
                panel.log_action("click", "quit", None, json!({}));
                panel.quit("user", cx);
            }),
        );
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(HEADER_H))
            .pr_1p5()
            .gap_0p5()
            .border_b_1()
            .border_color(t.border)
            .child(identity)
            .when(!self.compact, |d| d.child(new_task).child(search).child(drawer))
            .child(compact)
            .child(quit)
    }

    pub(super) fn render_error(
        &self,
        e: &ErrorBanner,
        t: &Tokens,
        ty: &TypeScale,
        cx: &mut Context<Panel>,
    ) -> impl IntoElement {
        let retry = e.retry.clone().map(|retry| {
            text_button("retry", None, "Tekrar dene", ButtonKind::Secondary, t, ty.secondary).on_click(cx.listener(
                move |panel, _, _, cx| {
                    panel.log_action("click", "error_retry", None, json!({"retry": format!("{retry:?}")}));
                    panel.error = None;
                    match &retry {
                        Retry::Open(id) => panel.open_thread(id, "retry"),
                    }
                    cx.notify();
                },
            ))
        });
        let logs = text_button("logs", None, "Kayıtları aç", ButtonKind::Ghost, t, ty.secondary).on_click(cx.listener(
            |panel, _, _, _| {
                let run_dir = panel.stats.run_dir.clone();
                panel.log_action("click", "error_logs", None, json!({"path": run_dir.display().to_string()}));
                super::metrics::open_in_finder(run_dir);
            },
        ));
        let dismiss =
            icon_button("dismiss", Icon::X, t, false).tooltip(tip("kapat")).on_click(cx.listener(|panel, _, _, cx| {
                panel.log_action("click", "error_dismiss", None, json!({}));
                panel.error = None;
                cx.notify();
            }));
        let message = e.message.clone();
        let card = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_2()
            .rounded(px(10.))
            .border_1()
            .border_color(alpha(t.danger, 0.5))
            .bg(alpha(t.danger, 0.10))
            .child(
                div()
                    .id("error_text")
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(20.))
                    .child(icon(Icon::TriangleAlert, 16., t.danger))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(ty.secondary))
                            .child(SharedString::from(format!("Bir şey ters gitti: {message}"))),
                    )
                    .tooltip(tip(message)),
            )
            .child(div().flex().items_center().justify_end().gap_1p5().children(retry).child(logs).child(dismiss));
        motion::fade_in(
            div().flex_none().h(px(BANNER_H)).px_2().pt_2().child(card),
            ("error", e.seq),
            motion::NORMAL,
            cx,
        )
    }

    pub(super) fn render_search(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> impl IntoElement {
        let close = icon_button("search_close", Icon::X, t, false)
            .tooltip(tip("aramayı kapat (Esc)"))
            .on_click(cx.listener(|panel, _, window, cx| panel.close_search("click", window, cx)));
        let count =
            (!self.search.query.is_empty()).then(|| pill(format!("{} sonuç", self.model.matched), t.text_muted, 0.12));
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(SEARCH_H))
            .px_2()
            .border_b_1()
            .border_color(t.border)
            .child(div().pl_1().child(icon(Icon::Search, 14., t.text_muted)))
            .child(div().flex_1().min_w(px(0.)).text_size(px(ty.body)).child(self.search.input.clone()))
            .children(count)
            .child(close)
    }

    /// `⚠ N thread seni bekliyor · Kazan, …  →` (only while ≥ 1 thread waits).
    pub(super) fn render_attention(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> Option<AnyElement> {
        let waiting = self.waiting();
        if waiting.is_empty() {
            return None;
        }
        let mut names: Vec<&str> = Vec::new();
        for w in &waiting {
            let name = if w.project.is_empty() { w.title.as_str() } else { w.project.as_str() };
            if !names.contains(&name) {
                names.push(name);
            }
        }
        let card = div()
            .id("attention")
            .flex()
            .items_center()
            .gap_2()
            .h(px(40.))
            .px_2p5()
            .rounded(px(10.))
            .border_1()
            .border_color(alpha(t.warning, 0.45))
            .bg(alpha(t.warning, 0.10))
            .cursor_pointer()
            .hover(|s| s.bg(alpha(t.warning, 0.18)))
            .active(|s| s.bg(alpha(t.warning, 0.24)))
            .tooltip(tip("tıkla: sıradaki bekleyen thread'e git"))
            .child(icon(Icon::CircleAlert, 16., t.warning))
            .child(
                div()
                    .flex_none()
                    .text_size(px(ty.secondary))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{} thread seni bekliyor", waiting.len())),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_size(px(ty.secondary))
                    .text_color(t.text_muted)
                    .child(format!("· {}", names.join(", "))),
            )
            .child(icon(Icon::ChevronRight, 14., t.text_muted))
            .on_click(cx.listener(|panel, _, _, cx| panel.jump_to_waiting("click", cx)));
        Some(motion::slide_up(div().flex_none().h(px(ATTENTION_H)).px_2().pt_2().child(card), "attention_in", 6., cx))
    }

    /// Selects the next waiting thread (expanding its group), scrolls to it and pulses it once.
    pub(super) fn jump_to_waiting(&mut self, source: &str, cx: &mut Context<Panel>) {
        let waiting: Vec<(String, Section)> = self.waiting().iter().map(|t| (t.id.clone(), Section::of(t))).collect();
        if waiting.is_empty() {
            return;
        }
        let (id, section) = waiting[self.attention_cursor % waiting.len()].clone();
        self.attention_cursor = self.attention_cursor.wrapping_add(1);
        let expand = Filter::collapsed_in(&self.toggled, section);
        if expand && !self.toggled.remove(&section) {
            self.toggled.insert(section);
        }
        self.rebuild_model();
        self.log_action(
            source,
            "attention_jump",
            Some(&id),
            json!({"waiting": waiting.len(), "section": section.name(), "expanded": expand}),
        );
        self.select(Some(id.clone()), source);
        let seq = self.next_seq();
        self.pulse = Some((id, seq));
        cx.notify();
    }

    pub(super) fn render_compact_idle(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> impl IntoElement {
        let listening = self.channels.values().filter(|c| c.listening).count();
        let expand = text_button("expand", Some(Icon::Maximize), "Listeyi aç", ButtonKind::Ghost, t, ty.secondary)
            .tooltip(tip("tam görünüm (⌘⇧C)"))
            .on_click(cx.listener(|panel, _, _, cx| panel.set_compact(false, "click", cx)));
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(COMPACT_IDLE_H))
            .px_3()
            .child(icon(if listening > 0 { Icon::Volume1 } else { Icon::VolumeX }, 15., t.text_muted))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_size(px(ty.secondary))
                    .text_color(t.text_muted)
                    .child(format!("Şu an çalan yok · {listening} dinleniyor")),
            )
            .child(expand)
    }
}

pub(super) fn render_toast(toast: &Toast, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> impl IntoElement {
    let bubble = div()
        .flex()
        .items_center()
        .gap_1p5()
        .h(px(30.))
        .px_3()
        .rounded(px(15.))
        .border_1()
        .border_color(alpha(t.success, 0.5))
        .bg(t.surface)
        .text_size(px(ty.secondary))
        .font_weight(FontWeight::MEDIUM)
        .child(icon(Icon::Check, 14., t.success))
        .child(toast.text.clone());
    div().absolute().left_0().right_0().bottom(px(STATUS_H + 10.)).flex().justify_center().child(motion::spring_in(
        bubble,
        ("toast", toast.seq),
        cx,
    ))
}
