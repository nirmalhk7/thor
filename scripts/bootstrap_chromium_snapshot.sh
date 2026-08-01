#!/usr/bin/env bash
# Explicit guard: stock Chromium snapshots are development-only and cannot be
# shipped as Thor because they lack Thor branding and configuration.
set -euo pipefail

echo "Snapshot bootstrap is disabled for Thor packages."
echo "Build a Thor-owned Chromium bundle with:"
echo "  ./scripts/fetch_chromium.sh && ./scripts/apply_thor_branding.sh && ./scripts/build_chromium.sh"
exit 2
