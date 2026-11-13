//! Global shortcuts (Carbon `RegisterEventHotKey`, no Accessibility permission needed): show/hide
//! (default ⌃⌥Space, `TELLAL_HOTKEY`) and the computer-use emergency stop (default ⌃⌥⎋,
//! `TELLAL_STOP_HOTKEY`). `<mods+key>` in `global-hotkey` syntax or `off`. One manager owns both;
//! presses arrive on its single global channel, which the panel's pump drains once per tick.

use std::str::FromStr;

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use serde_json::json;
use tellal_core::Core;

const DEFAULT_TOGGLE: &str = "ctrl+alt+space";
/// ⌃⌥⎋: next to macOS's Force Quit (⌘⌥⎋) in meaning, but not taken by the system.
const DEFAULT_STOP: &str = "ctrl+alt+escape";

pub struct Binding {
    id: u32,
    pub spec: String,
}

pub struct Hotkeys {
    /// Must stay alive (and on the main thread) for the registrations to hold.
    _manager: Option<GlobalHotKeyManager>,
    pub toggle: Option<Binding>,
    pub stop: Option<Binding>,
}

/// Which shortcuts were pressed since the last [`Hotkeys::drain`].
#[derive(Default)]
pub struct Pressed {
    pub toggle: bool,
    pub stop: bool,
}

impl Hotkeys {
    /// Registers both shortcuts; logs `ui.hotkey` / `ui.stop_hotkey` `{spec, ok, error}`.
    pub fn register(core: &Core) -> Hotkeys {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string());
        let toggle = bind(core, manager.as_ref(), "TELLAL_HOTKEY", DEFAULT_TOGGLE, "ui.hotkey");
        let stop = bind(core, manager.as_ref(), "TELLAL_STOP_HOTKEY", DEFAULT_STOP, "ui.stop_hotkey");
        Hotkeys { _manager: manager.ok(), toggle, stop }
    }

    pub fn drain(&self) -> Pressed {
        let id = |b: &Option<Binding>| b.as_ref().map(|b| b.id);
        let (toggle, stop) = (id(&self.toggle), id(&self.stop));
        GlobalHotKeyEvent::receiver().try_iter().filter(|e| e.state() == HotKeyState::Pressed).fold(
            Pressed::default(),
            |mut p, e| {
                p.toggle |= Some(e.id()) == toggle;
                p.stop |= Some(e.id()) == stop;
                p
            },
        )
    }
}

/// `None` when disabled (`off`), unparsable or taken by another app.
fn bind(
    core: &Core,
    manager: Result<&GlobalHotKeyManager, &String>,
    env: &str,
    default: &str,
    kind: &str,
) -> Option<Binding> {
    let spec = std::env::var(env).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    let spec = spec.unwrap_or_else(|| default.to_owned());
    if spec.eq_ignore_ascii_case("off") {
        core.log_ui(kind, json!({"spec": spec, "ok": false, "disabled": true}));
        return None;
    }
    let registered = HotKey::from_str(&spec).map_err(|e| e.to_string()).and_then(|hotkey| {
        manager.map_err(String::clone)?.register(hotkey).map_err(|e| e.to_string())?;
        Ok(hotkey.id())
    });
    match registered {
        Ok(id) => {
            core.log_ui(kind, json!({"spec": spec, "ok": true}));
            Some(Binding { id, spec })
        }
        Err(error) => {
            core.log_ui(kind, json!({"spec": spec, "ok": false, "error": error}));
            None
        }
    }
}
