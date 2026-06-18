## Implementation pointers

- Panel lives in `viewer-api/viewer-api/frontend/dioxus/src/components/theme_settings.rs`
  (shared component, mounted by the spec-viewer's settings page).
- Store factory: `ThemeStore::new()` in
  `viewer-api/viewer-api/frontend/dioxus/src/store/theme.rs` with
  `gpu_enabled: true` default.
- The spec-viewer Trunk shell in `memory-viewers/spec-viewer/frontend/dioxus/index.html` MUST load the shared `modal.css` bundle in addition to `glass-panel.css` and `theme-settings.css`.
- Shared modal CSS lives in `viewer-api/viewer-api/frontend/dioxus/public/css/modal.css`.