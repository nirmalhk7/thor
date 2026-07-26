#!/usr/bin/env bash
set -e

echo "============================================================"
echo "⚡ Starting Thor Developer Browser in Auto-Reload Dev Mode"
echo "============================================================"

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
