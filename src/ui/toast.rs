//! Toasts: top-center, 2.5 s (errors 5 s), accent left border.

use std::time::Duration;

use gpui::{
    AnimationExt as _, App, AppContext as _, Context, Entity, Global, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px,
};

use crate::ui::{motion, theme};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Success,
    Error,
}

struct Toast {
    id: u64,
    kind: Kind,
    text: SharedString,
    leaving: bool,
}

#[derive(Default)]
pub struct ToastHost {
    items: Vec<Toast>,
    next_id: u64,
}

struct ToastGlobal(Entity<ToastHost>);
impl Global for ToastGlobal {}

pub fn init(cx: &mut App) -> Entity<ToastHost> {
    let host = cx.new(|_| ToastHost::default());
    cx.set_global(ToastGlobal(host.clone()));
    host
}

pub fn success(cx: &mut App, text: impl Into<SharedString>) {
    push(cx, Kind::Success, text.into());
}

pub fn error(cx: &mut App, text: impl Into<SharedString>) {
    push(cx, Kind::Error, text.into());
}

fn push(cx: &mut App, kind: Kind, text: SharedString) {
    let Some(host) = cx.try_global::<ToastGlobal>().map(|g| g.0.clone()) else {
        return;
    };
    host.update(cx, |host, cx| {
        let id = host.next_id;
        host.next_id += 1;
        host.items.push(Toast {
            id,
            kind,
            text,
            leaving: false,
        });
        cx.notify();
        let ttl = if kind == Kind::Error { 5000 } else { 2500 };
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ttl))
                .await;
            this.update(cx, |host, cx| {
                if let Some(t) = host.items.iter_mut().find(|t| t.id == id) {
                    t.leaving = true;
                }
                cx.notify();
            })
            .ok();
            cx.background_executor().timer(motion::FAST).await;
            this.update(cx, |host, cx| {
                host.items.retain(|t| t.id != id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    });
}

impl Render for ToastHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(16.0))
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.0))
            .children(self.items.iter().map(|t| {
                let leaving = t.leaving;
                let border = if t.kind == Kind::Error {
                    theme::error()
                } else {
                    theme::accent()
                };
                div()
                    .id(("toast", t.id))
                    .occlude()
                    .max_w(px(480.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .rounded(px(10.0))
                    .bg(theme::bg_modal())
                    .border_l(px(4.0))
                    .border_color(border)
                    .shadow(vec![gpui::BoxShadow {
                        color: gpui::hsla(0.0, 0.0, 0.0, 0.45),
                        inset: false,
                        offset: gpui::point(px(0.0), px(8.0)),
                        blur_radius: px(24.0),
                        spread_radius: px(0.0),
                    }])
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme::text())
                    .child(t.text.clone())
                    .with_animation(
                        (if t.leaving { "toast-out" } else { "toast-in" }, t.id),
                        motion::once(if t.leaving {
                            motion::FAST
                        } else {
                            motion::BASE
                        }),
                        move |el, p| {
                            if leaving {
                                el.opacity(1.0 - p)
                            } else {
                                el.opacity(p).mt(px(-12.0 * (1.0 - p)))
                            }
                        },
                    )
            }))
    }
}
