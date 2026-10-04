//! `ItemsGrid`: the virtualized folder/comic grid shared by the Library, Search, Recent, Reading and
//! Filtered pages. Items are chunked into rows (the column count follows the available width) and a
//! `uniform_list` builds only the visible rows. "Scrolled into view" is therefore "built", which is
//! where covers and lazy identification are requested.

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use gpui::{
    AnyElement, Context, EventEmitter, IntoElement, ListAlignment, ListState, Styled, Window, list,
    px,
};

use crate::model::{ComicsType, LibraryEntry};
use crate::route::Navigate;
use crate::state::Stores;
use crate::ui::components::comic_card::{CardEnv, card_height, comic_card};
use crate::ui::components::folder_card::folder_card;
use crate::ui::theme;

pub const PAGE_PAD_X: f32 = 20.0;

/// `(columns, column width, horizontal gap, vertical gap)` for the layout in `avail` px of width.
fn geometry(kind: ComicsType, avail: f32) -> (usize, f32, f32, f32) {
    let (min_col, gap_x, gap_y) = match kind {
        ComicsType::Cover => (180.0_f32, 12.0_f32, 16.0_f32),
        ComicsType::Detail => (420.0, 14.0, 14.0),
    };
    let cols = (((avail + gap_x) / (min_col + gap_x)).floor() as usize).max(1);
    let col_w = ((avail - gap_x * (cols as f32 - 1.0)) / cols as f32).max(100.0);
    (cols, col_w, gap_x, gap_y)
}

/// Width available to the grid: the window minus the sidebar and the page padding.
pub fn available_width(window: &Window, sidebar_visible: bool) -> f32 {
    f32::from(window.viewport_size().width)
        - if sidebar_visible {
            f32::from(theme::sidebar_width())
        } else {
            0.0
        }
        - PAGE_PAD_X * 2.0
}

/// Scroll state of an [`items_grid`]. Rows differ in height (folder-only rows are shorter than
/// rows holding comics), so the grid uses a measured `list` rather than a `uniform_list`.
#[derive(Clone)]
pub struct GridScroll {
    state: ListState,
    shape: Rc<Cell<u64>>,
}

impl GridScroll {
    pub fn new() -> Self {
        Self {
            state: ListState::new(0, ListAlignment::Top, px(600.0)),
            shape: Rc::new(Cell::new(0)),
        }
    }

    pub fn scroll_by(&self, dy: f32) {
        self.state.scroll_by(px(dy));
    }

    /// Re-measure from scratch when the row structure changes, keeping the scroll position.
    fn sync(&self, rows: usize, shape: u64) {
        if self.shape.replace(shape) == shape {
            return;
        }
        let top = self.state.logical_scroll_top();
        self.state.reset(rows);
        self.state.scroll_to(top);
    }
}

impl Default for GridScroll {
    fn default() -> Self {
        Self::new()
    }
}

/// `items[visible[i]]` is the i-th card. The caller owns `scroll` so the position survives renders.
pub fn items_grid<V: EventEmitter<Navigate> + 'static>(
    items: Rc<Vec<LibraryEntry>>,
    visible: Rc<Vec<usize>>,
    kind: ComicsType,
    stores: &Stores,
    scroll: &GridScroll,
    avail: f32,
    cx: &mut Context<V>,
) -> AnyElement {
    let (cols, col_w, gap_x, gap_y) = geometry(kind, avail);
    let rows = visible.len().div_ceil(cols);
    let row_has_comics = |row: usize| {
        let first = row * cols;
        let last = (first + cols).min(visible.len());
        visible[first..last].iter().any(|&ix| !items[ix].did)
    };
    let row_h = move |has_comics: bool| card_height(kind, col_w, has_comics) + gap_y;

    let mut hasher = DefaultHasher::new();
    (rows, cols, col_w.to_bits(), kind == ComicsType::Detail).hash(&mut hasher);
    (0..rows).for_each(|row| row_has_comics(row).hash(&mut hasher));
    scroll.sync(rows, hasher.finish());

    let stores = stores.clone();
    list(
        scroll.state.clone(),
        cx.processor(move |_this, row: usize, _window, cx| {
            use gpui::{ParentElement as _, Styled as _};
            let env = CardEnv {
                stores: stores.clone(),
                kind,
            };
            let first = row * cols;
            let last = (first + cols).min(visible.len());
            let has_comics = visible[first..last].iter().any(|&ix| !items[ix].did);
            let cells = visible[first..last].iter().map(|&ix| {
                let item = &items[ix];
                if item.did {
                    return folder_card(item, kind, &stores, col_w, cx);
                }
                stores.thumbs.update(cx, |t, cx| t.ensure(&item.uid, cx));
                stores.identify.update(cx, |s, cx| s.ensure(item, cx));
                let thumb = stores.thumbs.read(cx).get(&item.uid).cloned();
                let info = stores.identify.read(cx).info(item);
                let cache = stores
                    .comics
                    .read(cx)
                    .get(&item.uid)
                    .cloned()
                    .unwrap_or_default();
                comic_card(item, &info, thumb, cache, &env, col_w, cx)
            });
            gpui::div()
                .flex()
                .items_start()
                .gap(px(gap_x))
                .h(px(row_h(has_comics)))
                .pb(px(gap_y))
                .children(cells)
                .into_any_element()
        }),
    )
    .size_full()
    .into_any_element()
}
