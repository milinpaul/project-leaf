#!/usr/bin/env bash
# Builds the spike into web/pkg (E14-S01). Needs: rustup target wasm32-unknown-unknown, wasm-bindgen-cli 0.2.129.
set -euo pipefail
cd "$(dirname "$0")"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-../../target/spike}"
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
  "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/leaf_web_spike.wasm"
echo "Serve web/ (e.g. python3 -m http.server -d web 8080) and open http://localhost:8080"
