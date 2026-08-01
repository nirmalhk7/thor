#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-0.1.0}"
echo "============================================================"
echo "📦 Packaging Thor for macOS (v${VERSION})"
echo "============================================================"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "THOR_DOWNSTREAM_UNSUPPORTED: macOS packaging must run on macOS."
  exit 2
fi
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "Invalid release version: $VERSION"
  exit 1
fi

python3 "$ROOT/scripts/privacy_regression_check.py"

if [[ "${NOTARIZE:-false}" == "true" ]]; then
  for required_var in CODESIGN_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID; do
    if [[ -z "${!required_var:-}" ]]; then
      echo "${required_var} is required when NOTARIZE=true."
      exit 1
    fi
  done
fi

DIST_DIR="dist"
APP_DIR="${DIST_DIR}/Thor.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
FRAMEWORKS_DIR="${CONTENTS_DIR}/Frameworks"
OUT="${THOR_CHROMIUM_OUT:-out/Thor}"

find_source_app() {
  local chromium_root="${THOR_CHROMIUM_SRC:-}"
  if [[ -z "$chromium_root" && -f .thor_chromium_src_path ]]; then
    chromium_root="$(<.thor_chromium_src_path)"
  fi
  chromium_root="${chromium_root:-$HOME/thor-chromium}"
  for candidate in \
    "$chromium_root/src/$OUT/Thor.app" \
    "$chromium_root/src/$OUT/Chromium.app"; do
    if [[ -d "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

SRC_OUT_APP="$(find_source_app || true)"
if [[ -z "$SRC_OUT_APP" ]]; then
  echo "No Thor-branded Chromium.app found."
  echo "Build first: ./scripts/fetch_chromium.sh && ./scripts/apply_thor_branding.sh && ./scripts/build_chromium.sh"
  exit 1
fi
MARKER_DIR="$(dirname "$SRC_OUT_APP")"
if [[ ! -f "$MARKER_DIR/ThorOwnedChromium.json" ]]; then
  echo "Source browser lacks Thor-owned Chromium provenance: $SRC_OUT_APP"
  exit 1
fi
"$ROOT/scripts/verify_thor_downstream.sh"

# Ensure release binaries are compiled only after the browser-product preflight.
echo "🔨 Compiling release binaries..."
cargo build --release --bin Thor --bin thor-labs

rm -rf "${DIST_DIR}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}" "${FRAMEWORKS_DIR}"

# Copy Rust binaries (Thor product launcher & Labs sidecar)
cp "target/release/Thor" "${MACOS_DIR}/Thor"
chmod +x "${MACOS_DIR}/Thor"
if [[ -f "target/release/thor-labs" ]]; then
  cp "target/release/thor-labs" "${MACOS_DIR}/thor-labs"
  chmod +x "${MACOS_DIR}/thor-labs"
fi

cp -R "browser/extensions/thor-shield" "${RESOURCES_DIR}/ThorShield"
cp "$MARKER_DIR/ThorOwnedChromium.json" "${RESOURCES_DIR}/ThorOwnedChromium.json"

# Stage the real Thor-owned Chromium bundle into Frameworks. A snapshot or a
# generated helper wrapper is not a browser product and must never be packaged.
echo "→ Bundling Chromium from source build: $SRC_OUT_APP"
cp -R "$SRC_OUT_APP" "${FRAMEWORKS_DIR}/Chromium.app"

INNER_INFO="${FRAMEWORKS_DIR}/Chromium.app/Contents/Info.plist"
INNER_EXECUTABLE_NAME="$(plutil -extract CFBundleExecutable raw -o - "$INNER_INFO")"
INNER_EXECUTABLE="${FRAMEWORKS_DIR}/Chromium.app/Contents/MacOS/${INNER_EXECUTABLE_NAME}"
if [[ ! -x "$INNER_EXECUTABLE" ]]; then
  echo "Bundled application does not contain a browser executable."
  exit 1
fi
python3 "$ROOT/scripts/generate_thor_provenance.py" \
  --shield-dir "${RESOURCES_DIR}/ThorShield" \
  --browser-executable "$INNER_EXECUTABLE" \
  --check "${RESOURCES_DIR}/ThorOwnedChromium.json"

# Create main App Info.plist
cat <<EOF > "${CONTENTS_DIR}/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Thor</string>
    <key>CFBundleIdentifier</key>
    <string>com.thor.browser</string>
    <key>CFBundleName</key>
    <string>Thor</string>
    <key>CFBundleDisplayName</key>
    <string>Thor Browser</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
EOF

echo "APPL????" > "${CONTENTS_DIR}/PkgInfo"

echo "✅ Thor.app bundle created."

# Sign inside-out. Signing the nested browser can update its Mach-O signature,
# so refresh packaged provenance afterwards and seal that marker with the outer
# application signature.
if [[ -n "${CODESIGN_IDENTITY:-}" ]]; then
  echo "🔑 Signing bundle with identity: ${CODESIGN_IDENTITY}..."
  codesign --deep --force --options runtime --timestamp \
    --sign "${CODESIGN_IDENTITY}" "${FRAMEWORKS_DIR}/Chromium.app"
  python3 "$ROOT/scripts/generate_thor_provenance.py" \
    --shield-dir "${RESOURCES_DIR}/ThorShield" \
    --browser-executable "$INNER_EXECUTABLE" \
    --output "${RESOURCES_DIR}/ThorOwnedChromium.json"
  codesign --force --options runtime --timestamp \
    --sign "${CODESIGN_IDENTITY}" "${MACOS_DIR}/thor-labs"
  codesign --force --options runtime --timestamp \
    --sign "${CODESIGN_IDENTITY}" "${MACOS_DIR}/Thor"
  codesign --force --options runtime --timestamp \
    --sign "${CODESIGN_IDENTITY}" "${APP_DIR}"
  codesign --verify --deep --strict "${APP_DIR}"
else
  echo "ℹ️ CODESIGN_IDENTITY not provided. Skipping codesign hook."
fi

# Notarize and staple Thor.app before constructing release archives. Both the
# ZIP and DMG then contain an independently verifiable application ticket.
if [[ "${NOTARIZE:-false}" == "true" ]]; then
  NOTARY_ZIP="${DIST_DIR}/.Thor-notarization.zip"
  echo "🔐 Submitting Thor.app for Apple Notarization..."
  ditto -c -k --sequesterRsrc --keepParent "${APP_DIR}" "${NOTARY_ZIP}"
  xcrun notarytool submit "${NOTARY_ZIP}" \
    --apple-id "${APPLE_ID}" \
    --password "${APPLE_PASSWORD}" \
    --team-id "${APPLE_TEAM_ID}" \
    --wait
  rm -f "${NOTARY_ZIP}"
  xcrun stapler staple "${APP_DIR}"
  xcrun stapler validate "${APP_DIR}"
fi

# Package ZIP archive. ditto preserves application metadata and symlinks.
ZIP_NAME="Thor-v${VERSION}-macos.zip"
echo "🤐 Creating ${ZIP_NAME}..."
ditto -c -k --sequesterRsrc --keepParent "${APP_DIR}" "${DIST_DIR}/${ZIP_NAME}"

# Package DMG installer via create-dmg or hdiutil
DMG_NAME="Thor-v${VERSION}-macos.dmg"
if command -v create-dmg &> /dev/null; then
  echo "💿 Creating ${DMG_NAME} using create-dmg..."
  create-dmg \
    --volname "Thor Browser" \
    --window-pos 200 120 \
    --window-size 600 400 \
    --icon-size 100 \
    --icon "Thor.app" 150 190 \
    --hide-extension "Thor.app" \
    --app-drop-link 450 190 \
    --no-internet-enable \
    "${DIST_DIR}/${DMG_NAME}" \
    "${APP_DIR}" > /dev/null
elif command -v hdiutil &> /dev/null; then
  echo "💿 Creating ${DMG_NAME} using hdiutil..."
  STAGING_DMG="${DIST_DIR}/dmg_staging"
  rm -rf "${STAGING_DMG}"
  mkdir -p "${STAGING_DMG}"
  cp -R "${APP_DIR}" "${STAGING_DMG}/"
  ln -s /Applications "${STAGING_DMG}/Applications"
  hdiutil create -volname "Thor Browser" -srcfolder "${STAGING_DMG}" -ov -format UDZO "${DIST_DIR}/${DMG_NAME}" > /dev/null
  rm -rf "${STAGING_DMG}"
  echo "✅ DMG created successfully via hdiutil."
else
  echo "⚠️ hdiutil and create-dmg not found, skipping DMG creation."
fi

# Sign, notarize, and staple the disk image as its own release artifact too.
if [[ "${NOTARIZE:-false}" == "true" ]]; then
  if [[ ! -f "${DIST_DIR}/${DMG_NAME}" ]]; then
    echo "Cannot notarize: ${DIST_DIR}/${DMG_NAME} was not created."
    exit 1
  fi
  codesign --force --timestamp --sign "${CODESIGN_IDENTITY}" "${DIST_DIR}/${DMG_NAME}"
  echo "🔐 Submitting ${DMG_NAME} for Apple Notarization..."
  xcrun notarytool submit "${DIST_DIR}/${DMG_NAME}" \
    --apple-id "${APPLE_ID}" \
    --password "${APPLE_PASSWORD}" \
    --team-id "${APPLE_TEAM_ID}" \
    --wait
  echo "📌 Stapling notarization ticket to ${DMG_NAME}..."
  xcrun stapler staple "${DIST_DIR}/${DMG_NAME}"
  xcrun stapler validate "${DIST_DIR}/${DMG_NAME}"
fi

# Calculate SHA256 checksums
echo "🔒 Computing SHA256 checksums..."
ARTIFACTS=("${ZIP_NAME}")
if [[ -f "${DIST_DIR}/${DMG_NAME}" ]]; then
  ARTIFACTS+=("${DMG_NAME}")
fi
(cd "${DIST_DIR}" && shasum -a 256 "${ARTIFACTS[@]}" > checksums.txt)

echo "============================================================"
echo "🎉 Thor macOS Packaging Complete!"
echo "Artifacts generated in ./${DIST_DIR}/:"
ls -la "${DIST_DIR}"
echo "============================================================"
