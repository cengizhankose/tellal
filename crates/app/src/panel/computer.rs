//! Computer use (cua-driver) and new tasks: the safety banner over the list while an agent may
//! act on the screen (with `Durdur`), the emergency stop (button, ⌘. and the global ⌃⌥⎋), the
//! permission row above the status line, the drawer's computer-use line, and the "Yeni görev"
//! flow (the reply dock in new-task mode → `Core::start_task` → select the new thread).

use gpui::{AnyElement, Context, FontWeight, SharedString, Window, div, prelude::*, px};
use serde_json::json;
use t3_panel_core::{ComputerSession, ComputerView, NEW_TASK};

use super::{COMPUTER_H, PERMISSION_H, Panel};
use crate::icons::{Icon, icon};
use crate::mac;
use crate::model::{Filter, Section};
use crate::motion;
use crate::theme::{Tokens, TypeScale, alpha};
use crate::time;
use crate::widgets::{ButtonKind, pill, tabular, text_button, tip};

/// What the computer-use parts of the UI show (`Panel::computer` plus the grant request).
#[derive(Default)]
pub(super) struct Computer {
    pub(super) view: ComputerView,
    /// Bumped each time the banner appears (its one enter transition).
    pub(super) seq: usize,
    /// `İzin ver` was pressed and the permissions are not reported granted yet.
    pub(super) grant_requested: bool,
}

impl Computer {
    pub(super) fn active(&self) -> bool {
        !self.view.sessions.is_empty()
    }

    /// Installed, but the permissions are missing or unknown (no daemon to ask).
    pub(super) fn needs_permission(&self) -> bool {
        self.view.installed && self.view.permissions_ok != Some(true)
    }

    /// `(label, value)` cells for the metrics drawer's "Bilgisayar kullanımı" section.
    pub(super) fn cells(&self) -> Vec<(&'static str, String)> {
        let v = &self.view;
        if !v.installed {
            return vec![("cua-driver", "kurulu değil".to_owned())];
        }
        let permissions = match v.permissions_ok {
            Some(true) => "var",
            Some(false) => "yok",
            None => "bilinmiyor",
        };
        vec![
            ("cua-driver", v.version.clone().unwrap_or_else(|| "kurulu".to_owned())),
            ("servis", if v.daemon_running { "çalışıyor" } else { "kapalı" }.to_owned()),
            ("mod", v.permission_mode.clone().unwrap_or_else(|| "–".to_owned())),
            ("izin", permissions.to_owned()),
            ("canlı oturum", v.sessions.len().to_string()),
        ]
    }
}

fn session_json(s: &ComputerSession) -> serde_json::Value {
    json!({"id": s.id, "client": s.client, "app": s.app, "last_action": s.last_action})
}

impl Panel {
    /// `ComputerUpdated`: re-reads `Core::computer()`; logs banner and permission transitions.
    pub(super) fn refresh_computer(&mut self) {
        let view = self.core.computer();
        if view == self.computer.view {
            return;
        }
        let before = std::mem::replace(&mut self.computer.view, view);
        let now = &self.computer.view;
        if before.sessions.len() != now.sessions.len() {
            self.core.log_ui(
                "ui.computer_banner",
                json!({
                    "visible": !now.sessions.is_empty(), "before": before.sessions.len(),
                    "sessions": now.sessions.iter().map(session_json).collect::<Vec<_>>(),
                }),
            );
            if before.sessions.is_empty() {
                self.computer.seq = self.next_seq();
            }
        }
        let now = &self.computer.view;
        if before.permissions_ok != now.permissions_ok || before.installed != now.installed {
            self.core.log_ui(
                "ui.computer_permissions",
                json!({
                    "installed": now.installed, "daemon": now.daemon_running, "before": before.permissions_ok,
                    "after": now.permissions_ok, "requested": self.computer.grant_requested,
                }),
            );
            if now.permissions_ok == Some(true) && self.computer.grant_requested {
                self.computer.grant_requested = false;
                self.show_toast("Bilgisayar kullanımı hazır");
            }
        }
    }

    /// Emergency stop (`Durdur`, ⌘., global ⌃⌥⎋): revokes every cua-driver session. Always
    /// sent, even when the panel sees no session (the stop is deny-only and harmless).
    pub(super) fn stop_computer(&mut self, source: &str, cx: &mut Context<Panel>) {
        let sessions: Vec<_> = self.computer.view.sessions.iter().map(session_json).collect();
        if source == "hotkey" {
            self.core.log_ui(
                "ui.stop_hotkey",
                json!({
                    "action": "pressed", "spec": self.hotkeys.stop.as_ref().map(|b| b.spec.clone()),
                    "sessions": sessions, "frontmost": mac::frontmost_app().map(|a| a.to_json()),
                }),
            );
        } else {
            self.log_action(source, "computer_stop", None, json!({"sessions": sessions}));
        }
        self.core.computer_stop_all();
        self.show_toast("Ekran kontrolü durduruldu");
        cx.notify();
    }

    /// `İzin ver` (I): CuaDriver's macOS permission flow; the user approves the system dialogs.
    pub(super) fn grant_computer_permissions(&mut self, source: &str, cx: &mut Context<Panel>) {
        if !self.computer.needs_permission() {
            self.log_action(source, "computer_grant", None, json!({"ignored": "not needed"}));
            return;
        }
        self.log_action(
            source,
            "computer_grant",
            None,
            json!({"before": self.computer.view.permissions_ok, "repeat": self.computer.grant_requested}),
        );
        self.computer.grant_requested = true;
        self.core.computer_grant_permissions();
        cx.notify();
    }

    /// `+` / N: the reply dock in new-task mode (dictation exactly as for a reply).
    pub(super) fn open_new_task(&mut self, source: &str, window: &mut Window, cx: &mut Context<Panel>) {
        self.log_action(source, "new_task", None, json!({"project": self.task_project_label().as_ref()}));
        let live = !matches!(self.selftest_mode, super::SelfTestMode::Shots { .. }) || self.demo;
        self.open_reply(NEW_TASK, source, live, window, cx);
    }

    /// Display name of `Config.task_project`: the listed project with that id, else the last
    /// path component; `None` = T3's "No project".
    pub(super) fn task_project_label(&self) -> SharedString {
        let Some(project) = self.task_project.as_deref() else {
            return "No project".into();
        };
        if let Some(t) = self.threads.iter().find(|t| t.project_id == project) {
            return t.project.clone().into();
        }
        let name = project.trim_end_matches('/').rsplit('/').next().unwrap_or(project);
        SharedString::from(name.to_owned())
    }

    /// `TaskStarted`: close the dock and select the new thread (once listed), or keep the
    /// text in the dock with the error (G2-5).
    pub(super) fn on_task_started(
        &mut self,
        thread_id: Option<String>,
        ok: bool,
        detail: String,
        cx: &mut Context<Panel>,
    ) {
        let open_ms = self.reply.as_ref().filter(|d| d.thread_id == NEW_TASK).map(|d| super::ms(d.opened_at.elapsed()));
        self.core.log_ui(
            "ui.task_started",
            json!({"ok": ok, "thread": thread_id, "detail": detail, "dock": open_ms.is_some(), "open_ms": open_ms}),
        );
        if ok {
            self.show_toast("Görev başladı");
            if open_ms.is_some() {
                self.close_reply(cx);
            }
            self.pending_task = thread_id;
            self.select_started_task(cx);
        } else if let Some(dock) = self.reply.as_mut().filter(|d| d.thread_id == NEW_TASK) {
            dock.task_failed(&detail);
        } else {
            self.show_error(format!("Görev başlatılamadı: {detail}").into(), None);
        }
        cx.notify();
    }

    /// Selects, reveals and pulses the thread `start_task` created as soon as it is listed.
    pub(super) fn select_started_task(&mut self, cx: &mut Context<Panel>) {
        let Some(id) = self.pending_task.clone() else {
            return;
        };
        let Some(section) = self.thread(&id).map(Section::of) else {
            return;
        };
        self.pending_task = None;
        let expand = Filter::collapsed_in(&self.toggled, section);
        if expand && !self.toggled.remove(&section) {
            self.toggled.insert(section);
        }
        self.rebuild_model();
        self.log_action("task", "task_select", Some(&id), json!({"section": section.name(), "expanded": expand}));
        self.select(Some(id.clone()), "task");
        let seq = self.next_seq();
        self.pulse = Some((id, seq));
        cx.notify();
    }

    /// `Ajan ekranını kullanıyor · App · last action · 3dk · [Durdur]` (only while ≥ 1 session).
    pub(super) fn render_computer_banner(
        &self,
        t: &Tokens,
        ty: &TypeScale,
        cx: &mut Context<Panel>,
    ) -> Option<AnyElement> {
        let sessions = &self.computer.view.sessions;
        if sessions.is_empty() {
            return None;
        }
        let action_at = |s: &ComputerSession| s.last_action_at.as_deref().and_then(time::parse_rfc3339_unix);
        let latest = sessions.iter().max_by_key(|s| action_at(s)).unwrap_or(&sessions[0]);
        let since = sessions.iter().filter_map(|s| s.started_at.as_deref().and_then(time::parse_rfc3339_unix)).min();
        let detail = match (&latest.app, &latest.last_action) {
            (Some(app), Some(action)) => format!("{app} · {action}"),
            (Some(app), None) => app.clone(),
            (None, Some(action)) => action.clone(),
            (None, None) => {
                let client = latest.client.as_deref().unwrap_or("ajan");
                match action_at(latest) {
                    Some(at) => format!("{client} · son işlem {}", time::relative(Some(at), self.now_unix)),
                    None => format!("{client} · bağlı"),
                }
            }
        };
        let stop_key = self.hotkeys.stop.as_ref().map_or_else(String::new, |b| format!(" · her yerden {}", b.spec));
        let stop = text_button("computer_stop", Some(Icon::OctagonX), "Durdur", ButtonKind::Danger, t, ty.secondary)
            .tooltip(tip(format!("ajanların ekran kontrolünü hemen kes (⌘.{stop_key})")))
            .on_click(cx.listener(|panel, _, _, cx| panel.stop_computer("click", cx)));
        let title = div()
            .flex()
            .items_center()
            .gap_1p5()
            .h(px(18.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_size(px(ty.secondary))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Ajan ekranını kullanıyor"),
            )
            .children((sessions.len() > 1).then(|| pill(format!("{} oturum", sessions.len()), t.danger, 0.16)))
            .children(since.map(|since| {
                tabular(div().flex_none().text_size(px(ty.secondary)).text_color(t.text_muted))
                    .child(time::working(since, self.now_unix))
            }));
        let card = div()
            .id("computer")
            .flex()
            .items_center()
            .gap_2()
            .h(px(COMPUTER_H - 8.))
            .pl_2p5()
            .pr_2()
            .rounded(px(10.))
            .border_1()
            .border_color(alpha(t.danger, 0.5))
            .bg(alpha(t.danger, 0.10))
            .tooltip(tip(format!(
                "{} canlı oturum · ajanlar cua-driver ile ekranı kullanabilir",
                sessions.len()
            )))
            .child(icon(Icon::MousePointer2, 16., t.danger))
            .child(
                div().flex_1().min_w(px(0.)).flex().flex_col().gap_0p5().child(title).child(
                    div()
                        .truncate()
                        .text_size(px(ty.secondary))
                        .text_color(t.text_muted)
                        .child(SharedString::from(detail)),
                ),
            )
            .child(stop);
        // One short enter transition, then static (no pulsing).
        Some(motion::slide_up(
            div().flex_none().h(px(COMPUTER_H)).px_2().pt_2().child(card),
            ("computer_in", self.computer.seq),
            6.,
            cx,
        ))
    }

    /// `Bilgisayar kullanımı: izin gerekli · İzin ver` above the status line (installed only).
    pub(super) fn render_permission_row(
        &self,
        t: &Tokens,
        ty: &TypeScale,
        cx: &mut Context<Panel>,
    ) -> Option<AnyElement> {
        if !self.computer.needs_permission() {
            return None;
        }
        let text = if self.computer.grant_requested {
            "macOS pencerelerini onayla: Erişilebilirlik ve Ekran Kaydı"
        } else if self.computer.view.permissions_ok.is_none() {
            "Bilgisayar kullanımı: izin durumu bilinmiyor"
        } else {
            "Bilgisayar kullanımı: izin gerekli"
        };
        let grant = text_button(
            "computer_grant",
            None,
            if self.computer.grant_requested { "Yeniden aç" } else { "İzin ver" },
            ButtonKind::Ghost,
            t,
            ty.badge + 0.5,
        )
        .h(px(24.))
        .tooltip(tip("CuaDriver'ın macOS izin pencerelerini aç (I)"))
        .on_click(cx.listener(|panel, _, _, cx| panel.grant_computer_permissions("click", cx)));
        let row = div()
            .id("computer_permissions")
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(px(PERMISSION_H))
            .px_3()
            .border_t_1()
            .border_color(t.border)
            .text_size(px(ty.secondary))
            .child(icon(Icon::ShieldAlert, 14., t.warning))
            .child(div().flex_1().min_w(px(0.)).truncate().child(text))
            .child(grant);
        Some(motion::fade_in(row, "computer_permissions_in", motion::NORMAL, cx))
    }
}
