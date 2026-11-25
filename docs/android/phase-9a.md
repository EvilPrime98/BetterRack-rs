# Phase 9a: browse lists

Status: **done (partial scope).** A new Browse tab shows "Keep reading" and "Recently added" shelves and a manual Refresh button. Tapping a shelf item opens the reader on its saved page. Verified on the Pixel 7 API 36 x86_64 emulator with persisted data. Not run on the tablet AVD or arm64 hardware.

## What was built

- `src/lists_view.rs`: loads `/api/library/reading`, `/api/library/recent` and `/api/comic-data` (for the percent read), keeps them in a global `State`, pushes `Item` rows to `reading` / `recent`. `reload` is called from `refresh_ui` in `lib.rs`. `register(ui, backend, refresh_ui)` wires `list-cover-needed`, `open-list-item` and `refresh` (rescan, then redraw everything, with a `refreshing` flag).
- `ui/lists.slint` (`ListsScreen`, `Shelf`): horizontal covers (`Flickable`), tab 1 in `app.slint`; Settings is now tab 2.
- `reader_view::start(ui, backend, uid)` is the single way to open the reader (the library issue tap now uses it too). `library_view::cover_item` and `ensure_cover` let other screens reuse the cover cache; finished covers are also pushed into the `reading` and `recent` models.

## Left open from the 9a spec

- Writer filter, library sort, folder view and bookmarks in the reader are not built.
- No "More" list in `nav.slint`: 9b and 9c should add their own tab (or create "More" then). Phone bar now has three tabs.
- Refresh shows "Scanning..." only; the server rescan is synchronous, there is no progress value.
- "Keep reading" subtitle uses `readPer`; the series detail does not show read state.
