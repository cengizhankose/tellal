//! The floating panel (DESIGN §5): state, the core event pump, window height and the top-level
//! layout. Each region renders in its own child module.

mod dock;
mod header;
mod keys;
mod list;
mod metrics;
mod player;
mod selftest;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, MouseButton, ScrollHandle, SharedString, Window, div,
    prelude::*, px,
};
use serde_json::{Value, json};
use t3_panel_core::{ChannelView, Core, CoreEvent, LiveStats, Receiver, ThreadView};

use crate::hotkey::Hotkey;
use crate::input::{InputEvent, TextInput};
use crate::mac::{self, FrontApp, NativeWindow};
use crate::model::{self, Filter, Item, ListModel, Section};
use crate::motion::{self, Motion};
use crate::theme::Theme;
use crate::time;

pub use keys::bind_keys;
pub use selftest::SelfTestMode;

use dock::ReplyDock;
use metrics::Sparks;
use selftest::SelfTest;

const PUMP_INTERVAL: Duration = Duration::from_millis(150);
const STATS_INTERVAL: Duration = Duration::from_secs(1);
/// Delay before recording which app ended up frontmost after an activate / restore.
const FOCUS_CHECK_DELAY: Duration = Duration::from_millis(500);
const TOAST_FOR: Duration = Duration::from_millis(1800);

// Fixed region heights (px). The window is as tall as its content (DESIGN §5: 160–640).
pub const WIDTH: f32 = 380.;
const MIN_H: f32 = 160.;
const MAX_H: f32 = 640.;
const HEADER_H: f32 = 40.;
const BANNER_H: f32 = 76.;
const ATTENTION_H: f32 = 48.;
const SEARCH_H: f32 = 44.;
const SECTION_H: f32 = 32.;
const ROW_H: f32 = 36.;
const ROW2_H: f32 = 56.;
const LIST_PAD: f32 = 8.;
const STATE_BLOCK_H: f32 = 132.;
const PLAYER_H: f32 = 112.;
const COMPACT_IDLE_H: f32 = 44.;
const DOCK_H: f32 = 116.;
const STATUS_H: f32 = 32.;
const DRAWER_H: f32 = 236.;
const RADIUS: f32 = 14.;

/// A row-body click remembered to detect the second click of a double-click.
struct BodyClick {
    thread_id: String,
    at: Instant,
}

/// Error banner under the header (DESIGN §5 States).
struct ErrorBanner {
    message: SharedString,
    retry: Option<Retry>,
    seq: usize,
}

/// What `Tekrar dene` re-runs.
#[derive(Clone, Debug)]
enum Retry {
    Open(String),
}

struct Toast {
    text: SharedString,
    until: Instant,
    seq: usize,
}

struct Search {
    open: bool,
    input: Entity<TextInput>,
    /// Lowercased query.
    query: String,
}

/// Window height animation (content height changes: dock, player, compact, drawer…).
struct Height {
    from: f32,
    to: f32,
    start: Instant,
    dur: Duration,
    applied: f32,
    /// `Panel::compact` when `to` was set.
    compact: bool,
}

impl Height {
    fn at(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.start);
        if self.dur.is_zero() || elapsed >= self.dur {
            return self.to;
        }
        let t = elapsed.as_secs_f32() / self.dur.as_secs_f32();
        self.from + (self.to - self.from) * motion::ease_in_out(t)
    }
}

pub struct Panel {
    core: Core,
    process_start: Instant,
    ready_logged: bool,
    native: Option<NativeWindow>,
    focus: FocusHandle,

    threads: Vec<ThreadView>,
    /// `updated_at` of each thread as Unix seconds (parsed once per snapshot).
    updated: Vec<Option<i64>>,
    /// The first thread snapshot arrived (else the list shows the loading state).
    loaded: bool,
    channels: HashMap<String, ChannelView>,
    stats: LiveStats,
    stats_at: Instant,
    sparks: Sparks,
    now_unix: i64,

    model: ListModel,
    /// Sections the user collapsed/expanded away from T3's default.
    toggled: HashSet<Section>,
    /// Threads the user has seen this session (listened, opened or selected): their completion
    /// no longer shows the green check (T3's "unseen completion").
    seen: HashSet<String>,
    start_unix: i64,
    /// Last thread `open_thread` was called for (the error banner's `Tekrar dene`).
    last_open: Option<String>,
    selected: Option<String>,
    hovered: Option<String>,
    hover_seq: usize,
    /// Row highlighted once after the attention card jumped to it.
    pulse: Option<(String, usize)>,
    attention_cursor: usize,
    last_body_click: Option<BodyClick>,
    double_click_interval: Duration,
    scroll: ScrollHandle,
    compact: bool,
    drawer_open: bool,
    search: Search,
    error: Option<ErrorBanner>,
    toast: Option<Toast>,
    seq: usize,

    input: Entity<TextInput>,
    reply: Option<ReplyDock>,
    /// Pending `ui.focus {action: "check"}` (records which app actually ended up frontmost).
    focus_check: Option<(Instant, &'static str)>,

    height: Height,
    hotkey: Option<Hotkey>,
    /// App that was frontmost when the global shortcut showed the panel.
    hotkey_prev_app: Option<FrontApp>,

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
        let search_input = cx.new(|cx| TextInput::new("Ara: başlık ya da proje", cx));
        cx.subscribe_in(&search_input, window, Panel::on_search_event).detach();
        let native = NativeWindow::of(window);
        if let Some(native) = native {
            native.round_corners(f64::from(RADIUS));
        }
        let hotkey = Hotkey::register(&core);
        let selftest = SelfTest::new(&selftest_mode, now);
        let height = f32::from(window.bounds().size.height);
        let mut panel = Panel {
            stats: core.live_stats(),
            channels: core.channels(),
            core,
            process_start,
            ready_logged: false,
            native,
            focus: cx.focus_handle(),
            threads: Vec::new(),
            updated: Vec::new(),
            loaded: false,
            stats_at: now,
            sparks: Sparks::default(),
            now_unix: time::now_unix(),
            model: ListModel::default(),
            toggled: HashSet::new(),
            seen: HashSet::new(),
            start_unix: time::now_unix(),
            last_open: None,
            selected: None,
            hovered: None,
            hover_seq: 0,
            pulse: None,
            attention_cursor: 0,
            last_body_click: None,
            double_click_interval: mac::double_click_interval(),
            scroll: ScrollHandle::new(),
            compact: false,
            drawer_open: false,
            search: Search { open: false, input: search_input, query: String::new() },
            error: None,
            toast: None,
            seq: 0,
            input,
            reply: None,
            focus_check: None,
            height: Height {
                from: height,
                to: height,
                start: now,
                dur: Duration::ZERO,
                applied: height,
                compact: false,
            },
            hotkey,
            hotkey_prev_app: None,
            selftest_mode,
            selftest,
        };
        panel.refresh_threads();
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

    fn next_seq(&mut self) -> usize {
        self.seq += 1;
        self.seq
    }

    /// Marks a thread seen (its unseen-completion check disappears).
    fn mark_seen(&mut self, thread_id: &str) {
        if !self.seen.contains(thread_id) {
            self.seen.insert(thread_id.to_owned());
        }
    }

    fn refresh_threads(&mut self) {
        self.threads = self.core.threads();
        self.updated = self.threads.iter().map(|t| time::parse_rfc3339_unix(&t.updated_at)).collect();
        if !self.threads.is_empty() {
            self.loaded = true;
        }
        self.rebuild_model();
    }

    fn rebuild_model(&mut self) {
        let filter = Filter { query: &self.search.query, toggled: &self.toggled };
        self.model = model::build(&self.threads, &self.channels, &filter);
    }

    fn thread(&self, id: &str) -> Option<&ThreadView> {
        self.threads.iter().find(|t| t.id == id)
    }

    fn channel(&self, id: &str) -> Option<&ChannelView> {
        self.channels.get(id)
    }

    /// Threads waiting for the user, in list order.
    fn waiting(&self) -> Vec<&ThreadView> {
        self.threads.iter().filter(|t| self.channel(&t.id).is_some_and(|c| c.waiting_for_user)).collect()
    }

    /// Writes `ui.click` / `ui.key` with the action, its target and the state around it.
    fn log_action(&self, source: &str, action: &str, target: Option<&str>, extra: Value) {
        let kind = match source {
            "key" => "ui.key",
            "hotkey" => "ui.hotkey",
            _ => "ui.click",
        };
        let mut input = json!({"action": action, "source": source, "thread": target});
        if let (Value::Object(map), Value::Object(more)) = (&mut input, extra) {
            map.extend(more);
        }
        self.core.log_ui(kind, input);
    }

    /// Drains core events, refreshes cached state, advances the dock, toast and self-test.
    fn pump(&mut self, events: &Receiver<CoreEvent>, window: &mut Window, cx: &mut Context<Panel>) {
        let mut changed = false;
        let mut threads_updated = false;
        let mut channels_updated = false;
        for event in events.try_iter() {
            changed = true;
            match event {
                CoreEvent::ThreadsUpdated => threads_updated = true,
                CoreEvent::ChannelsUpdated => channels_updated = true,
                CoreEvent::SpeakingStarted { thread_id } => {
                    self.stats = self.core.live_stats();
                    channels_updated = true;
                    self.selftest_speaking_started(&thread_id);
                }
                CoreEvent::SpeakingFinished { thread_id } => {
                    self.stats = self.core.live_stats();
                    channels_updated = true;
                    self.selftest_speaking_finished(&thread_id);
                }
                CoreEvent::DictationTranscript { thread_id, text } => self.on_transcript(thread_id, text),
                CoreEvent::DictationFailed { thread_id, error } => self.on_dictation_failed(&thread_id, error),
                CoreEvent::ReplySent { thread_id, ok, queued, detail } => {
                    self.on_reply_sent(&thread_id, ok, queued, detail, cx)
                }
                CoreEvent::Error(message) => {
                    let retry = self.last_open.as_ref().filter(|id| message.contains(id.as_str())).cloned();
                    self.show_error(message.into(), retry.map(Retry::Open));
                }
                CoreEvent::ThemeChanged => self.apply_theme(cx),
                CoreEvent::ComputerUpdated | CoreEvent::TaskStarted { .. } => {}
            }
        }
        if threads_updated {
            self.refresh_threads();
            self.loaded = true;
            self.selftest_threads_ready(window, cx);
        }
        let now = Instant::now();
        if now.duration_since(self.stats_at) >= STATS_INTERVAL {
            self.stats = self.core.live_stats();
            self.stats_at = now;
            self.sparks.push(&self.stats);
            let reduce = mac::reduce_motion();
            if reduce != motion::reduced(cx) {
                self.core.log_ui("ui.reduce_motion", json!({"reduce": reduce}));
                cx.set_global(Motion { reduce });
                changed = true;
            }
            let now_unix = time::now_unix();
            if now_unix / 60 != self.now_unix / 60 {
                changed = true;
            }
            self.now_unix = now_unix;
            // Only the open drawer shows per-second numbers; otherwise stay idle.
            changed |= self.drawer_open;
        }
        // Playback position is only in `channels()` (no event per tick): poll while audible.
        channels_updated |= self.channels.values().any(|c| c.speaking && !c.paused);
        if channels_updated {
            let channels = self.core.channels();
            if channels != self.channels {
                self.channels = channels;
                self.rebuild_model();
                changed = true;
            }
        }
        if self.hotkey.as_ref().is_some_and(Hotkey::pressed) {
            self.toggle_visible(window, cx);
            changed = true;
        }
        if self.toast.as_ref().is_some_and(|t| now >= t.until) {
            self.toast = None;
            changed = true;
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
        if self.advance_selftest(now, window, cx) {
            return;
        }
        if changed {
            cx.notify();
        }
    }

    fn apply_theme(&mut self, cx: &mut Context<Panel>) {
        let view = self.core.theme();
        let fade = motion::fade(cx, motion::NORMAL);
        let theme = cx.global_mut::<Theme>();
        if *theme.view() == view {
            return;
        }
        theme.apply(view, fade);
        let payload = theme.to_json();
        self.core.log_ui("ui.theme", payload);
    }

    fn show_error(&mut self, message: SharedString, retry: Option<Retry>) {
        let seq = self.next_seq();
        self.core.log_ui(
            "ui.error_banner",
            json!({"message": message.as_ref(), "retry": retry.as_ref().map(|r| format!("{r:?}"))}),
        );
        self.error = Some(ErrorBanner { message, retry, seq });
    }

    fn show_toast(&mut self, text: impl Into<SharedString>) {
        let seq = self.next_seq();
        self.toast = Some(Toast { text: text.into(), until: Instant::now() + TOAST_FOR, seq });
    }

    /// Global shortcut: hide, or show and take the keyboard (without activating the app).
    fn toggle_visible(&mut self, window: &mut Window, cx: &mut Context<Panel>) {
        let Some(native) = self.native else {
            return;
        };
        let visible = native.is_visible();
        if visible {
            native.hide();
            if let Some(app) = self.hotkey_prev_app.take() {
                mac::activate_app(app.pid);
            }
        } else {
            self.hotkey_prev_app = mac::frontmost_app().filter(|a| !a.is_self());
            native.show_key();
            window.focus(&self.focus);
            if self.selected.is_none() {
                self.selected = self.model.row_ids.first().cloned();
            }
        }
        self.log_action(
            "hotkey",
            if visible { "hide" } else { "show" },
            None,
            json!({
                "spec": self.hotkey.as_ref().map(|h| h.spec.clone()),
                "frontmost": mac::frontmost_app().map(|a| a.to_json()),
            }),
        );
        self.focus_check =
            Some((Instant::now() + FOCUS_CHECK_DELAY, if visible { "hotkey_hide" } else { "hotkey_show" }));
        cx.notify();
    }

    fn set_compact(&mut self, compact: bool, source: &str, cx: &mut Context<Panel>) {
        if self.compact == compact {
            return;
        }
        self.log_action(source, "compact", None, json!({"before": self.compact, "after": compact}));
        self.compact = compact;
        cx.notify();
    }

    fn set_drawer(&mut self, open: bool, source: &str, cx: &mut Context<Panel>) {
        if self.drawer_open == open {
            return;
        }
        self.log_action(source, "metrics_drawer", None, json!({"before": self.drawer_open, "after": open}));
        self.drawer_open = open;
        cx.notify();
    }

    fn open_search(&mut self, source: &str, window: &mut Window, cx: &mut Context<Panel>) {
        self.log_action(source, "search_open", None, json!({"before": self.search.open}));
        self.search.open = true;
        self.set_compact(false, source, cx);
        let handle = self.search.input.read(cx).focus_handle(cx);
        window.focus(&handle);
        cx.notify();
    }

    fn close_search(&mut self, source: &str, window: &mut Window, cx: &mut Context<Panel>) {
        if !self.search.open {
            return;
        }
        self.log_action(source, "search_close", None, json!({"query": self.search.query}));
        self.search.open = false;
        self.search.query.clear();
        self.search.input.update(cx, |input, cx| input.clear(cx));
        self.rebuild_model();
        window.focus(&self.focus);
        cx.notify();
    }

    fn on_search_event(
        &mut self,
        input: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Panel>,
    ) {
        match event {
            InputEvent::Edited(_) => {
                let query = input.read(cx).text().trim().to_lowercase();
                if query != self.search.query {
                    self.search.query = query;
                    self.rebuild_model();
                    if self.selected.as_ref().is_none_or(|s| !self.model.row_ids.contains(s)) {
                        self.selected = self.model.row_ids.first().cloned();
                    }
                    self.core.log_ui(
                        "ui.search",
                        json!({"query": self.search.query, "matched": self.model.matched, "rows": self.model.row_ids.len()}),
                    );
                    cx.notify();
                }
            }
            // Enter: keep the filter, hand the keyboard to the list (first match selected).
            InputEvent::Submit => {
                self.selected = self.model.row_ids.first().cloned();
                self.log_action("key", "search_submit", self.selected.as_deref(), json!({"query": self.search.query}));
                window.focus(&self.focus);
                cx.notify();
            }
            InputEvent::Cancel => self.close_search("key", window, cx),
        }
    }

    fn input_focused(&self, window: &Window, cx: &App) -> bool {
        self.input.read(cx).focus_handle(cx).is_focused(window)
            || self.search.input.read(cx).focus_handle(cx).is_focused(window)
    }

    /// Channel shown in the Now playing bar: the audible one, else a paused or queued one.
    fn player_channel(&self) -> Option<(&str, &ChannelView)> {
        let mut best: Option<(&str, &ChannelView, u8)> = None;
        for (id, c) in &self.channels {
            let rank = if c.speaking {
                3
            } else if c.paused && c.now_text.is_some() {
                2
            } else if c.listening && c.queued > 0 {
                1
            } else {
                continue;
            };
            if best.is_none_or(|(_, _, r)| rank > r) {
                best = Some((id.as_str(), c, rank));
            }
        }
        best.map(|(id, c, _)| (id, c))
    }

    fn list_height(&self) -> f32 {
        if !self.loaded {
            return 3. * ROW_H + LIST_PAD;
        }
        if self.model.items.is_empty() {
            return STATE_BLOCK_H;
        }
        LIST_PAD
            + self
                .model
                .items
                .iter()
                .map(|item| match item {
                    Item::Header { .. } => SECTION_H,
                    Item::Row { state_line: true, .. } => ROW2_H,
                    Item::Row { .. } => ROW_H,
                })
                .sum::<f32>()
    }

    fn content_height(&self) -> f32 {
        let mut h = HEADER_H + 2.;
        if self.compact {
            h += if self.player_channel().is_some() { PLAYER_H } else { COMPACT_IDLE_H };
            return h;
        }
        if self.error.is_some() {
            h += BANNER_H;
        }
        if self.search.open {
            h += SEARCH_H;
        }
        if !self.waiting().is_empty() {
            h += ATTENTION_H;
        }
        h += self.list_height();
        if self.player_channel().is_some() {
            h += PLAYER_H;
        }
        if self.reply.is_some() {
            h += DOCK_H;
        }
        h += STATUS_H;
        if self.drawer_open {
            h += DRAWER_H;
        }
        h.clamp(MIN_H, MAX_H)
    }

    /// Animates the window toward the content height (`slow` for compact, else `normal`).
    fn update_height(&mut self, window: &mut Window, cx: &mut Context<Panel>) {
        let now = Instant::now();
        let target = self.content_height();
        if (target - self.height.to).abs() >= 0.5 {
            // Compact ↔ full is the one `slow` change; everything else moves at `normal`.
            let base = if self.compact != self.height.compact { motion::SLOW } else { motion::NORMAL };
            self.height = Height {
                from: self.height.at(now),
                to: target,
                start: now,
                dur: motion::movement(cx, base),
                applied: self.height.applied,
                compact: self.compact,
            };
        }
        let h = self.height.at(now).round();
        if (h - self.height.applied).abs() >= 0.5 {
            self.height.applied = h;
            if let Some(native) = self.native {
                cx.spawn(async move |_, _| native.set_height_keep_top(h)).detach();
            }
        }
        if (h - self.height.to).abs() >= 0.5 {
            window.request_animation_frame();
        }
    }

    fn quit(&mut self, reason: &str, cx: &mut Context<Panel>) {
        if self.reply.is_some() {
            self.cancel_reply("quit", cx);
        }
        self.core.log_ui("ui.quit", json!({"reason": reason, "uptime_ms": ms(self.process_start.elapsed())}));
        cx.quit();
    }
}

impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.ready_logged {
            self.ready_logged = true;
            let theme = cx.global::<Theme>().to_json();
            self.core.log_ui(
                "ui.ready",
                json!({
                    "startup_ms": ms(self.process_start.elapsed()), "rows": self.threads.len(),
                    "assets": crate::assets::count(), "missing_icons": crate::icons::missing(),
                    "providers": crate::icons::provider_count(), "reduce_motion": motion::reduced(cx),
                    "window_number": self.native.map(|n| n.number()), "theme": theme,
                }),
            );
        }
        if cx.global_mut::<Theme>().tick() {
            window.request_animation_frame();
        }
        self.update_height(window, cx);
        let theme = cx.global::<Theme>();
        let t = theme.current;
        let ty = theme.ty.clone();

        let mut root = div()
            .id("panel")
            .key_context(keys::KEY_CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(RADIUS))
            .border_1()
            .border_color(t.border)
            .bg(t.window_bg())
            .text_color(t.text)
            .text_size(px(ty.body))
            .when_some(ty.family.clone(), |d, f| d.font_family(f))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|panel, _, window, cx| {
                    if !panel.input_focused(window, cx) {
                        window.focus(&panel.focus);
                    }
                }),
            );
        root = keys::on_actions(root, cx);
        root = root.child(self.render_header(&t, &ty, cx));
        if self.compact {
            root = root.child(match self.player_channel() {
                Some((id, c)) => self.render_player(id, c, &t, &ty, cx).into_any_element(),
                None => self.render_compact_idle(&t, &ty, cx).into_any_element(),
            });
        } else {
            root = root
                .when_some(self.error.as_ref(), |d, e| d.child(self.render_error(e, &t, &ty, cx)))
                .when(self.search.open, |d| d.child(self.render_search(&t, &ty, cx)))
                .children(self.render_attention(&t, &ty, cx))
                .child(self.render_list(&t, &ty, cx))
                .children(self.player_channel().map(|(id, c)| self.render_player(id, c, &t, &ty, cx)))
                .children(self.reply.as_ref().map(|dock| self.render_reply_dock(dock, &t, &ty, cx)))
                .child(self.render_status_line(&t, &ty, cx))
                .when(self.drawer_open, |d| d.child(self.render_drawer(&t, &ty, cx)));
        }
        root.children(self.toast.as_ref().map(|toast| header::render_toast(toast, &t, &ty, cx)))
    }
}

impl Focusable for Panel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
