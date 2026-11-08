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

use crate::model::{ComicsType, FilterOption, LibraryEntry, StoreBookmark};
use crate::route::Navigate;
use crate::state::Stores;
use crate::ui::components::bookmark_card::{self, bookmark_card};
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
    hash_cache: Rc<Cell<Option<(HashKey, u64)>>>,
}

type HashKey = (usize, usize, usize, usize, usize, u32, bool, usize);

impl GridScroll {
    pub fn new() -> Self {
        Self {
            state: ListState::new(0, ListAlignment::Top, px(600.0)),
            shape: Rc::new(Cell::new(0)),
            hash_cache: Rc::new(Cell::new(None)),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridCell {
    Item(usize),
    Bookmark(usize),
}

impl GridCell {
    pub fn items(visible: &[usize]) -> Vec<GridCell> {
        visible.iter().map(|&ix| GridCell::Item(ix)).collect()
    }
}

pub fn merge_bookmarks(
    items: &[LibraryEntry],
    visible: &[usize],
    bookmarks: &[StoreBookmark],
    sort: FilterOption,
) -> Vec<GridCell> {
    let mut order: Vec<usize> = (0..bookmarks.len()).collect();
    let title = |ix: usize| bookmarks[ix].title.replace('_', " ");
    match sort {
        FilterOption::Alphabetically => {
            order.sort_by(|&a, &b| br_core::directories::natural_cmp(&title(a), &title(b)))
        }
        FilterOption::CreationDate => {
            order.sort_by(|&a, &b| bookmarks[b].created_at.cmp(&bookmarks[a].created_at))
        }
        FilterOption::ReleaseDate => {}
    }
    let before = |b: usize, item: &LibraryEntry| match sort {
        FilterOption::Alphabetically => {
            !item.did
                && br_core::directories::natural_cmp(&title(b), &item.name.replace('_', " "))
                    .is_lt()
        }
        FilterOption::CreationDate => bookmarks[b].created_at > item.created_at,
        FilterOption::ReleaseDate => false,
    };
    let mut out = Vec::with_capacity(visible.len() + bookmarks.len());
    let mut pending = order.into_iter().peekable();
    for &ix in visible {
        while let Some(&b) = pending.peek() {
            if !before(b, &items[ix]) {
                break;
            }
            out.push(GridCell::Bookmark(b));
            pending.next();
        }
        out.push(GridCell::Item(ix));
    }
    out.extend(pending.map(GridCell::Bookmark));
    out
}

pub fn items_grid<V: EventEmitter<Navigate> + 'static>(
    items: Rc<Vec<LibraryEntry>>,
    visible: Rc<Vec<GridCell>>,
    bookmarks: Rc<Vec<StoreBookmark>>,
    kind: ComicsType,
    stores: &Stores,
    scroll: &GridScroll,
    avail: f32,
    cx: &mut Context<V>,
) -> AnyElement {
    let (cols, col_w, gap_x, gap_y) = geometry(kind, avail);
    let total = visible.len();
    let rows = total.div_ceil(cols);
    let row_flags = {
        let (items, visible) = (items.clone(), visible.clone());
        move |row: usize| {
            let first = row * cols;
            let last = (first + cols).min(total);
            let cells = &visible[first..last];
            let has_comics = cells
                .iter()
                .any(|c| matches!(c, GridCell::Item(ix) if !items[*ix].did));
            let has_bookmarks = cells.iter().any(|c| matches!(c, GridCell::Bookmark(_)));
            (has_comics, has_bookmarks)
        }
    };
    let row_h = move |(has_comics, has_bookmarks): (bool, bool)| {
        let comic = card_height(kind, col_w, has_comics || has_bookmarks);
        let h = if has_bookmarks {
            comic.max(bookmark_card::card_height(col_w))
        } else {
            comic
        };
        h + gap_y
    };

    let key: HashKey = (
        Rc::as_ptr(&visible) as usize,
        visible.len(),
        Rc::as_ptr(&items) as usize,
        items.len(),
        cols,
        col_w.to_bits(),
        kind == ComicsType::Detail,
        bookmarks.len(),
    );
    let shape = match scroll.hash_cache.get() {
        Some((k, h)) if k == key => h,
        _ => {
            let mut hasher = DefaultHasher::new();
            (rows, cols, col_w.to_bits(), kind == ComicsType::Detail).hash(&mut hasher);
            (0..rows).for_each(|row| row_flags(row).hash(&mut hasher));
            let h = hasher.finish();
            scroll.hash_cache.set(Some((key, h)));
            h
        }
    };
    scroll.sync(rows, shape);

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
            let last = (first + cols).min(total);
            let flags = row_flags(row);
            let cells = visible[first..last].iter().map(|cell| {
                let ix = match *cell {
                    GridCell::Bookmark(bx) => {
                        let b = &bookmarks[bx];
                        let thumb = match b.thumbnail_url.as_deref().filter(|u| !u.is_empty()) {
                            Some(url) => {
                                stores.thumbs.update(cx, |t, cx| t.ensure_external(url, cx));
                                stores.thumbs.read(cx).get(url).cloned()
                            }
                            None => Some(crate::state::thumbnails::Thumb::Missing),
                        };
                        return bookmark_card(b, thumb, &stores, col_w, cx);
                    }
                    GridCell::Item(ix) => ix,
                };
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
                let fresh = stores.thumbs.read(cx).is_fresh(&item.uid);
                comic_card(item, &info, thumb, cache, &env, col_w, fresh, cx)
            });
            gpui::div()
                .flex()
                .items_start()
                .gap(px(gap_x))
                .h(px(row_h(flags)))
                .pb(px(gap_y))
                .children(cells)
                .into_any_element()
        }),
    )
    .size_full()
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uid: &str, did: bool, name: &str, created_at: i64) -> LibraryEntry {
        serde_json::from_value(serde_json::json!({
            "uid": uid, "did": did, "name": name, "createdAt": created_at
        }))
        .unwrap()
    }

    fn bookmark(key: &str, title: &str, created_at: i64) -> StoreBookmark {
        StoreBookmark {
            key: key.into(),
            title: title.into(),
            created_at,
            ..Default::default()
        }
    }

    fn keys(cells: &[GridCell], items: &[LibraryEntry], bm: &[StoreBookmark]) -> Vec<String> {
        cells
            .iter()
            .map(|c| match c {
                GridCell::Item(ix) => items[*ix].uid.clone(),
                GridCell::Bookmark(ix) => bm[*ix].key.clone(),
            })
            .collect()
    }

    #[test]
    fn alphabetical_bookmarks_follow_folders_and_sort_by_name() {
        let items = [
            entry("f", true, "Zeta", 0),
            entry("a", false, "Alpha 1", 0),
            entry("c", false, "Charlie 1", 0),
        ];
        let bm = [bookmark("late", "Zulu 9", 0), bookmark("mid", "Bravo 2", 0)];
        let cells = merge_bookmarks(&items, &[0, 1, 2], &bm, FilterOption::Alphabetically);
        assert_eq!(keys(&cells, &items, &bm), ["f", "a", "mid", "c", "late"]);
    }

    #[test]
    fn creation_date_puts_newest_first() {
        let items = [entry("new", false, "n", 30), entry("old", false, "o", 10)];
        let bm = [bookmark("mid", "m", 20), bookmark("newest", "x", 40)];
        let cells = merge_bookmarks(&items, &[0, 1], &bm, FilterOption::CreationDate);
        assert_eq!(keys(&cells, &items, &bm), ["newest", "new", "mid", "old"]);
    }

    #[test]
    fn release_date_puts_bookmarks_last() {
        let items = [entry("a", false, "a", 0)];
        let bm = [bookmark("b", "b", 0)];
        let cells = merge_bookmarks(&items, &[0], &bm, FilterOption::ReleaseDate);
        assert_eq!(keys(&cells, &items, &bm), ["a", "b"]);
    }
}
