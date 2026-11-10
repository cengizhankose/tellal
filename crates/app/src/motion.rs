//! Motion tokens (DESIGN §4) and the few animation wrappers the panel uses. With macOS Reduce
//! Motion every movement is dropped and fades are capped at `FAST`.

use std::time::Duration;

use gpui::{Animation, AnimationExt, AnyElement, App, ElementId, Global, IntoElement, ParentElement, Styled, div, px};

pub const FAST: Duration = Duration::from_millis(120);
pub const NORMAL: Duration = Duration::from_millis(200);
pub const SLOW: Duration = Duration::from_millis(320);
pub const SPRING: Duration = Duration::from_millis(240);

/// Whether macOS "Reduce motion" is on (refreshed by the panel once a second).
pub struct Motion {
    pub reduce: bool,
}

impl Global for Motion {}

pub fn reduced(cx: &App) -> bool {
    cx.try_global::<Motion>().is_some_and(|m| m.reduce)
}

/// Duration of a movement (zero under Reduce Motion).
pub fn movement(cx: &App, d: Duration) -> Duration {
    if reduced(cx) { Duration::ZERO } else { d }
}

/// Duration of an opacity fade (capped at `FAST` under Reduce Motion).
pub fn fade(cx: &App, d: Duration) -> Duration {
    if reduced(cx) { d.min(FAST) } else { d }
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

pub fn ease_in_out(t: f32) -> f32 {
    gpui::ease_in_out(t.clamp(0., 1.))
}

/// 0 → 1 with a single ≤ 4 % overshoot (the result may exceed 1; use it for offsets, not deltas).
pub fn spring(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    ease_out(t) + 0.04 * (std::f32::consts::PI * t).sin() * t
}

/// Fades `el` in over `d` (Reduce Motion: capped at `FAST`).
pub fn fade_in<E>(el: E, id: impl Into<ElementId>, d: Duration, cx: &App) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    let d = fade(cx, d);
    if d.is_zero() {
        return el.into_any_element();
    }
    el.with_animation(id, Animation::new(d).with_easing(ease_out), |el, t| el.opacity(t)).into_any_element()
}

/// Slides `el` up by `dist` px while fading it in (`normal`; Reduce Motion: fade only).
pub fn slide_up<E>(el: E, id: impl Into<ElementId>, dist: f32, cx: &App) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    let id = id.into();
    if reduced(cx) {
        return fade_in(el, id, FAST, cx);
    }
    // Offset a wrapper so the element's own positioning stays untouched.
    div()
        .flex_none()
        .relative()
        .child(el)
        .with_animation(id, Animation::new(NORMAL).with_easing(ease_in_out), move |w, t| {
            w.top(px(dist * (1. - t))).opacity(t)
        })
        .into_any_element()
}

/// Springs `el` in (`spring`: a 4 px rise with ≤ 4 % overshoot; Reduce Motion: plain fade).
pub fn spring_in<E>(el: E, id: impl Into<ElementId>, cx: &App) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    let id = id.into();
    if reduced(cx) {
        return fade_in(el, id, FAST, cx);
    }
    div()
        .flex_none()
        .relative()
        .child(el)
        .with_animation(id, Animation::new(SPRING), |w, t| w.top(px(4. * (1. - spring(t)))).opacity((t * 3.).min(1.)))
        .into_any_element()
}

/// One highlight pulse (attention card → row): `alpha` peaks then decays over `slow`.
pub fn pulse_alpha(t: f32) -> f32 {
    (std::f32::consts::PI * t.clamp(0., 1.)).sin()
}
