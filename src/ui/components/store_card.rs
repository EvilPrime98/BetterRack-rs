use gpui::{
    Context, ObjectFit, SharedString, Styled, StyledImage as _, div, img, linear_color_stop,
    linear_gradient, prelude::*, px, relative, rgb, rgba,
};

use crate::model::StorePost;
use crate::state::Stores;
use crate::state::pull_list::keyword_from_title;
use crate::state::store::{CardState, display_date, post_key};
use crate::state::thumbnails::Thumb;
use crate::ui::components::comic_card::action_button;
use crate::ui::icons::{Icon, icon};
use crate::ui::{theme, toast};

const CARD_PAD: f32 = 10.0;
const COVER_RATIO: f32 = 77.0 / 50.0;
const TITLE_H: f32 = 34.0;
const DATE_H: f32 = 14.0;

pub fn card_height(col_w: f32) -> f32 {
    let cover_h = (col_w - CARD_PAD * 2.0) * COVER_RATIO;
    CARD_PAD * 2.0 + cover_h + 6.0 + TITLE_H + 4.0 + DATE_H
}

pub fn store_card<V: 'static>(
    post: &StorePost,
    col_w: f32,
    stores: &Stores,
    cx: &mut Context<V>,
) -> gpui::Stateful<gpui::Div> {
    let key = post_key(post);
    let inner_w = col_w - CARD_PAD * 2.0;
    let thumb = match post.thumbnail_url.as_deref().filter(|u| !u.is_empty()) {
        Some(url) => {
            stores.thumbs.update(cx, |t, cx| t.ensure_external(url, cx));
            stores.thumbs.read(cx).get(url).cloned()
        }
        None => Some(Thumb::Missing),
    };
    let state = stores.store.read(cx).card(&key);

    let group: SharedString = format!("store-card-{key}").into();
    let frame = div()
        .relative()
        .w(px(inner_w))
        .h(px(inner_w * COVER_RATIO))
        .overflow_hidden()
        .rounded(px(6.0))
        .bg(rgb(0x232222));
    let cover = match thumb {
        Some(Thumb::Ready(image)) => frame.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .group_hover(group.clone(), |s| {
                    s.top(relative(-0.015))
                        .left(relative(-0.015))
                        .w(relative(1.03))
                        .h(relative(1.03))
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

    let (label, glyph, loading, failed) = match &state {
        CardState::Error(_) => ("Retry", Icon::Refresh, false, true),
        CardState::LinksLoading => ("Download", Icon::Download, true, false),
        CardState::Idle => ("Download", Icon::Download, false, false),
    };
    let pinned = loading || failed;
    let fill = if failed { rgb(0xf0707a) } else { rgb(0x34c3d1) };
    let fill_hover = if failed { rgb(0xf58d95) } else { rgb(0x5fd8e4) };
    let tint = rgb(0x0b0b0b);
    let action = {
        let post = post.clone();
        let stores = stores.clone();
        let id: SharedString = format!("store-dl-{key}").into();
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(5.0))
            .px(px(11.0))
            .py(px(4.0))
            .rounded_full()
            .bg(fill)
            .shadow_md()
            .text_size(px(11.0))
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(tint)
            .child(icon(glyph, px(12.0)).text_color(tint))
            .child(label)
            .when(loading, |d| d.opacity(0.6))
            .when(!loading, move |d| {
                d.cursor_pointer()
                    .hover(move |s| s.bg(fill_hover))
                    .on_click(move |_, _, cx| {
                        let post = post.clone();
                        stores.store.update(cx, |s, cx| s.download(&post, cx));
                    })
            })
    };
    let bookmarked = stores.bookmarks.read(cx).contains(&key);
    let bookmark = action_button(
        format!("store-bm-{key}").into(),
        Icon::Bookmark,
        bookmarked,
        {
            let post = post.clone();
            let stores = stores.clone();
            move |cx| toggle_bookmark(&stores, &post, cx)
        },
    );
    let followed = stores.pull_list.read(cx).follows(&post.title);
    let pull = action_button(format!("store-pull-{key}").into(), Icon::Pin, followed, {
        let title = post.title.clone();
        let stores = stores.clone();
        move |cx| follow_series(&stores, &title, cx)
    });
    let overlay = div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .pt(px(28.0))
        .pb(px(8.0))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgba(0x00000000), 0.0),
            linear_color_stop(rgba(0x000000d0), 1.0),
        ))
        .child(action)
        .child(bookmark)
        .child(pull);
    let overlay = if pinned {
        overlay
    } else {
        overlay
            .opacity(0.0)
            .group_hover(group.clone(), |s| s.opacity(1.0))
    };
    let cover = cover.child(overlay);

    div()
        .id(SharedString::from(format!("store-card-{key}")))
        .group(group.clone())
        .flex()
        .flex_col()
        .items_center()
        .flex_none()
        .w(px(col_w))
        .h(px(card_height(col_w)))
        .p(px(CARD_PAD))
        .rounded(px(10.0))
        .group_hover(group, |s| s.bg(rgba(0xffffff0b)))
        .child(cover)
        .child(
            div()
                .w_full()
                .min_w_0()
                .mt(px(6.0))
                .h(px(TITLE_H))
                .overflow_hidden()
                .text_center()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xffffff))
                .line_height(px(17.0))
                .line_clamp(2)
                .text_ellipsis()
                .child(post.title.replace('_', " ")),
        )
        .child(
            div()
                .w_full()
                .mt(px(4.0))
                .h(px(DATE_H))
                .text_center()
                .text_size(px(10.0))
                .font_family(theme::FONT_MONO)
                .text_color(rgb(0x808080))
                .child(
                    post.upload_date
                        .as_deref()
                        .map(display_date)
                        .unwrap_or_default(),
                ),
        )
}

/// The pin: follows the series named by the post title right away, or unfollows it.
fn follow_series(stores: &Stores, title: &str, cx: &mut gpui::App) {
    let keyword = keyword_from_title(title);
    if keyword.is_empty() {
        toast::error(cx, "Could not tell which series this is.");
        return;
    }
    let followed = stores
        .pull_list
        .update(cx, |s, cx| s.toggle(title, &keyword, cx));
    if followed {
        toast::success(cx, format!("Added “{keyword}” to your pull list."));
    } else {
        toast::success(cx, format!("Removed “{keyword}” from your pull list."));
    }
}

fn toggle_bookmark(stores: &Stores, post: &StorePost, cx: &mut gpui::App) {
    let key = post_key(post);
    if stores.bookmarks.read(cx).contains(&key) {
        stores.bookmarks.update(cx, |s, cx| s.remove(&key, cx));
        return;
    }
    let (stores, post) = (stores.clone(), post.clone());
    crate::ui::modals::open_pick_folder(cx, "Bookmark in:", "", move |folder, cx| {
        stores
            .bookmarks
            .update(cx, |s, cx| s.add(&post, folder, cx));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_height_grows_with_the_cover() {
        assert!(card_height(240.0) > card_height(180.0));
    }
}
