## Implementation pointers

- Panel lives in `viewer-api/viewer-api/frontend/dioxus/src/components/theme_settings.rs`
  (shared component).
- Store: `ThemeStore` in `viewer-api/viewer-api/frontend/dioxus/src/store/theme.rs` with
  `gpu_enabled: true` default.
- The ticket-viewer Trunk shell in `memory-viewers/ticket-viewer/frontend/dioxus/index.html` MUST load the shared `modal.css` bundle in addition to `glass-panel.css` and `theme-settings.css`.
- Shared modal CSS lives in `viewer-api/viewer-api/frontend/dioxus/public/css/modal.css`.
- Canvas ownership arbitration with graph3d: `GPU_CANVAS_OWNER` thread-local in
  `viewer-api/viewer-api/frontend/dioxus/src/effects/mod.rs`.