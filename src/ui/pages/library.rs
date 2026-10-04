//! Library page.
//!
//! The grid is virtualized: items are chunked into rows (the column count follows the available
//! width) and a `uniform_list` renders only the visible rows. Because only visible rows are built,
//! "scrolled into view" is "built", which is where covers and lazy identification are
//! requested.

use std::rc::Rc;

use gpui::{
    Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, div, prelude::*, px, rgb,
};

use crate::model::{FilterOption, LibraryEntry};
use crate::route::{GoBack, Navigate};
use crate::state::Stores;
use crate::state::library::apply_sort;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::items_grid::{PAGE_PAD_X, available_width, items_grid};
use crate::ui::components::smooth_scroll::{SmoothScroll, wheel_capture};
use crate::ui::icons::{Icon, icon};
use crate::ui::pages::common::{note, prefs_dropdown, view_controls};
use crate::ui::theme;

struct Memo {
    key: (u64, FilterOption),
    items: Rc<Vec<LibraryEntry>>,
}

pub struct LibraryPage {
    uid: Option<String>,
    stores: Stores,
    scroll: UniformListScrollHandle,
    smooth: SmoothScroll,
    memo: Option<Memo>,
}

impl EventEmitter<Navigate> for LibraryPage {}
impl EventEmitter<GoBack> for LibraryPage {}

impl LibraryPage {
    pub fn new(uid: Option<String>, stores: Stores, cx: &mut Context<Self>) -> Self {
        cx.observe(&stores.library, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.comics, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.thumbs, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.identify, |_, _, cx| cx.notify())
            .detach();
        cx.observe(&stores.prefs, |_, _, cx| cx.notify()).detach();
        // `useEffect(() => fetchLibrary(), [uid])`: a stale cache is refreshed on every visit.
        stores.library.update(cx, |s, cx| s.fetch(cx).detach());
        Self {
            uid,
            stores,
            scroll: UniformListScrollHandle::new(),
            smooth: SmoothScroll::default(),
            memo: None,
        }
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
        let base_handle = self.scroll.0.borrow().base_handle.clone();
        self.smooth.step(&base_handle, window);
        let (read_filter, kind, sidebar_collapsed, sort) = {
            let p = self.stores.prefs.read(cx);
            (
                p.read_filter,
                p.prefs.comic_type,
                p.prefs.sidebar_collapsed,
                p.prefs.filter,
            )
        };

        // Folders always show; comics only when they pass the read filter.
        let visible: Rc<Vec<usize>> = {
            let comics = self.stores.comics.read(cx);
            Rc::new(
                base.iter()
                    .enumerate()
                    .filter(|(_, i)| i.did || read_filter.matches(comics.read_per(&i.uid)))
                    .map(|(ix, _)| ix)
                    .collect(),
            )
        };

        let (loading, title) = {
            let lib = self.stores.library.read(cx);
            (lib.loading, lib.title_for(self.uid.as_deref()))
        };
        let count = visible.len();

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
                            .child(title),
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

        let body = if visible.is_empty() {
            let msg = if loading && base.is_empty() {
                "Loading library…"
            } else {
                "No items to show."
            };
            note(msg).into_any_element()
        } else {
            let avail = available_width(window, !sidebar_collapsed);
            items_grid(
                "library-grid",
                base.clone(),
                visible.clone(),
                kind,
                &self.stores,
                &self.scroll,
                avail,
                cx,
            )
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
