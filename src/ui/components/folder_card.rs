//! `FolderCard` (basic variant).

use gpui::{
    AnyElement, Context, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px, rgb, rgba,
};

use crate::model::{ComicsType, LibraryEntry};
use crate::route::{Navigate, Route};
use crate::state::Stores;
use crate::ui::components::comic_card::{COVER_MAX_CARD_W, COVER_RATIO};
use crate::ui::confirm::{self, ConfirmOptions};
use crate::ui::icons::{Icon, icon};
use crate::ui::theme;

pub fn folder_card<V: EventEmitter<Navigate> + 'static>(
    item: &LibraryEntry,
    kind: ComicsType,
    stores: &Stores,
    col_w: f32,
    cx: &mut Context<V>,
) -> AnyElement {
    let uid = item.uid.clone();
    let open = {
        let uid = uid.clone();
        cx.listener(move |_, _, _, cx| {
            cx.emit(Navigate(Route::Library {
                uid: Some(uid.clone()),
                search: None,
            }))
        })
    };
    let delete = {
        let (library, uid, name) = (stores.library.clone(), uid.clone(), item.name.clone());
        move |cx: &mut gpui::App| {
            let (library, uid) = (library.clone(), uid.clone());
            confirm::ask(
                cx,
                ConfirmOptions::new(
                    "Delete folder?",
                    format!(
                        "\"{name}\" and everything inside it will be permanently deleted from disk."
                    ),
                )
                .labels("Delete", "Cancel"),
                move |answer, _, cx| {
                    if answer == Some(true) {
                        library.update(cx, |s, cx| s.delete_folder(uid, cx));
                    }
                },
            );
        }
    };
    let delete_button = div()
        .id(SharedString::from(format!("folder-delete-{uid}")))
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded_full()
        .border_1()
        .border_color(rgba(0xffffff1f))
        .bg(rgba(0x121212bf))
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|s| {
            s.bg(rgba(0xe85d5d2e))
                .border_color(rgba(0xe85d5d73))
                .text_color(rgb(0xe85d5d))
        })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            delete(cx)
        })
        .child(icon(Icon::Trash, px(13.0)).text_color(theme::text()));

    let group: SharedString = format!("folder-{uid}").into();

    if kind == ComicsType::Detail {
        return div()
            .w(px(col_w))
            .child(
                div()
                    .id(SharedString::from(format!("folder-{uid}")))
                    .group(group)
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .w_full()
                    .p(px(14.0))
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(rgba(0xffffff12))
                    .bg(rgba(0xffffff0b))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0xffffff12)))
                    .on_click(open)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(76.0))
                            .flex_none()
                            .rounded(px(12.0))
                            .border_1()
                            .border_color(rgba(0xffffff0f))
                            .bg(rgb(0x202020))
                            .text_color(theme::accent())
                            .child(icon(Icon::Folder, px(34.0)).text_color(theme::accent())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme::accent())
                                    .child("FOLDER"),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(16.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(item.name.clone()),
                            ),
                    )
                    .child(delete_button)
                    .child(icon(Icon::ChevronDown, px(14.0)).text_color(rgb(0x666666))),
            )
            .into_any_element();
    }

    let card_w = col_w.min(COVER_MAX_CARD_W);
    let inner_w = card_w - 20.0;
    div()
        .w(px(col_w))
        .flex()
        .justify_center()
        .child(
            div()
                .id(SharedString::from(format!("folder-{uid}")))
                .group(group.clone())
                .flex()
                .flex_col()
                .items_center()
                .w(px(card_w))
                .p(px(10.0))
                .cursor_pointer()
                .on_click(open)
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(6.0))
                        .w(px(inner_w))
                        .h(px(inner_w * COVER_RATIO))
                        .px(px(10.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x222121))
                        .hover(|s| s.bg(rgb(0x2a2929)))
                        .text_color(theme::text())
                        .child(icon(Icon::Folder, px(40.0)).text_color(theme::accent()))
                        .child(
                            div()
                                .w_full()
                                .text_center()
                                .line_clamp(3)
                                .child(item.name.clone()),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(px(6.0))
                                .right(px(6.0))
                                .opacity(0.0)
                                .group_hover(group, |s| s.opacity(1.0))
                                .child(delete_button),
                        ),
                ),
        )
        .into_any_element()
}
