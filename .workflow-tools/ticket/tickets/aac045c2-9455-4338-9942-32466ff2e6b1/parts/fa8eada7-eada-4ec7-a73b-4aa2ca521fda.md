## Validation
- `wasm-pack test --headless --chrome memory-viewers/ticket-viewer/frontend/dioxus` (and viewer-api graph3d tests)
- Manual + Playwright drag/orbit verification in external Chromium with screenshots
- `cargo check --target wasm32-unknown-unknown` for affected crates