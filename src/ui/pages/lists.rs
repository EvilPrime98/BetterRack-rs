//! The grid pages that are not the folder browser: Recently added, Keep reading,
//! Filtered by writer and Search.
//! They share one entity because they differ only in where the items come from and in the header.
//!
//! Recent and Reading fetch a snapshot from the server and drop entries when the library store
//! broadcasts a delete (`last_deleted`); Filtered and Search derive their items from the loaded
//! library, so edits show up by themselves.

use std::rc::Rc;

use gpui::{
    Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::model::{
    FilterOption, LibraryEntry, RECENT_WINDOW_HOURS, RECENT_WINDOW_LABELS, ReadFilter,
};
use crate::route::{GoBack, Navigate, Route};
use crate::runtime;
use crate::state::Stores;
use crate::state::library::apply_sort;
use crate::ui::components::dropdown::dropdown;
use crate::ui::components::items_grid::{GridScroll, PAGE_PAD_X, available_width, items_grid};
use crate::ui::icons::{Icon, icon};
use crate::ui::pages::common::{back_button, counter, header_bar, note, summary, view_controls};
use crate::ui::theme;

#[derive(Debug, Clone)]
pub enum Source {
    Recent,
    Reading,
    /// Empty writer = every comic.
    Filtered {
        writer: String,
    },
    Search {
        query: String,
    },
}

enum Load {
    Loading,
    Failed(String),
    Ready,
}

pub struct ListPage {
    source: Source,
    stores: Stores,
    scroll: GridScroll,
    /// Index into `RECENT_WINDOW_*`.
    window_ix: usize,
    /// Server snapshot (Recent, Reading).
    fetched: Rc<Vec<LibraryEntry>>,
    load: Load,
    /// Discards responses of superseded requests.
    request: u64,
    seen_delete: u64,
    /// Items derived from the library, keyed by `LibraryStore::revision` (Filtered, Search).
    derived: Option<(u64, Rc<Vec<LibraryEntry>>)>,
}

impl EventEmitter<Navigate> for ListPage {}
impl EventEmitter<GoBack> for ListPage {}

impl ListPage {
    pub fn new(source: Source, stores: Stores, cx: &mut Context<Self>) -> Self {
        cx.observe(&stores.library, |this, _, cx| {
            this.drop_deleted(cx);
            cx.notify();
        })
        .detach();
        cx.observe(&stores.comics, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.thumbs, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.identify, |_, _, cx| cx.notify())
            .detach();
        cx.observe(&stores.prefs, |_, _, cx| cx.notify()).detach();

        let seen_delete = stores
            .library
            .read(cx)
            .last_deleted
            .as_ref()
            .map_or(0, |(n, _)| *n);
        let mut this = Self {
            source,
            stores,
            scroll: GridScroll::new(),
            window_ix: 0,
            fetched: Rc::new(Vec::new()),
            load: Load::Ready,
            request: 0,
            seen_delete,
            derived: None,
        };
        match &this.source {
            Source::Recent | Source::Reading => this.fetch(cx),
            Source::Search { query } => {
                // Keep the sidebar filter and this page in step, and refresh a stale library.
                let query = query.clone();
                this.stores.library.update(cx, |s, cx| {
                    s.set_search_query(query, cx);
                    s.fetch(cx).detach();
                });
            }
            Source::Filtered { .. } => this.stores.library.update(cx, |s, cx| s.fetch(cx).detach()),
        }
        this
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.stores.library.read(cx).client.clone() else {
            return;
        };
        self.request += 1;
        let request = self.request;
        self.load = Load::Loading;
        let reading = matches!(self.source, Source::Reading);
        let hours = RECENT_WINDOW_HOURS[self.window_ix];
        let flush = reading.then(|| self.stores.comics.update(cx, |s, cx| s.flush_pending(cx)));
        cx.notify();
        cx.spawn(async move |this, cx| {
            // Progress is debounced on the client; the server's "reading" list must see it first.
            if let Some(flush) = flush {
                flush.await;
            }
            let result = runtime::run(async move {
                if reading {
                    client.library_reading().await.map(|r| r.items)
                } else {
                    client.library_recent(hours).await.map(|r| r.items)
                }
            })
            .await;
            this.update(cx, |this, cx| {
                if this.request != request {
                    return;
                }
                match result {
                    Ok(items) => {
                        this.fetched = Rc::new(items);
                        this.load = Load::Ready;
                    }
                    Err(e) => {
                        tracing::warn!("loading list page failed: {e}");
                        this.load = Load::Failed(if reading {
                            "Comics in progress could not be loaded.".into()
                        } else {
                            "Recently added comics could not be loaded.".into()
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A delete goes through the library store: drop the entry from the snapshot so its card leaves
    /// without a reload.
    fn drop_deleted(&mut self, cx: &mut Context<Self>) {
        let Some((n, uid)) = self.stores.library.read(cx).last_deleted.clone() else {
            return;
        };
        if n <= self.seen_delete {
            return;
        }
        self.seen_delete = n;
        if self.fetched.iter().any(|e| e.uid == uid) {
            self.fetched = Rc::new(
                self.fetched
                    .iter()
                    .filter(|e| e.uid != uid)
                    .cloned()
                    .collect(),
            );
        }
    }

    fn items(&mut self, cx: &gpui::App) -> Rc<Vec<LibraryEntry>> {
        match &self.source {
            Source::Recent | Source::Reading => self.fetched.clone(),
            Source::Filtered { .. } | Source::Search { .. } => {
                let lib = self.stores.library.read(cx);
                if let Some((rev, items)) = &self.derived {
                    if *rev == lib.revision {
                        return items.clone();
                    }
                }
                let all = lib.items(false, None);
                let mut items: Vec<LibraryEntry> = match &self.source {
                    Source::Filtered { writer } => all
                        .into_iter()
                        .filter(|e| !e.did)
                        .filter(|e| {
                            writer.is_empty()
                                || e.comic
                                    .as_ref()
                                    .is_some_and(|c| c.writers().iter().any(|w| w == writer))
                        })
                        .collect(),
                    Source::Search { query } => {
                        let q = query.trim().to_lowercase();
                        all.into_iter()
                            .filter(|e| !e.did && e.name.to_lowercase().contains(&q))
                            .collect()
                    }
                    _ => unreachable!(),
                };
                if matches!(self.source, Source::Search { .. }) {
                    let sort = self.stores.prefs.read(cx).prefs.filter;
                    apply_sort(&mut items, sort);
                }
                let items = Rc::new(items);
                self.derived = Some((lib.revision, items.clone()));
                items
            }
        }
    }
}

impl Render for ListPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.items(cx);
        let (read_filter, kind, sidebar_collapsed) = {
            let p = self.stores.prefs.read(cx);
            (p.read_filter, p.prefs.comic_type, p.prefs.sidebar_collapsed)
        };
        let reading_only = matches!(self.source, Source::Reading);
        let visible: Rc<Vec<usize>> = {
            let comics = self.stores.comics.read(cx);
            Rc::new(
                items
                    .iter()
                    .enumerate()
                    .filter(|(_, i)| {
                        i.did || {
                            let rp = comics.read_per(&i.uid);
                            read_filter.matches(rp)
                                && (!reading_only || ReadFilter::Reading.matches(rp))
                        }
                    })
                    .map(|(ix, _)| ix)
                    .collect(),
            )
        };

        let (header, empty_text) = match &self.source {
            Source::Recent => {
                let label = RECENT_WINDOW_LABELS[self.window_ix];
                let header = header_bar(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(back_button("page-back", home_click(cx)))
                        .child(summary("Recently added", label)),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(view_controls(
                            read_filter,
                            kind,
                            true,
                            &self.stores.prefs,
                            cx,
                        ))
                        .child({
                            let page = cx.entity();
                            let prefs = &self.stores.prefs;
                            let for_open = prefs.clone();
                            dropdown(
                                "recent-window",
                                RECENT_WINDOW_LABELS
                                    .iter()
                                    .enumerate()
                                    .map(|(ix, l)| (ix, SharedString::from(*l)))
                                    .collect(),
                                self.window_ix,
                                prefs.read(cx).open_menu == Some("recent-window"),
                                true,
                                move |open, cx| {
                                    for_open.update(cx, |p, cx| {
                                        p.set_menu_open("recent-window", open, cx)
                                    })
                                },
                                move |ix, cx| {
                                    page.update(cx, |this, cx| {
                                        if this.window_ix != ix {
                                            this.window_ix = ix;
                                            this.scroll = GridScroll::new();
                                            this.fetch(cx);
                                        }
                                    })
                                },
                            )
                        }),
                );
                (
                    header,
                    format!("Nothing added in the {}.", label.to_lowercase()),
                )
            }
            Source::Reading => (
                header_bar(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(back_button("page-back", home_click(cx)))
                        .child(summary("Keep reading", "Currently reading")),
                    view_controls(read_filter, kind, false, &self.stores.prefs, cx),
                ),
                "Nothing in progress.".to_string(),
            ),
            Source::Filtered { writer } => {
                let clear = (!writer.is_empty()).then(|| {
                    div()
                        .id("clear-filter")
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .h(px(30.0))
                        .px(px(12.0))
                        .rounded_full()
                        .border_1()
                        .border_color(gpui::rgba(0xffffff24))
                        .text_size(px(12.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(gpui::rgba(0xffffff0f)))
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(Navigate(Route::Filtered {
                                writer: String::new(),
                            }))
                        }))
                        .child(icon(Icon::Close, px(12.0)).text_color(theme::text()))
                        .child("Clear filter")
                });
                let title = if writer.is_empty() {
                    "All comics".to_string()
                } else {
                    writer.clone()
                };
                (
                    header_bar(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(back_button("page-back", home_click(cx)))
                            .child(summary("Filtered by writer", title))
                            .children(clear),
                        view_controls(read_filter, kind, true, &self.stores.prefs, cx),
                    ),
                    "No comics in your library yet.".to_string(),
                )
            }
            Source::Search { query } => (
                header_bar(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(back_button("page-back", home_click(cx)))
                        .child(summary("Search", format!("\"{query}\"")))
                        .child(counter(visible.len())),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(view_controls(
                            read_filter,
                            kind,
                            true,
                            &self.stores.prefs,
                            cx,
                        )),
                ),
                "No items to show.".to_string(),
            ),
        };
        let _ = FilterOption::Alphabetically;

        let body = match &self.load {
            Load::Loading => note(match self.source {
                Source::Reading => "Loading comics in progress…",
                _ => "Loading recently added comics…",
            })
            .into_any_element(),
            Load::Failed(msg) => note(msg.clone()).into_any_element(),
            Load::Ready if visible.is_empty() => note(empty_text).into_any_element(),
            Load::Ready => {
                let avail = available_width(window, !sidebar_collapsed);
                items_grid(
                    items.clone(),
                    visible.clone(),
                    kind,
                    &self.stores,
                    &self.scroll,
                    avail,
                    cx,
                )
            }
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px(px(PAGE_PAD_X))
                    .pt(px(12.0))
                    .child(body),
            )
    }
}

/// Back button target: these pages go to `/`.
fn home_click(
    cx: &mut Context<ListPage>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
    cx.listener(|_, _, _, cx| cx.emit(Navigate(Route::home())))
}
