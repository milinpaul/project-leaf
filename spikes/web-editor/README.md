# Web editor spike (E14-S01)

Throwaway code: Leaf's `leaf-editor` running in a browser through WebAssembly, with two tabs kept in step. It is
**not** part of the workspace build, CI, coverage or `just check`. Findings are in
[docs/spikes/E14-S01-web-editor.md](../../docs/spikes/E14-S01-web-editor.md).

## Run it

Needs the `wasm32-unknown-unknown` target (already in `rust-toolchain.toml`) and `wasm-bindgen-cli` 0.2.129
(`cargo install wasm-bindgen-cli --version 0.2.129 --locked`).

```
./build.sh                                  # builds web/pkg
python3 -m http.server -d web 8080          # then open http://localhost:8080 in two tabs
node drive.cjs /tmp                         # optional: types in two headless tabs, checks they converge, saves screenshots
```

`drive.cjs` uses Playwright (`npm install playwright` if it isn't installed).

## Files

- `src/lib.rs`: the WebAssembly module, one `PageEditor` plus HTML drawing from its view rows.
- `web/index.html`, `web/main.js`: the page, a hidden textarea for typing, and sync over a `BroadcastChannel`.
- `drive.cjs`: the two-tab test and the measurements.
- `Cargo.lock`: copied from the workspace so the spike builds the same dependency versions.
