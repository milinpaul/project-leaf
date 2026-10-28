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
    cargo llvm-cov nextest --workspace {{ui_crates}} --fail-under-lines 90

# Coverage report in the browser
cov-html:
    cargo llvm-cov nextest --workspace {{ui_crates}} --html --open

# Licenses, advisories, unused deps
deny:
    cargo deny check
    cargo machete

# Core crates must compile to WASM
wasm:
    cargo check --target wasm32-unknown-unknown -p leaf-core -p leaf-model -p leaf-editor

# Validate backlog and regenerate backlog.json
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

# Launch the app
run:
    cargo run -p leaf-app

# Benchmarks
bench:
    cargo bench --workspace

# Install the dev tools once
setup:
    cargo install --locked cargo-nextest cargo-llvm-cov cargo-deny cargo-machete typos-cli just lefthook
    rustup target add wasm32-unknown-unknown
    rustup component add llvm-tools-preview
    lefthook install
