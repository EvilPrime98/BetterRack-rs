//! Store page (`pages/store.page.tsx` + `components/store-card`). Title, search box and a
//! virtualized grid of posts that loads the next page when its last rows come into view.
//!
//! The data (query, results, page, scroll position, per-card download state) lives in
//! [`StoreState`](crate::state::store::StoreState), so it survives leaving the page. Clicking a
//! card's Download runs links → (link picker) → download folder → job; see `StoreState::download`.

use gpui::{
    AppContext as _, Context, EventEmitter, IntoElement, ObjectFit, ParentElement, Render,
    SharedString, Styled, StyledImage, Subscription, Window, div, img, prelude::*, px,
    rgb, uniform_list,
};

use crate::model::StorePost;
use crate::route::{Navigate, Route};
use crate::state::Stores;
use crate::state::store::{CardState, display_date, post_key};
use crate::state::thumbnails::Thumb;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::items_grid::{PAGE_PAD_X, available_width};
use crate::ui::components::text_input::{TextInput, TextInputEvent};
use crate::ui::icons::{Icon, icon};
use crate::ui::theme;

/// Minimum card width and the gaps of `.grid` in `store.page.module.css`.
const MIN_COL: f32 = 180.0;
const GAP_X: f32 = 22.0;
const GAP_Y: f32 = 28.0;
/// Card padding, cover aspect (`50 / 77`), and the fixed-height parts under the cover.
const CARD_PAD: f32 = 10.0;
const COVER_RATIO: f32 = 77.0 / 50.0;
const TITLE_H: f32 = 34.0;
const DATE_H: f32 = 14.0;
const ACTION_H: f32 = 34.0;
/// Rows from the end at which the next page is requested.
const PREFETCH_ROWS: usize = 2;

/// `(columns, column width)` for `avail` px of grid width.
fn geometry(avail: f32) -> (usize, f32) {
    let cols = (((avail + GAP_X) / (MIN_COL + GAP_X)).floor() as usize).max(1);
    let col_w = ((avail - GAP_X * (cols as f32 - 1.0)) / cols as f32).max(100.0);
    (cols, col_w)
}

/// Height of one card for a column `col_w` wide.
fn card_height(col_w: f32) -> f32 {
    let cover_h = (col_w - CARD_PAD * 2.0) * COVER_RATIO;
    CARD_PAD * 2.0 + cover_h + 8.0 + TITLE_H + 4.0 + DATE_H + 4.0 + 6.0 + ACTION_H
}

pub struct StorePage {
    stores: Stores,
    input: Entity<TextInput>,
    _subs: Vec<Subscription>,
}

use gpui::Entity;

impl EventEmitter<Navigate> for StorePage {}

impl StorePage {
    pub fn new(stores: Stores, cx: &mut Context<Self>) -> Self {
        let query = stores.store.read(cx).query.clone();
        let input = cx.new(|cx| TextInput::new("Search for a comic…", cx).with_value(query));
        let subs = vec![
            cx.subscribe(&input, |this, input, ev: &TextInputEvent, cx| match ev {
                TextInputEvent::Changed => {
                    let text = input.read(cx).value().to_string();
                    this.stores.store.update(cx, |s, _| s.query = text);
                }
                TextInputEvent::Submit => this.stores.store.update(cx, |s, cx| s.run_search(cx)),
                TextInputEvent::Cancel => {}
            }),
            cx.observe(&stores.store, |_, _, cx| cx.notify()),
            cx.observe(&stores.thumbs, |_, _, cx| cx.notify()),
            // The source URL may be set in Settings while this page is open.
            cx.observe(&stores.settings, |this, _, cx| {
                this.load_if_configured(cx);
                cx.notify();
            }),
        ];
        let this = Self { stores, input, _subs: subs };
        this.load_if_configured(cx);
        this
    }

    /// Without a store source the server refuses every request: do not provoke an error toast on
    /// each visit, `render` shows a hint instead.
    fn is_configured(&self, cx: &gpui::App) -> bool {
        let s = self.stores.settings.read(cx);
        !(s.loaded && s.settings.api_url.trim().is_empty())
    }

    fn load_if_configured(&self, cx: &mut Context<Self>) {
        if self.is_configured(cx) {
            self.stores.store.update(cx, |s, cx| s.ensure_loaded(cx));
        }
    }
}

impl Render for StorePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let configured = self.is_configured(cx);
        let sidebar_collapsed = self.stores.prefs.read(cx).prefs.sidebar_collapsed;
        let (results, scroll, loading, loading_more, more_failed, error) = {
            let s = self.stores.store.read(cx);
            (s.results.clone(), s.scroll.clone(), s.loading, s.loading_more, s.more_failed, s.error.clone())
        };

        let search_row = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .max_w(px(480.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .h(px(36.0))
                    .px(px(14.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(theme::border_subtle())
                    .bg(theme::bg_panel())
                    .child(self.input.clone()),
            )
            .child(button(
                "store-search",
                "Search",
                ButtonVariant::Classic,
                cx.listener(|this, _, _, cx| this.stores.store.update(cx, |s, cx| s.run_search(cx))),
            ));

        let body = if !configured {
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(12.0))
                .py(px(20.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0x9a9a9a))
                        .child("The store source is not configured. Set it in Settings to browse and download comics."),
                )
                .child(button(
                    "store-open-settings",
                    "Open Settings",
                    ButtonVariant::Secondary,
                    cx.listener(|_, _, _, cx| cx.emit(Navigate(Route::Settings))),
                ))
                .into_any_element()
        } else if results.is_empty() {
            let text = if loading { "Loading…" } else { "No comics found." };
            div().text_size(px(13.0)).text_color(rgb(0x808080)).child(text).into_any_element()
        } else {
            let (cols, col_w) = geometry(available_width(window, !sidebar_collapsed));
            let row_h = card_height(col_w) + GAP_Y;
            let rows = results.len().div_ceil(cols);
            let stores = self.stores.clone();
            let list = uniform_list(
                "store-grid",
                rows,
                cx.processor(move |_this, range: std::ops::Range<usize>, _w, cx| {
                    // Near the end: ask for the next page (a no-op while one is loading or none is left).
                    if range.end + PREFETCH_ROWS >= rows {
                        stores.store.update(cx, |s, cx| s.load_more(cx));
                    }
                    range
                        .map(|row| {
                            let first = row * cols;
                            let last = (first + cols).min(results.len());
                            let cells: Vec<_> =
                                results[first..last].iter().map(|post| store_card(post, col_w, &stores, cx)).collect();
                            div().flex().items_start().gap(px(GAP_X)).h(px(row_h)).pb(px(GAP_Y)).children(cells)
                        })
                        .collect()
                }),
            )
            .track_scroll(&scroll)
            .size_full();
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(div().flex_1().min_h_0().child(list))
                .when(loading_more, |s| s.child(footer_note("Loading more…")))
                .when(more_failed, |s| {
                    s.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(10.0))
                            .py(px(8.0))
                            .child(footer_note("Could not load more comics."))
                            .child(button(
                                "store-retry-more",
                                "Retry",
                                ButtonVariant::Secondary,
                                cx.listener(|this, _, _, cx| this.stores.store.update(cx, |s, cx| s.retry_more(cx))),
                            )),
                    )
                })
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(px(20.0))
            .px(px(PAGE_PAD_X))
            .pt(px(24.0))
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0xf2f2f2))
                    .child("Store"),
            )
            .child(search_row)
            .when(!error.is_empty(), |s| s.child(div().text_size(px(12.0)).text_color(rgb(0xe05a5a)).child(error)))
            .child(div().flex_1().min_h_0().child(body))
    }
}

fn footer_note(text: &'static str) -> impl IntoElement {
    div().py(px(8.0)).text_center().text_size(px(12.0)).text_color(rgb(0x808080)).child(text)
}

/// One `StoreCard`: cover, two-line title, date and the Download / Retry button.
fn store_card(post: &StorePost, col_w: f32, stores: &Stores, cx: &mut Context<StorePage>) -> gpui::Div {
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

    let frame = div().w(px(inner_w)).h(px(inner_w * COVER_RATIO)).overflow_hidden().rounded(px(6.0)).bg(rgb(0x232222));
    let cover = match thumb {
        Some(Thumb::Ready(image)) => frame.child(img(image).size_full().object_fit(ObjectFit::Cover)),
        Some(Thumb::Missing) => frame
            .flex()
            .items_center()
            .justify_center()
            .child(icon(Icon::BookOpen, px(36.0)).text_color(rgb(0x5c5c5c))),
        // Loading: the plain dark frame stands in for the shimmer.
        _ => frame,
    };

    let (label, loading) = match &state {
        CardState::Error(_) => ("Retry", false),
        CardState::LinksLoading => ("Download", true),
        CardState::Idle => ("Download", false),
    };
    // The button keeps its look while links load, but dims and ignores clicks.
    let action = {
        let post = post.clone();
        let stores = stores.clone();
        let id: SharedString = format!("store-dl-{key}").into();
        let b = button(id, label, ButtonVariant::Classic, move |_, _, cx| {
            let post = post.clone();
            stores.store.update(cx, |s, cx| s.download(&post, cx));
        });
        if loading { b.opacity(0.6) } else { b }
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .flex_none()
        .w(px(col_w))
        .h(px(card_height(col_w)))
        .p(px(CARD_PAD))
        .child(cover)
        .child(
            div()
                .w_full()
                .mt(px(8.0))
                .h(px(TITLE_H))
                .overflow_hidden()
                .text_center()
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xffffff))
                .line_height(px(17.0))
                .line_clamp(2)
                .child(post.title.clone()),
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
                .child(post.upload_date.as_deref().map(display_date).unwrap_or_default()),
        )
        .child(div().mt(px(10.0)).flex().justify_center().child(action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_the_width() {
        assert_eq!(geometry(100.0).0, 1);
        // 4 × 180 + 3 × 22 = 786
        assert_eq!(geometry(786.0).0, 4);
        assert_eq!(geometry(785.0).0, 3);
    }

    #[test]
    fn columns_fill_the_available_width() {
        for avail in [400.0_f32, 786.0, 1100.0] {
            let (cols, w) = geometry(avail);
            assert!((cols as f32 * w + (cols as f32 - 1.0) * GAP_X - avail).abs() < 0.01);
        }
    }

    #[test]
    fn card_height_grows_with_the_cover() {
        assert!(card_height(240.0) > card_height(180.0));
    }
}
