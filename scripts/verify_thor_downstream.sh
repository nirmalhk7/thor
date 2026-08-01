#!/usr/bin/env bash
# Verify that the local Chromium source tree is Thor-branded and package-ready.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-}"
if [[ -z "$CHROMIUM_ROOT" && -f "$ROOT/.thor_chromium_src_path" ]]; then
  CHROMIUM_ROOT="$(cat "$ROOT/.thor_chromium_src_path")"
fi
CHROMIUM_ROOT="${CHROMIUM_ROOT:-$HOME/thor-chromium}"
SRC="$CHROMIUM_ROOT/src"
OUT="${THOR_CHROMIUM_OUT:-out/Thor}"
MANIFEST="$ROOT/thor_browser_manifest.json"

[[ -e "$SRC/.git" ]] || { echo "Missing Chromium source: $SRC"; exit 1; }
python3 "$ROOT/scripts/privacy_regression_check.py" \
  --chromium-src "$SRC" --patch-state applied
EXPECTED_VERSION="$(python3 - "$MANIFEST" <<'PY'
import json
import sys

print(json.load(open(sys.argv[1]))["chromium"]["ref"])
PY
)"
EXPECTED_COMMIT="$(python3 - "$MANIFEST" <<'PY'
import json
import sys

print(json.load(open(sys.argv[1]))["chromium"]["commit"])
PY
)"
PIN_REF="refs/tags/$EXPECTED_VERSION"
if ! PIN_COMMIT="$(git -C "$SRC" rev-parse "$PIN_REF^{commit}" 2>/dev/null)"; then
  echo "Missing local Chromium pin $PIN_REF; rerun ./scripts/fetch_chromium.sh"
  exit 1
fi
ACTUAL_COMMIT="$(git -C "$SRC" rev-parse HEAD)"
if [[ "$PIN_COMMIT" != "$EXPECTED_COMMIT" || "$ACTUAL_COMMIT" != "$EXPECTED_COMMIT" ]]; then
  echo "Chromium source commit mismatch: expected $EXPECTED_COMMIT, found $ACTUAL_COMMIT"
  exit 1
fi
EXPECTED_REPO="$(python3 - "$MANIFEST" <<'PY'
import json
import sys

print(json.load(open(sys.argv[1]))["chromium"]["repository"])
PY
)"
ACTUAL_REPO="$(git -C "$SRC" remote get-url origin)"
if [[ "${ACTUAL_REPO%.git}" != "${EXPECTED_REPO%.git}" ]]; then
  echo "Chromium origin mismatch: expected $EXPECTED_REPO, found $ACTUAL_REPO"
  exit 1
fi
ACTUAL_VERSION="$(awk -F= '
  $1 == "MAJOR" { major=$2 }
  $1 == "MINOR" { minor=$2 }
  $1 == "BUILD" { build=$2 }
  $1 == "PATCH" { patch=$2 }
  END { print major "." minor "." build "." patch }
' "$SRC/chrome/VERSION")"
if [[ "$ACTUAL_VERSION" != "$EXPECTED_VERSION" ]]; then
  echo "Chromium source version mismatch: expected $EXPECTED_VERSION, found $ACTUAL_VERSION"
  echo "Fetch the pinned source into a clean checkout with ./scripts/fetch_chromium.sh"
  exit 1
fi

[[ -f "$SRC/$OUT/args.gn" ]] || { echo "Missing Thor GN args: $SRC/$OUT/args.gn"; exit 1; }
cmp -s "$ROOT/browser/gn_args.thor.gn" "$SRC/$OUT/args.gn" || {
  echo "Thor GN args are stale; rerun ./scripts/apply_thor_branding.sh"
  exit 1
}
[[ -f "$SRC/$OUT/ThorOwnedChromium.json" ]] || { echo "Missing Thor-owned Chromium provenance marker"; exit 1; }
[[ -f "$SRC/$OUT/ThorShield/manifest.json" ]] || { echo "Missing staged Thor Shield extension"; exit 1; }
APP=""
BROWSER_EXECUTABLE=""
for candidate in "$SRC/$OUT/Thor.app" "$SRC/$OUT/Chromium.app"; do
  if [[ -d "$candidate" ]]; then
    APP="$candidate"
    BUNDLE_EXECUTABLE="$(plutil -extract CFBundleExecutable raw -o - "$APP/Contents/Info.plist")"
    BROWSER_EXECUTABLE="$APP/Contents/MacOS/$BUNDLE_EXECUTABLE"
    break
  fi
done
PROVENANCE_ARGS=(
  --chromium-src "$SRC"
  --check "$SRC/$OUT/ThorOwnedChromium.json"
)
if [[ "${THOR_VERIFY_SOURCE_INPUTS_ONLY:-0}" == "1" ]]; then
  PROVENANCE_ARGS+=(--source-inputs-only)
elif [[ -n "$BROWSER_EXECUTABLE" ]]; then
  [[ -x "$BROWSER_EXECUTABLE" ]] || { echo "Built browser executable is missing: $BROWSER_EXECUTABLE"; exit 1; }
  PROVENANCE_ARGS+=(--browser-executable "$BROWSER_EXECUTABLE")
fi
python3 "$ROOT/scripts/generate_thor_provenance.py" "${PROVENANCE_ARGS[@]}"
[[ -f "$SRC/chrome/app/theme/chromium/BRANDING" ]] || { echo "Missing Chromium branding file"; exit 1; }
grep -q '^PRODUCT_SHORTNAME=Thor$' "$SRC/chrome/app/theme/chromium/BRANDING" || {
  echo "Chromium source is not Thor-branded"
  exit 1
}

shopt -s nullglob
PATCHES=("$ROOT/browser/patches/"[0-9][0-9][0-9][0-9]-*.patch)
shopt -u nullglob
if (( ${#PATCHES[@]} == 0 )); then
  echo "No numbered Thor Chromium patches found."
  exit 1
fi
# privacy_regression_check.py --patch-state applied above reconstructs the
# pristine exact-pin surface, applies the complete ordered patch series in a
# temporary tree, and byte-compares that final surface with this checkout.
# Reversing patches independently is invalid when later patches intentionally
# overlap earlier files.

PATCH_PATHS="$(sed -n 's|^+++ b/||p' "${PATCHES[@]}" | sort -u)"
while IFS= read -r changed_path; do
  [[ -z "$changed_path" ]] && continue
  if ! grep -Fqx "$changed_path" <<< "$PATCH_PATHS"; then
    echo "Unexpected tracked Chromium source modification: $changed_path"
    exit 1
  fi
done < <(git -C "$SRC" diff --name-only)
if [[ -n "$(git -C "$SRC" diff --cached --name-only)" ]]; then
  echo "Chromium source contains staged modifications."
  exit 1
fi
while IFS= read -r untracked_path; do
  [[ -z "$untracked_path" ]] && continue
  case "$untracked_path" in
    chrome/browser/thor/*) ;;
    *)
      if ! grep -Fqx "$untracked_path" <<< "$PATCH_PATHS"; then
        echo "Unexpected untracked Chromium source file: $untracked_path"
        exit 1
      fi
      ;;
  esac
done < <(git -C "$SRC" ls-files --others --exclude-standard)

cmp -s "$ROOT/browser/native_messaging/com.thor.labs.json" \
  "$SRC/chrome/browser/thor/com.thor.labs.json" || {
  echo "Staged native-messaging manifest is stale."
  exit 1
}
cmp -s "$ROOT/browser/isolation/MAPPING.md" \
  "$SRC/chrome/browser/thor/ISOLATION.md" || {
  echo "Staged isolation mapping is stale."
  exit 1
}
diff -qr "$ROOT/browser/extensions/thor-shield" "$SRC/$OUT/ThorShield" >/dev/null || {
  echo "Staged Thor Shield assets are stale."
  exit 1
}

if [[ -n "$APP" && "${THOR_VERIFY_SOURCE_INPUTS_ONLY:-0}" != "1" ]]; then
  INFO_PLIST="$APP/Contents/Info.plist"
  [[ -f "$INFO_PLIST" ]] || { echo "Missing browser Info.plist: $INFO_PLIST"; exit 1; }
  BUNDLE_ID="$(plutil -extract CFBundleIdentifier raw -o - "$INFO_PLIST")"
  BUNDLE_EXECUTABLE="$(plutil -extract CFBundleExecutable raw -o - "$INFO_PLIST")"
  [[ "$BUNDLE_ID" == "com.thor.browser" ]] || {
    echo "Built browser has unexpected bundle identifier: $BUNDLE_ID"
    exit 1
  }
  [[ -x "$APP/Contents/MacOS/$BUNDLE_EXECUTABLE" ]] || {
    echo "Built browser executable is missing: $APP/Contents/MacOS/$BUNDLE_EXECUTABLE"
    exit 1
  }
  echo "✓ Verified Thor-branded Chromium bundle: $APP"
else
  echo "✓ Verified Thor-branded Chromium source tree; bundle has not been built yet."
fi
