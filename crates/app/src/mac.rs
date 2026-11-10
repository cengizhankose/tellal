//! Thin AppKit calls gpui 0.2.2 does not expose, via the `objc` 0.2 crate gpui already links.

// objc 0.2's `sel!` expands to `cfg(feature = "cargo-clippy")`, which rustc ≥1.80 flags.
#![allow(unexpected_cfgs)]

use gpui::Window;
use objc::runtime::{BOOL, Object, YES};
use objc::{class, msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

type Id = *mut Object;

const NS_APPLICATION_ACTIVATION_POLICY_ACCESSORY: isize = 1;
const NS_EVENT_TYPE_LEFT_MOUSE_DOWN: usize = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Space reserved by the menu bar (top) and a right-side Dock on the primary display, in points.
pub struct ScreenInsets {
    pub top: f32,
    pub right: f32,
}

/// Native window properties worth recording in `ui.window`.
pub struct NativeWindowInfo {
    pub level: isize,
    pub collection_behavior: usize,
    pub style_mask: usize,
    pub is_panel: bool,
}

fn shared_app() -> Id {
    unsafe { msg_send![class!(NSApplication), sharedApplication] }
}

/// gpui sets `Regular` in `applicationDidFinishLaunching`; switch to `Accessory` (no Dock icon,
/// allowed to float over other apps' fullscreen Spaces). Returns the policy read back from AppKit.
pub fn make_accessory_app() -> isize {
    let app = shared_app();
    unsafe {
        let _: BOOL = msg_send![app, setActivationPolicy: NS_APPLICATION_ACTIVATION_POLICY_ACCESSORY];
        msg_send![app, activationPolicy]
    }
}

pub fn activation_policy_name(policy: isize) -> &'static str {
    match policy {
        0 => "regular",
        1 => "accessory",
        2 => "prohibited",
        _ => "unknown",
    }
}

pub fn primary_screen_insets() -> Option<ScreenInsets> {
    unsafe {
        let screens: Id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return None;
        }
        let screen: Id = msg_send![screens, objectAtIndex: 0usize];
        let frame: NSRect = msg_send![screen, frame];
        let visible: NSRect = msg_send![screen, visibleFrame];
        Some(ScreenInsets {
            top: ((frame.y + frame.h) - (visible.y + visible.h)) as f32,
            right: ((frame.x + frame.w) - (visible.x + visible.w)) as f32,
        })
    }
}

fn ns_window(window: &Window) -> Option<Id> {
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw() else {
        return None;
    };
    let view = handle.ns_view.as_ptr() as Id;
    let ns_window: Id = unsafe { msg_send![view, window] };
    (!ns_window.is_null()).then_some(ns_window)
}

/// Hides the close/minimize/zoom buttons AppKit draws for gpui's titled-but-titlebarless windows.
pub fn hide_traffic_lights(window: &Window) {
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    for button_kind in 0usize..3 {
        unsafe {
            let button: Id = msg_send![ns_window, standardWindowButton: button_kind];
            if !button.is_null() {
                let _: () = msg_send![button, setHidden: YES];
            }
        }
    }
}

pub fn native_window_info(window: &Window) -> Option<NativeWindowInfo> {
    let ns_window = ns_window(window)?;
    unsafe {
        let is_panel: BOOL = msg_send![ns_window, isKindOfClass: class!(NSPanel)];
        Some(NativeWindowInfo {
            level: msg_send![ns_window, level],
            collection_behavior: msg_send![ns_window, collectionBehavior],
            style_mask: msg_send![ns_window, styleMask],
            is_panel: is_panel == YES,
        })
    }
}

/// gpui 0.2.2's `Window::start_window_move` is a no-op on macOS, so hand the current left-mouse-down
/// event to AppKit's window-server drag. Returns false if there was no such event to drag with.
pub fn drag_window(window: &Window) -> bool {
    let Some(ns_window) = ns_window(window) else {
        return false;
    };
    unsafe {
        let event: Id = msg_send![shared_app(), currentEvent];
        if event.is_null() {
            return false;
        }
        let event_type: usize = msg_send![event, type];
        if event_type != NS_EVENT_TYPE_LEFT_MOUSE_DOWN {
            return false;
        }
        let _: () = msg_send![ns_window, performWindowDragWithEvent: event];
    }
    true
}
