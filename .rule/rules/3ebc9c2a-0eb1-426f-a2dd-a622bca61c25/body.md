## Validation

- `npm run test:e2e:release -- e2e-release/viewer-api-primitives.spec.ts -g "P5.4 Overlay: theme settings open in a role=dialog modal-backdrop"` in `memory-viewers/spec-viewer/frontend/dioxus`
- The focused Playwright test MUST attach a screenshot of `.theme-settings.glass-panel` so the open-state is visually reviewable in addition to DOM assertions.