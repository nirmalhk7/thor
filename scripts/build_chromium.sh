#!/usr/bin/env bash
set -euo pipefail

echo "Disabled: Thor must use embedded CEF; this script will not build Chromium."
echo "Build the CEF product with: cargo build --locked --release --features cef-bundle --bin Thor --bin thor-labs"
exit 1
