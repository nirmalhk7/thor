#!/usr/bin/env bash
# Fetch Thor's pinned Chromium source tree. Thor owns its feature policy and
# branding; it does not import another browser's source, patches, or resources.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPOT_TOOLS="${THOR_DEPOT_TOOLS:-$HOME/depot_tools}"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-$HOME/thor-chromium}"
MANIFEST="$ROOT/thor_browser_manifest.json"
SRC="$CHROMIUM_ROOT/src"

export PATH="$DEPOT_TOOLS:$PATH"

read_manifest() {
  python3 - "$MANIFEST" "$1" <<'PY'
import json
import sys

value = json.load(open(sys.argv[1]))
for part in sys.argv[2].split('.'):
    value = value[part]
print(value)
PY
}

CHROMIUM_REPO="$(read_manifest chromium.repository)"
CHROMIUM_REF="$(read_manifest chromium.ref)"
EXPECTED_COMMIT="$(read_manifest chromium.commit)"
PIN_REF="refs/tags/$CHROMIUM_REF"

echo "============================================================"
echo "Thor Chromium source fetch"
echo "  root:     $CHROMIUM_ROOT"
echo "  source:   $CHROMIUM_REPO"
echo "  ref:      $CHROMIUM_REF"
echo "  expect:   50–100 GB disk, multi-hour first sync"
echo "============================================================"

if [[ -e "$SRC" && ! -e "$SRC/.git" ]]; then
  echo "Refusing to replace non-git source directory: $SRC"
  exit 1
fi
if [[ -e "$SRC/.git" && -n "$(git -C "$SRC" status --porcelain)" ]]; then
  echo "Refusing to reset dirty Chromium source directory: $SRC"
  exit 1
fi

mkdir -p "$CHROMIUM_ROOT"
if [[ ! -e "$SRC/.git" ]]; then
  if ! command -v gclient >/dev/null 2>&1; then
    echo "depot_tools not found. Run ./scripts/setup_depot_tools.sh, then rerun."
    exit 1
  fi
  if [[ -f "$CHROMIUM_ROOT/.gclient" ]]; then
    echo "→ Resuming existing gclient checkout"
    (cd "$CHROMIUM_ROOT" && gclient sync --no-history -D)
  else
    if ! command -v fetch >/dev/null 2>&1; then
      echo "fetch is unavailable. Run ./scripts/setup_depot_tools.sh, then rerun."
      exit 1
    fi
    echo "→ Initial Chromium fetch"
    (cd "$CHROMIUM_ROOT" && fetch --nohooks chromium)
  fi
fi

echo "→ Checking out pinned Chromium source"
ACTUAL_REPO="$(git -C "$SRC" remote get-url origin)"
if [[ "${ACTUAL_REPO%.git}" != "${CHROMIUM_REPO%.git}" ]]; then
  echo "Chromium origin mismatch: expected $CHROMIUM_REPO, found $ACTUAL_REPO"
  exit 1
fi
git -C "$SRC" fetch --depth=2 origin "$PIN_REF:$PIN_REF"
PIN_COMMIT="$(git -C "$SRC" rev-parse "$PIN_REF^{commit}")"
if [[ "$PIN_COMMIT" != "$EXPECTED_COMMIT" ]]; then
  echo "Chromium tag $CHROMIUM_REF resolved to $PIN_COMMIT; expected $EXPECTED_COMMIT"
  exit 1
fi
git -C "$SRC" checkout --detach "$PIN_COMMIT"

if ! command -v gclient >/dev/null 2>&1; then
  echo "gclient is required after checkout. Run ./scripts/setup_depot_tools.sh, then rerun."
  exit 1
fi

echo "→ Syncing Chromium dependencies and running hooks"
(cd "$SRC" && gclient sync --no-history -D && gclient runhooks)
ACTUAL_COMMIT="$(git -C "$SRC" rev-parse HEAD)"
if [[ "$ACTUAL_COMMIT" != "$PIN_COMMIT" ]]; then
  echo "Chromium sync moved HEAD: expected $PIN_COMMIT, found $ACTUAL_COMMIT"
  exit 1
fi
python3 "$ROOT/scripts/privacy_regression_check.py" \
  --chromium-src "$SRC" --patch-state unapplied

echo "$CHROMIUM_ROOT" > "$ROOT/.thor_chromium_src_path"
echo "✓ Thor Chromium source tree ready at $SRC ($PIN_COMMIT)"
echo "  Next: ./scripts/apply_thor_branding.sh && ./scripts/build_chromium.sh"
