## Validation

- `npm run test:e2e:release -- e2e-release/ticket-viewer.release.spec.ts -g "theme settings palette button opens and closes the theme settings panel"` in `memory-viewers/ticket-viewer/frontend/dioxus`
- The focused Playwright test MUST attach a screenshot of `.theme-settings.glass-panel` so the open-state is visually reviewable in addition to DOM assertions.