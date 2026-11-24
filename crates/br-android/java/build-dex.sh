#!/usr/bin/env bash
# Rebuilds java/picker.dex from the Java sources. Needs a JDK and the Android SDK (ANDROID_HOME).
set -euo pipefail
cd "$(dirname "$0")"
JAR="${ANDROID_JAR:-$(ls -d "$ANDROID_HOME"/platforms/android-*/android.jar | sort -V | tail -1)}"
D8="$(ls -d "$ANDROID_HOME"/build-tools/*/d8* | grep -v '\.jar$' | sort -V | tail -1)"
OUT="$(mktemp -d)"
javac --release 8 -classpath "$JAR" -d "$OUT" dev/betterrack/android/*.java
"$D8" --min-api 26 --lib "$JAR" --output "$OUT" $(find "$OUT" -name '*.class')
cp "$OUT/classes.dex" picker.dex
