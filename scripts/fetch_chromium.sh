#!/usr/bin/env bash
# Fetch pinned Chromium source into THOR_CHROMIUM_SRC (outside the Rust tree by default).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPOT_TOOLS="${THOR_DEPOT_TOOLS:-$HOME/depot_tools}"
CHROMIUM_ROOT="${THOR_CHROMIUM_SRC:-$HOME/thor-chromium}"
MANIFEST="$ROOT/chromium_manifest.json"

export PATH="$DEPOT_TOOLS:$PATH"

if ! command -v fetch >/dev/null 2>&1; then
  echo "depot_tools not on PATH. Run ./scripts/setup_depot_tools.sh first."
  exit 1
fi

REV="$(python3 -c "import json; print(json.load(open('$MANIFEST')).get('chromium_src_tag') or json.load(open('$MANIFEST'))['revision'])")"
echo "============================================================"
echo "Thor Chromium fetch"
echo "  root:     $CHROMIUM_ROOT"
echo "  pin:      $REV"
echo "  expect:   50–100 GB disk, multi-hour first sync"
echo "============================================================"

mkdir -p "$CHROMIUM_ROOT"
cd "$CHROMIUM_ROOT"

if [[ -d src/.git ]]; then
  echo "→ Existing checkout — gclient sync"
  cd src
  git fetch origin
  # Prefer numeric snapshot position via gclient; fall back to sync at HEAD of main.
  gclient sync --with_branch_heads --with_tags -D
else
  echo "→ Initial fetch --nohooks chromium (no history depth limit)"
  fetch --nohooks chromium
  cd src
  gclient runhooks
fi

echo "$CHROMIUM_ROOT" > "$ROOT/.thor_chromium_src_path"
echo "✓ Chromium tree ready at $CHROMIUM_ROOT/src"
echo "  Next: ./scripts/apply_thor_branding.sh && ./scripts/build_chromium.sh"
