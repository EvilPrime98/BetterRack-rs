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

# cargo-apk cannot declare a Service or pack Java, so repack: patch the manifest it generated
# (code present, KeepAliveService), rebuild the APK with `java/service.dex` as classes.dex, then
# align and sign with the debug key. Skipped for release builds, which Phase 12 signs itself.
PROFILE=debug
for a in "$@"; do [ "$a" = "--release" ] && PROFILE=release; done
APK_DIR="target/$PROFILE/apk"
if [ "$PROFILE" = debug ]; then
  BT="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)"
  mkdir -p "$APK_DIR/patched"
  sed -e 's/android:hasCode="false"/android:hasCode="true"/' \
      -e 's#</application>#<service android:name="dev.betterrack.android.KeepAliveService" android:exported="false" android:foregroundServiceType="dataSync"/></application>#' \
      "$APK_DIR/AndroidManifest.xml" > "$APK_DIR/patched/AndroidManifest.xml"
  rm -f "$APK_DIR/repacked.apk" "$APK_DIR/aligned.apk"
  "$BT/aapt" package -f -F "$APK_DIR/repacked.apk" -M "$APK_DIR/patched/AndroidManifest.xml" -I "$ANDROID_JAR"
  cp java/service.dex "$APK_DIR/classes.dex"
  (cd "$APK_DIR" && "$BT/aapt" add repacked.apk classes.dex $(find lib -type f) >/dev/null)
  "$BT/zipalign" -f -p 4 "$APK_DIR/repacked.apk" "$APK_DIR/aligned.apk"
  KS="${DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
  "$BT/apksigner.bat" sign --ks "$KS" --ks-pass pass:android --out "$APK_DIR/br-android.apk" "$APK_DIR/aligned.apk"
  rm -f "$APK_DIR/repacked.apk" "$APK_DIR/aligned.apk" "$APK_DIR/classes.dex"
fi
