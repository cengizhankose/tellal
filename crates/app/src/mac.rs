//! Thin AppKit calls gpui 0.2.2 does not expose, via the `objc` 0.2 crate gpui already links.

// objc 0.2's `sel!` expands to `cfg(feature = "cargo-clippy")`, which rustc ≥1.80 flags.
#![allow(unexpected_cfgs)]

use gpui::Window;
use objc::runtime::{BOOL, NO, Object, YES};
use objc::{class, msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

type Id = *mut Object;

const NS_APPLICATION_ACTIVATION_POLICY_ACCESSORY: isize = 1;
const NS_EVENT_TYPE_LEFT_MOUSE_DOWN: usize = 1;
const NS_EVENT_TYPE_MOUSE_MOVED: usize = 5;

#[repr(C)]
#[derive(Clone, Copy)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NSPoint {
    x: f64,
    y: f64,
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

/// The panel's `NSWindow`, for AppKit calls made outside gpui's window borrow (resize and
/// show/hide run from a foreground task so AppKit's synchronous resize callback can re-enter gpui).
#[derive(Clone, Copy)]
pub struct NativeWindow(Id);

impl NativeWindow {
    pub fn of(window: &Window) -> Option<NativeWindow> {
        ns_window(window).map(NativeWindow)
    }

    /// Sets the content height, keeping the window's top edge where it is (AppKit frames grow
    /// from the bottom-left corner).
    pub fn set_height_keep_top(self, height: f32) {
        unsafe {
            let frame: NSRect = msg_send![self.0, frame];
            let content: NSRect = msg_send![self.0, contentRectForFrameRect: frame];
            let chrome = frame.h - content.h;
            let h = f64::from(height) + chrome;
            if (h - frame.h).abs() < 0.5 {
                return;
            }
            let next = NSRect { x: frame.x, y: frame.y + frame.h - h, w: frame.w, h };
            let _: () = msg_send![self.0, setFrame: next display: YES];
        }
    }

    pub fn is_visible(self) -> bool {
        let visible: BOOL = unsafe { msg_send![self.0, isVisible] };
        visible == YES
    }

    /// Shows the panel and makes it key; a non-activating panel takes the keyboard without
    /// activating the app, so the frontmost app stays frontmost.
    pub fn show_key(self) {
        unsafe {
            let _: () = msg_send![self.0, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
        }
    }

    pub fn hide(self) {
        unsafe {
            let _: () = msg_send![self.0, orderOut: std::ptr::null_mut::<Object>()];
        }
    }

    /// `CGWindowID` (what `screencapture -l` takes).
    pub fn number(self) -> isize {
        unsafe { msg_send![self.0, windowNumber] }
    }

    /// Clicks and hover go to whatever lies below: a scripted recording cannot be changed by
    /// the user's pointer.
    pub fn ignore_mouse(self) {
        unsafe {
            let _: () = msg_send![self.0, setIgnoresMouseEvents: YES];
        }
    }

    /// Hands the content view a pointer move far outside the window, so nothing stays hovered
    /// (gpui keeps the last position it saw, or the one it read at launch). Delivered on the next
    /// run-loop turn: inside a gpui update of this window gpui would drop it.
    pub fn forget_pointer(self) {
        unsafe {
            let view: Id = msg_send![self.0, contentView];
            if view.is_null() {
                return;
            }
            let away = NSPoint { x: -10_000., y: -10_000. };
            let event: Id = msg_send![class!(NSEvent), mouseEventWithType: NS_EVENT_TYPE_MOUSE_MOVED
                location: away
                modifierFlags: 0usize
                timestamp: 0f64
                windowNumber: self.number()
                context: std::ptr::null_mut::<Object>()
                eventNumber: 0isize
                clickCount: 0isize
                pressure: 0f32];
            if !event.is_null() {
                let _: () = msg_send![view, performSelector: sel!(mouseMoved:) withObject: event afterDelay: 0f64];
            }
        }
    }

    /// Clips the content view (gpui's Metal layer and its blur view) to `radius`, so the
    /// `Blurred` backdrop follows the panel's rounded corners.
    pub fn round_corners(self, radius: f64) {
        unsafe {
            let view: Id = msg_send![self.0, contentView];
            if view.is_null() {
                return;
            }
            let _: () = msg_send![view, setWantsLayer: YES];
            let layer: Id = msg_send![view, layer];
            if !layer.is_null() {
                let _: () = msg_send![layer, setCornerRadius: radius];
                let _: () = msg_send![layer, setMasksToBounds: YES];
            }
            let _: () = msg_send![self.0, invalidateShadow];
        }
    }
}

/// macOS Accessibility › Display › Reduce motion.
pub fn reduce_motion() -> bool {
    unsafe {
        let workspace: Id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduce: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduce == YES
    }
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

/// gpui's PopUp `NSPanel` keeps AppKit's panel default `hidesOnDeactivate = YES`. The panel now
/// becomes key while replying (which activates the app), so it must survive deactivation.
pub fn keep_visible_when_inactive(window: &Window) {
    if let Some(ns_window) = ns_window(window) {
        unsafe {
            let _: () = msg_send![ns_window, setHidesOnDeactivate: NO];
        }
    }
}

/// The app that owns the menu bar / keyboard focus (`NSWorkspace.frontmostApplication`).
#[derive(Clone, Debug, PartialEq)]
pub struct FrontApp {
    pub pid: i32,
    pub bundle_id: String,
    pub name: String,
}

impl FrontApp {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"pid": self.pid, "bundle_id": self.bundle_id, "name": self.name})
    }

    pub fn is_self(&self) -> bool {
        u32::try_from(self.pid).is_ok_and(|pid| pid == std::process::id())
    }
}

pub fn frontmost_app() -> Option<FrontApp> {
    unsafe {
        let workspace: Id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: Id = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        Some(FrontApp {
            pid: msg_send![app, processIdentifier],
            bundle_id: ns_string(msg_send![app, bundleIdentifier]),
            name: ns_string(msg_send![app, localizedName]),
        })
    }
}

/// Re-activates a previously frontmost app. Returns AppKit's answer (false if it is gone).
pub fn activate_app(pid: i32) -> bool {
    const NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;
    unsafe {
        let app: Id = msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return false;
        }
        let ok: BOOL = msg_send![app, activateWithOptions: NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS];
        ok == YES
    }
}

/// Whether the panel is AppKit's key window (receives keystrokes and Wispr's paste).
pub fn is_key_window(window: &Window) -> bool {
    ns_window(window).is_some_and(|w| unsafe {
        let key: BOOL = msg_send![w, isKeyWindow];
        key == YES
    })
}

/// The user's double-click interval (System Settings), used for the row double-click.
pub fn double_click_interval() -> std::time::Duration {
    let secs: f64 = unsafe { msg_send![class!(NSEvent), doubleClickInterval] };
    std::time::Duration::from_secs_f64(if secs.is_finite() && secs > 0. { secs } else { 0.5 })
}

fn ns_string(s: Id) -> String {
    if s.is_null() {
        return String::new();
    }
    unsafe {
        let utf8: *const std::os::raw::c_char = msg_send![s, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}
