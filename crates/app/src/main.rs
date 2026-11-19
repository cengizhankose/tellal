#[cfg(not(target_os = "macos"))]
compile_error!("tellal is a macOS-only floating panel");

mod assets;
mod hotkey;
mod icons;
mod input;
mod mac;
mod model;
mod motion;
mod panel;
mod strings;
mod theme;
mod time;
mod widgets;

use std::time::Instant;

use gpui::{
    App, AppContext, Application, Bounds, Pixels, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
};
use serde_json::json;
use tellal_core::{Config, Core};

use crate::motion::Motion;
use crate::panel::{Panel, SelfTestMode};
use crate::theme::Theme;

const PANEL_WIDTH: f32 = panel::WIDTH;
/// First frame only; the panel then sizes itself to its content.
const PANEL_HEIGHT: f32 = 480.;
const SCREEN_MARGIN: f32 = 16.;

fn main() -> anyhow::Result<()> {
    let process_start = Instant::now();
    let selftest = SelfTestMode::from_env();
    let config = Config::from_env();
    let (core, events) = Core::start(config.clone())?;

    Application::new().with_assets(assets::Assets).run(move |cx: &mut App| {
        // Runs after gpui's applicationDidFinishLaunching set the policy to Regular.
        let activation_policy = mac::make_accessory_app();
        input::bind_keys(cx);
        panel::bind_keys(cx);
        cx.set_global(Theme::new(core.theme()));
        cx.set_global(Motion { reduce: mac::reduce_motion() });
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
            window_background: WindowBackgroundAppearance::Blurred,
            ..Default::default()
        };

        let ui_core = core.clone();
        let opened = cx.open_window(options, |window, cx| {
            mac::hide_traffic_lights(window);
            mac::keep_visible_when_inactive(window);
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
                    "selftest": selftest.to_json(),
                }),
            );
            cx.new(|cx| Panel::new(ui_core, events, process_start, selftest, &config, window, cx))
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
