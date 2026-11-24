# Native Android app in Rust

Decision record for building a native (non-WebView) Android version of the desktop comic library manager, reusing the existing Rust crates. It replaces the current Capacitor shell in `web-react/android`, which wraps the React client in a WebView. Outcome: build a separate mobile UI in a mature Rust UI framework (Slint is the leading candidate), share `br-core` and `br-server` with the desktop app, and settle the final framework with a spike. Status: In progress. Phases 0 to 11 are done (Phase 9 in slices 9a-9c); Slint is the chosen framework (Phase 4 gate decided, Makepad dropped). The implementation is split into phases, each sized for one agent (see Implementation phases and Progress).

## Decisions

- **Native UI, no WebView.** The Capacitor shell is rejected because it runs the React client in a WebView. The goal is a native Android app for phones and tablets, using Rust so most of the existing code is reused.
- **Standalone on-device app.** The Android app runs the whole application on the device and does everything the desktop app does. The only remote behaviour is the existing optional connection to a server for syncing between devices, with that server as the source of truth. No other remote server is involved.
- **Separate mobile UI, shared core.** The mobile UI is its own codebase, written in a mature framework, rather than the GPUI views running on Android. `br-core` and `br-server` are shared with desktop. The recurring cost is that each feature is built twice. This was accepted on the expectation that agent skills and workflows will keep the desktop and mobile versions in sync.
- **Mobile UI calls `br-server` in-process.** The desktop UI already does this through `ApiClient::in_process`, so the mobile UI follows the same contract without opening a socket.
- **Framework criteria.** Maturity comes first. Migration effort and ongoing maintenance next to the desktop app come second.
- **Candidates.** Slint is the leading candidate, with Makepad as the alternative. `gpui-mobile` (a third-party Android layer for GPUI) would give one shared UI codebase, but it is not chosen because it is a 0.1.0 crate. It is kept only as a fallback to evaluate. Dioxus (mostly WebView on mobile) and Iced (Android support still a work in progress) are not pursued. The doc recommends but does not fix the final pick, which comes from a spike.
- **Archives are read in-process.** The external 7-Zip subprocess is replaced or supplemented with libraries that work on Android. Desktop benefits too.
- **CBR is mandatory.** CBR support is a staple of the app and must work on Android, even though it is the hardest format to handle in-process. It needs a dedicated investigation.
- **Storage uses the Storage Access Framework.** The user picks library folders once and the app keeps persistent permission, matching the desktop "point at my folders" behaviour. `br-core` file code needs an abstraction over content URIs.

## Progress

**Agent instructions: when this doc is given to you with no other task, continue the work. Read `docs/android/phase-N.md` for every finished phase, then pick the next unfinished phase from the Parallelism table (lowest wave whose dependencies are done), do only that phase, write `docs/android/phase-<N>.md`, and update this section and the Status column below before finishing. Respect the ground rules and the gates.**

| Phase | Status | Handoff |
|---|---|---|
| 0 | Done | `docs/android/phase-0.md` |
| 1 | Done | `docs/android/phase-1.md` |
| 3 | Done | `docs/android/phase-3.md` |
| 4 | Done: Slint chosen, Makepad dropped | `docs/android/phase-4.md` |
| 2 | Done | `docs/android/phase-2.md` |
| 5 | Done | `docs/android/phase-5.md` |
| 6 | Done | `docs/android/phase-6.md` |
| 10 | Done | `docs/android/phase-10.md` |
| 11 | Done | `docs/android/phase-11.md` |
| 7 | Done | `docs/android/phase-7.md` |
| 8 | Done | `docs/android/phase-8.md` |
| 9 | Done (slices 9a-9c) | `docs/android/phase-9.md` |
| 9a | Done (partial: shelves and refresh) | `docs/android/phase-9a.md` |
| 9b | Done (built, not tested against a real store) | `docs/android/phase-9b.md` |
| 9c | Done (built, not tested against a real store or wiki) | `docs/android/phase-9c.md` |
| 12 | Ready | |

Facts from finished phases that later phases rely on:
- `br-core`, `br-server` and `br-wiki` pass `cargo ndk -t arm64-v8a check` (NDK r27.3, `cargo-ndk` 4.1.2). Only `aarch64` and only `cargo check` so far.
- All `reqwest` dependencies use rustls (no `openssl-sys`). Not yet smoke-tested against real HTTPS.
- CBZ and CB7 are read in-process (`zip`, `sevenz-rust2`). CBR/RAR is read in-process too (`rar-rs`, pure Rust; `unrar` does not cross-build from Windows). 7-Zip is only spawned for `.cbt` and as a fallback. On Android call `Archives::with_cache_dir` (RAR copies from content URIs go there).
- Library reads (scan, directories, archive open, stamps, comic download) go through `LibraryFs` (`br-core/src/fs.rs`), injected via `Archives::with_fs`. Mutations (rename, delete, create folder) and the 7-Zip subprocess still use real paths; Phases 2 and 6 own those.
- Phase 4: Slint 1.19 runs the router in-process on an x86_64 emulator (`spikes/slint-android`). It needs `ANDROID_JAR` set and, on NDK r27, a `__hash_memory` shim for Skia (or NDK r28). Slint's GPL-3.0 option fits this repo's licence. Decision: Slint; Makepad not evaluated.
- Phase 5: `crates/br-android` is its own workspace (root lists members explicitly and excludes it); build with `crates/br-android/build-apk.sh`. Screens are Slint files under `ui/`, server calls go through `Backend` in `src/backend.rs`. Still on NDK r27 shim.
- Phase 6: library storage on Android is `TreeFs` (`br-core/src/tree_fs.rs`) with path-like ids `/saf/<key>/<label>/...`; `NetOptions.archives` injects it. Picker is a Fragment in `crates/br-android/java/picker.dex` loaded at runtime. Read-only; mutations unsupported on `/saf` paths. Verified on the phone emulator, including persistence across restart.
- Phase 10: "sync" is a switchable backend like desktop remote mode (`Backend::link_remote`, no data merge). The API key is sealed by an Android Keystore AES key (`vault.rs`, `KeyVault.java`, shared dex loader `saf::dex_class`); the manifest now has INTERNET. Verified on the phone emulator.
- Phase 11: a foreground service (`KeepAliveService`, type dataSync) runs while download jobs are active, started by `keepalive::watch`. cargo-apk cannot declare services or pack Java, so `build-apk.sh` repacks the debug APK (patched manifest, `java/service.dex` as `classes.dex`, re-sign); Phase 12 must repeat that step for release. Verified on the phone emulator.
- Phase 7: library is a series cover grid (`library_view.rs`, `ui/library.slint`, `ui/series.slint`) with search, filters, lazy thumbnails and series detail. Issue taps are unwired (Phase 8). Logs go to logcat via `tracing_subscriber`. Verified on the phone emulator.
- Phase 8: reader is `reader_view.rs` + `ui/reader.slint` (tap zones, swipe, double-tap zoom, LTR/RTL, progress via `PATCH /api/comic-data`). `br-server` `pages_of` now stats through `LibraryFs`. CBZ and CB7 verified on the phone emulator; CBR pixels not (fixtures have no images); no pinch zoom.
- Phase 9: audit found missing Browse lists (9a), Store and downloads (9b), and Metadata and pull list (9c) screens; each is one agent, spec in `docs/android/phase-9.md`. Mutations stay unsupported on `/saf` paths.
- Phase 9b: Store and Downloads tabs (`store_view.rs`, `ui/store.slint`, `ui/downloads.slint`); on-device downloads go to `<data>/downloads`, registered as a library folder on first use. Not run against a real store.
- Phase 9c: the wiki now runs on-device (`LiveWiki`); identify sheet per issue (`identify_view.rs`), pull list under the Store tab (`pull_view.rs`), metadata and store address in Settings. Not run against a real store or wiki.
- Phase 9a: Browse tab (`lists_view.rs`, `ui/lists.slint`) with Keep reading and Recently added shelves and manual refresh; `reader_view::start(ui, backend, uid)` opens the reader from any screen, `library_view::ensure_cover` loads covers. Writer filter, sort, folder view and reader bookmarks not built.
- On Windows, 3 `tests/run.sh` cases (`auth-header-key`, `auth-query-key`, `refuse-public-bind-without-key`) fail from CRLF in `env` files, unrelated to Android.

## Implementation phases

The work is split so one agent can finish each phase within roughly 100-120k tokens of context. Phases 0 and 4 are gates: stop after each and re-plan before continuing.

### Ground rules for every phase

- **Handoff file.** Each phase ends by writing `docs/android/phase-N.md` with what was done, decisions made, public interfaces added and what is left open. The next agent reads only this decision doc, the handoff files it depends on, and `graphify query` results, not the whole repo.
- **Self-contained exit criteria.** Each phase has a check that passes without later phases (a build, a test or a device run).
- **Stop rule.** If a blocker is not solved within the budget, write the findings in the handoff file and stop instead of pushing on. This matters most for Phases 0 and 2.
- **Existing tests.** The black-box suite (`tests/run.sh`) must stay green on desktop after every phase.

### Stage A: de-risk the shared core (no UI)

| # | Phase | Output | Depends on |
|---|---|---|---|
| 0 | **Android build feasibility. (Done, see `docs/android/phase-0.md`.)** Install the NDK and Rust targets, cross-compile `br-core` and `br-server` for `aarch64-linux-android`, and gate or fix whatever fails (rusqlite, `keyring`, `rfd`, process spawning). | Both crates compile for Android and a written list of blockers. | none |
| 1 | **In-process CBZ/CB7 reading. (Done, see `docs/android/phase-1.md`.)** Replace or supplement the 7-Zip subprocess in `archive.rs` with Rust libraries, with fixture tests. Desktop benefits too. | No external binary needed for CBZ/CB7. | 0 |
| 2 | **CBR on Android. (Done, see `docs/android/phase-2.md`.)** Compare `unrar`, libarchive and other options for licence and Android build. Choose one, implement it behind the archive API and test it. | CBR reads in-process. | 0, 1 |
| 3 | **Storage abstraction. (Done, see `docs/android/phase-3.md`.)** Add a `LibraryFs` trait in `br-core` (list, open, stat, watch) with a desktop `std::fs` implementation, and move scan and reader code onto it. | The core no longer assumes file paths. | 0 |

### Stage B: choose the UI framework

| # | Phase | Output | Depends on |
|---|---|---|---|
| 4 | **Framework spike (gate).** Build a minimal Slint app and a minimal Makepad app. Each runs the `br-server` router in-process and shows a library list on a device. Record performance, IME, lifecycle and licence findings. | A decision record that picks the framework. | 0 |

### Stage C: the app

| # | Phase | Output | Depends on |
|---|---|---|---|
| 5 | **App scaffold.** `crates/br-android` crate, build pipeline, in-process server startup, app data dir, SQLite, phone and tablet navigation. | An empty app that boots and serves its own API. | 4 |
| 6 | **SAF bridge.** JNI glue for the folder picker and persistent URI permission, plus the Android `LibraryFs` implementation. | A folder survives an app restart and a scan works. | 3, 5 |
| 7 | **Library screens.** List, search, filters, series detail. | Browsing works on phone and tablet. | 5, 6 |
| 8 | **Reader.** Page view, gestures, zoom, reading direction, saved progress. | CBZ, CB7 and CBR open on-device. | 1, 2, 6 |
| 9a-9c | **Remaining desktop features.** Phase 9 starts with a short audit of the desktop feature list, then is split into slices (for example metadata editing, collections, wiki), one agent each. | Feature parity. | 7 |
| 10 | **Server sync and credentials.** The optional remote server connection, with credentials in the Android Keystore replacing `keyring`. Offline use must keep working. | Sync works and the device stays usable offline. | 5 |
| 11 | **Background work.** Downloads and jobs under WorkManager or a foreground service. | Downloads survive backgrounding. | 5 |
| 12 | **Polish and release.** Tablet layout pass, adaptive icons, release signing, a CI job for the APK, and the end-to-end checklist from Verification. | A signed APK. | all |

### Parallelism

Run each wave in separate git worktrees. Merge a wave before starting the next.

| Wave | Phases that can run at the same time | Notes |
|---|---|---|
| 1 | 0 | Gate. Everything else depends on its findings. |
| 2 | 1, 3, 4 | Independent of each other. 4 only needs Phase 0 to know the crates build. |
| 3 | 2, and 5 once 4 has chosen the framework | 2 needs the archive API from 1. Wave 3 does not wait for 3 to merge unless Phase 6 is next. |
| 4 | 6, 10, 11 | 6 needs 3 and 5. 10 and 11 only need 5. |
| 5 | 7, 8 | 7 needs 6. 8 needs 1, 2 and 6. They touch different screens. |
| 6 | 9a, 9b, 9c | Parallel as long as each slice owns separate screens. |
| 7 | 12 | Serial, needs everything. |

Conflict hotspots when running in parallel: `crates/br-core/src/archive.rs` (Phases 1 and 2, so they are serial), the scan and reader code (Phase 3 against Phases 1 and 2), and the shared navigation and app shell (Phases 7, 8 and 9, which should each add their own module and register it in one line).

### Sizing risks

- Phases 0 and 2 are the most uncertain. Cross-compilation and RAR extraction can each turn into long debugging.
- Phase 9 could exceed the budget as one phase, which is why it is split.

## Findings from the code

- The desktop app is GPUI (`gpui-ce` 0.2.2) with an embedded axum server. `Cargo.toml` is a workspace with `crates/br-core`, `crates/br-server` and `crates/br-wiki`.
- `src/api/mod.rs` defines `ApiClient`, which can run in-process over the `br_server::app(...)` router (`ApiClient::in_process`). `src/platform/server_process.rs` builds it with `br_server::app_with_web` and also spawns a separate web server (`spawn_web_server`) for the web client.
- `crates/br-core/src/archive.rs` finds a 7-Zip executable (`Archives`, `SEVEN_ZIP_ENV_VAR` = `SEVEN_ZIP_PATH`, then `7zz`/`7z`/`7za` on PATH) and fails with "7z executable not found" otherwise. Android cannot rely on this.
- `crates/br-core/src/db.rs` wraps SQLite (`rusqlite`, bundled), which should build for Android with the NDK.
- `keyring` is a dependency of the root crate, with only Windows, macOS and Linux backends enabled. It is used in `src/platform/server_config.rs`.
- The desktop uses `rfd` for the folder picker and `src/platform/` for single-instance, close guard, update check and logging. These are desktop-specific.
- `web-react/capacitor.config.ts` (appId `com.amin.BetterRackweb`) and `web-react/android/` are the existing Capacitor shell.

## Findings from research

These come from web search snippets and were not verified against the projects' repositories or releases.

- **Slint:** declarative `.slint` markup compiled to native code. It has an Android backend built with `cargo-apk` and the NDK (first shipped in Slint 1.3), and handles IME through Java glue. Its licence terms are less conventional than other Rust projects.
- **Makepad:** active development and working Android support through `cargo makepad android`. Robrix (a Matrix client) runs on it, needing Android 13 or later. It has rough edges and a small ecosystem. Android lifecycle support is described as partial.
- **gpui-mobile 0.1.0:** gives GPUI an Android platform layer (wgpu on Vulkan, GL ES fallback, touch input with momentum scrolling). The Android arm64 API 26+ target is listed as supported. Compatibility with `gpui-ce` was not confirmed.
- **Dioxus:** `dx serve` supports Android, but mobile rendering is largely WebView-based.
- **Iced:** Android support is a stated goal, still in progress.
- No Android-specific information was found for Xilem.

## Edge cases

- **CBR on Android.** RAR extraction has no pure-Rust option comparable to ZIP or 7z. Candidates to investigate are `unrar` bindings, libarchive and others, including their licences and Android build story.
- **Content URIs.** SAF returns content URIs rather than filesystem paths, so every `br-core` code path that opens, lists or watches library files needs an abstraction layer.
- **Secure storage.** The `keyring` crate has no Android backend enabled, so credential storage for the server connection needs an Android equivalent.
- **Background work.** Downloads and jobs (`br-core` `downloads.rs`, `jobs.rs`) will be constrained by Android background execution limits.
- **Desktop-only platform code.** Single-instance handling, close guard and update check in `src/platform/` have no direct Android equivalent.
- **Phone and tablet layouts.** The UI must work across both form factors.

## Out of scope

- Running the GPUI views on Android through `gpui-mobile` (a fallback only).
- Keeping the Capacitor/WebView client as the Android solution.
- A remote-only thin client.
- iOS.

## Verification

- A spike builds a minimal app in Slint and in Makepad that opens the `br-server` router in-process and shows the library list on a real Android device and a tablet.
- Open a CBZ, a CB7 and a CBR file in the reader on-device without any external binary.
- Pick a library folder through SAF, scan it, and reopen the app with permission persisted.
- Sync against a remote server and confirm the device remains usable fully offline.

## Open questions

- The user referred to a framework called "NCHASE", which they said is maintained by the author of the GPUI component kit. It was not identified, so it is not in the comparison.
- Final choice between Slint and Makepad, to be decided by the spike.
- How CBR is handled on Android (which library or approach).
- Android replacements for `keyring`, background downloads and jobs, and the update check.
