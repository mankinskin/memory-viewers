<!-- rule-api:file generated=true -->

<!-- rule-api:entry id=1ca2fc7e-3b33-4a7c-899e-7eb635abbaf5 slug=ticket-viewer/theme-settings/ticket-viewer-theme-settings/l1 -->
# ticket-viewer: theme settings

The ticket-viewer's theme settings panel **MUST** conform to the canonical spec
[`viewer-api/theme-settings`](../viewer-api/theme-settings) — same layout, same 17 sections,
same controls, same persistence keys.

<!-- rule-api:entry id=59a1678d-9e8c-4daf-bafc-4898b2afc164 slug=memory-viewers/theme-settings/open-close-contract -->
## Open/close contract

- Clicking the header `Theme settings` action MUST open a visible modal overlay, not just mount the panel in the DOM.
- The overlay MUST use `.modal-backdrop[role="dialog"][aria-label="Theme settings"]` with fixed positioning, a non-transparent backdrop tint, and a centered `.modal-panel.theme-settings-modal` container.
- The underlying page MUST remain visibly dimmed behind the modal while the theme settings panel is open.

<!-- rule-api:entry id=e8b6fa08-6fe8-4c98-9523-6b6ad21b66c5 slug=memory-viewers/theme-settings/viewer-specific-overrides -->
## Viewer-specific overrides

_None._ The viewer conforms to the canonical defaults verbatim:

| Field | Value |
|---|---|
| GPU master toggle default | **ON** (`viewer-api-gpu-enabled` defaults to `"true"`) |
| Default `EffectSettings` | `DEFAULT_EFFECT_SETTINGS` (all effects on at default intensity) |
| Presets | inherited from `viewer-api`'s shared preset list |

<!-- rule-api:entry id=8385525a-5e3d-4c76-9adb-4d0b33fdd027 slug=ticket-viewer/theme-settings/ticket-viewer-theme-settings/rationale/l23 -->
## Rationale

The ticket-viewer is fully GPU-accelerated by default — the 3D graph view, glass
panels, animated background and particle effects are part of the visual identity of
the app. The master GPU toggle is provided so users can opt out of the overlay if
they prefer a lightweight, static presentation. The graph3d view continues to use
WebGPU regardless of the master toggle (it owns the canvas via `GPU_CANVAS_OWNER`).

<!-- rule-api:entry id=b95c38f7-7cd6-47d5-9ecf-d13645e0bf29 slug=ticket-viewer/theme-settings/ticket-viewer-theme-settings/implementation-pointers/l31 -->
## Implementation pointers

- Panel lives in `memory-viewers/viewer-api/viewer-api/frontend/dioxus/src/components/theme_settings.rs`
  (shared component).
- Store: `ThemeStore` in `memory-viewers/viewer-api/viewer-api/frontend/dioxus/src/store/theme.rs` with
  `gpu_enabled: true` default.
- The ticket-viewer Trunk shell in `memory-viewers/ticket-viewer/frontend/dioxus/index.html` MUST load the shared `modal.css` bundle in addition to `glass-panel.css` and `theme-settings.css`.
- Shared modal CSS lives in `memory-viewers/viewer-api/viewer-api/frontend/dioxus/public/css/modal.css`.
- Canvas ownership arbitration with graph3d: `GPU_CANVAS_OWNER` thread-local in
  `memory-viewers/viewer-api/viewer-api/frontend/dioxus/src/effects/mod.rs`.

<!-- rule-api:entry id=a22fae2d-f471-4cd9-8241-5f1607d235fb slug=ticket-viewer/theme-settings/ticket-viewer-theme-settings/validation/l42 -->
## Validation

- `npm run test:e2e:release -- e2e-release/ticket-viewer.release.spec.ts -g "theme settings palette button opens and closes the theme settings panel"` in `memory-viewers/ticket-viewer/frontend/dioxus`
- The focused Playwright test MUST attach a screenshot of `.theme-settings.glass-panel` so the open-state is visually reviewable in addition to DOM assertions.
