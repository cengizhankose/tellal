//! Global show/hide shortcut (default ⌃⌥Space, `T3P_HOTKEY=<mods+key>` or `off`). Carbon's
//! `RegisterEventHotKey` needs no Accessibility permission; presses arrive on a channel the
//! panel's pump drains.

use std::str::FromStr;

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde_json::json;
use t3_panel_core::Core;

const DEFAULT: &str = "ctrl+alt+space";

pub struct Hotkey {
    /// Must stay alive (and on the main thread) for the registration to hold.
    _manager: GlobalHotKeyManager,
    id: u32,
    pub spec: String,
}

impl Hotkey {
    /// Registers the shortcut; logs `ui.hotkey {spec, ok, error}`. `None` when disabled or taken.
    pub fn register(core: &Core) -> Option<Hotkey> {
        let spec = std::env::var("T3P_HOTKEY").ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        let spec = spec.unwrap_or_else(|| DEFAULT.to_owned());
        if spec.eq_ignore_ascii_case("off") {
            core.log_ui("ui.hotkey", json!({"spec": spec, "ok": false, "disabled": true}));
            return None;
        }
        let registered = HotKey::from_str(&spec).map_err(|e| e.to_string()).and_then(|hotkey| {
            let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
            manager.register(hotkey).map_err(|e| e.to_string())?;
            Ok(Hotkey { _manager: manager, id: hotkey.id(), spec: spec.clone() })
        });
        match registered {
            Ok(hotkey) => {
                core.log_ui("ui.hotkey", json!({"spec": spec, "ok": true}));
                Some(hotkey)
            }
            Err(error) => {
                core.log_ui("ui.hotkey", json!({"spec": spec, "ok": false, "error": error}));
                None
            }
        }
    }

    /// Drains pending events; true if the shortcut was pressed since the last call.
    pub fn pressed(&self) -> bool {
        GlobalHotKeyEvent::receiver()
            .try_iter()
            .fold(false, |hit, e| hit | (e.id() == self.id && e.state() == HotKeyState::Pressed))
    }
}
