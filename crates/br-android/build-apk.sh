#!/usr/bin/env bash
# Builds a debug APK for arm64 (devices) and x86_64 (emulator).
# Needs: cargo-apk, the Android SDK (ANDROID_HOME) and NDK r27+, rustup targets
# aarch64-linux-android and x86_64-linux-android.
# Output: target/<debug|release>/apk/br-android.apk (pass --release for a release build).
# Release builds are signed with RELEASE_KEYSTORE (+ RELEASE_KEYSTORE_PASS, optional RELEASE_KEY_ALIAS,
# RELEASE_KEY_PASS); without them the release APK is signed with the debug key and is not publishable.
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
PROFILE=debug
for a in "$@"; do [ "$a" = "--release" ] && PROFILE=release; done
if [ "$PROFILE" = release ]; then
  KS="${RELEASE_KEYSTORE:-${DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}}"
  KS_PASS="${RELEASE_KEYSTORE_PASS:-android}"
  KEY_ALIAS="${RELEASE_KEY_ALIAS:-}"
  KEY_PASS="${RELEASE_KEY_PASS:-$KS_PASS}"
  [ -n "${RELEASE_KEYSTORE:-}" ] || echo "warning: RELEASE_KEYSTORE not set, signing with the debug key" >&2
  # cargo-apk insists on a release key; the repack below re-signs anyway.
  export CARGO_APK_RELEASE_KEYSTORE="$KS" CARGO_APK_RELEASE_KEYSTORE_PASSWORD="$KS_PASS"
else
  KS="${DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
  KS_PASS=android
  KEY_ALIAS=""
  KEY_PASS=android
fi
cargo apk build --lib "$@"

# cargo-apk cannot declare a Service or pack Java, so repack: patch the manifest it generated
# (code present, KeepAliveService), rebuild the APK with `java/service.dex` as classes.dex and the
# resources (icon), then align and sign.
APK_DIR="target/$PROFILE/apk"
BT="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)"
APKSIGNER="$BT/apksigner"; [ -e "$BT/apksigner.bat" ] && APKSIGNER="$BT/apksigner.bat"
mkdir -p "$APK_DIR/patched"
sed -e 's/android:hasCode="false"/android:hasCode="true"/'     -e 's#</application>#<service android:name="dev.betterrack.android.KeepAliveService" android:exported="false" android:foregroundServiceType="dataSync"/></application>#'     "$APK_DIR/AndroidManifest.xml" > "$APK_DIR/patched/AndroidManifest.xml"
rm -f "$APK_DIR/repacked.apk" "$APK_DIR/aligned.apk"
"$BT/aapt" package -f -F "$APK_DIR/repacked.apk" -M "$APK_DIR/patched/AndroidManifest.xml" -S res -I "$ANDROID_JAR"
cp java/service.dex "$APK_DIR/classes.dex"
(cd "$APK_DIR" && "$BT/aapt" add repacked.apk classes.dex $(find lib -type f) >/dev/null)
"$BT/zipalign" -f -p 4 "$APK_DIR/repacked.apk" "$APK_DIR/aligned.apk"
SIGN=(sign --ks "$KS" --ks-pass "pass:$KS_PASS" --out "$APK_DIR/br-android.apk")
if [ -n "$KEY_ALIAS" ]; then SIGN+=(--ks-key-alias "$KEY_ALIAS" --key-pass "pass:$KEY_PASS"); fi
"$APKSIGNER" "${SIGN[@]}" "$APK_DIR/aligned.apk"
rm -f "$APK_DIR/repacked.apk" "$APK_DIR/aligned.apk" "$APK_DIR/classes.dex"
echo "built $APK_DIR/br-android.apk"
