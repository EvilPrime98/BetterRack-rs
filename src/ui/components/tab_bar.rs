//! Tab strip for local archives, hosted by the reader under its own header. `AppRoot` owns the
//! tab list and pushes it in with [`TabBar::set`]; clicks come back as [`TabEvent`]s.

use std::path::PathBuf;

use gpui::{
    Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::ui::icons::{Icon, icon};
use crate::ui::theme;

pub enum TabEvent {
    Open(usize),
    Close(usize),
}

#[derive(Default)]
pub struct TabBar {
    tabs: Vec<PathBuf>,
    active: usize,
}

impl EventEmitter<TabEvent> for TabBar {}

impl TabBar {
    pub fn set(&mut self, tabs: &[PathBuf], active: usize, cx: &mut Context<Self>) {
        self.tabs = tabs.to_vec();
        self.active = active;
        cx.notify();
    }
}

impl Render for TabBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.tabs.iter().enumerate().map(|(i, path)| {
            let active = i == self.active;
            let title = path
                .file_stem()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned();
            div()
                .id(("tab", i))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.0))
                .max_w(px(220.0))
                .h_full()
                .pl(px(12.0))
                .pr(px(6.0))
                .cursor_pointer()
                .text_size(px(12.0))
                .border_b_2()
                .border_color(if active {
                    theme::text()
                } else {
                    gpui::rgba(0x00000000).into()
                })
                .text_color(if active {
                    theme::text()
                } else {
                    theme::text_muted()
                })
                .hover(|s| s.bg(gpui::rgba(0xffffff0f)))
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(TabEvent::Open(i))))
                .child(div().min_w_0().truncate().child(title))
                .child(
                    div()
                        .id(("tab-close", i))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(20.0))
                        .rounded_full()
                        .hover(|s| s.bg(gpui::rgba(0xffffff1f)))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(TabEvent::Close(i))
                        }))
                        .child(icon(Icon::Close, px(12.0)).text_color(theme::text())),
                )
        });
        div()
            .flex()
            .flex_none()
            .h(px(34.0))
            .w_full()
            .overflow_hidden()
            .border_t_1()
            .border_color(theme::border_subtle())
            .bg(theme::bg_app())
            .children(tabs)
    }
}
