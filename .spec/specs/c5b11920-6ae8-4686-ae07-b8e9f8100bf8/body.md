# spec-viewer: theme settings

The spec-viewer's theme settings panel **MUST** conform to the canonical spec
[`viewer-api/theme-settings`](../viewer-api/theme-settings) — same layout, same 17 sections,
same controls, same persistence keys.

## Viewer-specific overrides

_None._ The spec-viewer conforms to the canonical defaults verbatim:

| Field | Value |
|---|---|
| GPU master toggle default | **ON** (`viewer-api-gpu-enabled` defaults to `"true"`) |
| Default `EffectSettings` | `DEFAULT_EFFECT_SETTINGS` (all effects on at default intensity) |
| Presets | inherited from `viewer-api`'s shared preset list |

## Rationale

The spec-viewer is fully GPU-accelerated by default — the animated background,
glass panels, and particle effects are part of the visual identity of the app and
are expected to be visible on first load. Users who prefer a lightweight, static
presentation can disable the master GPU toggle in ThemeSettings; the choice is
persisted to `viewer-api-gpu-enabled` localStorage.

## Implementation pointers

- Panel lives in `tools/viewer/viewer-api/frontend/dioxus/src/components/theme_settings.rs`
  (shared component, mounted by the spec-viewer's settings page).
- Store factory: `ThemeStore::new()` in
  `tools/viewer/viewer-api/frontend/dioxus/src/store/theme.rs` with
  `gpu_enabled: true` default.
- CSS: `tools/viewer/spec-viewer/frontend/dioxus/public/viewer-api.css` — kept in sync with
  the shared copy under `tools/viewer/viewer-api/frontend/dioxus/public/`.
