# Phase 12: polish and release

Status: **done, with open items** (no real release key, arm64 hardware not verified).

## Built

- Adaptive icon: `crates/br-android/res/` (`mipmap-anydpi-v26/ic_launcher.xml`, dark background colour, "BR" foreground PNG cut from `packaging/icon.png`, also used as the monochrome layer). `Cargo.toml` sets `resources = "res"` and `icon = "@mipmap/ic_launcher"`. `aapt dump badging` shows the icon on the debug APK.
- `build-apk.sh` now repacks (service, dex, resources) for debug and release. `aapt package` needs `-S res` or the icon is lost. Release signing reads `RELEASE_KEYSTORE`, `RELEASE_KEYSTORE_PASS`, optional `RELEASE_KEY_ALIAS`, `RELEASE_KEY_PASS`; without a keystore it warns and signs with the debug key. It also picks `apksigner` on Linux.
- `.github/workflows/android.yml`: debug APK on PRs touching the Android or shared crates; signed release APK on `v*` tags and manual runs, using secrets `ANDROID_KEYSTORE_B64`, `ANDROID_KEYSTORE_PASS`, `ANDROID_KEY_ALIAS`. Uploads the APK as an artifact. The workflow has never run (SDK package names and runner paths unverified).
- Tablet pass: on the tablet AVD (2560x1600) the app shows the navigation rail and the Library screen fills the width.

## Checklist (from Verification), status

- CBZ/CB7/CBR open on-device without an external binary: verified Phases 8 (CBR pixels not verified, fixtures lack images).
- SAF folder, scan, persistence across restart: verified Phase 6 (emulator).
- Sync against a remote server, offline still usable: Phase 10 (emulator); not against a real remote.
- Not done: real arm64 device, real store and wiki, a real download, tablet grid and reader density tuning, pinch zoom.

## Left open

- Release build (`./build-apk.sh --release`, debug key) succeeds and boots on the tablet AVD (process stays up). It was not signed with a real release key.
- NDK is still r27.3 with the `__hash_memory` shim.
- Real release keystore must be created and its secrets added to GitHub.
