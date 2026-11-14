//! Store page. Title, search box and a
//! virtualized grid of posts that loads the next page when its last rows come into view.
//!
//! The data (query, results, page, scroll position, per-card download state) lives in
//! [`StoreState`](crate::state::store::StoreState), so it survives leaving the page. Clicking a
//! card's Download runs links → (link picker) → download folder → job; see `StoreState::download`.

use gpui::{
    AppContext as _, Context, EventEmitter, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, div, prelude::*, px, rgb, uniform_list,
};

use crate::route::{Navigate, Route};
use crate::state::Stores;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::items_grid::{PAGE_PAD_X, available_width};
use crate::ui::components::store_card::{card_height, store_card};
use crate::ui::components::text_input::{TextInput, TextInputEvent};
use crate::ui::theme;

/// Minimum card width and the gaps of `.grid` in `store.page.module.css`.
const MIN_COL: f32 = 180.0;
pub(crate) const GAP_X: f32 = 22.0;
pub(crate) const GAP_Y: f32 = 28.0;
/// Rows from the end at which the next page is requested.
pub(crate) const PREFETCH_ROWS: usize = 2;

/// `(columns, column width)` for `avail` px of grid width.
pub(crate) fn geometry(avail: f32) -> (usize, f32) {
    let cols = (((avail + GAP_X) / (MIN_COL + GAP_X)).floor() as usize).max(1);
    let col_w = ((avail - GAP_X * (cols as f32 - 1.0)) / cols as f32).max(100.0);
    (cols, col_w)
}

/// Without a store source the server refuses every request: do not provoke an error toast on
/// each visit, the pages show a hint instead.
pub(crate) fn is_configured(stores: &Stores, cx: &gpui::App) -> bool {
    let s = stores.settings.read(cx);
    !(s.loaded && s.settings.api_url.trim().is_empty())
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
            cx.observe(&stores.bookmarks, |_, _, cx| cx.notify()),
            // The source URL may be set in Settings while this page is open.
            cx.observe(&stores.settings, |this, _, cx| {
                this.load_if_configured(cx);
                cx.notify();
            }),
        ];
        let this = Self {
            stores,
            input,
            _subs: subs,
        };
        this.load_if_configured(cx);
        this
    }

    fn load_if_configured(&self, cx: &mut Context<Self>) {
        if is_configured(&self.stores, cx) {
            self.stores.store.update(cx, |s, cx| s.ensure_loaded(cx));
        }
    }
}

impl Render for StorePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let configured = is_configured(&self.stores, cx);
        let sidebar_docked = self.stores.prefs.read(cx).prefs.sidebar_docked();
        let (results, scroll, loading, loading_more, more_failed, error) = {
            let s = self.stores.store.read(cx);
            (
                s.results.clone(),
                s.scroll.clone(),
                s.is_loading(),
                s.loading_more(),
                s.more_failed(),
                s.error().unwrap_or_default().to_string(),
            )
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
                cx.listener(|this, _, _, cx| {
                    this.stores.store.update(cx, |s, cx| s.run_search(cx))
                }),
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
            let text = if loading {
                "Loading…"
            } else {
                "No comics found."
            };
            div()
                .text_size(px(13.0))
                .text_color(rgb(0x808080))
                .child(text)
                .into_any_element()
        } else {
            let (cols, col_w) = geometry(available_width(window, sidebar_docked));
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
                            let cells: Vec<_> = results[first..last]
                                .iter()
                                .map(|post| store_card(post, col_w, &stores, cx))
                                .collect();
                            div()
                                .flex()
                                .items_start()
                                .gap(px(GAP_X))
                                .h(px(row_h))
                                .pb(px(GAP_Y))
                                .children(cells)
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
                                cx.listener(|this, _, _, cx| {
                                    this.stores.store.update(cx, |s, cx| s.retry_more(cx))
                                }),
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
            .when(!error.is_empty(), |s| {
                s.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xe05a5a))
                        .child(error),
                )
            })
            .child(div().flex_1().min_h_0().child(body))
    }
}

fn footer_note(text: &'static str) -> impl IntoElement {
    div()
        .py(px(8.0))
        .text_center()
        .text_size(px(12.0))
        .text_color(rgb(0x808080))
        .child(text)
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
}
