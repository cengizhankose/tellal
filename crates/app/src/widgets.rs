//! Small shared pieces: tooltip bubble, icon/text buttons, pills. Colours from `theme::Tokens`.

use std::sync::{Arc, LazyLock};

use gpui::{
    AnyView, App, Context, Div, ElementId, FontFeatures, FontWeight, Hsla, Rgba, SharedString, Stateful, Window, div,
    prelude::*, px,
};

use crate::icons::{Icon, icon};
use crate::theme::{Theme, Tokens, alpha};

/// Control height; every hit area is at least this tall (DESIGN §5 Rows: ≥ 28 px).
pub const HIT: f32 = 28.;

/// Tooltip bubble.
pub struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let t = theme.current;
        div()
            .px_2()
            .py_1()
            .rounded(px(6.))
            .border_1()
            .border_color(t.border)
            .bg(t.surface)
            .text_size(px(theme.ty.badge + 0.5))
            .text_color(t.text)
            .when_some(theme.ty.family.clone(), |d, f| d.font_family(f))
            .child(self.0.clone())
    }
}

pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

/// Square 28 px icon button (`active` = accent-tinted on state).
pub fn icon_button(id: impl Into<ElementId>, i: Icon, t: &Tokens, active: bool) -> Stateful<Div> {
    let (fg, bg): (Rgba, Option<Hsla>) =
        if active { (t.accent, Some(alpha(t.accent, 0.16))) } else { (t.text_muted, None) };
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(HIT))
        .rounded(px(6.))
        .cursor_pointer()
        .when_some(bg, |d, bg| d.bg(bg))
        .hover(|s| s.bg(t.surface_hover))
        .active(|s| s.bg(t.border))
        .child(icon(i, 15., fg))
}

pub enum ButtonKind {
    Primary,
    Secondary,
    Ghost,
    /// Destructive / emergency action: canvas-coloured text on a `danger` fill (core keeps
    /// `danger` ≥ 4.5:1 against `canvas`, so the label keeps that contrast).
    Danger,
}

/// Labelled button (icon optional), 28 px tall.
pub fn text_button(
    id: impl Into<ElementId>,
    i: Option<Icon>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    t: &Tokens,
    size: f32,
) -> Stateful<Div> {
    let (fg, bg, hover): (Rgba, Option<Rgba>, Rgba) = match kind {
        ButtonKind::Primary => (t.accent_text, Some(t.accent), crate::theme::mix(t.accent, t.canvas, 0.15)),
        ButtonKind::Secondary => (t.text, Some(t.surface_hover), t.border),
        ButtonKind::Ghost => (t.text, None, t.surface_hover),
        ButtonKind::Danger => (t.canvas, Some(t.danger), crate::theme::mix(t.danger, t.canvas, 0.15)),
    };
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap_1p5()
        .h(px(HIT))
        .px_2p5()
        .rounded(px(6.))
        .text_size(px(size))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg)
        .cursor_pointer()
        .when_some(bg, |d, bg| d.bg(bg))
        .when(matches!(kind, ButtonKind::Secondary | ButtonKind::Ghost), |d| d.border_1().border_color(t.border))
        .hover(move |s| s.bg(hover))
        .when_some(i, |d, i| d.child(icon(i, 14., fg)))
        .child(label.into())
}

/// Rounded pill (badges, chips): 11 px text on a tint of `color`.
pub fn pill(label: impl Into<SharedString>, color: Rgba, fill: f32) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px(18.))
        .px(px(6.))
        .rounded(px(9.))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .bg(alpha(color, fill))
        .child(label.into())
}

static TNUM: LazyLock<FontFeatures> = LazyLock::new(|| FontFeatures(Arc::new(vec![("tnum".into(), 1)])));

/// Tabular numerals (times, metrics): monospaced digits so numbers do not jitter.
pub fn tabular<S: Styled>(mut el: S) -> S {
    el.text_style().get_or_insert_with(Default::default).font_features = Some(TNUM.clone());
    el
}
