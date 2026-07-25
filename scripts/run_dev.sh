#!/bin/bash
cd "$(dirname "$0")/.."
echo "⚡ Launching Thor Developer Browser in native macOS graphical session..."
export PATH="/opt/homebrew/bin:$PATH"
cargo run
