## Rationale

The spec-viewer is fully GPU-accelerated by default — the animated background,
glass panels, and particle effects are part of the visual identity of the app and
are expected to be visible on first load. Users who prefer a lightweight, static
presentation can disable the master GPU toggle in ThemeSettings; the choice is
persisted to `viewer-api-gpu-enabled` localStorage.