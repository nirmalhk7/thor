#!/usr/bin/env bash
set -e

echo "============================================================"
echo "⚡ Starting Thor Developer Browser in Auto-Reload Dev Mode"
echo "============================================================"

# Prefer Thor-owned Chromium (snapshot or source build). Legacy dual-window:
#   THOR_LEGACY_DUAL_WINDOW=1 ./scripts/run_dev.sh

CACHE_EXE="${HOME}/Library/Application Support/Thor/ChromiumCache/Chromium.app/Contents/MacOS/Chromium"
if [[ ! -x "$CACHE_EXE" ]] \
   && [[ -z "${THOR_CHROMIUM_PATH:-}" ]] \
   && [[ "${THOR_LEGACY_DUAL_WINDOW:-}" != "1" ]]; then
  echo "→ No owned Chromium cache — bootstrapping snapshot..."
  "$(cd "$(dirname "$0")" && pwd)/bootstrap_chromium_snapshot.sh" || true
fi

# Disable git hook conflicts if present
git config core.hooksPath /dev/null || true

# Run with cargo-watch to automatically recompile & rerun on code changes
if command -v cargo-watch &> /dev/null; then
  echo "🚀 Watching src/ for changes with cargo-watch..."
  cargo watch -w src -x 'run --bin Thor'
else
  echo "🚀 Running Thor with cargo run..."
  cargo run --bin Thor
fi
