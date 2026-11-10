#[cfg(not(target_os = "macos"))]
compile_error!("t3-panel is a macOS-only floating panel");

mod mac;
mod panel;

use std::time::Instant;

use gpui::{
    App, AppContext, Application, Bounds, Pixels, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
};
use serde_json::json;
use t3_panel_core::{Config, Core};

use crate::panel::Panel;

const PANEL_WIDTH: f32 = 360.;
const PANEL_HEIGHT: f32 = 520.;
const SCREEN_MARGIN: f32 = 16.;

fn main() -> anyhow::Result<()> {
    let process_start = Instant::now();
    let selftest = std::env::var_os("T3P_SELFTEST").is_some_and(|v| v == "1");
    let (core, events) = Core::start(Config::from_env())?;

    Application::new().run(move |cx: &mut App| {
        // Runs after gpui's applicationDidFinishLaunching set the policy to Regular.
        let activation_policy = mac::make_accessory_app();
        let insets = mac::primary_screen_insets();
        let (inset_top, inset_right) = insets.as_ref().map_or((0., 0.), |i| (i.top, i.right));

        let display = cx.primary_display();
        let display_bounds = display.as_ref().map(|d| d.bounds());
        let panel_size = size(px(PANEL_WIDTH), px(PANEL_HEIGHT));
        let origin = display_bounds.map_or(point(px(SCREEN_MARGIN), px(SCREEN_MARGIN)), |b| {
            point(
                b.origin.x + b.size.width - px(inset_right + SCREEN_MARGIN + PANEL_WIDTH),
                b.origin.y + px(inset_top + SCREEN_MARGIN),
            )
        });

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: panel_size })),
            titlebar: None,
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: true,
            is_resizable: false,
            is_minimizable: false,
            display_id: display.as_ref().map(|d| d.id()),
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };

        let ui_core = core.clone();
        let opened = cx.open_window(options, |window, cx| {
            mac::hide_traffic_lights(window);
            let native = mac::native_window_info(window);
            ui_core.log_ui(
                "ui.window",
                json!({
                    "bounds": bounds_json(window.bounds()),
                    "display_bounds": display_bounds.map(bounds_json),
                    "screen_insets": {"top": inset_top, "right": inset_right},
                    "kind": "PopUp",
                    "activation_policy": mac::activation_policy_name(activation_policy),
                    "ns_panel": native.as_ref().map(|n| n.is_panel),
                    "ns_level": native.as_ref().map(|n| n.level),
                    "ns_collection_behavior": native.as_ref().map(|n| n.collection_behavior),
                    "ns_style_mask": native.as_ref().map(|n| n.style_mask),
                    "focus": false,
                    "selftest": selftest,
                }),
            );
            cx.new(|cx| Panel::new(ui_core, events, process_start, selftest, cx))
        });
        if let Err(err) = opened {
            core.log_ui("ui.window_error", json!({"error": format!("{err:#}")}));
            cx.quit();
        }
    });
    Ok(())
}

fn bounds_json(b: Bounds<Pixels>) -> serde_json::Value {
    json!({
        "x": f32::from(b.origin.x),
        "y": f32::from(b.origin.y),
        "w": f32::from(b.size.width),
        "h": f32::from(b.size.height),
    })
}
