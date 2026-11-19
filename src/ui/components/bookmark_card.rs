use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ObjectFit, ParentElement, SharedString,
    Styled, StyledImage as _, div, img, linear_color_stop, linear_gradient, px, rgb, rgba,
};

use crate::model::StoreBookmark;
use crate::state::Stores;
use crate::state::thumbnails::Thumb;
use crate::ui::components::comic_card::{
    COVER_MAX_CARD_W, COVER_RATIO, PAD_COVER, action_button, menu_item, menu_shell, rating_with,
    read_bar, set_card_menu,
};
use crate::ui::icons::{Icon, icon};
use crate::ui::theme;

pub fn card_height(col_w: f32) -> f32 {
    crate::ui::components::comic_card::card_height(crate::model::ComicsType::Cover, col_w, true)
}

pub fn menu_id(key: &str) -> String {
    format!("bookmark:{key}")
}

fn cover(
    b: &StoreBookmark,
    thumb: Option<Thumb>,
    w: f32,
    downloading: bool,
    group: &SharedString,
) -> impl IntoElement {
    let frame = div()
        .relative()
        .w(px(w))
        .h(px(w * COVER_RATIO))
        .overflow_hidden()
        .bg(rgb(0x232222));
    let frame = match thumb {
        Some(Thumb::Ready(image)) => frame.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .group_hover(group.clone(), |s| {
                    s.top(gpui::relative(-0.015))
                        .left(gpui::relative(-0.015))
                        .w(gpui::relative(1.03))
                        .h(gpui::relative(1.03))
                })
                .child(img(image).size_full().object_fit(ObjectFit::Cover)),
        ),
        Some(Thumb::Missing) => frame
            .flex()
            .items_center()
            .justify_center()
            .child(icon(Icon::BookOpen, px(36.0)).text_color(rgb(0x5c5c5c))),
        _ => frame,
    };
    let dim = (b.read || downloading).then(|| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(if downloading { 0x000000a6 } else { 0x00000052 }))
    });
    let check = b.read.then(|| {
        div()
            .absolute()
            .top(px(6.0))
            .left(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.0))
            .rounded_full()
            .bg(rgb(0x13a629))
            .child(icon(Icon::Check, px(12.0)).text_color(rgb(0xffffff)))
    });
    let badge = div()
        .absolute()
        .top(px(6.0))
        .right(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .size(px(20.0))
        .rounded_full()
        .bg(rgba(0x121212d9))
        .border_1()
        .border_color(rgba(0x34c3d173))
        .child(icon(Icon::Bookmark, px(11.0)).text_color(theme::accent()));
    let rating_badge = (b.rating > 0.0).then(|| {
        div()
            .absolute()
            .left(px(6.0))
            .bottom(px(6.0))
            .flex()
            .items_center()
            .gap(px(3.0))
            .px(px(6.0))
            .py(px(2.0))
            .rounded_full()
            .bg(rgba(0x121212d9))
            .text_size(px(10.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme::text())
            .group_hover(group.clone(), |s| s.opacity(0.0))
            .child(icon(Icon::Star, px(10.0)).text_color(rgb(0xfefcf3)))
            .child(format!("{}", b.rating))
    });
    let status = downloading.then(|| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme::text())
            .child("Downloading…")
    });
    frame
        .children(dim)
        .children(check)
        .child(badge)
        .children(rating_badge)
        .children(status)
}

fn overlay(
    b: &StoreBookmark,
    menu_open: bool,
    group: &SharedString,
    stores: &Stores,
) -> impl IntoElement {
    let key = b.key.clone();
    let menu_key = menu_id(&key);
    let is_read = b.read;
    let on_rating: Rc<dyn Fn(f32, &mut App)> = {
        let (stores, key) = (stores.clone(), key.clone());
        Rc::new(move |value, cx| {
            stores.bookmarks.update(cx, |s, cx| {
                s.set_rating(&key, value, cx);
                if value > 0.0 {
                    s.set_read(&key, true, cx);
                }
            });
        })
    };
    let overlay = div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.0))
        .px(px(6.0))
        .pt(px(28.0))
        .pb(px(7.0))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgba(0x00000000), 0.0),
            linear_color_stop(rgba(0x000000d0), 1.0),
        ));
    let overlay = if menu_open {
        overlay
    } else {
        overlay
            .opacity(0.0)
            .group_hover(group.clone(), |s| s.opacity(1.0))
    };
    overlay
        .child(
            div()
                .px(px(8.0))
                .py(px(3.0))
                .rounded_full()
                .bg(rgba(0x000000a6))
                .child(rating_with(&key, b.rating, 18.0, on_rating)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.0))
                .w_full()
                .child(action_button(
                    format!("bm-read-{key}").into(),
                    Icon::Check,
                    is_read,
                    {
                        let (stores, key) = (stores.clone(), key.clone());
                        move |cx| {
                            stores
                                .bookmarks
                                .update(cx, |s, cx| s.set_read(&key, !is_read, cx))
                        }
                    },
                ))
                .child(div().flex_1())
                .child(action_button(
                    format!("bm-more-{key}").into(),
                    Icon::Dots,
                    menu_open,
                    {
                        let stores = stores.clone();
                        move |cx| set_card_menu(&stores, &menu_key, !menu_open, cx)
                    },
                )),
        )
}

fn menu(b: &StoreBookmark, stores: &Stores) -> impl IntoElement {
    let key = b.key.clone();
    let menu_key = menu_id(&key);
    let close = {
        let (stores, menu_key) = (stores.clone(), menu_key.clone());
        Rc::new(move |cx: &mut App| set_card_menu(&stores, &menu_key, false, cx))
    };
    let then_close = |f: Box<dyn Fn(&mut App)>| {
        let close = close.clone();
        move |cx: &mut App| {
            close(cx);
            f(cx);
        }
    };
    menu_shell(
        close.clone(),
        vec![
            menu_item(
                format!("bm-download-{key}").into(),
                Icon::Download,
                "Download",
                false,
                then_close(Box::new({
                    let (stores, key) = (stores.clone(), key.clone());
                    move |cx| crate::state::bookmarks::download(&stores, &key, cx)
                })),
            )
            .into_any_element(),
            menu_item(
                format!("bm-move-{key}").into(),
                Icon::Folder,
                "Move",
                false,
                then_close(Box::new({
                    let (stores, key) = (stores.clone(), key.clone());
                    move |cx| {
                        let (stores, key) = (stores.clone(), key.clone());
                        crate::ui::modals::open_pick_folder(
                            cx,
                            "Move bookmark to:",
                            "",
                            move |folder, cx| {
                                stores
                                    .bookmarks
                                    .update(cx, |s, cx| s.move_to(&key, folder, cx))
                            },
                        );
                    }
                })),
            )
            .into_any_element(),
            menu_item(
                format!("bm-remove-{key}").into(),
                Icon::Trash,
                "Remove",
                true,
                then_close(Box::new({
                    let (stores, key) = (stores.clone(), key.clone());
                    move |cx| stores.bookmarks.update(cx, |s, cx| s.remove(&key, cx))
                })),
            )
            .into_any_element(),
        ],
    )
}

pub fn bookmark_card(
    b: &StoreBookmark,
    thumb: Option<Thumb>,
    stores: &Stores,
    col_w: f32,
    cx: &App,
) -> AnyElement {
    let key = b.key.clone();
    let group: SharedString = format!("bm-card-{key}").into();
    let card_w = col_w.min(COVER_MAX_CARD_W);
    let inner_w = card_w - PAD_COVER * 2.0;
    let downloading = stores.bookmarks.read(cx).is_downloading(&key);
    let menu_open = stores.prefs.read(cx).card_menu.as_deref() == Some(menu_id(&key).as_str());
    let read_per = if b.read { 100.0 } else { 0.0 };

    div()
        .w(px(col_w))
        .flex()
        .justify_center()
        .child(
            div()
                .id(SharedString::from(format!("bookmark-{key}")))
                .group(group.clone())
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.0))
                .w(px(card_w))
                .p(px(PAD_COVER))
                .rounded(px(10.0))
                .group_hover(group.clone(), |s| s.bg(rgba(0xffffff0b)))
                .child(
                    div()
                        .relative()
                        .child(cover(b, thumb, inner_w, downloading, &group))
                        .children((!downloading).then(|| overlay(b, menu_open, &group, stores)))
                        .children((menu_open && !downloading).then(|| menu(b, stores))),
                )
                .child(read_bar(read_per))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .mt(px(2.0))
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .line_height(px(17.0))
                        .line_clamp(2)
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_center()
                        .text_color(theme::text())
                        .child(b.title.replace('_', " ")),
                ),
        )
        .into_any_element()
}
