# Leaf task runner. `just` lists tasks.

ui_crates := "--exclude leaf-ui --exclude leaf-app"

default:
    @just --list

# Format all code
fmt:
    cargo fmt --all

# Lint: format check, clippy (warnings = errors), spelling
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    typos

# Run all tests
test:
    cargo nextest run --workspace --locked
    cargo test --doc --workspace --locked

# Coverage with the 90% gate on core crates
cov:
    cargo llvm-cov nextest --workspace {{ui_crates}} --locked --fail-under-lines 90

# Coverage report in the browser
cov-html:
    cargo llvm-cov nextest --workspace {{ui_crates}} --html --open

# Licenses, advisories, unused deps
deny:
    cargo deny check
    cargo machete

# Core crates must compile to WASM
wasm:
    cargo check --locked --target wasm32-unknown-unknown -p leaf-core -p leaf-model -p leaf-editor

# Validate backlog and regenerate backlog.json (local only, gitignored)
backlog:
    python3 scripts/backlog.py

# Next ready story for an agent
next:
    python3 scripts/backlog.py next

# Live board with auto-refresh at http://127.0.0.1:4317
serve:
    python3 scripts/backlog.py serve --open

# Story counts per status
board:
    python3 scripts/backlog.py summary

# Everything CI runs (run before declaring work done)
check: lint test cov deny wasm
    python3 scripts/backlog.py --check
    python3 -m unittest discover -s scripts -p 'test_*.py'

# Launch the app (on macOS from a Leaf.app bundle, so the Dock shows its name and icon)
[macos]
run *args:
    cargo build -p leaf-app
    python3 scripts/macos_app.py target/debug/leaf-app target/debug
    target/debug/Leaf.app/Contents/MacOS/Leaf {{args}}

# Launch the app
[linux]
[windows]
run *args:
    cargo run -p leaf-app -- {{args}}

# Screenshot a workspace off screen (works with the screen locked; macOS):
# just screenshot <workspace folder> <out.png> [first block] [dark] [bench]
screenshot *args:
    cargo run -q -p leaf-ui --example screenshot -- {{args}}

# Regenerate the app icons from crates/leaf-app/resources/icon.svg (needs resvg; .icns needs macOS)
icons:
    python3 scripts/icons.py

# Benchmarks
bench:
    cargo bench --workspace

# Install the dev tools once. lefthook is a Go tool, not a crate: install it first with
# `brew install lefthook` (macOS/Linux) or `scoop install lefthook` (Windows).
setup:
    cargo install --locked cargo-nextest cargo-llvm-cov cargo-deny cargo-machete typos-cli just resvg
    rustup target add wasm32-unknown-unknown
    rustup component add llvm-tools-preview
    lefthook install
