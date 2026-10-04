//! Reader (`pages/reader.page.tsx` + `hooks/useReader*.ts`): continuous vertical scroll of page
//! images, resume position, zoom, bookmarks, progress saving.
//!
//! **Page numbering.** The server is 1-based with `totalPages = N`. The React viewer rendered only
//! `N-1` images (`ind = 1..N-1`) and labelled them `2..N`, which dropped the last image and shifted
//! every label by one. This port uses a clean 1..N model: item `i` is server page `i + 1`.
//!
//! The viewer is a GPUI variable-height `list`. Each page's height is `width * aspect`, where the
//! aspect comes from the image header (`PageImages`) and defaults to [`DEFAULT_ASPECT`] until known.

use gpui::{
    AnyElement, App, Context, DispatchPhase, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ListAlignment, ListOffset, ListState, ObjectFit, ParentElement, Render,
    ScrollDelta, ScrollHandle, ScrollWheelEvent, StatefulInteractiveElement, Styled, StyledImage as _, Task, Window,
    WindowControlArea, canvas, div, img, list, prelude::*, px, relative, rgb, rgba,
};

use crate::api::ApiClient;
use crate::model::{Bookmark, LibraryEntry};
use crate::route::{GoBack, Navigate, Route};
use crate::runtime;
use crate::state::Stores;
use crate::state::reader_images::{PageImages, PageImg};
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::icons::{Icon, icon};
use crate::ui::shell::window_controls::window_controls;
use crate::ui::theme;

pub const BASE_WIDTH: f32 = 900.0;
pub const ZOOM_MIN: f32 = 0.5;
pub const ZOOM_STEP: f32 = 0.1;
/// Page height / width used until the real image has been seen.
pub const DEFAULT_ASPECT: f32 = 1.5;
/// Pages loaded on each side of the saved page before the reader opens (`PRELOAD_WINDOW`).
pub const PRELOAD_WINDOW: u32 = 2;
const TOOLBAR_HEIGHT: f32 = 56.0;
/// Pixels per wheel line for notched (non-pixel) wheels.
const WHEEL_LINE_PX: f32 = 24.0;
/// Time constant of the scroll easing: ~63 % of the remaining distance is covered per `TAU`.
const SCROLL_TAU: f32 = 0.08;

/// `floor(min(3, viewerWidth / 900) * 100) / 100`, never below 1 (`useReaderZoom`).
pub fn max_zoom(viewer_w: f32) -> f32 {
    (((viewer_w / BASE_WIDTH).min(3.0) * 100.0).floor() / 100.0).max(1.0)
}

pub fn clamp_zoom(zoom: f32, viewer_w: f32) -> f32 {
    ((zoom * 100.0).round() / 100.0).clamp(ZOOM_MIN, max_zoom(viewer_w))
}

pub fn page_width(zoom: f32, viewer_w: f32) -> f32 {
    (BASE_WIDTH * zoom).min(viewer_w.max(1.0))
}

/// Pages (1-based, inclusive) to warm around `saved`.
pub fn preload_range(total: u32, saved: u32) -> Option<std::ops::RangeInclusive<u32>> {
    if total == 0 {
        return None;
    }
    let saved = saved.clamp(1, total);
    Some(saved.saturating_sub(PRELOAD_WINDOW).max(1)..=(saved + PRELOAD_WINDOW).min(total))
}

/// `(readPer, read)` for being on `page` of `total`: `readPer = round2(page / total * 100)`.
pub fn progress(page: u32, total: u32) -> (f32, bool) {
    let total = total.max(1);
    let per = (page as f32 / total as f32 * 100.0 * 100.0).round() / 100.0;
    (per, page >= total)
}

/// The item (0-based) with the largest visible area. `top_ix`/`offset` come from the list's scroll
/// position; `height(ix)` is the item height. When the last item's bottom is on screen the last
/// page wins, so the reader can reach 100 % even if the previous page still covers more.
pub fn most_visible(
    top_ix: usize,
    offset: f32,
    viewport_h: f32,
    total: usize,
    height: impl Fn(usize) -> f32,
) -> usize {
    if total == 0 {
        return 0;
    }
    let top_ix = top_ix.min(total - 1);
    let mut y = -offset;
    let mut best = (top_ix, -1.0_f32);
    let mut ix = top_ix;
    while ix < total && y < viewport_h {
        let h = height(ix);
        if ix == total - 1 && y + h <= viewport_h + 1.0 {
            return ix;
        }
        let overlap = ((y + h).min(viewport_h) - y.max(0.0)).max(0.0);
        if overlap > best.1 + 0.5 {
            best = (ix, overlap);
        }
        y += h;
        ix += 1;
    }
    best.0
}

enum Load {
    Loading,
    Error(String),
    Ready,
}

pub struct ReaderPage {
    uid: String,
    stores: Stores,
    images: gpui::Entity<PageImages>,
    load: Load,
    total: u32,
    bookmarks: Vec<Bookmark>,
    list: Option<ListState>,
    /// 1-based page currently most visible.
    current: u32,
    /// Page the reader opened on. Progress is only written once the reader has moved off it, so
    /// only opening a comic (e.g. one marked read) never rewrites its progress.
    initial: u32,
    touched: bool,
    header_visible: bool,
    bookmarks_open: bool,
    bookmarks_scroll: ScrollHandle,
    refreshing: bool,
    zoom: f32,
    viewer_w: f32,
    viewer_h: f32,
    last_page_w: f32,
    /// Pixels still to scroll (positive = down); eased out a little every frame.
    pending_scroll: f32,
    last_frame: Option<std::time::Instant>,
    focus: FocusHandle,
    _task: Option<Task<()>>,
}

impl EventEmitter<Navigate> for ReaderPage {}
impl EventEmitter<GoBack> for ReaderPage {}

impl ReaderPage {
    pub fn new(uid: String, stores: Stores, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let client = stores.comics.read(cx).client.clone();
        let images = cx.new(|_| PageImages::new(uid.clone(), client));
        cx.observe(&images, |this, images, cx| {
            // A page's real size is known: its list item must be measured again.
            let sized = images.update(cx, |s, _| s.take_newly_sized());
            if let Some(list) = &this.list {
                for p in sized {
                    list.remeasure_items(p as usize - 1..p as usize);
                }
            }
            cx.notify();
        })
        .detach();
        cx.observe(&stores.library, |_, _, cx| cx.notify()).detach();

        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let zoom = stores.prefs.read(cx).prefs.zoom;
        let size = window.viewport_size();
        let mut this = Self {
            uid,
            stores,
            images,
            load: Load::Loading,
            total: 0,
            bookmarks: Vec::new(),
            list: None,
            current: 1,
            initial: 1,
            touched: false,
            header_visible: true,
            bookmarks_open: false,
            bookmarks_scroll: ScrollHandle::new(),
            refreshing: false,
            zoom,
            viewer_w: f32::from(size.width),
            viewer_h: f32::from(size.height),
            last_page_w: 0.0,
            pending_scroll: 0.0,
            last_frame: None,
            focus,
            _task: None,
        };
        this.load(cx);
        this
    }

    /// `useComicPages.loadPages`: wait for the comic cache, list pages, warm the pages around the
    /// saved position, then show the viewer scrolled there. Bookmarks are optional.
    fn load(&mut self, cx: &mut Context<Self>) {
        self.load = Load::Loading;
        let Some(client) = self.stores.comics.read(cx).client.clone() else {
            self.load = Load::Error("Server is not ready".into());
            return;
        };
        let uid = self.uid.clone();
        let comics = self.stores.comics.clone();
        let images = self.images.clone();
        cx.notify();
        self._task = Some(cx.spawn(async move |this, cx| {
            for _ in 0..100 {
                if cx.update(|cx| comics.read(cx).ready) {
                    break;
                }
                cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
            let saved = cx.update(|cx| comics.read(cx).get(&uid).map(|c| c.current_page).unwrap_or(0));

            let (c, id) = (client.clone(), uid.clone());
            let pages = runtime::run(async move { c.reader_pages(&id).await }).await;
            let total = match pages {
                Ok(p) if !p.pages.is_empty() => p.pages.len() as u32,
                Ok(_) => return fail(&this, cx, "No pages found in comic".into()),
                Err(e) => return fail(&this, cx, e.to_string()),
            };
            let saved = saved.clamp(1, total);
            if let Some(range) = preload_range(total, saved) {
                let warm = cx.update(|cx| images.update(cx, |s, cx| s.load_window(range, saved, cx)));
                warm.await;
            }
            let bookmarks = fetch_bookmarks(client, uid).await;
            this.update(cx, |t, cx| t.apply_pages(total, saved, bookmarks, cx)).ok();
        }));
    }

    /// Install a fresh page list and scroll to `page`.
    fn apply_pages(&mut self, total: u32, page: u32, bookmarks: Vec<Bookmark>, cx: &mut Context<Self>) {
        let list = ListState::new(total as usize, ListAlignment::Top, px(1500.0));
        list.scroll_to(ListOffset { item_ix: page as usize - 1, offset_in_item: px(0.0) });
        self.list = Some(list);
        self.total = total;
        self.current = page;
        self.initial = page;
        self.touched = false;
        self.bookmarks = bookmarks;
        self.load = Load::Ready;
        self.refreshing = false;
        self.last_page_w = 0.0;
        cx.notify();
    }

    /// Re-list pages (evicts the server's archive cache), keeping the reading position.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        let Some(client) = self.stores.comics.read(cx).client.clone() else { return };
        self.refreshing = true;
        let (uid, page) = (self.uid.clone(), self.current);
        let images = self.images.clone();
        cx.notify();
        self._task = Some(cx.spawn(async move |this, cx| {
            let (c, id) = (client.clone(), uid.clone());
            match runtime::run(async move { c.reader_refresh(&id).await }).await {
                Ok(p) if !p.pages.is_empty() => {
                    let total = p.pages.len() as u32;
                    let bookmarks = fetch_bookmarks(client, uid).await;
                    cx.update(|cx| images.update(cx, |s, _| s.reset()));
                    this.update(cx, |t, cx| t.apply_pages(total, page.min(total), bookmarks, cx)).ok();
                }
                Ok(_) => fail(&this, cx, "No pages found in comic".into()),
                Err(e) => fail(&this, cx, e.to_string()),
            }
        }));
    }

    /// Queue `dy` pixels (positive = down) to be scrolled with easing instead of in one jump.
    fn scroll_smooth(&mut self, dy: f32, cx: &mut Context<Self>) {
        let cap = self.viewer_h * 4.0;
        self.pending_scroll = (self.pending_scroll + dy).clamp(-cap, cap);
        cx.notify();
    }

    /// Apply this frame's share of the pending scroll and ask for another frame while any is left.
    fn step_scroll(&mut self, list: &ListState, window: &mut Window) {
        if self.pending_scroll == 0.0 {
            self.last_frame = None;
            return;
        }
        let now = std::time::Instant::now();
        let dt = self.last_frame.map_or(1.0 / 60.0, |t| (now - t).as_secs_f32()).min(0.05);
        self.last_frame = Some(now);
        let step = if self.pending_scroll.abs() < 0.5 {
            self.pending_scroll
        } else {
            self.pending_scroll * (1.0 - (-dt / SCROLL_TAU).exp())
        };
        self.pending_scroll -= step;
        list.scroll_by(px(step));
        if self.pending_scroll != 0.0 {
            window.request_animation_frame();
        }
    }

    fn go_to_page(&mut self, page: u32, cx: &mut Context<Self>) {
        self.pending_scroll = 0.0;
        if let Some(list) = &self.list {
            let page = page.clamp(1, self.total.max(1));
            list.scroll_to(ListOffset { item_ix: page as usize - 1, offset_in_item: px(0.0) });
            self.bookmarks_open = false;
            cx.notify();
        }
    }

    fn zoom_to(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let zoom = clamp_zoom(zoom, self.viewer_w);
        if (zoom - self.zoom).abs() < 1e-4 {
            return;
        }
        self.zoom = zoom;
        self.stores.prefs.update(cx, |p, cx| p.update(cx, |p| p.zoom = zoom));
        cx.notify();
    }

    fn zoom_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        let current = self.zoom.min(max_zoom(self.viewer_w));
        self.zoom_to(current + delta, cx);
    }

    /// `useReaderProgress`: every page change is written through the debounced comic cache.
    fn on_page_changed(&mut self, cx: &mut Context<Self>) {
        if !self.touched && self.current == self.initial {
            return;
        }
        self.touched = true;
        let (page, total) = (self.current, self.total);
        let (per, read) = progress(page, total);
        let uid = self.uid.clone();
        self.stores.comics.update(cx, |s, cx| {
            s.update(&uid, cx, |c| {
                c.read_per = per;
                c.current_page = page;
                c.read = read;
            })
        });
    }

    /// `ReaderNext`: the entry after this one in its folder.
    fn next_entry(&self, cx: &App) -> Option<LibraryEntry> {
        let lib = self.stores.library.read(cx);
        let entry = lib.find_entry(&self.uid)?;
        let parent = (!entry.parent_id.is_empty()).then(|| entry.parent_id.clone());
        let siblings = lib.items(false, parent.as_deref());
        let ix = siblings.iter().position(|e| e.uid == self.uid)?;
        siblings.into_iter().skip(ix + 1).find(|e| !e.did)
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let m = &ev.keystroke.modifiers;
        if m.secondary() {
            match key {
                "=" | "+" | "add" => self.zoom_by(ZOOM_STEP, cx),
                "-" | "subtract" => self.zoom_by(-ZOOM_STEP, cx),
                "0" => self.zoom_to(1.0, cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        let page_px = self.viewer_h * 0.9;
        match key {
            "escape" if self.bookmarks_open => {
                self.bookmarks_open = false;
                cx.notify();
            }
            "escape" => cx.emit(GoBack),
            "pagedown" => self.scroll_smooth(page_px, cx),
            "space" if m.shift => self.scroll_smooth(-page_px, cx),
            "space" => self.scroll_smooth(page_px, cx),
            "pageup" => self.scroll_smooth(-page_px, cx),
            "down" => self.scroll_smooth(80.0, cx),
            "up" => self.scroll_smooth(-80.0, cx),
            "home" => self.go_to_page(1, cx),
            "end" => {
                self.pending_scroll = 0.0;
                if let Some(l) = &self.list {
                    l.scroll_to_end();
                }
            }
            _ => return,
        }
        let _ = window;
        cx.stop_propagation();
    }
}

fn fail(this: &gpui::WeakEntity<ReaderPage>, cx: &mut gpui::AsyncApp, message: String) {
    this.update(cx, |t, cx| {
        t.load = Load::Error(message);
        t.refreshing = false;
        cx.notify();
    })
    .ok();
}

async fn fetch_bookmarks(client: ApiClient, uid: String) -> Vec<Bookmark> {
    runtime::run(async move { client.bookmarks(&uid).await }).await.unwrap_or_default()
}

impl Focusable for ReaderPage {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn tool_button(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.0))
        .h(px(36.0))
        .px(px(12.0))
        .rounded(px(8.0))
        .text_size(px(13.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
}

impl ReaderPage {
    fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let spinning = self.refreshing;
        div()
            .flex()
            .items_center()
            .h(px(TOOLBAR_HEIGHT))
            .w_full()
            .pl(px(8.0))
            .gap(px(4.0))
            .child(
                tool_button("reader-back")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(GoBack)))
                    .child(icon(Icon::ArrowLeft, px(16.0)).text_color(theme::text()))
                    .child("Library"),
            )
            .when(!self.bookmarks.is_empty(), |s| {
                s.child(
                    tool_button("reader-bookmarks")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.bookmarks_open = !this.bookmarks_open;
                            cx.notify();
                        }))
                        .child(icon(Icon::Bookmark, px(16.0)).text_color(theme::text()))
                        .child("Bookmarks"),
                )
            })
            // Drag region between the controls and the window controls.
            .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .px(px(8.0))
                    .child(format!("{} / {}", self.current, self.total)),
            )
            .child(
                tool_button("reader-refresh")
                    .opacity(if spinning { 0.4 } else { 1.0 })
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))
                    .child(icon(Icon::Refresh, px(16.0)).text_color(theme::text())),
            )
            .children(window_controls(window))
    }

    fn header(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frac = if self.total == 0 { 0.0 } else { self.current as f32 / self.total as f32 };
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            // The viewer underneath toggles the header on click; keep clicks here from reaching it.
            .block_mouse_except_scroll()
            .bg(rgba(0x313238f0))
            .border_b_1()
            .border_color(theme::border_subtle())
            .child(self.toolbar(window, cx))
            .child(
                div()
                    .h(px(3.0))
                    .w_full()
                    .bg(rgba(0xffffff1a))
                    .child(div().h_full().w(relative(frac)).bg(theme::accent())),
            )
            .when(self.bookmarks_open, |s| s.child(self.bookmark_menu(cx)))
    }

    fn bookmark_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Thumb geometry from the tracked scroll handle (all zero until the first layout).
        let view_h = f32::from(self.bookmarks_scroll.bounds().size.height);
        let max = f32::from(self.bookmarks_scroll.max_offset().y);
        let thumb = (view_h > 0.0 && max > 0.5).then(|| {
            let thumb_h = (view_h * view_h / (view_h + max)).max(24.0);
            let scrolled = (-f32::from(self.bookmarks_scroll.offset().y) / max).clamp(0.0, 1.0);
            div()
                .absolute()
                .right(px(2.0))
                .top(px(scrolled * (view_h - thumb_h)))
                .w(px(4.0))
                .h(px(thumb_h))
                .rounded(px(2.0))
                .bg(rgba(0xffffff40))
        });
        div()
            // Hide the menu from the viewer underneath, so wheel events over it don't scroll the reader.
            .occlude()
            .absolute()
            .top(px(TOOLBAR_HEIGHT + 4.0))
            .left(px(110.0))
            .min_w(px(220.0))
            .rounded(px(8.0))
            .bg(theme::bg_modal())
            .border_1()
            .border_color(gpui::rgba(0xffffff29))
            .child(
                div()
                    .id("reader-bookmark-menu")
                    .track_scroll(&self.bookmarks_scroll)
                    // Repaint so the thumb follows the wheel.
                    .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                    .max_h(px(360.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .p(px(4.0))
                    .children(self.bookmarks.iter().map(|b| {
                let page = b.page;
                div()
                    .id(("bookmark", page as usize))
                    .flex_none()
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(6.0))
                    .text_size(px(13.0))
                    .text_color(theme::text())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .on_click(cx.listener(move |this, _, _, cx| this.go_to_page(page, cx)))
                    .child(format!("{} · p.{}", b.label, page))
                    })),
            )
            .children(thumb)
    }

    fn state_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let body = match &self.load {
            Load::Ready => return None,
            Load::Loading => div().child("Loading pages…").text_color(theme::text_muted()).into_any_element(),
            Load::Error(msg) => div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(12.0))
                .child(div().text_color(theme::text()).child("Something went wrong while loading this comic."))
                .child(div().text_size(px(12.0)).text_color(theme::text_muted()).child(msg.clone()))
                .child(button("reader-retry", "Retry", ButtonVariant::Classic, cx.listener(|this, _, _, cx| this.load(cx))))
                .into_any_element(),
        };
        Some(div().absolute().size_full().flex().items_center().justify_center().child(body).into_any_element())
    }

    fn next_button(&self, next: LibraryEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let target = next.uid.clone();
        div()
            .id("reader-next")
            .absolute()
            .bottom(px(24.0))
            .right(px(24.0))
            .flex()
            .items_center()
            .gap(px(14.0))
            .max_w(px(360.0))
            .pl(px(16.0))
            .pr(px(10.0))
            .py(px(10.0))
            .rounded(px(999.0))
            .bg(rgba(0x1c1c1cf2))
            .border_1()
            .border_color(rgba(0x34c3d1b3))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(0x262626f2)))
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(Navigate(Route::Reader { uid: target.clone() }))))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(div().text_size(px(11.0)).text_color(theme::accent()).child("Up next"))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme::text())
                            .truncate()
                            .child(next.name),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(32.0))
                    .rounded(px(999.0))
                    .bg(theme::accent())
                    .child(icon(Icon::ArrowRight, px(16.0)).text_color(rgb(0x0a0a0a))),
            )
    }
}

/// One page: a fixed-size frame (so layout never jumps), with the image, a spinner or a retry.
fn page_item(ix: usize, page_w: f32, images: &gpui::Entity<PageImages>, cx: &mut App) -> AnyElement {
    let page = ix as u32 + 1;
    let store = images.read(cx);
    let h = page_w * store.aspect(page).unwrap_or(DEFAULT_ASPECT);
    let frame = div().relative().flex_none().w(px(page_w)).h(px(h)).bg(theme::bg_panel());
    let frame = match store.get(page) {
        Some(PageImg::Ready(image)) => {
            frame.child(img(image.clone()).w(px(page_w)).h(px(h)).object_fit(ObjectFit::Fill))
        }
        Some(PageImg::Failed) => {
            let images = images.clone();
            frame.flex().flex_col().items_center().justify_center().gap(px(10.0)).child(
                div().text_color(theme::text_muted()).child(format!("Page {page} failed to load")),
            )
            .child(button(("page-retry", ix), "Retry", ButtonVariant::Secondary, move |_, _, cx| {
                images.update(cx, |s, cx| s.retry(page, cx))
            }))
        }
        _ => frame
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme::text_muted())
            .child(format!("{page}")),
    };
    div().w_full().flex().justify_center().child(frame).into_any_element()
}

impl Render for ReaderPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = window.viewport_size();
        self.viewer_w = f32::from(size.width);
        self.viewer_h = f32::from(size.height);
        // Re-clamp on resize without touching the saved preference.
        let zoom = self.zoom.clamp(ZOOM_MIN, max_zoom(self.viewer_w));
        let page_w = page_width(zoom, self.viewer_w);

        let mut viewer = None;
        if let (Load::Ready, Some(list_state)) = (&self.load, self.list.clone()) {
            if (page_w - self.last_page_w).abs() > 0.5 {
                self.last_page_w = page_w;
                list_state.remeasure();
            }
            self.step_scroll(&list_state, window);

            let top = list_state.logical_scroll_top();
            let total = self.total as usize;
            let current_ix = {
                let store = self.images.read(cx);
                most_visible(top.item_ix, f32::from(top.offset_in_item), self.viewer_h, total, |ix| {
                    page_w * store.aspect(ix as u32 + 1).unwrap_or(DEFAULT_ASPECT)
                })
            };
            let page = current_ix as u32 + 1;
            if page != self.current {
                self.current = page;
                // Side effects (progress write) must not run inside render.
                cx.spawn(async move |this, cx| {
                    this.update(cx, |t, cx| t.on_page_changed(cx)).ok();
                })
                .detach();
            }

            // Prefetch what is about to scroll into view; the list only builds visible items.
            let first = top.item_ix as u32 + 1;
            let visible = (self.viewer_h / (page_w * DEFAULT_ASPECT)).ceil() as u32 + 1;
            let (lo, hi) = (first.saturating_sub(1).max(1), (first + visible + 2).min(self.total));
            self.images.update(cx, |s, cx| s.want(lo..=hi, first, cx));

            let images = self.images.clone();
            let me = cx.entity();
            viewer = Some(
                div()
                    .id("reader-viewer")
                    .size_full()
                    .on_click(cx.listener(|this, ev: &gpui::ClickEvent, window, cx| {
                        if ev.click_count() >= 2 {
                            window.toggle_fullscreen();
                        }
                        this.header_visible = !this.header_visible;
                        cx.notify();
                    }))
                    // Ctrl/Cmd+wheel zooms instead of scrolling: handled in the capture phase so
                    // the list never sees the event.
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |_, _, window, _| {
                                window.on_mouse_event(move |ev: &ScrollWheelEvent, phase, _, cx| {
                                    if phase != DispatchPhase::Capture {
                                        return;
                                    }
                                    // Let the bookmarks menu scroll itself.
                                    if me.read(cx).bookmarks_open {
                                        return;
                                    }
                                    if ev.modifiers.secondary() {
                                        let dy = f32::from(ev.delta.pixel_delta(px(20.0)).y);
                                        if dy != 0.0 {
                                            let step = if dy > 0.0 { ZOOM_STEP } else { -ZOOM_STEP };
                                            me.update(cx, |t, cx| t.zoom_by(step, cx));
                                        }
                                        cx.stop_propagation();
                                    } else if let ScrollDelta::Lines(lines) = ev.delta {
                                        // Notched wheels jump; ease them. Touchpads (pixel deltas)
                                        // are already smooth and go straight to the list.
                                        me.update(cx, |t, cx| t.scroll_smooth(-lines.y * WHEEL_LINE_PX, cx));
                                        cx.stop_propagation();
                                    }
                                });
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        list(list_state, move |ix, _, cx| page_item(ix, page_w, &images, cx))
                            .size_full(),
                    ),
            );
        }

        let next = (self.total > 0 && self.current == self.total).then(|| self.next_entry(cx)).flatten();

        div()
            .id("reader")
            .key_context("Reader")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| this.on_key(ev, window, cx)))
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme::bg_canvas())
            .children(viewer)
            .children(self.state_panel(cx))
            .when(self.header_visible || !matches!(self.load, Load::Ready), |s| s.child(self.header(window, cx)))
            .children(next.map(|n| self.next_button(n, cx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_limits_follow_viewer_width() {
        assert_eq!(max_zoom(600.0), 1.0);
        assert_eq!(max_zoom(1240.0), 1.37);
        assert_eq!(max_zoom(5000.0), 3.0);
        assert_eq!(clamp_zoom(9.0, 1240.0), 1.37);
        assert_eq!(clamp_zoom(0.1, 1240.0), 0.5);
        assert_eq!(clamp_zoom(0.7000001, 1240.0), 0.7);
    }

    #[test]
    fn page_width_never_exceeds_viewer() {
        assert_eq!(page_width(1.0, 1240.0), 900.0);
        assert_eq!(page_width(1.0, 700.0), 700.0);
        assert_eq!(page_width(0.5, 1240.0), 450.0);
    }

    #[test]
    fn preload_window_is_clamped() {
        assert_eq!(preload_range(0, 1), None);
        assert_eq!(preload_range(10, 1), Some(1..=3));
        assert_eq!(preload_range(10, 5), Some(3..=7));
        assert_eq!(preload_range(10, 10), Some(8..=10));
        assert_eq!(preload_range(2, 99), Some(1..=2));
        assert_eq!(preload_range(1, 0), Some(1..=1));
    }

    #[test]
    fn progress_uses_one_to_n_model() {
        assert_eq!(progress(1, 4), (25.0, false));
        assert_eq!(progress(3, 7), (42.86, false));
        assert_eq!(progress(4, 4), (100.0, true));
        assert_eq!(progress(1, 1), (100.0, true));
    }

    #[test]
    fn most_visible_picks_largest_overlap() {
        let h = |_| 1000.0;
        // Viewport 800: page 0 has 300 visible, page 1 has 500.
        assert_eq!(most_visible(0, 700.0, 800.0, 5, h), 1);
        assert_eq!(most_visible(0, 0.0, 800.0, 5, h), 0);
        // Ties keep the earlier page.
        assert_eq!(most_visible(0, 600.0, 800.0, 5, h), 0);
    }

    #[test]
    fn most_visible_reaches_last_page_at_the_end() {
        let h = |_| 1000.0;
        // Scrolled to the very bottom of 3 pages with an 800px viewport: page 2's bottom 800px is
        // visible while the previous page has 0.
        assert_eq!(most_visible(1, 1000.0 - 0.0, 800.0, 3, h), 2);
        // Last page bottom on screen but previous page still covers more: last wins.
        assert_eq!(most_visible(1, 600.0, 800.0, 3, |i| if i == 2 { 300.0 } else { 1000.0 }), 2);
    }
}
