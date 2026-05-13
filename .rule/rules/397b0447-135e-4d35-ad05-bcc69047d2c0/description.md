## Quality Gates

- Tests relevant to your change must pass before completion.
- **Browser verification is mandatory** for any change to a server interface or frontend feature:
  open the affected viewer in the browser and confirm the feature works visually before marking work done.
- **Write Playwright end-to-end tests** for all browser-facing features and server interface changes.
  Shared managed-viewer suites live under `memory-viewers/viewer-api/viewer-api/frontend/dioxus/e2e/shared/`.
  Spec-viewer release E2E lives under `memory-viewers/spec-viewer/frontend/dioxus/`; run it with `npm run test:e2e:release`.
  Ticket-viewer release E2E lives under `memory-viewers/ticket-viewer/frontend/dioxus/`; run it with `npm run test:e2e:release`.
  Doc-viewer and log-viewer keep local Playwright wrappers under `tools/viewer/doc-viewer/e2e/` and `tools/viewer/log-viewer/e2e/`, importing shared suites from `memory-viewers/viewer-api`.
- For tracing-based tests, use: