use std::f32::consts::TAU;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, div, ease_in_out, px, rgb, rgba,
};

use crate::ui::shell::header::logo;
use crate::ui::theme;

const LOGO: f32 = 88.0;
const RIPPLE_BOX: f32 = 168.0;
const RIPPLE_PERIOD_MS: u64 = 2400;
const BAR_W: f32 = 160.0;
const BAR_SEGMENT: f32 = 56.0;

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn enter<E>(el: E, id: &'static str, delay_ms: u64) -> impl IntoElement
where
    E: IntoElement + 'static,
    gpui::AnimationElement<E>: IntoElement,
    E: Styled,
{
    let total = 500 + delay_ms;
    el.with_animation(
        id,
        Animation::new(Duration::from_millis(total)),
        move |el, t| {
            let local = ((t * total as f32 - delay_ms as f32) / 500.0).clamp(0.0, 1.0);
            let e = ease_out_cubic(local);
            el.opacity(e).mt(px((1.0 - e) * 10.0))
        },
    )
}

fn ripple(id: &'static str, phase: f32) -> impl IntoElement {
    div()
        .absolute()
        .rounded_full()
        .border_2()
        .border_color(theme::accent())
        .with_animation(
            id,
            Animation::new(Duration::from_millis(RIPPLE_PERIOD_MS)).repeat(),
            move |el, t| {
                let p = (t + phase).fract();
                let size = LOGO * 0.9 + (RIPPLE_BOX - LOGO * 0.9) * ease_out_cubic(p);
                let off = (RIPPLE_BOX - size) / 2.0;
                el.size(px(size))
                    .left(px(off))
                    .top(px(off))
                    .opacity(0.45 * (1.0 - p).powi(2))
            },
        )
}

fn progress_bar() -> impl IntoElement {
    div()
        .relative()
        .w(px(BAR_W))
        .h(px(3.0))
        .rounded_full()
        .overflow_hidden()
        .bg(rgba(0xffffff14))
        .child(
            div()
                .absolute()
                .top_0()
                .h_full()
                .w(px(BAR_SEGMENT))
                .rounded_full()
                .bg(theme::accent())
                .with_animation(
                    "loader-bar",
                    Animation::new(Duration::from_millis(1300))
                        .repeat()
                        .with_easing(ease_in_out),
                    |el, t| el.left(px(-BAR_SEGMENT + t * (BAR_W + BAR_SEGMENT))),
                ),
        )
}

pub fn app_loader(message: impl Into<SharedString>) -> impl IntoElement {
    let breathing_logo = div().child(logo(LOGO)).with_animation(
        "loader-breathe",
        Animation::new(Duration::from_millis(2400))
            .repeat()
            .with_easing(ease_in_out),
        |el, t| {
            let wave = 0.5 - 0.5 * (t * TAU).cos();
            let s = LOGO * (1.0 + 0.06 * wave);
            el.size(px(s)).opacity(0.85 + 0.15 * wave)
        },
    );

    let emblem = div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .size(px(RIPPLE_BOX))
        .child(ripple("loader-ripple-a", 0.0))
        .child(ripple("loader-ripple-b", 0.5))
        .child(breathing_logo);

    let wordmark = div()
        .flex()
        .text_size(px(22.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme::text())
        .child("Better")
        .child(div().text_color(theme::accent()).child("Rack"));

    let status = div()
        .text_size(px(13.0))
        .text_color(rgb(0x8f8f8f))
        .child(message.into());

    div()
        .id("app-loader")
        .absolute()
        .inset_0()
        .occlude()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .bg(theme::bg_chrome())
        .child(enter(emblem, "loader-in-emblem", 0))
        .child(enter(wordmark, "loader-in-word", 120))
        .child(enter(
            div().mt(px(10.0)).child(progress_bar()),
            "loader-in-bar",
            240,
        ))
        .child(enter(status, "loader-in-status", 360))
}
