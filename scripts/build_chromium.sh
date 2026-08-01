#!/usr/bin/env bash
# Build Thor's branded Chromium product and ChromeDriver into out/Thor.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPOT_TOOLS="${THOR_DEPOT_TOOLS:-$HOME/depot_tools}"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-}"
if [[ -z "$CHROMIUM_ROOT" && -f "$ROOT/.thor_chromium_src_path" ]]; then
  CHROMIUM_ROOT="$(cat "$ROOT/.thor_chromium_src_path")"
fi
CHROMIUM_ROOT="${CHROMIUM_ROOT:-$HOME/thor-chromium}"
SRC="$CHROMIUM_ROOT/src"
OUT="${THOR_CHROMIUM_OUT:-out/Thor}"
TARGET="${1:-all}" # chrome | chromedriver | all

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "THOR_DOWNSTREAM_UNSUPPORTED: this build script currently produces and verifies only the macOS Thor browser bundle."
  exit 2
fi

if [[ -d "$DEPOT_TOOLS" ]]; then
  export PATH="$DEPOT_TOOLS:$PATH"
fi

if [[ ! -d "$SRC" ]]; then
  echo "Missing $SRC — run ./scripts/fetch_chromium.sh"
  exit 1
fi

# Toolchain and dependency checks must run before build_chromium.sh applies any
# downstream patch. A missing local prerequisite must not turn a clean pinned
# Chromium checkout into a dirty, partially prepared tree.
if ! xcodebuild -version >/dev/null 2>&1 && \
   [[ -z "${DEVELOPER_DIR:-}" && -x "/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild" ]]; then
  export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
  echo "→ Using installed Xcode at $DEVELOPER_DIR"
fi
if ! xcodebuild -version >/dev/null 2>&1; then
  echo "A licensed full Xcode installation is required to build Chromium."
  echo "Command Line Tools alone are insufficient; Chromium invokes xcodebuild during GN generation."
  exit 1
fi
if ! xcodebuild -checkFirstLaunchStatus >/dev/null 2>&1; then
  echo "Xcode first-launch setup or license acceptance is incomplete."
  echo "The user must run: sudo xcodebuild -license accept"
  exit 1
fi
if ! xcrun --sdk macosx --show-sdk-path >/dev/null; then
  echo "Xcode macOS SDK is unavailable."
  exit 1
fi

if ! command -v autoninja >/dev/null 2>&1; then
  echo "autoninja not found — run ./scripts/setup_depot_tools.sh or set THOR_DEPOT_TOOLS."
  exit 1
fi

if [[ ! -f "$CHROMIUM_ROOT/.gclient_entries" ]]; then
  echo "Chromium dependency metadata is missing: $CHROMIUM_ROOT/.gclient_entries"
  echo "Run ./scripts/fetch_chromium.sh to complete the pinned gclient sync and hooks."
  exit 1
fi

declare -a REQUIRED_SYNCED_FILES=(
  "buildtools/mac/gn"
  "third_party/angle/dotfile_settings.gni"
  "third_party/icu/source/common/unicode/utypes.h"
  "third_party/llvm-build/Release+Asserts/bin/clang"
  "v8/BUILD.gn"
)
if [[ "$(uname -m)" == "arm64" ]]; then
  REQUIRED_SYNCED_FILES+=("third_party/node/mac_arm64/node-darwin-arm64/bin/node")
else
  REQUIRED_SYNCED_FILES+=("third_party/node/mac/node-darwin-x64/bin/node")
fi
for required_file in "${REQUIRED_SYNCED_FILES[@]}"; do
  if [[ ! -f "$SRC/$required_file" ]]; then
    echo "Chromium dependency or hook output is missing: $SRC/$required_file"
    echo "Run ./scripts/fetch_chromium.sh to complete the pinned gclient sync and hooks."
    exit 1
  fi
done
for required_executable in \
  "$SRC/buildtools/mac/gn" \
  "$SRC/third_party/llvm-build/Release+Asserts/bin/clang"; do
  if [[ ! -x "$required_executable" ]]; then
    echo "Chromium build tool is not executable: $required_executable"
    exit 1
  fi
done

if [[ ! -f "$SRC/$OUT/args.gn" ]]; then
  "$ROOT/scripts/apply_thor_branding.sh"
fi
python3 "$ROOT/scripts/privacy_regression_check.py" \
  --chromium-src "$SRC" --patch-state applied
THOR_VERIFY_SOURCE_INPUTS_ONLY=1 "$ROOT/scripts/verify_thor_downstream.sh"

cd "$SRC"

GN="$SRC/buildtools/mac/gn"
echo "→ Using Chromium's pinned GN: $GN"

echo "→ $GN gen $OUT --fail-on-unused-args"
"$GN" gen "$OUT" --fail-on-unused-args

build_one() {
  local t="$1"
  echo "→ autoninja -C $OUT $t"
  autoninja -C "$OUT" "$t"
}

case "$TARGET" in
  chrome)
    build_one chrome
    ;;
  chromedriver)
    build_one chromedriver
    ;;
  all)
    build_one chrome
    build_one chromedriver
    ;;
  *)
    build_one "$TARGET"
    ;;
esac

APP=""
for candidate in "$SRC/$OUT/Thor.app" "$SRC/$OUT/Chromium.app"; do
  if [[ -d "$candidate" ]]; then
    APP="$candidate"
    break
  fi
done
if [[ -n "$APP" ]]; then
  INFO_PLIST="$APP/Contents/Info.plist"
  [[ -f "$INFO_PLIST" ]] || { echo "Built browser Info.plist is missing: $INFO_PLIST"; exit 1; }
  BUNDLE_EXECUTABLE="$(plutil -extract CFBundleExecutable raw -o - "$INFO_PLIST")"
  BROWSER_EXECUTABLE="$APP/Contents/MacOS/$BUNDLE_EXECUTABLE"
  [[ -x "$BROWSER_EXECUTABLE" ]] || { echo "Built browser executable is missing: $BROWSER_EXECUTABLE"; exit 1; }
  python3 "$ROOT/scripts/generate_thor_provenance.py" \
    --chromium-src "$SRC" \
    --browser-executable "$BROWSER_EXECUTABLE" \
    --output "$SRC/$OUT/ThorOwnedChromium.json"
  echo "✓ Built: $APP"
  "$ROOT/scripts/verify_thor_downstream.sh"
else
  echo "Expected macOS browser bundle was not produced in $SRC/$OUT"
  exit 1
fi
echo "  Run: ./scripts/run_thor_chromium.sh"
