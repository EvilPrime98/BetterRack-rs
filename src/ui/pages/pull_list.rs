//! Pull-list page: the Store feed, narrowed to the posts that match a followed series.
//!
//! It shows one Wednesday-anchored week at a time. The week's posts are fetched with the store's
//! date window (`after`/`before`), page by page, and filtered to the followed series.
//! Series are followed and unfollowed with the pin on a store card.

use std::rc::Rc;

use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Subscription, UniformListScrollHandle,
    Window, div, prelude::*, px, rgb, uniform_list,
};

use crate::model::StorePost;
use crate::route::Navigate;
use crate::runtime;
use crate::state::Stores;
use crate::state::epoch::Epoch;
use crate::state::pull_list::matches;
use crate::state::store::append_unique;
use crate::state::week;
use crate::ui::components::items_grid::{PAGE_PAD_X, available_width};
use crate::ui::components::store_card::{card_height, store_card};
use crate::ui::pages::store::{GAP_X, GAP_Y, geometry, is_configured};
use crate::ui::{theme, toast};

/// The WordPress maximum, so a busy week fits in a couple of requests.
const WEEK_PAGE_SIZE: usize = 100;
/// Bound on the paging of one week.
const MAX_PAGES: u32 = 10;

#[derive(Debug, Clone, PartialEq)]
enum WeekFeed {
    Loading,
    Ready,
    Failed(String),
}

pub struct PullListPage {
    stores: Stores,
    scroll: UniformListScrollHandle,
    /// Day number of the selected week's Wednesday.
    week_start: i64,
    /// Every post of the selected week, followed or not.
    posts: Rc<Vec<StorePost>>,
    feed: WeekFeed,
    /// Bumped by every week change so a late answer for an older week is dropped.
    epoch: Epoch,
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
        let mut page = Self {
            stores,
            scroll: UniformListScrollHandle::new(),
            week_start: week::week_start(week::today()),
            posts: Rc::new(Vec::new()),
            feed: WeekFeed::Loading,
            epoch: Epoch::default(),
            _subs: subs,
        };
        page.load_week(cx);
        page
    }

    fn select_week(&mut self, week_start: i64, cx: &mut Context<Self>) {
        self.week_start = week_start;
        self.load_week(cx);
    }

    /// Fetch every post of the selected week.
    fn load_week(&mut self, cx: &mut Context<Self>) {
        self.posts = Rc::new(Vec::new());
        let client = self
            .stores
            .store
            .read(cx)
            .client
            .clone()
            .filter(|_| is_configured(&self.stores, cx));
        let Some(client) = client else {
            self.feed = WeekFeed::Ready;
            cx.notify();
            return;
        };
        let ticket = self.epoch.next();
        self.feed = WeekFeed::Loading;
        cx.notify();
        let after = week::day_start(self.week_start);
        let before = week::day_start(self.week_start + 7);
        cx.spawn(async move |this, cx| {
            let result = runtime::run(async move {
                let mut all = Vec::new();
                for page in 1..=MAX_PAGES {
                    let items = client
                        .store_posts_between(&after, &before, page, WEEK_PAGE_SIZE)
                        .await
                        .map_err(|e| e.to_string())?;
                    let last = items.len() < WEEK_PAGE_SIZE;
                    all = append_unique(&all, items);
                    if last {
                        break;
                    }
                }
                Ok::<_, String>(all)
            })
            .await;
            this.update(cx, |s, cx| {
                if !s.epoch.is_current(ticket) {
                    return;
                }
                match result {
                    Ok(items) => {
                        s.posts = Rc::new(items);
                        s.feed = WeekFeed::Ready;
                    }
                    Err(message) => {
                        toast::error(cx, message.clone());
                        s.feed = WeekFeed::Failed(message);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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
                    this.select_week(this.week_start - 7, cx);
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
                        this.select_week(this.week_start + 7, cx);
                    }))
                }),
            )
            .child(
                nav_button("week-now", "This week", !at_current).when(!at_current, |d| {
                    d.on_click(cx.listener(move |this, _, _, cx| {
                        this.select_week(current_week, cx);
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
        let loading = self.feed == WeekFeed::Loading;
        let error = match &self.feed {
            WeekFeed::Failed(m) => m.clone(),
            _ => String::new(),
        };
        let current_week = week::week_start(week::today());
        let shown: Vec<StorePost> = self
            .posts
            .iter()
            .filter(|p| entries.iter().any(|e| matches(e, &p.title)))
            .cloned()
            .collect();

        let body = if !configured {
            note("The store source is not configured. Set it in Settings.").into_any_element()
        } else if entries.is_empty() {
            note("Use the pin on a store card to follow a series.").into_any_element()
        } else if shown.is_empty() {
            note(if loading {
                "Loading…"
            } else {
                "No posts match this week."
            })
            .into_any_element()
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
