//! Pieces shared by the library-style pages: header bar, back button, cycle buttons, empty notes.

use gpui::{
    App, ClickEvent, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::model::{ComicsType, ReadFilter};
use crate::state::prefs::PrefsStore;
use crate::ui::components::dropdown::dropdown;
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
    div()
        .text_size(px(12.0))
        .text_color(rgb(0x9a9a9a))
        .child(format!("{count} comics"))
}

/// Centered note in place of the grid ("Nothing in progress.", errors, loading).
pub fn note(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .py(px(40.0))
        .text_center()
        .text_size(px(13.0))
        .text_color(rgb(0x808080))
        .child(text.into())
}

fn capitalize(s: &str) -> SharedString {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect::<String>())
        .unwrap_or_default()
        .into()
}

/// Dropdown whose open state lives in the prefs store, so it works from stateless render code.
pub fn prefs_dropdown<V: Copy + PartialEq + 'static>(
    id: &'static str,
    options: impl IntoIterator<Item = (V, &'static str)>,
    current: V,
    align_right: bool,
    prefs: &gpui::Entity<PrefsStore>,
    cx: &App,
    apply: impl Fn(&mut PrefsStore, V, &mut gpui::Context<PrefsStore>) + 'static,
) -> impl IntoElement {
    let open = prefs.read(cx).open_menu == Some(id);
    let for_open = prefs.clone();
    let for_select = prefs.clone();
    dropdown(
        id,
        options
            .into_iter()
            .map(|(v, l)| (v, capitalize(l)))
            .collect(),
        current,
        open,
        align_right,
        move |open, cx| for_open.update(cx, |p, cx| p.set_menu_open(id, open, cx)),
        move |v, cx| for_select.update(cx, |p, cx| apply(p, v, cx)),
    )
}

/// `StateFilter` + `LayoutSelector`: the two dropdowns that live in every grid page header.
pub fn view_controls(
    read_filter: ReadFilter,
    kind: ComicsType,
    with_read_filter: bool,
    prefs: &gpui::Entity<PrefsStore>,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .when(with_read_filter, |s| {
            s.child(prefs_dropdown(
                "read-filter",
                [
                    ReadFilter::All,
                    ReadFilter::Read,
                    ReadFilter::Unread,
                    ReadFilter::Reading,
                ]
                .map(|v| (v, v.label())),
                read_filter,
                true,
                prefs,
                cx,
                |p, v, cx| p.set_read_filter(v, cx),
            ))
        })
        .child(prefs_dropdown(
            "layout",
            [ComicsType::Cover, ComicsType::Detail].map(|v| (v, v.label())),
            kind,
            true,
            prefs,
            cx,
            |p, v, cx| p.set_comics_type(v, cx),
        ))
}

use gpui::prelude::FluentBuilder as _;
