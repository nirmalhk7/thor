#!/usr/bin/env bash
# Apply Thor's downstream branding and build configuration to Chromium.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-}"
if [[ -z "$CHROMIUM_ROOT" && -f "$ROOT/.thor_chromium_src_path" ]]; then
  CHROMIUM_ROOT="$(cat "$ROOT/.thor_chromium_src_path")"
fi
CHROMIUM_ROOT="${CHROMIUM_ROOT:-$HOME/thor-chromium}"
SRC="$CHROMIUM_ROOT/src"
MANIFEST="$ROOT/thor_browser_manifest.json"

if [[ ! -e "$SRC/.git" ]]; then
  echo "Chromium source not found at $SRC"
  echo "Run ./scripts/fetch_chromium.sh first (or set THOR_CHROMIUM_SRC)."
  exit 1
fi
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
  echo "Refusing to brand Chromium commit $ACTUAL_COMMIT; expected $EXPECTED_COMMIT."
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
  echo "Refusing to brand Chromium $ACTUAL_VERSION; Thor is pinned to $EXPECTED_VERSION."
  echo "Fetch the pinned source into a clean checkout with ./scripts/fetch_chromium.sh"
  exit 1
fi
if [[ -n "$(git -C "$SRC" status --porcelain)" ]]; then
  echo "Refusing to apply Thor patches to a dirty Chromium source tree: $SRC"
  exit 1
fi
python3 "$ROOT/scripts/privacy_regression_check.py" \
  --chromium-src "$SRC" --patch-state unapplied

echo "→ Applying Thor-owned branding into $SRC"
shopt -s nullglob
PATCHES=("$ROOT/browser/patches/"[0-9][0-9][0-9][0-9]-*.patch)
shopt -u nullglob
if (( ${#PATCHES[@]} == 0 )); then
  echo "No numbered Thor Chromium patches found."
  exit 1
fi
for patch in "${PATCHES[@]}"; do
  # Later patches may intentionally depend on earlier patches in this ordered
  # series. Check each patch against the current sequential state immediately
  # before applying it; the browserless contract already preflights the whole
  # series atomically in a temporary exact-pin tree.
  git -C "$SRC" apply --whitespace=error --check "$patch"
  echo "→ Applying $(basename "$patch")"
  git -C "$SRC" apply --whitespace=error "$patch"
done

mkdir -p "$SRC/out/Thor"
cp "$ROOT/browser/gn_args.thor.gn" "$SRC/out/Thor/args.gn"

NM_DIR="$SRC/chrome/browser/thor"
mkdir -p "$NM_DIR"
cp "$ROOT/browser/native_messaging/com.thor.labs.json" "$NM_DIR/com.thor.labs.json"
cp "$ROOT/browser/isolation/MAPPING.md" "$NM_DIR/ISOLATION.md"
python3 "$ROOT/scripts/generate_thor_provenance.py" \
  --chromium-src "$SRC" \
  --output "$SRC/out/Thor/ThorOwnedChromium.json"
rm -rf "$SRC/out/Thor/ThorShield"
cp -R "$ROOT/browser/extensions/thor-shield" "$SRC/out/Thor/ThorShield"

THOR_VERIFY_SOURCE_INPUTS_ONLY=1 "$ROOT/scripts/verify_thor_downstream.sh"

echo "✓ Thor branding applied. Configure with:"
echo "    cd $SRC && gn gen out/Thor"
echo "  Then: ./scripts/build_chromium.sh"
