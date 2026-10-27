#!/usr/bin/env bash
set -euo pipefail

echo "Disabled: Thor must use embedded CEF; this script will not download a Chromium snapshot."
echo "The CEF build uses the pinned prebuilt CEF distribution only."
exit 1
