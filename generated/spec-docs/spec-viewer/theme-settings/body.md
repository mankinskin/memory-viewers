<!-- rule-api:file generated=true -->

<!-- rule-api:entry id=97b8a613-d1c8-4ebc-bf89-0cc8902e268d slug=spec-viewer/theme-settings/spec-viewer-theme-settings/l1 -->
# spec-viewer: theme settings

The spec-viewer's theme settings panel **MUST** conform to the canonical spec
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

<!-- rule-api:entry id=1dffcb64-e9b4-4de6-b3bc-02a0c479c3ff slug=spec-viewer/theme-settings/spec-viewer-theme-settings/rationale/l23 -->
## Rationale

The spec-viewer is fully GPU-accelerated by default — the animated background,
glass panels, and particle effects are part of the visual identity of the app and
are expected to be visible on first load. Users who prefer a lightweight, static
presentation can disable the master GPU toggle in ThemeSettings; the choice is
persisted to `viewer-api-gpu-enabled` localStorage.

<!-- rule-api:entry id=e687195a-467a-4175-886a-c6c9040cadee slug=spec-viewer/theme-settings/spec-viewer-theme-settings/implementation-pointers/l31 -->
## Implementation pointers

- Panel lives in `memory-viewers/viewer-api/viewer-api/frontend/dioxus/src/components/theme_settings.rs`
  (shared component, mounted by the spec-viewer's settings page).
- Store factory: `ThemeStore::new()` in
  `memory-viewers/viewer-api/viewer-api/frontend/dioxus/src/store/theme.rs` with
  `gpu_enabled: true` default.
- The spec-viewer Trunk shell in `memory-viewers/spec-viewer/frontend/dioxus/index.html` MUST load the shared `modal.css` bundle in addition to `glass-panel.css` and `theme-settings.css`.
- Shared modal CSS lives in `memory-viewers/viewer-api/viewer-api/frontend/dioxus/public/css/modal.css`.

<!-- rule-api:entry id=3ebc9c2a-0eb1-426f-a2dd-a622bca61c25 slug=spec-viewer/theme-settings/spec-viewer-theme-settings/validation/l41 -->
## Validation

- `npm run test:e2e:release -- e2e-release/viewer-api-primitives.spec.ts -g "P5.4 Overlay: theme settings open in a role=dialog modal-backdrop"` in `memory-viewers/spec-viewer/frontend/dioxus`
- The focused Playwright test MUST attach a screenshot of `.theme-settings.glass-panel` so the open-state is visually reviewable in addition to DOM assertions.
