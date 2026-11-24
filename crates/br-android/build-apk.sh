#!/usr/bin/env bash
# Builds a debug APK for arm64 (devices) and x86_64 (emulator).
# Needs: cargo-apk, the Android SDK (ANDROID_HOME) and NDK r27+, rustup targets
# aarch64-linux-android and x86_64-linux-android.
# Output: target/debug/apk/br-android.apk
set -euo pipefail
cd "$(dirname "$0")"
: "${ANDROID_HOME:?set ANDROID_HOME}"
NDK="${ANDROID_NDK_ROOT:-$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)}"
export ANDROID_NDK_ROOT="$NDK" ANDROID_NDK_HOME="$NDK"
# Slint's build script panics when the SDK has no plain `android-NN` platform dir.
if [ -z "${ANDROID_JAR:-}" ]; then
  export ANDROID_JAR="$(ls -d "$ANDROID_HOME"/platforms/android-*/android.jar | sort -V | tail -1)"
fi
HOST=$(ls "$NDK"/toolchains/llvm/prebuilt)
LIBS="$NDK/toolchains/llvm/prebuilt/$HOST/sysroot/usr/lib"
for pair in arm64-v8a:aarch64-linux-android x86_64:x86_64-linux-android; do
  mkdir -p "runtime_libs/${pair%%:*}"
  cp "$LIBS/${pair##*:}/libc++_shared.so" "runtime_libs/${pair%%:*}/"
done
cargo apk build --lib "$@"
