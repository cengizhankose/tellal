//! The thread list in T3's default sidebar layout: section headers (pinned, working, active,
//! snoozed, settled) and 36 px rows of project mark · title · status · provider · listen ·
//! time/actions, plus the loading / empty / no-match states.

use std::time::Instant;

use gpui::{Animation, AnimationExt, AnyElement, Context, FontWeight, Rgba, SharedString, div, prelude::*, px, rgb};
use serde_json::json;
use tellal_core::{ChannelView, ThreadView};

use super::{BodyClick, LIST_PAD, Panel, ROW_H, ROW2_H, SECTION_H, STATE_BLOCK_H};
use crate::icons::{self, Icon, asset_svg, icon};
use crate::model::{Filter, Item, Section};
use crate::motion;
use crate::theme::{Theme, Tokens, TypeScale, alpha, mix};
use crate::time;
use crate::widgets::{ButtonKind, icon_button, pill, tabular, text_button, tip};

/// Row status (T3 F15, plus our interrupted state): glyph, colour, tooltip and optional text.
/// Static glyphs only; never colour alone (tooltip + shape).
struct Status {
    icon: Icon,
    color: Rgba,
    label: &'static str,
    text: Option<String>,
}

impl Panel {
    fn status(&self, thread: &ThreadView, updated: Option<i64>, waiting: bool, t: &Tokens) -> Option<Status> {
        if waiting {
            return Some(Status { icon: Icon::CircleAlert, color: t.warning, label: self.s.status_waiting, text: None });
        }
        match thread.status.as_str() {
            "running" => {
                let since = thread.running_since.as_deref().and_then(time::parse_rfc3339_unix);
                Some(Status {
                    icon: Icon::CircleDashed,
                    color: t.info,
                    label: self.s.status_working,
                    text: since.map(|s| time::working(s, self.now_unix, self.s)),
                })
            }
            // T3 shows Done only for a completion the user has not seen yet.
            "completed" | "idle"
                if !self.seen.contains(&thread.id) && updated.is_some_and(|u| u >= self.start_unix) =>
            {
                Some(Status { icon: Icon::CircleCheck, color: t.success, label: self.s.status_done, text: None })
            }
            "interrupted" | "cancelled" => {
                Some(Status { icon: Icon::CirclePause, color: t.text_muted, label: self.s.status_interrupted, text: None })
            }
            _ => None,
        }
    }
}

/// T3's project icon (lucide glyph in its colour), the "No project" glyph, or T3's generated
/// two-letter monogram tile in the project's identity colour.
fn project_mark(thread: &ThreadView, t: &Tokens, dark: bool) -> AnyElement {
    let tint = |name: Option<&str>| icons::project_color(name, dark).map_or(t.text_muted, rgb);
    if let Some(i) = &thread.project_icon {
        return asset_svg(icons::project_glyph(&i.name), 14., tint(i.color.as_deref())).into_any_element();
    }
    if thread.project_id.is_empty() {
        return icon(Icon::MessageSquareDashed, 14., tint(Some("gray"))).into_any_element();
    }
    let color = tint(Some(icons::project_identity_color(&thread.project)));
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .h(px(16.))
        .min_w(px(16.))
        .px(px(2.))
        .rounded(px(4.))
        .bg(alpha(color, 0.14))
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(icons::project_monogram(&thread.project))
        .into_any_element()
}

/// Provider mark as T3 draws it: 14 px at 60 % (brand colour where T3 has one), plus the
/// instance initials for generic ACP-registry marks.
fn provider_mark(thread: &ThreadView, t: &Tokens, dark: bool, ty: &TypeScale) -> AnyElement {
    let mark = icons::provider_mark(&thread.provider);
    let color = mark.color.map_or(t.text, |(d, l)| rgb(if dark { d } else { l }));
    let label: SharedString = if thread.model.is_empty() {
        mark.label.clone().into()
    } else {
        format!("{} · {}", mark.label, thread.model).into()
    };
    let tile = mark.monogram.map(|m| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(16.))
            .px(px(3.))
            .rounded(px(4.))
            .border_1()
            .border_color(t.border)
            .text_size(px(ty.badge))
            .text_color(t.text_muted)
            .child(m)
    });
    div()
        .id("provider")
        .flex_none()
        .flex()
        .items_center()
        .gap_0p5()
        .when_some(mark.glyph, |d, g| d.child(asset_svg(g, 14., alpha(color, 0.6))))
        .children(tile)
        .tooltip(tip(label))
        .into_any_element()
}

impl Panel {
    pub(super) fn render_list(&self, t: &Tokens, ty: &TypeScale, cx: &mut Context<Panel>) -> impl IntoElement {
        let list = div()
            .id("threads")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .py(px(LIST_PAD / 2.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        if !self.loaded {
            // Placeholder rows without shimmer (DESIGN §5 States; E: skeleton myth).
            return list.children([180., 140., 200.].into_iter().map(|w| {
                div()
                    .flex_none()
                    .h(px(ROW_H))
                    .mx_1()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(14.)).rounded(px(4.)).bg(t.surface_hover))
                    .child(div().h(px(10.)).w(px(w)).rounded(px(5.)).bg(t.surface_hover))
            }));
        }
        if self.model.items.is_empty() {
            let s = self.s;
            let (i, title, body) = if self.threads.is_empty() {
                (Icon::MessageSquare, s.empty_title, s.empty_body)
            } else {
                (Icon::Search, s.no_match_title, s.no_match_body)
            };
            return list.child(
                div()
                    .flex_none()
                    .h(px(STATE_BLOCK_H - LIST_PAD))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .px_6()
                    .child(icon(i, 24., t.text_muted))
                    .child(div().text_size(px(ty.body)).font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().text_size(px(ty.secondary)).text_color(t.text_muted).text_center().child(body)),
            );
        }
        let dark = cx.global::<Theme>().view().dark;
        let items: Vec<AnyElement> = self
            .model
            .items
            .iter()
            .map(|item| match item {
                Item::Header { section, count, collapsed, signal } => {
                    self.render_section(*section, *count, *collapsed, *signal, t, ty, cx)
                }
                Item::Row { thread, state_line } => {
                    self.render_row(&self.threads[*thread], self.updated[*thread], *state_line, t, ty, dark, cx)
                }
            })
            .collect();
        list.children(items)
    }

    /// T3 shelf header: 12 px / 500 sentence-case label, count, chevron; collapsible.
    #[allow(clippy::too_many_arguments)]
    fn render_section(
        &self,
        section: Section,
        count: usize,
        collapsed: bool,
        signal: bool,
        t: &Tokens,
        ty: &TypeScale,
        cx: &mut Context<Panel>,
    ) -> AnyElement {
        div()
            .id(section.name())
            .flex_none()
            .flex()
            .items_center()
            .gap_1p5()
            .h(px(SECTION_H))
            .mx_1()
            .px_2()
            .rounded(px(6.))
            .cursor_pointer()
            .text_size(px(ty.secondary))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.text_muted)
            .hover(|s| s.bg(t.surface_hover))
            .active(|s| s.bg(t.border))
            .tooltip(tip(if collapsed { self.s.section_expand } else { self.s.section_collapse }))
            .child(self.s.section(section))
            .child(tabular(div().font_weight(FontWeight::NORMAL).child(count.to_string())))
            .when(signal, |d| {
                d.child(
                    div()
                        .id("signal")
                        .size(px(6.))
                        .rounded_full()
                        .bg(t.warning)
                        .tooltip(tip(self.s.section_signal_tip)),
                )
            })
            .child(div().flex_1())
            .child(icon(if collapsed { Icon::ChevronRight } else { Icon::ChevronDown }, 12., t.text_muted))
            .on_click(cx.listener(move |panel, _, _, cx| panel.toggle_section(section, "click", cx)))
            .into_any_element()
    }

    pub(super) fn toggle_section(&mut self, section: Section, source: &str, cx: &mut Context<Panel>) {
        let was = Filter::collapsed_in(&self.toggled, section);
        if !self.toggled.remove(&section) {
            self.toggled.insert(section);
        }
        self.log_action(
            source,
            "section_toggle",
            None,
            json!({"section": section.name(), "before_collapsed": was, "after_collapsed": !was}),
        );
        self.rebuild_model();
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    fn render_row(
        &self,
        thread: &ThreadView,
        updated: Option<i64>,
        state_line: bool,
        t: &Tokens,
        ty: &TypeScale,
        dark: bool,
        cx: &mut Context<Panel>,
    ) -> AnyElement {
        let id: SharedString = thread.id.clone().into();
        let ch: Option<&ChannelView> = self.channel(&thread.id);
        let waiting = ch.is_some_and(|c| c.waiting_for_user);
        let listening = ch.is_some_and(|c| c.listening);
        let speaking = ch.is_some_and(|c| c.speaking);
        let selected = self.selected.as_deref() == Some(thread.id.as_str());
        let hovered = self.hovered.as_deref() == Some(thread.id.as_str());
        let show_actions = hovered || selected;

        let body_id = thread.id.clone();
        let project: SharedString =
            if thread.project.is_empty() { self.s.no_project.into() } else { thread.project.clone().into() };
        let body = div()
            .id("body")
            .flex()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .items_center()
            .gap_2()
            .child(div().id("project").flex_none().child(project_mark(thread, t, dark)).tooltip(tip(project)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .child(SharedString::from(thread.title.clone())),
            )
            .tooltip(tip(self.s.row_body_tip))
            .on_click(cx.listener(move |panel, _, _, cx| panel.on_body_click(&body_id, cx)));

        let st = self.status(thread, updated, waiting, t);
        // A working row shows its timer where the relative time would be (T3: "Working 16m").
        let timer = st.as_ref().and_then(|s| s.text.clone().map(|text| (text, s.color)));
        let status = st.map(|s| {
            div()
                .id("status")
                .flex_none()
                .flex()
                .items_center()
                .gap_1()
                .text_color(s.color)
                .text_size(px(ty.secondary))
                .child(icon(s.icon, 14., s.color))
                .tooltip(tip(s.label))
        });

        let unheard = ch.filter(|c| !c.listening && c.unheard > 0).map(|c| {
            // Keyed by the count, so the pill springs in again whenever it increments.
            motion::spring_in(pill(self.s.unheard(c.unheard), t.accent, 0.16), ("unheard", c.unheard), cx)
        });

        let listen_id = thread.id.clone();
        let listen_face: AnyElement = if speaking {
            super::player::equalizer(t.accent_text, 14., cx)
        } else if listening {
            motion::spring_in(icon(Icon::Volume2, 15., t.accent_text), ("listen_on", 0usize), cx)
        } else {
            icon(Icon::Volume1, 15., t.text_muted).into_any_element()
        };
        let listen = div()
            .id("listen")
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(listening, |d| d.bg(t.accent).hover(|s| s.bg(mix(t.accent, t.canvas, 0.15))))
            .when(!listening, |d| d.border_1().border_color(t.border).hover(|s| s.bg(t.surface_hover)))
            .active(|s| s.opacity(0.8))
            .tooltip(tip(if listening { self.s.listen_on_tip } else { self.s.listen_off_tip }))
            .child(listen_face)
            .on_click(cx.listener(move |panel, _, _, cx| {
                panel.toggle_listen(&listen_id, "click");
                cx.notify();
            }));

        let right: AnyElement = if show_actions {
            let reply_id = thread.id.clone();
            let open_id = thread.id.clone();
            let replying_here = self.reply.as_ref().is_some_and(|d| d.thread_id == thread.id);
            let kind = if replying_here { ButtonKind::Primary } else { ButtonKind::Secondary };
            let actions = div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    text_button("reply", Some(Icon::MessageSquare), self.s.reply, kind, t, ty.secondary)
                        .tooltip(tip(self.s.reply_tip))
                        .on_click(cx.listener(move |panel, _, window, cx| {
                            panel.last_body_click = None;
                            panel.log_action("click", "reply", Some(&reply_id), json!({}));
                            panel.open_reply(&reply_id, "click", true, window, cx);
                        })),
                )
                .child(icon_button("open", Icon::ArrowUpRight, t, false).tooltip(tip(self.s.open_tip)).on_click(
                    cx.listener(move |panel, _, _, cx| {
                        panel.last_body_click = None;
                        panel.log_action("click", "open", Some(&open_id), json!({}));
                        panel.open_thread(&open_id, "click");
                        cx.notify();
                    }),
                ));
            motion::fade_in(actions, ("actions", self.hover_seq), motion::FAST, cx)
        } else {
            let (text, color) =
                timer.unwrap_or_else(|| (time::relative(updated, self.now_unix, self.s), t.text_muted));
            tabular(
                div().flex_none().min_w(px(32.)).text_right().text_size(px(ty.secondary)).text_color(color).child(text),
            )
            .into_any_element()
        };

        let line1 = div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(ROW_H))
            .child(body)
            .children(status)
            .children(unheard)
            .child(provider_mark(thread, t, dark, ty))
            .child(listen)
            .child(right);
        let state = state_line.then(|| {
            let (text, color): (SharedString, Rgba) = match ch {
                Some(c) if c.listening && c.now_text.is_some() => {
                    (c.now_text.clone().unwrap_or_default().into(), t.text_muted)
                }
                _ => (self.s.waiting_line.into(), t.warning),
            };
            div()
                .flex_none()
                .h(px(ROW2_H - ROW_H))
                .pl(px(24.))
                .pb_1()
                .truncate()
                .italic()
                .text_size(px(ty.secondary))
                .text_color(color)
                .child(text)
        });

        let hover_id = thread.id.clone();
        let row = div()
            .id(id.clone())
            .flex_none()
            .flex()
            .flex_col()
            .h(px(if state_line { ROW2_H } else { ROW_H }))
            .mx_1()
            .pl_2()
            .pr_1()
            .rounded(px(6.))
            .text_size(px(ty.title))
            .when(selected, |d| d.bg(t.selected))
            .when(!selected, |d| d.hover(|s| s.bg(t.surface_hover)))
            .on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                let was = panel.hovered.as_deref() == Some(hover_id.as_str());
                if *hovered && !was {
                    panel.hovered = Some(hover_id.clone());
                    panel.hover_seq += 1;
                    cx.notify();
                } else if !*hovered && was {
                    panel.hovered = None;
                    cx.notify();
                }
            }))
            .child(line1)
            .children(state);

        let row: AnyElement = match &self.pulse {
            Some((pulse_id, seq)) if *pulse_id == thread.id => {
                let (base, flash) = (t.selected, t.warning);
                row.with_animation(("pulse", *seq), Animation::new(motion::fade(cx, motion::SLOW * 2)), move |el, d| {
                    el.bg(mix(base, flash, 0.35 * motion::pulse_alpha(d)))
                })
                .into_any_element()
            }
            _ => row.into_any_element(),
        };
        // New rows (and rows revealed by expanding a section) enter with a short slide.
        motion::slide_up(div().flex_none().child(row), (id, 0usize), 6., cx)
    }

    /// Single click selects; a second click on the same row body within the system
    /// double-click interval opens the thread in T3 (v0.2 behaviour).
    fn on_body_click(&mut self, thread_id: &str, cx: &mut Context<Panel>) {
        let now = Instant::now();
        let is_double = self
            .last_body_click
            .as_ref()
            .is_some_and(|c| c.thread_id == thread_id && now.duration_since(c.at) <= self.double_click_interval);
        self.select(Some(thread_id.to_owned()), "click");
        if is_double {
            self.last_body_click = None;
            self.core.log_ui("ui.dblclick", json!({"thread": thread_id}));
            self.open_thread(thread_id, "dblclick");
        } else {
            self.last_body_click = Some(BodyClick { thread_id: thread_id.to_owned(), at: now });
        }
        cx.notify();
    }

    /// Moves the selection (logs it, marks the thread seen, keeps it scrolled into view).
    pub(super) fn select(&mut self, id: Option<String>, source: &str) {
        if self.selected == id {
            return;
        }
        let before = std::mem::replace(&mut self.selected, id.clone());
        self.pulse = None;
        self.log_action(source, "select", id.as_deref(), json!({"before": before}));
        if let Some(id) = id {
            self.mark_seen(&id);
            self.scroll_to_selected();
        }
    }

    pub(super) fn toggle_listen(&mut self, thread_id: &str, source: &str) {
        let before = self.channel(thread_id).is_some_and(|c| c.listening);
        self.last_body_click = None;
        self.log_action(
            source,
            if before { "mute" } else { "listen" },
            Some(thread_id),
            json!({"before": before, "after": !before}),
        );
        self.mark_seen(thread_id);
        self.core.set_listening(thread_id, !before);
    }

    pub(super) fn open_thread(&mut self, thread_id: &str, source: &str) {
        self.core.log_ui("ui.open_thread", json!({"thread": thread_id, "source": source}));
        self.last_open = Some(thread_id.to_owned());
        self.mark_seen(thread_id);
        self.core.open_thread(thread_id);
    }

    /// Keeps the selected row visible (list children are flat, so item index = child index).
    pub(super) fn scroll_to_selected(&self) {
        let Some(sel) = self.selected.as_deref() else {
            return;
        };
        let ix = self
            .model
            .items
            .iter()
            .position(|item| matches!(item, Item::Row { thread, .. } if self.threads[*thread].id == sel));
        if let Some(ix) = ix {
            self.scroll.scroll_to_item(ix);
        }
    }
}
