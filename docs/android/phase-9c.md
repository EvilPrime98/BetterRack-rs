# Phase 9c: pull list, identification and metadata settings

Status: **done, partly verified.** Built and run on the Pixel 7 API 36 x86_64 emulator. The identify sheet opens and shows the file's own metadata, the pull list shows its week navigator and the store-not-configured message, and Settings shows the new sections. A wiki search, accepting a match and a populated pull-list week were **not** run: no reachable store or wiki from the emulator session.

## What was built

- **Wiki on-device.** `Backend::start` now builds the server with `br_server::wiki::LiveWiki` (`br-wiki` dependency) instead of `NoWiki`, using the app runtime handle. `/api/wiki/*` and wiki lookups during identify now work on the phone (needs internet).
- `src/identify_view.rs` + `ui/identify.slint` (`IdentifySheet`): an "Identify" button on each issue row of the series detail opens a full-screen sheet. It shows the current metadata (`GET /api/library/{uid}/identify`), "Re-identify from the file" (`POST .../identify/reset`), and a wiki search (`GET /api/wiki/comics?title=`) whose tapped result is committed with `POST /api/library/file/identify {fileUid, comic}` (comic JSON verbatim). Library is refreshed afterwards. `library_view::issue_ref(index)` returns uid and name of an issue.
- `src/pull_view.rs` + `ui/pull_list.slint` (`PullListScreen`): the Store tab now has a Search / Pull list switch (`store-mode` in `app.slint`). The pull list shows one Wednesday-anchored week (`GET /api/comics?latest=true&after=&before=`, up to 5 pages of 100), followed series highlighted, tap to follow or unfollow (`POST /api/pull-list`, `DELETE /api/pull-list/{id}`), week prev/next/this week. Date and keyword matching ported from `src/state/week.rs` and `src/state/pull_list.rs`.
- `ui/settings.slint`: scrollable now; new "Metadata" (wiki search switch via `PUT /api/settings {wikiSearch}`, Re-identify all via `POST /api/library/identify/reset-all`) and "Store" (API address, shares the Store tab's save callback). Registered in `lib.rs` next to the other views.

## Left open

- Real run against a store and the wiki: search, accept a match, follow a series, populated week.
- Identify-all progress (`/api/library/identify/all`) is not surfaced; only "Re-identify all" flagging.
- Download folder and identity rotation settings not built (on-device downloads are fixed to `<data>/downloads`, see 9b).
- Series metadata editing and per-issue move/delete are not built (mutations stay unsupported on `/saf` paths).
- The pull list reported "Store API URL is not configured" with the leftover test address `10.0.2.2:9` saved by 9b testing; check how the server decides "configured" when testing with a real store.
- Pull list "new post" badges (`lastSeenId`) are not used.
- Not run on the tablet AVD or arm64 hardware (Phase 12).
