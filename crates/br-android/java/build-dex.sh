#!/usr/bin/env bash
# Rebuilds java/picker.dex (loaded at runtime) and java/service.dex (packed into the APK as
# classes.dex by build-apk.sh) from the Java sources. Needs a JDK and the Android SDK (ANDROID_HOME).
set -euo pipefail
cd "$(dirname "$0")"
JAR="${ANDROID_JAR:-$(ls -d "$ANDROID_HOME"/platforms/android-*/android.jar | sort -V | tail -1)}"
D8="$(ls -d "$ANDROID_HOME"/build-tools/*/d8* | grep -v '\.jar$' | sort -V | tail -1)"
build() { # <out.dex> <source dir>
  local out
  out="$(mktemp -d)"
  javac --release 8 -classpath "$JAR" -d "$out" "$2"/dev/betterrack/android/*.java
  "$D8" --min-api 26 --lib "$JAR" --output "$out" $(find "$out" -name '*.class')
  cp "$out/classes.dex" "$1"
}
build picker.dex .
build service.dex app
