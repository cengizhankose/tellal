//! Keyboard map (DESIGN §6): every pointer action has a key. Single-letter keys are bound only
//! while no text field has focus (`Panel && !ReplyInput`).

use gpui::{App, Context, Div, Focusable, KeyBinding, Stateful, Window, actions, prelude::*};
use serde_json::json;

use super::Panel;

pub const KEY_CONTEXT: &str = "Panel";
const BARE: &str = "Panel && !ReplyInput";

actions!(
    panel,
    [
        SelectPrev,
        SelectNext,
        ToggleListen,
        Reply,
        OpenSelected,
        PauseResume,
        Skip,
        JumpLive,
        MuteAll,
        Search,
        Compact,
        Dismiss
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, Some(KEY_CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-o", OpenSelected, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-k", Search, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-shift-c", Compact, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(KEY_CONTEXT)),
        KeyBinding::new("space", ToggleListen, Some(BARE)),
        KeyBinding::new("enter", Reply, Some(BARE)),
        KeyBinding::new("r", Reply, Some(BARE)),
        KeyBinding::new("p", PauseResume, Some(BARE)),
        KeyBinding::new("right", Skip, Some(BARE)),
        KeyBinding::new("l", JumpLive, Some(BARE)),
        KeyBinding::new("m", MuteAll, Some(BARE)),
    ]);
}

pub(super) fn on_actions(root: Stateful<Div>, cx: &mut Context<Panel>) -> Stateful<Div> {
    root.on_action(cx.listener(|p, _: &SelectPrev, _, cx| p.move_selection(-1, cx)))
        .on_action(cx.listener(|p, _: &SelectNext, _, cx| p.move_selection(1, cx)))
        .on_action(cx.listener(|p, _: &ToggleListen, _, cx| {
            match p.selected.clone() {
                Some(id) => p.toggle_listen(&id, "key"),
                // Nothing selected: Space drives the player.
                None => {
                    if let Some(id) = p.player_channel().map(|(id, _)| id.to_owned()) {
                        p.player_action("key", "toggle_pause", &id);
                    }
                }
            }
            cx.notify();
        }))
        .on_action(cx.listener(|p, _: &Reply, window, cx| {
            if let Some(id) = p.selected.clone() {
                p.log_action("key", "reply", Some(&id), json!({}));
                p.open_reply(&id, "key", true, window, cx);
            }
        }))
        .on_action(cx.listener(|p, _: &OpenSelected, _, cx| {
            if let Some(id) = p.selected.clone() {
                p.log_action("key", "open", Some(&id), json!({}));
                p.open_thread(&id, "key");
                cx.notify();
            }
        }))
        .on_action(cx.listener(|p, _: &PauseResume, _, cx| p.player_key("toggle_pause", cx)))
        .on_action(cx.listener(|p, _: &Skip, _, cx| p.player_key("skip", cx)))
        .on_action(cx.listener(|p, _: &JumpLive, _, cx| p.player_key("jump_to_live", cx)))
        .on_action(cx.listener(|p, _: &MuteAll, _, cx| {
            p.mute_all("key");
            cx.notify();
        }))
        .on_action(cx.listener(|p, _: &Search, window, cx| {
            if p.search.open {
                let handle = p.search.input.read(cx).focus_handle(cx);
                window.focus(&handle);
            } else {
                p.open_search("key", window, cx);
            }
        }))
        .on_action(cx.listener(|p, _: &Compact, _, cx| p.set_compact(!p.compact, "key", cx)))
        .on_action(cx.listener(|p, _: &Dismiss, window, cx| p.dismiss(window, cx)))
}

impl Panel {
    /// ↑/↓ wrap inside the visible rows.
    fn move_selection(&mut self, delta: isize, cx: &mut Context<Panel>) {
        let rows = &self.model.row_ids;
        if rows.is_empty() {
            return;
        }
        let len = rows.len() as isize;
        let current = self.selected.as_ref().and_then(|s| rows.iter().position(|r| r == s));
        let next = match current {
            Some(ix) => (ix as isize + delta).rem_euclid(len),
            None if delta > 0 => 0,
            None => len - 1,
        };
        let id = rows[next as usize].clone();
        self.select(Some(id), "key");
        cx.notify();
    }

    /// P / → / L act on the channel in the Now playing bar.
    fn player_key(&mut self, action: &str, cx: &mut Context<Panel>) {
        let Some(id) = self.player_channel().map(|(id, _)| id.to_owned()) else {
            self.log_action("key", action, None, json!({"ignored": "nothing playing"}));
            return;
        };
        if action == "jump_to_live" && self.channel(&id).is_none_or(|c| c.queued == 0) {
            return;
        }
        self.player_action("key", action, &id);
        cx.notify();
    }

    /// Esc closes the dock, then search, then the drawer, then clears the selection.
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Panel>) {
        if self.reply.is_some() {
            self.cancel_reply("escape", cx);
        } else if self.search.open {
            self.close_search("key", window, cx);
        } else if self.drawer_open {
            self.set_drawer(false, "key", cx);
        } else if self.selected.is_some() {
            self.select(None, "key");
            cx.notify();
        }
    }
}
