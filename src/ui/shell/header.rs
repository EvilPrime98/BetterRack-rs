//! Header (`components/header`): burger, logo → home, title, drag region, window controls.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, WindowControlArea, div, px,
};

use crate::app::AppRoot;
use crate::route::Route;
use crate::ui::icons::{Icon, icon};
use crate::ui::shell::window_controls::window_controls;
use crate::ui::theme;

/// The `BetterRackIcon`: rounded square with an accent border and "BR" (drawn natively because
/// GPUI's `svg()` cannot render text).
pub fn logo(size: f32) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(size * 0.28))
        .border_1()
        .border_color(theme::accent())
        .bg(gpui::rgb(0x111111))
        .text_color(theme::accent())
        .text_size(px(size * 0.4))
        .font_weight(gpui::FontWeight::BOLD)
        .child("BR")
}

pub fn header(window: &Window, cx: &mut Context<AppRoot>) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(theme::header_height())
        .w_full()
        .bg(theme::bg_chrome())
        .border_b_1()
        .border_color(theme::border_subtle())
        .child(
            div()
                .id("burger")
                .flex()
                .items_center()
                .justify_center()
                .size(px(40.0))
                .ml(px(8.0))
                .rounded(px(8.0))
                .text_color(theme::text())
                .hover(|s| s.bg(theme::hover()))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx)))
                .child(icon(Icon::Burger, px(24.0)).text_color(theme::text())),
        )
        .child(
            div()
                .id("logo")
                .flex()
                .items_center()
                .gap(px(10.0))
                .ml(px(8.0))
                .cursor_pointer()
                // Clicking the logo goes home.
                .on_click(
                    cx.listener(|this, _, window, cx| this.navigate(Route::home(), window, cx)),
                )
                .child(logo(32.0))
                .child(
                    div()
                        .text_color(theme::text())
                        .text_size(px(16.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("BetterRack"),
                ),
        )
        // Drag region: everything between the title and the controls moves the window.
        .child(
            div()
                .flex_1()
                .h_full()
                .window_control_area(WindowControlArea::Drag),
        )
        .children(window_controls(window))
}
