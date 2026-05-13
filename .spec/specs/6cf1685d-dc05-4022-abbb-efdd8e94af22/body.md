# ticket-viewer: theme settings

The ticket-viewer's theme settings panel **MUST** conform to the canonical spec
[`viewer-api/theme-settings`](../viewer-api/theme-settings) — same layout, same 17 sections,
same controls, same persistence keys.

## Viewer-specific overrides

_None._ The ticket-viewer conforms to the canonical defaults verbatim:

| Field | Value |
|---|---|
| GPU master toggle default | **ON** (`viewer-api-gpu-enabled` defaults to `"true"`) |
| Default `EffectSettings` | `DEFAULT_EFFECT_SETTINGS` (all effects on at default intensity) |
| Presets | inherited from `viewer-api`'s shared preset list |

## Rationale

The ticket-viewer is fully GPU-accelerated by default — the 3D graph view, glass
panels, animated background and particle effects are part of the visual identity of
the app. The master GPU toggle is provided so users can opt out of the overlay if
they prefer a lightweight, static presentation. The graph3d view continues to use
WebGPU regardless of the master toggle (it owns the canvas via `GPU_CANVAS_OWNER`).

## Implementation pointers

- Panel lives in `tools/viewer/viewer-api/frontend/dioxus/src/components/theme_settings.rs`
  (shared component).
- Store: `ThemeStore` in `tools/viewer/viewer-api/frontend/dioxus/src/store/theme.rs` with
  `gpu_enabled: true` default.
- CSS: `tools/viewer/ticket-viewer/frontend/dioxus/public/viewer-api.css` — kept in sync with
  the shared copy under `tools/viewer/viewer-api/frontend/dioxus/public/`.
- Canvas ownership arbitration with graph3d: `GPU_CANVAS_OWNER` thread-local in
  `tools/viewer/viewer-api/frontend/dioxus/src/effects/mod.rs`.
