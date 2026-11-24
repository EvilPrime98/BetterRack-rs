# Phase 9b: store and downloads

Status: **done, partly verified.** The Store and Downloads tabs are built and the APK builds for both ABIs. On the Pixel 7 API 36 x86_64 emulator both tabs open, the Store tab shows the "set store address" prompt, and saving an address does not crash. A real search and download were **not** run: no reachable store was available.

## What was built

- `ui/store.slint` (`StoreScreen`, `StoreRow`): search box, result rows with a Download button, a store-address form (shown while the server answers 409 or `apiUrl` is empty), and a link-picker overlay when a post has several links.
- `ui/downloads.slint` (`DownloadsScreen`, `JobRow`): job rows with a progress bar and Retry (failed) or Stop (running or queued).
- `src/store_view.rs`: `register(ui, backend, data_dir, refresh_ui)`.
  - Search: `GET /api/comics?search=` (empty query lists `latest=true`). Store address is saved with `PUT /api/settings {apiUrl}`.
  - Download: `GET /api/comics/{id}/links`, then `POST /api/downloads` with `strat: all`. Stop is `DELETE /api/downloads/{id}`, Retry is `POST /api/downloads/{id}/retry`.
  - A poller reads `/api/downloads/jobs` every 2 s. When a job turns `done` on-device it rescans and redraws.
  - `POST_NOTIFICATIONS` is requested over JNI before the first download.
- `Backend::send_json(method, uri, body)` returns the status with the JSON reply. `saf::activity` is now `pub(crate)`.
- `app.slint`: tabs are now Library, Browse, Store, Downloads, Settings (Settings is index 4).

## Decision: where downloads land

On-device downloads go to `<data>/downloads` (`outputDir: "downloads"`, resolved against the data dir). The first download registers that folder as a library folder through `add_folder`, so finished comics appear in the library. This avoids a write path in `TreeFs`. With a linked server, no `outputDir` is sent and the server picks the folder.

## Left open

- End-to-end check against a real store: search, link picker, a download that survives backgrounding, and the comic showing in the library.
- No thumbnails in store results. The store address form lives on the Store tab; Phase 9c may move it into Settings.
- The notification permission result is not handled, only requested.
- Five tabs are tight on a narrow phone; 9c adds more, so a "More" list may be needed.
