//! Pull-list page: the Store feed, narrowed to the posts that match a followed series.
//!
//! It shows one Wednesday-anchored week at a time over the shared
//! [`StoreState`](crate::state::store::StoreState) feed. Pages are fetched until the loaded posts
//! reach back past the selected week, up to [`MAX_PAGES`].
//! Series are followed and unfollowed with the pin on a store card.

use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Subscription, UniformListScrollHandle,
    Window, div, prelude::*, px, rgb, uniform_list,
};

use crate::model::StorePost;
use crate::route::Navigate;
use crate::state::Stores;
use crate::state::pull_list::matches;
use crate::state::week;
use crate::ui::components::items_grid::{PAGE_PAD_X, available_width};
use crate::ui::components::store_card::{card_height, store_card};
use crate::ui::pages::store::{GAP_X, GAP_Y, geometry, is_configured};
use crate::ui::theme;

/// Bound on the automatic fetching, so a series that never matches cannot page through the site.
const MAX_PAGES: u32 = 30;

pub struct PullListPage {
    stores: Stores,
    scroll: UniformListScrollHandle,
    /// Day number of the selected week's Wednesday.
    week_start: i64,
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
            week_start: week::week_start(week::today()),
            _subs: subs,
        }
    }
}

fn nav_button(id: &'static str, label: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(10.0))
        .py(px(4.0))
        .rounded_md()
        .border_1()
        .border_color(theme::border_subtle())
        .text_size(px(12.0))
        .text_color(if enabled {
            theme::text()
        } else {
            rgb(0x555555)
        })
        .when(enabled, |d| d.cursor_pointer())
        .child(label)
}

impl PullListPage {
    fn navigator(&self, current_week: i64, cx: &mut Context<Self>) -> impl IntoElement {
        let at_current = self.week_start >= current_week;
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                nav_button("week-prev", "‹", true).on_click(cx.listener(|this, _, _, cx| {
                    this.week_start -= 7;
                    cx.notify();
                })),
            )
            .child(
                div()
                    .min_w(px(140.0))
                    .text_center()
                    .text_size(px(13.0))
                    .text_color(theme::text())
                    .child(week::range_label(self.week_start)),
            )
            .child(
                nav_button("week-next", "›", !at_current).when(!at_current, |d| {
                    d.on_click(cx.listener(|this, _, _, cx| {
                        this.week_start += 7;
                        cx.notify();
                    }))
                }),
            )
            .child(
                nav_button("week-now", "This week", !at_current).when(!at_current, |d| {
                    d.on_click(cx.listener(move |this, _, _, cx| {
                        this.week_start = current_week;
                        cx.notify();
                    }))
                }),
            )
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
        let week_start = self.week_start;
        let current_week = week::week_start(week::today());
        let in_selected_week = |p: &StorePost| {
            p.upload_date
                .as_deref()
                .and_then(week::parse_day)
                .is_some_and(|d| week::in_week(d, week_start))
        };
        let shown: Vec<StorePost> = results
            .iter()
            .filter(|p| in_selected_week(p) && entries.iter().any(|e| matches(e, &p.title)))
            .cloned()
            .collect();
        let oldest = results
            .iter()
            .filter_map(|p| p.upload_date.as_deref().and_then(week::parse_day))
            .min();

        // The loaded posts do not yet reach back past the week: keep fetching (a no-op while a page
        // is loading or none is left).
        if !entries.is_empty() && oldest.is_none_or(|o| o >= week_start) && page < MAX_PAGES {
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
                "No posts match this week."
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
            .child(self.navigator(current_week, cx))
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
