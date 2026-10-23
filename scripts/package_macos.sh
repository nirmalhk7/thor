#!/usr/bin/env bash
# Final build/package step: invoke only after implementation and quality gates.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
VERSION="$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -n 1)"
if [[ -n "${1:-}" && "$1" != "$VERSION" ]]; then
  echo "Requested version differs from Cargo.toml ($VERSION)" >&2
  exit 1
fi

cargo build --release --features cef-bundle --bin Thor --bin thor-labs --bin thor-package
TARGET_DIR="${CARGO_TARGET_DIR:-target}/release"
"$TARGET_DIR/thor-package" dist/cef

APP="dist/cef/Thor.app"
FRAMEWORK="$APP/Contents/Frameworks/Chromium Embedded Framework.framework"
[[ -d "$FRAMEWORK" ]] || { echo "CEF framework missing from bundle" >&2; exit 1; }
[[ -x "$APP/Contents/MacOS/Thor" ]] || { echo "Thor executable missing" >&2; exit 1; }
[[ ! -e "$APP/Contents/Frameworks/Chromium.app" ]] || { echo "Legacy Chromium found in CEF bundle" >&2; exit 1; }
for helper in "Helper" "Helper (GPU)" "Helper (Renderer)" "Helper (Plugin)" "Helper (Alerts)"; do
  [[ -x "$APP/Contents/Frameworks/Thor $helper.app/Contents/MacOS/Thor $helper" ]] || {
    echo "CEF helper missing: $helper" >&2
    exit 1
  }
done

ZIP="dist/Thor-v${VERSION}-macos.zip"
DMG="dist/Thor-v${VERSION}-macos.dmg"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"
hdiutil create -volname Thor -srcfolder "$APP" -ov -format UDZO "$DMG"
shasum -a 256 "$ZIP" "$DMG" > dist/checksums.txt
echo "Staged CEF application and development installers in dist/. Signing/notarization required for distribution."
