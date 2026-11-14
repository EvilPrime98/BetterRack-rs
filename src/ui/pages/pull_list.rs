//! Pull-list page: the Store feed, narrowed to the posts that match a followed series.
//!
//! It scrolls like the Store page (a virtualized grid that loads the next page near its end) over
//! the shared [`StoreState`](crate::state::store::StoreState) feed. Because most posts do not
//! match, pages are also fetched while too few matches are on screen, up to [`MAX_PAGES`].
//! Series are followed and unfollowed with the pin on a store card.

use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Subscription, UniformListScrollHandle,
    Window, div, prelude::*, px, rgb, uniform_list,
};

use crate::model::StorePost;
use crate::route::Navigate;
use crate::state::Stores;
use crate::state::pull_list::matches;
use crate::ui::components::items_grid::{PAGE_PAD_X, available_width};
use crate::ui::components::store_card::{card_height, store_card};
use crate::ui::pages::store::{GAP_X, GAP_Y, PREFETCH_ROWS, geometry, is_configured};

/// Matches wanted on screen before the page stops fetching on its own.
const FILL_TARGET: usize = 24;
/// Bound on the automatic fetching, so a series that never matches cannot page through the site.
const MAX_PAGES: u32 = 30;

pub struct PullListPage {
    stores: Stores,
    scroll: UniformListScrollHandle,
    _subs: Vec<Subscription>,
}

impl gpui::EventEmitter<Navigate> for PullListPage {}

impl PullListPage {
    pub fn new(stores: Stores, cx: &mut Context<Self>) -> Self {
        let subs = vec![
            cx.observe(&stores.pull_list, |_, _, cx| cx.notify()),
            cx.observe(&stores.store, |_, _, cx| cx.notify()),
            cx.observe(&stores.thumbs, |_, _, cx| cx.notify()),
            cx.observe(&stores.bookmarks, |_, _, cx| cx.notify()),
        ];
        if is_configured(&stores, cx) {
            // The feed is shared with the Store page: a leftover search would hide the latest posts.
            stores.store.update(cx, |s, cx| {
                if !s.active_search.is_empty() {
                    s.query.clear();
                    s.run_search(cx);
                } else {
                    s.ensure_loaded(cx);
                }
            });
        }
        Self {
            stores,
            scroll: UniformListScrollHandle::new(),
            _subs: subs,
        }
    }
}

fn note(text: &'static str) -> gpui::Div {
    div()
        .text_size(px(13.0))
        .text_color(rgb(0x808080))
        .child(text)
}

impl Render for PullListPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let configured = is_configured(&self.stores, cx);
        let sidebar_docked = self.stores.prefs.read(cx).prefs.sidebar_docked();
        let entries = self.stores.pull_list.read(cx).items.clone();
        let (results, loading, loading_more, page, error) = {
            let s = self.stores.store.read(cx);
            (
                s.results.clone(),
                s.is_loading(),
                s.loading_more(),
                s.page,
                s.error().unwrap_or_default().to_string(),
            )
        };
        let shown: Vec<StorePost> = results
            .iter()
            .filter(|p| entries.iter().any(|e| matches(e, &p.title)))
            .cloned()
            .collect();

        // Too few matches to scroll: keep fetching (a no-op while a page is loading or none is left).
        if !entries.is_empty() && shown.len() < FILL_TARGET && page < MAX_PAGES {
            self.stores.store.update(cx, |s, cx| s.load_more(cx));
        }

        let body = if !configured {
            note("The store source is not configured. Set it in Settings.").into_any_element()
        } else if entries.is_empty() {
            note("Use the pin on a store card to follow a series.").into_any_element()
        } else if shown.is_empty() {
            let text = if loading || loading_more {
                "Loading…"
            } else {
                "No posts match your pull list yet."
            };
            note(text).into_any_element()
        } else {
            let (cols, col_w) = geometry(available_width(window, sidebar_docked));
            let row_h = card_height(col_w) + GAP_Y;
            let rows = shown.len().div_ceil(cols);
            let stores = self.stores.clone();
            let list = uniform_list(
                "pull-grid",
                rows,
                cx.processor(move |_this, range: std::ops::Range<usize>, _w, cx| {
                    // Scroll-driven paging only once there is a list to scroll; a sparse list is
                    // filled (and bounded) by the fetch in `render`.
                    if shown.len() >= FILL_TARGET && range.end + PREFETCH_ROWS >= rows {
                        stores.store.update(cx, |s, cx| s.load_more(cx));
                    }
                    range
                        .map(|row| {
                            let first = row * cols;
                            let last = (first + cols).min(shown.len());
                            let cells: Vec<_> = shown[first..last]
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
            .track_scroll(&self.scroll)
            .size_full();
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(div().flex_1().min_h_0().child(list))
                .child(
                    div()
                        .h(px(30.0))
                        .flex_none()
                        .py(px(8.0))
                        .text_center()
                        .when(loading_more, |d| d.child(note("Loading more…"))),
                )
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
                    .child("Pull list"),
            )
            .when(!error.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xe05a5a))
                        .child(error),
                )
            })
            .child(div().flex_1().min_h_0().child(body))
    }
}
