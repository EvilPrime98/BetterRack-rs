# BetterRack

[![CI](https://github.com/EvilPrime98/better-rack-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/EvilPrime98/better-rack-rust/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

BetterRack is a desktop app for organizing and reading a local comic book library (CBR/CBZ). It also includes a store to search and download comics from a configurable source, and metadata lookups through an integrated wiki.

<img width="1239" height="949" alt="image" src="https://github.com/user-attachments/assets/4cd129e6-ea8d-4bf2-8f00-249398f78dd5" />

<img width="1240" height="949" alt="image" src="https://github.com/user-attachments/assets/9f8203c0-1e25-4874-88ce-e49b3357a8ba" />

<img width="1234" height="946" alt="image" src="https://github.com/user-attachments/assets/f6725fa2-c505-49e4-9a0c-8b5f47ef5573" />

<img width="1237" height="946" alt="image" src="https://github.com/user-attachments/assets/6f51ccba-3772-4e24-a183-2b9558594bf8" />

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
