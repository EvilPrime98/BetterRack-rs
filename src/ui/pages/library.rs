//! Library page.
//!
//! The grid is virtualized: items are chunked into rows (the column count follows the available
//! width) and a `uniform_list` renders only the visible rows. Because only visible rows are built,
//! "scrolled into view" is "built", which is where covers and lazy identification are
//! requested.

use std::rc::Rc;

use gpui::{
    AnimationExt as _, Context, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, prelude::*, px, rgb,
};

use crate::model::{FilterOption, LibraryEntry, ReadFilter, StoreBookmark};
use crate::route::{GoBack, Navigate};
use crate::state::Stores;
use crate::state::library::apply_sort;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::items_grid::{
    GridScroll, PAGE_PAD_X, available_width, items_grid, merge_bookmarks,
};
use crate::ui::components::smooth_scroll::{SmoothScroll, wheel_capture};
use crate::ui::icons::{Icon, icon};
use crate::ui::pages::common::{actions_menu, note, prefs_dropdown, view_controls};
use crate::ui::text::capitalize_words;
use crate::ui::theme;

struct Memo {
    key: (u64, FilterOption),
    items: Rc<Vec<LibraryEntry>>,
}

struct VisibleMemo {
    base: Rc<Vec<LibraryEntry>>,
    read_filter: ReadFilter,
    indices: Rc<Vec<usize>>,
}

pub struct LibraryPage {
    uid: Option<String>,
    stores: Stores,
    scroll: GridScroll,
    smooth: SmoothScroll,
    memo: Option<Memo>,
    visible_memo: Option<VisibleMemo>,
}

impl EventEmitter<Navigate> for LibraryPage {}
impl EventEmitter<GoBack> for LibraryPage {}

impl LibraryPage {
    pub fn new(uid: Option<String>, stores: Stores, cx: &mut Context<Self>) -> Self {
        cx.observe(&stores.library, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.comics, |this, _, cx| {
            this.visible_memo = None;
            cx.notify();
        })
        .detach();
        cx.observe(&stores.thumbs, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.bookmarks, |_, _, cx| cx.notify())
            .detach();
        cx.observe(&stores.identify, |_, _, cx| cx.notify())
            .detach();
        cx.observe(&stores.prefs, |_, _, cx| cx.notify()).detach();
        stores.library.update(cx, |s, cx| s.fetch(cx).detach());
        Self {
            uid,
            stores,
            scroll: GridScroll::new(),
            smooth: SmoothScroll::default(),
            memo: None,
            visible_memo: None,
        }
    }

    fn visible_items(
        &mut self,
        base: &Rc<Vec<LibraryEntry>>,
        read_filter: ReadFilter,
        cx: &gpui::App,
    ) -> Rc<Vec<usize>> {
        if let Some(m) = &self.visible_memo {
            if Rc::ptr_eq(&m.base, base) && m.read_filter == read_filter {
                return m.indices.clone();
            }
        }
        let comics = self.stores.comics.read(cx);
        let indices = Rc::new(
            base.iter()
                .enumerate()
                .filter(|(_, i)| i.did || read_filter.matches(comics.read_per(&i.uid)))
                .map(|(ix, _)| ix)
                .collect::<Vec<_>>(),
        );
        self.visible_memo = Some(VisibleMemo {
            base: base.clone(),
            read_filter,
            indices: indices.clone(),
        });
        indices
    }

    fn bookmarks_here(&self, read_filter: ReadFilter, cx: &gpui::App) -> Vec<StoreBookmark> {
        let query = self
            .stores
            .library
            .read(cx)
            .search_query
            .trim()
            .to_lowercase();
        self.stores
            .bookmarks
            .read(cx)
            .in_folder(self.uid.as_deref())
            .into_iter()
            .filter(|b| read_filter.matches(if b.read { 100.0 } else { 0.0 }))
            .filter(|b| query.is_empty() || b.title.to_lowercase().contains(&query))
            .collect()
    }

    /// Sorted page items, recomputed only when the library, folder or sort changes.
    fn base_items(&mut self, cx: &gpui::App) -> Rc<Vec<LibraryEntry>> {
        let lib = self.stores.library.read(cx);
        let sort = self.stores.prefs.read(cx).prefs.filter;
        let key = (lib.revision, sort);
        if let Some(m) = &self.memo {
            if m.key == key {
                return m.items.clone();
            }
        }
        let mut items = lib.page_items(self.uid.as_deref());
        apply_sort(&mut items, sort);
        let items = Rc::new(items);
        self.memo = Some(Memo {
            key,
            items: items.clone(),
        });
        items
    }
}

impl Render for LibraryPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let base = self.base_items(cx);
        let scroll = self.scroll.clone();
        self.smooth.step(window, |dy| scroll.scroll_by(dy));
        let (read_filter, kind, sidebar_docked, sort) = {
            let p = self.stores.prefs.read(cx);
            (
                p.read_filter,
                p.prefs.comic_type,
                p.prefs.sidebar_docked(),
                p.prefs.filter,
            )
        };

        // Folders always show; comics only when they pass the read filter.
        let visible = self.visible_items(&base, read_filter, cx);

        let (loading, title) = {
            let lib = self.stores.library.read(cx);
            (lib.loading(), lib.title_for(self.uid.as_deref()))
        };
        let bookmarks = Rc::new(self.bookmarks_here(read_filter, cx));
        let count = visible.len() + bookmarks.len();

        let unread: Vec<String> = {
            let comics = self.stores.comics.read(cx);
            base.iter()
                .filter(|i| !i.did && comics.read_per(&i.uid) < 100.0)
                .map(|i| i.uid.clone())
                .collect()
        };

        let has_uid = self.uid.is_some();
        let header = div()
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .when(has_uid, |s| {
                        s.child(
                            div()
                                .id("page-back")
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(34.0))
                                .rounded(px(4.0))
                                .border_1()
                                .border_color(theme::border_subtle())
                                .bg(theme::bg_panel())
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x2a2a2a)))
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(GoBack)))
                                .child(icon(Icon::ArrowLeft, px(16.0)).text_color(theme::text())),
                        )
                    })
                    .child(
                        div()
                            .max_w(px(260.0))
                            .truncate()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(capitalize_words(&title)),
                    )
                    .child(prefs_dropdown(
                        "sort",
                        [
                            FilterOption::Alphabetically,
                            FilterOption::CreationDate,
                            FilterOption::ReleaseDate,
                        ]
                        .map(|v| (v, v.label())),
                        sort,
                        false,
                        &self.stores.prefs,
                        cx,
                        |p, v, cx| p.update(cx, |p| p.filter = v),
                    ))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0x9a9a9a))
                            .child(format!("{count} Comics")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(actions_menu(
                        "actions",
                        "Actions",
                        vec![(
                            "Mark entire folder as read",
                            !unread.is_empty(),
                            Rc::new(move |cx: &mut gpui::App| {
                                crate::ui::modals::open_mark_folder_read(cx, unread.clone())
                            }) as Rc<dyn Fn(&mut gpui::App)>,
                        )],
                        &self.stores.prefs,
                        cx,
                    ))
                    .child(button(
                        "new-folder",
                        "New Folder",
                        ButtonVariant::Secondary,
                        {
                            let parent = self.uid.clone();
                            move |_, _, cx| crate::ui::modals::open_new_folder(cx, parent.clone())
                        },
                    ))
                    .child(view_controls(
                        read_filter,
                        kind,
                        true,
                        &self.stores.prefs,
                        cx,
                    )),
            );

        let body = if visible.is_empty() && bookmarks.is_empty() {
            let msg = if loading && base.is_empty() {
                "Loading library…"
            } else {
                "No items to show."
            };
            note(msg).into_any_element()
        } else {
            let avail = available_width(window, sidebar_docked);
            div()
                .size_full()
                .child(items_grid(
                    base.clone(),
                    Rc::new(merge_bookmarks(&base, &visible, &bookmarks, sort)),
                    bookmarks,
                    kind,
                    &self.stores,
                    &self.scroll,
                    avail,
                    cx,
                ))
                .with_animation(
                    "library-grid-in",
                    crate::ui::motion::once(crate::ui::motion::BASE),
                    |el, t| el.opacity(t),
                )
                .into_any_element()
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
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .px(px(PAGE_PAD_X))
                    .pt(px(12.0))
                    .child(body)
                    .child(wheel_capture(cx, |this: &mut Self, dy, cx| {
                        this.smooth.push(dy);
                        cx.notify();
                    })),
            )
    }
}
