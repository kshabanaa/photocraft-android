#!/usr/bin/env bash
# Phase 0 iteration loop: build the Rust library, package the APK, install, launch, screenshot.
# Usage: packaging/android/deploy.sh [abis] [devices]
#   abis:    default "arm64-v8a x86_64"
#   devices: adb serials, default every device adb lists as ready
#
# Two build modes, each with its own target dir so switching never throws the other's cache away:
# - draft (default), for every test round: the release profile without LTO, with 256 codegen units
#   and incremental compilation, set through environment variables (Cargo.toml unchanged).
# - FULL=1 deploy.sh ...: the release profile as committed (thin LTO, codegen-units = 1). Only for
#   size, start-up and speed measurements in the Phase 0 report.
#
# Environment: ANDROID_HOME (default: the Android Studio SDK location), ANDROID_NDK_HOME (default: the
# newest NDK under $ANDROID_HOME/ndk), JAVA_HOME (default on Windows: Android Studio's bundled JDK).
set -euo pipefail
ANDROID_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$ANDROID_DIR/../.." && pwd)
WINDOWS=0
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) WINDOWS=1 ;; esac

if [ -z "${ANDROID_HOME:-}" ]; then
  if [ "$WINDOWS" = 1 ]; then ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"; else ANDROID_HOME="$HOME/Android/Sdk"; fi
fi
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME=$(ls -d "$ANDROID_HOME"/ndk/*/ 2>/dev/null | sort -V | tail -1)
  ANDROID_NDK_HOME=${ANDROID_NDK_HOME%/}
fi
[ -d "$ANDROID_NDK_HOME" ] || { echo "no Android NDK found: set ANDROID_NDK_HOME or install one under $ANDROID_HOME/ndk" >&2; exit 1; }
export ANDROID_HOME ANDROID_NDK_HOME
if [ "$WINDOWS" = 1 ]; then
  ADB="$ANDROID_HOME/platform-tools/adb.exe"; GRADLEW=./gradlew.bat
  export JAVA_HOME="${JAVA_HOME:-/c/Program Files/Android/Android Studio/jbr}"
  # Gradle's unix-domain sockets fail under a long Windows temp path; a short directory avoids it.
  mkdir -p /c/t
  export JAVA_TOOL_OPTIONS="${JAVA_TOOL_OPTIONS:--Djdk.net.unixdomain.tmpdir=C:\\t}"
else
  ADB="$ANDROID_HOME/platform-tools/adb"; GRADLEW=./gradlew
fi

ABIS=${1:-"arm64-v8a x86_64"}
DEVICES=${2:-$(timeout 10 "$ADB" devices 2>/dev/null | awk 'NR > 1 && $1 != "" { print $1 }' | tr '\n' ' ' || true)}
SHOTS=${SHOTS:-$ANDROID_DIR/shots}
if [ "${FULL:-0}" = "1" ]; then
  MODE=full
  export CARGO_TARGET_DIR="$REPO/target/agent-android"
else
  MODE=draft
  export CARGO_TARGET_DIR="$REPO/target/agent-android-fast"
  export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=256 CARGO_PROFILE_RELEASE_INCREMENTAL=true
fi
mkdir -p "$SHOTS" "$CARGO_TARGET_DIR"
targets=(); for a in $ABIS; do targets+=(-t "$a"); done
# A failed Rust build stops the round: the APK would otherwise package the last good .so, and the
# round would test (or measure) old code without saying so.
LOG="$CARGO_TARGET_DIR/deploy-build.log"
started=$SECONDS
if (cd "$REPO" && cargo ndk "${targets[@]}" -P 29 -o "$ANDROID_DIR/app/src/main/jniLibs" build --release -p photocraft-android --features heif >"$LOG" 2>&1); then
  grep -E "^(error|warning)|-->|Finished" "$LOG" || true
  echo "rust build: $MODE mode, $((SECONDS - started))s for $ABIS (target dir ${CARGO_TARGET_DIR##*/})"
else
  grep -E "^error|-->|could not compile" "$LOG" | head -20 || true
  echo "rust build FAILED after $((SECONDS - started))s ($MODE mode); full log: $LOG" >&2
  exit 1
fi
started=$SECONDS
(cd "$ANDROID_DIR" && "$GRADLEW" --no-daemon -q assembleDebug 2>&1 | grep -vE "Picked up|Daemon|single-use|honour|problems-report|Deprecated|warning-mode|docs.gradle|^$" || true)
echo "apk packaging: $((SECONDS - started))s"
APK="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
[ -n "${DEVICES// /}" ] || { echo "no devices connected: APK built at $APK"; exit 0; }

# Every adb call is time-bounded. A device that locks, unplugs or loses its USB-debugging authorization
# mid-run makes adb wait forever (an unbounded `logcat -c` once hung this script for an hour), so a call
# that times out only skips that device's step instead of hanging or aborting the whole run.
adb_t() { local secs=$1; shift; timeout -k 5 "$secs" "$ADB" "$@"; }
ready() { [ "$(adb_t 5 -s "$1" get-state 2>/dev/null | tr -d '\r')" = "device" ]; }
step() { local S=$1 what=$2; shift 2; "$@" && return 0; echo "$S: $what failed or timed out (device unauthorized, locked or offline?)" >&2; return 1; }
filter_ready() {
  local out="" S
  for S in $DEVICES; do
    if ready "$S"; then out="$out $S"; else echo "$S: not ready (unauthorized or offline), skipped"; fi
  done
  DEVICES=$out
}

filter_ready
for S in $DEVICES; do
  ready "$S" || { echo "$S: dropped before install, skipped"; continue; }
  step "$S" install adb_t 180 -s "$S" install -r -t "$APK" >/dev/null && echo "$S: installed" || continue
  step "$S" "logcat clear" adb_t 15 -s "$S" logcat -c || true
  { adb_t 60 -s "$S" shell am start -W -n local.photocraft.spike/.PhotoCraftActivity | grep -E "TotalTime" | sed "s/^/$S: /"; } \
    || echo "$S: launch failed or timed out"
done
sleep "${WAIT:-6}"
filter_ready
for S in $DEVICES; do
  pid=$(adb_t 15 -s "$S" shell pidof local.photocraft.spike 2>/dev/null | tr -d '\r' || true)
  echo "$S: pid=${pid:-DEAD}"
  { adb_t 30 -s "$S" logcat -d -v brief | grep -E "safe area|gpu plan|Fatal|SIGSEGV|panic|could not start" | head -6 | sed "s/^/$S: /"; } || true
  if adb_t 30 -s "$S" exec-out screencap -p > "$SHOTS/$S.png"; then echo "$S: shot $SHOTS/$S.png"; else echo "$S: screenshot failed or timed out"; fi
done
