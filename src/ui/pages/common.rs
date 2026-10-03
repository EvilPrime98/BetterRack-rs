//! Pieces shared by the library-style pages: header bar, back button, cycle buttons, empty notes.

use gpui::{
    App, ClickEvent, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::model::{ComicsType, ReadFilter};
use crate::state::prefs::PrefsStore;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::items_grid::PAGE_PAD_X;
use crate::ui::icons::{Icon, icon};
use crate::ui::theme;

/// The bordered bar every page starts with: `left` and `right` clusters, wrapping on narrow windows.
pub fn header_bar(left: impl IntoElement, right: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_y(px(10.0))
        .px(px(PAGE_PAD_X))
        .pt(px(14.0))
        .pb(px(18.0))
        .border_b_1()
        .border_color(theme::border_subtle())
        .bg(theme::bg_app())
        .child(left)
        .child(right)
}

pub fn back_button(
    id: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(34.0))
        .flex_none()
        .rounded(px(4.0))
        .border_1()
        .border_color(theme::border_subtle())
        .bg(theme::bg_panel())
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x2a2a2a)))
        .on_click(on_click)
        .child(icon(Icon::ArrowLeft, px(16.0)).text_color(theme::text()))
}

/// Small caps line over a title (`eyebrow` + `title` in the Recent/Reading/Filtered headers).
pub fn summary(eyebrow: &'static str, title: impl Into<SharedString>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            div()
                .text_size(px(10.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme::accent())
                .child(eyebrow.to_uppercase()),
        )
        .child(
            div()
                .truncate()
                .text_size(px(18.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.into()),
        )
}

pub fn counter(count: usize) -> impl IntoElement {
    div().text_size(px(12.0)).text_color(rgb(0x9a9a9a)).child(format!("{count} comics"))
}

/// Centered note in place of the grid ("Nothing in progress.", errors, loading).
pub fn note(text: impl Into<SharedString>) -> impl IntoElement {
    div().py(px(40.0)).text_center().text_size(px(13.0)).text_color(rgb(0x808080)).child(text.into())
}

pub fn cycle_button(
    id: &'static str,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    button(id, label, ButtonVariant::Secondary, on_click)
}

/// `StateFilter` + `LayoutSelector`: the two cycle buttons that live in every grid page header.
pub fn view_controls(
    read_filter: ReadFilter,
    kind: ComicsType,
    with_read_filter: bool,
    prefs: &gpui::Entity<PrefsStore>,
) -> impl IntoElement {
    let for_filter = prefs.clone();
    let for_layout = prefs.clone();
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .when(with_read_filter, |s| {
            s.child(cycle_button("read-filter", read_filter.label(), move |_, _, cx| {
                for_filter.update(cx, |p, cx| p.cycle_read_filter(cx))
            }))
        })
        .child(cycle_button("layout", kind.label(), move |_, _, cx| {
            for_layout.update(cx, |p, cx| p.cycle_comics_type(cx))
        }))
}

use gpui::prelude::FluentBuilder as _;
