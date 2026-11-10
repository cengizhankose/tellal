//! The floating panel view: thread rows, header controls, live HUD footer, core event pump and
//! the `T3P_SELFTEST=1` driver.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::{
    Context, Div, ElementId, FontWeight, MouseButton, SharedString, Stateful, Window, div,
    prelude::*, px, rgb, rgba,
};
use serde_json::json;
use t3_panel_core::{Core, CoreEvent, LiveStats, Receiver, ThreadView};

use crate::mac;

const PUMP_INTERVAL: Duration = Duration::from_millis(150);
const STATS_INTERVAL: Duration = Duration::from_secs(1);
const SELFTEST_TIMEOUT: Duration = Duration::from_secs(60);
const SELFTEST_QUIT_DELAY: Duration = Duration::from_secs(1);

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

enum SelfTest {
    Off,
    WaitingForThreads { deadline: Instant },
    Speaking { thread_id: String, deadline: Instant },
    Quitting { at: Instant },
}

pub struct Panel {
    core: Core,
    process_start: Instant,
    ready_logged: bool,
    rows: Vec<Row>,
    stats: LiveStats,
    stats_at: Instant,
    last_error: Option<SharedString>,
    selftest: SelfTest,
}

impl Panel {
    pub fn new(
        core: Core,
        events: Receiver<CoreEvent>,
        process_start: Instant,
        selftest: bool,
        cx: &mut Context<Panel>,
    ) -> Panel {
        let now = Instant::now();
        let mut panel = Panel {
            stats: core.live_stats(),
            core,
            process_start,
            ready_logged: false,
            rows: Vec::new(),
            stats_at: now,
            last_error: None,
            selftest: if selftest {
                SelfTest::WaitingForThreads { deadline: now + SELFTEST_TIMEOUT }
            } else {
                SelfTest::Off
            },
        };
        panel.refresh_rows();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PUMP_INTERVAL).await;
                if this.update(cx, |panel, cx| panel.pump(&events, cx)).is_err() {
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

    /// Drains core events, refreshes cached state and drives the self-test; re-renders on change.
    fn pump(&mut self, events: &Receiver<CoreEvent>, cx: &mut Context<Panel>) {
        let mut changed = false;
        let mut threads_updated = false;
        for event in events.try_iter() {
            changed = true;
            match event {
                CoreEvent::ThreadsUpdated => threads_updated = true,
                CoreEvent::SpeakingStarted { .. } => self.stats = self.core.live_stats(),
                CoreEvent::SpeakingFinished { thread_id } => {
                    self.stats = self.core.live_stats();
                    if matches!(&self.selftest, SelfTest::Speaking { thread_id: t, .. } if *t == thread_id)
                    {
                        self.selftest = SelfTest::Quitting { at: Instant::now() + SELFTEST_QUIT_DELAY };
                    }
                }
                CoreEvent::Error(message) => self.last_error = Some(message.into()),
            }
        }
        if threads_updated {
            let threads = self.refresh_rows();
            if matches!(self.selftest, SelfTest::WaitingForThreads { .. }) && !threads.is_empty() {
                self.start_selftest(&threads);
            }
        }
        let now = Instant::now();
        if now.duration_since(self.stats_at) >= STATS_INTERVAL {
            self.stats = self.core.live_stats();
            self.stats_at = now;
            changed = true;
        }
        match &self.selftest {
            SelfTest::WaitingForThreads { deadline } | SelfTest::Speaking { deadline, .. }
                if now >= *deadline =>
            {
                let phase = match &self.selftest {
                    SelfTest::WaitingForThreads { .. } => "waiting_for_threads",
                    _ => "speaking",
                };
                self.core.log_ui(
                    "ui.selftest_timeout",
                    json!({"phase": phase, "timeout_s": SELFTEST_TIMEOUT.as_secs()}),
                );
                self.quit("selftest_timeout", cx);
                return;
            }
            SelfTest::Quitting { at } if now >= *at => {
                self.quit("selftest_done", cx);
                return;
            }
            _ => {}
        }
        if changed {
            cx.notify();
        }
    }

    fn start_selftest(&mut self, threads: &[ThreadView]) {
        let chosen = threads
            .iter()
            .find(|t| t.status == "completed")
            .unwrap_or(&threads[0]);
        self.core.log_ui(
            "ui.selftest",
            json!({
                "threads": threads.len(),
                "thread": chosen.id,
                "status": chosen.status,
                "title": chosen.title,
            }),
        );
        self.core.log_ui("ui.click", json!({"thread": chosen.id, "action": "speak", "source": "selftest"}));
        self.core.speak_thread(&chosen.id);
        self.selftest = SelfTest::Speaking {
            thread_id: chosen.id.clone(),
            deadline: Instant::now() + SELFTEST_TIMEOUT,
        };
    }

    fn quit(&self, reason: &str, cx: &mut Context<Panel>) {
        self.core.log_ui(
            "ui.quit",
            json!({"reason": reason, "uptime_ms": ms(self.process_start.elapsed())}),
        );
        cx.quit();
    }

    fn render_header(&self, cx: &mut Context<Panel>) -> Div {
        let core = self.core.clone();
        let stop = icon_button("stop", "🔇").on_click(move |_, _, _| {
            core.log_ui("ui.click", json!({"action": "stop"}));
            core.stop_speaking();
        });
        let quit = icon_button("quit", "✕").on_click(cx.listener(|panel, _, _, cx| {
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
                    json!({
                        "x": f32::from(event.position.x),
                        "y": f32::from(event.position.y),
                        "native_drag": dragging,
                    }),
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
            .child(stop)
            .child(quit)
    }

    fn render_row(&self, ix: usize, row: &Row, now_unix: i64) -> Stateful<Div> {
        let speaking = self.stats.speaking.as_deref() == Some(row.id.as_ref());
        let mut subtitle = String::with_capacity(row.project.len() + row.provider.len() + 16);
        subtitle.push_str(&row.project);
        subtitle.push_str(" · ");
        push_relative_time(&mut subtitle, row.updated_unix, now_unix);
        if !row.provider.is_empty() {
            subtitle.push_str(" · ");
            subtitle.push_str(&row.provider);
        }

        let core = self.core.clone();
        let id = row.id.clone();
        let speak = icon_button(("speak", ix), "▶").on_click(move |_, _, _| {
            core.log_ui("ui.click", json!({"thread": id.as_ref(), "action": "speak"}));
            core.speak_thread(&id);
        });
        let core = self.core.clone();
        let id = row.id.clone();
        let open = icon_button(("open", ix), "⤢").on_click(move |_, _, _| {
            core.log_ui("ui.click", json!({"thread": id.as_ref(), "action": "open"}));
            core.open_thread(&id);
        });

        div()
            .id(("row", ix))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1p5()
            .rounded_md()
            .when(speaking, |this| this.bg(rgba(0x0a84ff38)))
            .when(!speaking, |this| this.hover(|this| this.bg(rgba(0xffffff0d))))
            .child(
                div()
                    .flex_none()
                    .size(px(8.))
                    .rounded_full()
                    .bg(rgb(row.status_color)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .child(div().text_size(px(13.)).truncate().child(row.title.clone()))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(TEXT_DIM))
                            .truncate()
                            .child(subtitle),
                    ),
            )
            .child(speak)
            .child(open)
    }

    fn render_footer(&self, cx: &mut Context<Panel>) -> Stateful<Div> {
        let s = &self.stats;
        let run_name = s
            .run_dir
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        let line1 = format!(
            "RSS {:.0} MB · CPU {:.1}% · t3 {} · tts {} · RTF {}",
            s.rss_mb,
            s.cpu_pct,
            fmt_ms(s.last_t3_poll_ms),
            fmt_ms(s.last_tts_ms),
            s.last_rtf.map_or_else(|| "–".to_owned(), |r| format!("{r:.2}")),
        );
        let line2 = format!(
            "click→audio {} · queue {} · {run_name}",
            fmt_ms(s.last_click_to_audio_ms),
            s.queue_len,
        );
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
            .child(div().truncate().child(line1))
            .child(div().truncate().child(line2))
            .on_click(cx.listener(|panel, _, _, _| {
                let run_dir = panel.core.live_stats().run_dir;
                panel.core.log_ui(
                    "ui.click",
                    json!({"action": "open_run_dir", "path": run_dir.display().to_string()}),
                );
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
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);

        let list = div()
            .id("threads")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p_1()
            .overflow_y_scroll()
            .when(self.rows.is_empty(), |this| {
                this.child(
                    div()
                        .p_3()
                        .text_size(px(12.))
                        .text_color(rgb(TEXT_DIM))
                        .child("No recent threads yet…"),
                )
            })
            .children(
                self.rows
                    .iter()
                    .enumerate()
                    .map(|(ix, row)| self.render_row(ix, row, now_unix)),
            );

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
                this.child(
                    div()
                        .flex_none()
                        .px_3()
                        .py_1()
                        .text_size(px(10.5))
                        .text_color(rgb(RED))
                        .truncate()
                        .child(err),
                )
            })
            .child(self.render_footer(cx))
    }
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
    v.map_or_else(|| "–".to_owned(), |v| format!("{v:.0} ms"))
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
