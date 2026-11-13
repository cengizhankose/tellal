//! Colour and type tokens (DESIGN §3). Every colour the app paints comes from here, derived from
//! `Core::theme()`; a theme change cross-fades all tokens over `motion::NORMAL`.

use std::time::{Duration, Instant};

use gpui::{Global, Hsla, Rgba, SharedString, rgb};
use serde_json::{Value, json};
use tellal_core::ThemeView;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub canvas: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,
    /// `accent` at 14 % over `canvas`.
    pub selected: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub accent_text: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub info: Rgba,
}

impl Tokens {
    fn from_view(v: &ThemeView) -> Tokens {
        let canvas = rgb(v.canvas);
        let accent = rgb(v.accent);
        Tokens {
            canvas,
            surface: rgb(v.surface),
            surface_hover: rgb(v.surface_hover),
            selected: mix(canvas, accent, 0.14),
            border: rgb(v.border),
            text: rgb(v.text),
            text_muted: rgb(v.text_muted),
            accent,
            accent_text: rgb(v.accent_text),
            success: rgb(v.success),
            warning: rgb(v.warning),
            danger: rgb(v.danger),
            info: rgb(v.info),
        }
    }

    fn lerp(&self, to: &Tokens, t: f32) -> Tokens {
        Tokens {
            canvas: mix(self.canvas, to.canvas, t),
            surface: mix(self.surface, to.surface, t),
            surface_hover: mix(self.surface_hover, to.surface_hover, t),
            selected: mix(self.selected, to.selected, t),
            border: mix(self.border, to.border, t),
            text: mix(self.text, to.text, t),
            text_muted: mix(self.text_muted, to.text_muted, t),
            accent: mix(self.accent, to.accent, t),
            accent_text: mix(self.accent_text, to.accent_text, t),
            success: mix(self.success, to.success, t),
            warning: mix(self.warning, to.warning, t),
            danger: mix(self.danger, to.danger, t),
            info: mix(self.info, to.info, t),
        }
    }

    /// Window background: canvas at 92 % over the blurred desktop.
    pub fn window_bg(&self) -> Hsla {
        alpha(self.canvas, 0.92)
    }
}

/// `color` with opacity `a` (tints such as a status colour behind a pill).
pub fn alpha(color: Rgba, a: f32) -> Hsla {
    Rgba { a, ..color }.into()
}

/// `base` blended toward `over` by `t` (opaque result).
pub fn mix(base: Rgba, over: Rgba, t: f32) -> Rgba {
    let l = |a: f32, b: f32| a + (b - a) * t;
    Rgba { r: l(base.r, over.r), g: l(base.g, over.g), b: l(base.b, over.b), a: l(base.a, over.a) }
}

/// Type scale (DESIGN §3): T3's interface font at `font_size - 1` for body.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeScale {
    /// `None` = the system font.
    pub family: Option<SharedString>,
    pub body: f32,
    pub secondary: f32,
    /// Badges and section labels; never below 11.
    pub badge: f32,
    pub title: f32,
}

impl TypeScale {
    fn from_view(v: &ThemeView) -> TypeScale {
        let base = if v.font_size.is_finite() && v.font_size >= 12. { v.font_size } else { 14. };
        let family = v.font_family.trim();
        TypeScale {
            family: (!family.is_empty()).then(|| SharedString::from(family.to_owned())),
            body: (base - 1.).max(12.),
            secondary: (base - 2.).max(11.5),
            badge: 11.,
            title: (base - 1.).max(12.),
        }
    }
}

pub struct Theme {
    view: ThemeView,
    from: Tokens,
    to: Tokens,
    started: Instant,
    fade: Duration,
    /// Tokens for the frame being rendered (refreshed by `Panel::render`, read by every widget).
    pub current: Tokens,
    pub ty: TypeScale,
}

impl Global for Theme {}

impl Theme {
    pub fn new(view: ThemeView) -> Theme {
        let tokens = Tokens::from_view(&view);
        Theme {
            ty: TypeScale::from_view(&view),
            view,
            from: tokens,
            to: tokens,
            started: Instant::now(),
            fade: Duration::ZERO,
            current: tokens,
        }
    }

    pub fn view(&self) -> &ThemeView {
        &self.view
    }

    /// Starts a cross-fade from whatever is on screen now to `view`.
    pub fn apply(&mut self, view: ThemeView, fade: Duration) {
        let now = Instant::now();
        self.from = self.tokens_at(now);
        self.to = Tokens::from_view(&view);
        self.ty = TypeScale::from_view(&view);
        self.view = view;
        self.started = now;
        self.fade = fade;
    }

    fn tokens_at(&self, now: Instant) -> Tokens {
        let elapsed = now.duration_since(self.started);
        if self.fade.is_zero() || elapsed >= self.fade {
            return self.to;
        }
        let t = elapsed.as_secs_f32() / self.fade.as_secs_f32();
        self.from.lerp(&self.to, crate::motion::ease_in_out(t))
    }

    /// Updates `current` for this frame; returns true while the cross-fade still runs.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        self.current = self.tokens_at(now);
        now.duration_since(self.started) < self.fade
    }

    /// `ui.theme` payload: source, name, colours and the WCAG contrast of every text pairing.
    pub fn to_json(&self) -> Value {
        let v = &self.view;
        let t = &self.to;
        let hex = |c: u32| format!("#{c:06x}");
        json!({
            "source": v.source, "name": v.name, "dark": v.dark,
            "font_family": v.font_family, "font_size": v.font_size,
            "colors": {
                "canvas": hex(v.canvas), "surface": hex(v.surface), "surface_hover": hex(v.surface_hover),
                "border": hex(v.border), "text": hex(v.text), "text_muted": hex(v.text_muted),
                "accent": hex(v.accent), "accent_text": hex(v.accent_text), "success": hex(v.success),
                "warning": hex(v.warning), "danger": hex(v.danger), "info": hex(v.info),
            },
            "contrast": {
                "text/canvas": round2(contrast(t.text, t.canvas)),
                "text/surface": round2(contrast(t.text, t.surface)),
                "text/selected": round2(contrast(t.text, t.selected)),
                "text_muted/canvas": round2(contrast(t.text_muted, t.canvas)),
                "text_muted/surface": round2(contrast(t.text_muted, t.surface)),
                "text_muted/selected": round2(contrast(t.text_muted, t.selected)),
                "accent_text/accent": round2(contrast(t.accent_text, t.accent)),
                "accent/canvas": round2(contrast(t.accent, t.canvas)),
                "danger/surface": round2(contrast(t.danger, t.surface)),
                // Durdur button label, and the computer-use banner's text on its 10 % danger tint.
                "canvas/danger": round2(contrast(t.canvas, t.danger)),
                "text/danger_tint": round2(contrast(t.text, mix(t.canvas, t.danger, 0.10))),
                "text_muted/danger_tint": round2(contrast(t.text_muted, mix(t.canvas, t.danger, 0.10))),
            },
            "min_text_contrast": round2(
                [contrast(t.text, t.canvas), contrast(t.text_muted, t.canvas), contrast(t.text_muted, t.surface),
                 contrast(t.accent_text, t.accent)].into_iter().fold(f32::MAX, f32::min)
            ),
        })
    }
}

fn round2(v: f32) -> f64 {
    (f64::from(v) * 100.).round() / 100.
}

/// WCAG 2 contrast ratio of two opaque colours.
pub fn contrast(a: Rgba, b: Rgba) -> f32 {
    fn channel(c: f32) -> f32 {
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }
    let lum = |c: Rgba| 0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b);
    let (la, lb) = (lum(a), lum(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}
