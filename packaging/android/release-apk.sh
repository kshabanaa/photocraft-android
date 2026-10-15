#!/usr/bin/env bash
# Builds the APKs for a GitHub release: the committed release profile (thin LTO, codegen-units = 1),
# one APK per ABI with the native library compressed (app/build.gradle.kts, photocraft.download),
# signed with the development key until the release key exists. Prints their paths and SHA-256.
# Usage: packaging/android/release-apk.sh            (then: gh release create … the printed files)
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
SDK=${ANDROID_HOME:-${LOCALAPPDATA:-$HOME}/Android/Sdk}
export ANDROID_HOME="$SDK" ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$SDK/ndk/29.0.14206865}"
export CARGO_TARGET_DIR="$REPO/target/agent-android"
(cd "$REPO" && cargo ndk -t arm64-v8a -t x86_64 -P 29 -o "$HERE/app/src/main/jniLibs" build --release -p photocraft-android --features heif)
if [ -n "${LOCALAPPDATA:-}" ]; then
  GRADLEW=./gradlew.bat
  export JAVA_HOME=${JAVA_HOME:-"/c/Program Files/Android/Android Studio/jbr"}
  export JAVA_TOOL_OPTIONS=${JAVA_TOOL_OPTIONS:-'-Djdk.net.unixdomain.tmpdir=C:\t'}
else
  GRADLEW=./gradlew
fi
(cd "$HERE" && $GRADLEW --no-daemon -q -Pphotocraft.download=true assembleRelease)
for apk in "$HERE"/app/build/outputs/apk/release/*.apk; do
  echo "$apk  $(sha256sum "$apk" | cut -d' ' -f1)  $(du -h "$apk" | cut -f1)"
done
