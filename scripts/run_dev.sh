#!/usr/bin/env bash
set -e

echo "============================================================"
echo "⚡ Starting Thor Developer Browser in Auto-Reload Dev Mode"
echo "============================================================"

# Thor loads its embedded CEF framework from the app bundle or CEF_PATH.
# Development requires a matching CEF distribution; this script never fetches Chromium.

# Run with cargo-watch to automatically recompile & rerun on code changes
if command -v cargo-watch &> /dev/null; then
  echo "🚀 Watching src/ for changes with cargo-watch..."
  cargo watch -w src -x 'run --bin Thor'
else
  echo "🚀 Running Thor with cargo run..."
  cargo run --bin Thor
fi
