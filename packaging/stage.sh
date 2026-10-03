#!/usr/bin/env bash
# Assemble the self-contained app folder `dist/BetterRack/` (what the installer / tarball ships).
#
#   SEVEN_ZIP_DIR=/path/to/BetterRack/vendor/7zip packaging/stage.sh
#
# Layout (must match `platform::server_process::seven_zip_exe`):
#   better-rack-rus(.exe)                 the app, backend (br-core/br-server) linked in
#   bin/7z(.exe) [+ 7z.dll]               bundled 7-Zip (SEVEN_ZIP_PATH)
#   LICENSE, NOTICE.txt
#
# Needs: cargo. No bun, no Node, no sharp. 7-Zip is the one external input: point
# SEVEN_ZIP_DIR at a folder holding the per-platform builds (`win32/`, `linux-x64/`), by default
# `$BETTERRACK_SERVER_ROOT/vendor/7zip` of a BetterRack checkout.
set -euo pipefail

SEVEN_ZIP_DIR="${SEVEN_ZIP_DIR:-${BETTERRACK_SERVER_ROOT:+$BETTERRACK_SERVER_ROOT/vendor/7zip}}"
: "${SEVEN_ZIP_DIR:?set SEVEN_ZIP_DIR (or BETTERRACK_SERVER_ROOT) to the 7-Zip builds}"
SEVEN_ZIP_DIR="$(cd "$SEVEN_ZIP_DIR" && pwd)"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$HERE/dist/BetterRack"

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) PLATFORM=win32; EXE=.exe; SEVEN=win32 ;;
  Linux)                PLATFORM=linux; EXE=;     SEVEN=linux-x64 ;;
  *) echo "unsupported platform $(uname -s)" >&2; exit 1 ;;
esac

echo "==> cargo build --release"
(cd "$HERE" && cargo build --release --locked)

rm -rf "$OUT"
mkdir -p "$OUT/bin"
cp "$HERE/target/release/better-rack-rus$EXE" "$OUT/"

echo "==> 7-Zip"
cp "$SEVEN_ZIP_DIR/$SEVEN/"* "$OUT/bin/"
chmod +x "$OUT/bin/7zz" 2>/dev/null || true

cp "$HERE/packaging/NOTICE.txt" "$OUT/NOTICE.txt"
[ -f "$HERE/LICENSE" ] && cp "$HERE/LICENSE" "$OUT/LICENSE" || true
echo "==> staged at $OUT"
