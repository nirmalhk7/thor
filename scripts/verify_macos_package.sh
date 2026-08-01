#!/usr/bin/env bash
# Verify generated macOS release artifacts without launching the browser.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-0.1.0}"
DIST="$ROOT/dist"
APP="$DIST/Thor.app"
ZIP="$DIST/Thor-v${VERSION}-macos.zip"
DMG="$DIST/Thor-v${VERSION}-macos.dmg"
CHECKSUMS="$DIST/checksums.txt"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "THOR_DOWNSTREAM_UNSUPPORTED: macOS artifact verification must run on macOS."
  exit 2
fi

for path in "$APP" "$ZIP" "$DMG" "$CHECKSUMS"; do
  if [[ ! -e "$path" ]]; then
    echo "Missing macOS release artifact: $path"
    exit 1
  fi
done
[[ -s "$ZIP" ]] || { echo "Empty macOS ZIP: $ZIP"; exit 1; }
[[ -s "$DMG" ]] || { echo "Empty macOS DMG: $DMG"; exit 1; }
[[ -s "$CHECKSUMS" ]] || { echo "Empty checksum manifest: $CHECKSUMS"; exit 1; }

verify_app() {
  local candidate="$1"
  local label="$2"
  local bundle_id bundle_executable inner_info inner_bundle_id
  local inner_executable_name inner_executable asset

  [[ -d "$candidate" ]] || { echo "$label Thor.app is missing."; exit 1; }
  bundle_id="$(plutil -extract CFBundleIdentifier raw -o - "$candidate/Contents/Info.plist")"
  bundle_executable="$(plutil -extract CFBundleExecutable raw -o - "$candidate/Contents/Info.plist")"
  [[ "$bundle_id" == "com.thor.browser" ]] || {
    echo "$label has unexpected outer bundle identifier: $bundle_id"
    exit 1
  }
  [[ -x "$candidate/Contents/MacOS/$bundle_executable" ]] || {
    echo "$label launcher is missing or not executable."
    exit 1
  }
  [[ -x "$candidate/Contents/MacOS/thor-labs" ]] || {
    echo "$label Labs sidecar is missing or not executable."
    exit 1
  }
  inner_info="$candidate/Contents/Frameworks/Chromium.app/Contents/Info.plist"
  [[ -f "$inner_info" ]] || { echo "$label Chromium Info.plist is missing."; exit 1; }
  inner_bundle_id="$(plutil -extract CFBundleIdentifier raw -o - "$inner_info")"
  inner_executable_name="$(plutil -extract CFBundleExecutable raw -o - "$inner_info")"
  inner_executable="$candidate/Contents/Frameworks/Chromium.app/Contents/MacOS/$inner_executable_name"
  [[ "$inner_bundle_id" == "com.thor.browser" ]] || {
    echo "$label has unexpected Chromium bundle identifier: $inner_bundle_id"
    exit 1
  }
  [[ -x "$inner_executable" ]] || {
    echo "$label Chromium executable is missing or not executable."
    exit 1
  }
  for asset in manifest.json service_worker.js fingerprint.js rules.json filter_bundle.json; do
    [[ -s "$candidate/Contents/Resources/ThorShield/$asset" ]] || {
      echo "$label Thor Shield asset is missing or empty: $asset"
      exit 1
    }
  done
  [[ -f "$candidate/Contents/Resources/ThorOwnedChromium.json" ]] || {
    echo "$label provenance marker is missing."
    exit 1
  }
  python3 "$ROOT/scripts/generate_thor_provenance.py" \
    --shield-dir "$candidate/Contents/Resources/ThorShield" \
    --browser-executable "$inner_executable" \
    --check "$candidate/Contents/Resources/ThorOwnedChromium.json"
}

verify_app "$APP" "staged application"

(cd "$DIST" && shasum -a 256 -c checksums.txt)
unzip -tq "$ZIP" >/dev/null
unzip -Z1 "$ZIP" | grep -q '^Thor.app/Contents/Frameworks/Chromium.app/' || {
  echo "ZIP does not contain Thor-owned Chromium."
  exit 1
}
hdiutil verify "$DMG" >/dev/null

VERIFY_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/thor-package-verify.XXXXXX")"
DMG_MOUNT="$VERIFY_ROOT/dmg"
DMG_ATTACHED=false
cleanup() {
  if [[ "$DMG_ATTACHED" == "true" ]]; then
    hdiutil detach "$DMG_MOUNT" >/dev/null || true
  fi
  rm -rf "$VERIFY_ROOT"
}
trap cleanup EXIT

mkdir -p "$VERIFY_ROOT/zip" "$DMG_MOUNT"
ditto -x -k "$ZIP" "$VERIFY_ROOT/zip"
ZIP_APP="$VERIFY_ROOT/zip/Thor.app"
verify_app "$ZIP_APP" "ZIP application"

hdiutil attach -readonly -nobrowse -mountpoint "$DMG_MOUNT" "$DMG" >/dev/null
DMG_ATTACHED=true
DMG_APP="$DMG_MOUNT/Thor.app"
verify_app "$DMG_APP" "DMG application"

if [[ "${REQUIRE_NOTARIZED:-false}" == "true" ]]; then
  for signed_app in "$APP" "$ZIP_APP" "$DMG_APP"; do
    codesign --verify --deep --strict "$signed_app"
    xcrun stapler validate "$signed_app"
    spctl --assess --type execute --verbose=2 "$signed_app"
  done
  xcrun stapler validate "$DMG"
fi

hdiutil detach "$DMG_MOUNT" >/dev/null
DMG_ATTACHED=false

echo "✓ Verified macOS Thor package artifacts for v$VERSION"
