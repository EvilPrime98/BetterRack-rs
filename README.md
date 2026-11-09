# BetterRack

[![CI](https://github.com/EvilPrime98/better-rack-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/EvilPrime98/better-rack-rust/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

BetterRack is a desktop app for organizing and reading a local comic book library (CBR/CBZ). It also includes a store to search and download comics from a configurable source, and metadata lookups through an integrated wiki.

## What BetterRack does

- **Library**: scans your comic folders and keeps a browsable library of CBR and CBZ files, with thumbnails.
- **Reader**: built-in reader with zoom controls and per-comic reading progress.
- **Store**: search and download comics from GetComics.org (the only supported source at the moment), with a download queue you can watch, retry or cancel.
- **Wiki metadata**: look up comic information through the integrated wiki.
- **Settings**: configure source URLs, library folders and download/output directories from the UI.
- **Remote mode**: connect to a remote BetterRack deployment instead of the built-in server, sharing that deployment's library and data.
- **Targets**: Windows installer and Linux tarball.

## Installation

### Download a release

Prebuilt packages are published on the [GitHub Releases](https://github.com/EvilPrime98/better-rack-rust/releases) page for each version tag:

| Platform | Package |
|---|---|
| Windows | Installer (`.exe`) |
| Linux | Tarball (`.tar.gz`) |

### Run from source

Requires a recent stable [Rust](https://rustup.rs) toolchain.

```bash
git clone https://github.com/EvilPrime98/better-rack-rust.git
cd better-rack-rust
cargo run --release
```

CBR/RAR/7z archives are extracted with [7-Zip](https://www.7-zip.org). Release packages bundle it; when running from source it falls back to `7z` on your `PATH`, or to the binary named by `SEVEN_ZIP_PATH`.

### Run headless (server only)

`br-server` is a standalone HTTP backend with no GUI dependency, so building it skips the GUI libraries. It needs Rust 1.85 or newer and 7-Zip on the host.

```bash
BR_BIND_ADDR=0.0.0.0 BR_API_KEY=change-me cargo run --release -p br-server
```

| Variable | Purpose |
|---|---|
| `PORT` | Port to listen on. Default `3000`; `0` lets the OS pick. |
| `BR_BIND_ADDR` | Host or IP to listen on (no port). Default `127.0.0.1`. |
| `BR_API_KEY` | Required for `/api/*` and `/read/*` (header `x-br-api-key` or `?key=`). |
| `BR_DATA_DIR` | Base directory for the SQLite data. Defaults to the working directory. |
| `LOG_LEVEL` | Tracing filter. Default `info`. |
| `SEVEN_ZIP_PATH` | Path to the 7-Zip executable if it is not on `PATH`. |
| `BR_WEB_DIR` | Folder of the built web client (see below). Unset or missing: API only. |

A non-loopback `BR_BIND_ADDR` (such as `0.0.0.0` or `::`) without `BR_API_KEY` makes the server refuse to start, and so does an invalid address. The server does not do TLS; put a reverse proxy in front of it.

### Web client and Android

`web/` is a browser client of `br-server` (Vite + TypeScript, pnpm). Build it and point the server at the output:

```bash
cd web && pnpm install && pnpm build
BR_WEB_DIR=web/dist BR_BIND_ADDR=0.0.0.0 BR_API_KEY=change-me cargo run --release -p br-server
```

The server then serves the app at `/` on the same origin, so the client needs no URL configuration. The static files are served without the key; `/api/*` and `/read/*` still require it. In the browser, open the server dialog, enable remote mode, enter the server's own address and the API key.

The desktop app serves the web client itself, over the same library and data as its own window, at `http://127.0.0.1:3000/` (`PORT`, `BR_BIND_ADDR` and `BR_API_KEY` apply as for the headless server). It looks for the client in `BR_WEB_DIR`, then in a `web/` folder next to the app (where release packages put it), then in `web/dist` of the working directory. If none exists, or the port is taken, the app starts without it.

The Android app uses Capacitor and talks to a remote `br-server`. The server address is set in the app; to preset it, build with `VITE_DEFAULT_API_URL` (see `web/.env.example`). From `web/`: `pnpm android:apk` builds a debug APK (needs JDK 21 and the Android SDK). Releases attach the debug APK.

### Build distributables

```bash
SEVEN_ZIP_DIR=/path/to/BetterRack/vendor/7zip packaging/stage.sh
```

This assembles the self-contained app folder `dist/BetterRack/`. The Windows installer is then built from `packaging/windows/betterrack.iss` with Inno Setup.

## Configuration

Settings are read from environment variables.

| Variable | Purpose |
|---|---|
| `BETTERRACK_SERVER_URL` | Connect to a remote BetterRack server instead of starting the built-in one. |
| `BR_API_KEY` | API key sent to the remote server (remote mode). |
| `SEVEN_ZIP_PATH` | Path to the 7-Zip executable used to extract archives. |
| `BETTERRACK_UPDATE_REPO` | GitHub repository checked for new releases. |

The source URLs and the download/output directories are set from the Settings page in the app.

## Built with

| Tool | Purpose |
|---|---|
| [GPUI](https://www.gpui.rs) (`gpui-ce`) | Native GPU-accelerated UI |
| [Tokio](https://tokio.rs) and [Axum](https://github.com/tokio-rs/axum) | Async runtime and local HTTP API |
| [7-Zip](https://www.7-zip.org) | Extracting CBR/CBZ archives (bundled) |
| [image](https://github.com/image-rs/image) | Thumbnail generation |

## FAQ

**Does it need an internet connection?**
Reading and organizing your local library does not. The Store and the wiki metadata lookups do.

**Which store does it support?**
Currently only GetComics.org. BetterRack is not affiliated with GetComics.org.

**Which file formats are supported?**
CBZ and CBR.

**Which operating systems are supported?**
Windows and Linux.

**Can I use one library from several devices?**
Yes. Run a BetterRack deployment, set `BR_API_KEY` if it is reachable beyond a trusted LAN, and point the app at it using remote mode.

**Where do I report a bug or ask for a feature?**
Open an issue on the [issue tracker](https://github.com/EvilPrime98/better-rack-rust/issues).

## Contributing

Bug reports and pull requests are welcome. Open an issue first to discuss larger changes. Before submitting a pull request, run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

## License

[GPL-3.0](LICENSE)

The Windows installer and the Linux tarball bundle 7-Zip, which is distributed under the GNU LGPL with the unRAR license restriction on its RAR decoder. See [`packaging/NOTICE.txt`](packaging/NOTICE.txt).

## Author

[EvilPrime98](https://github.com/EvilPrime98)
