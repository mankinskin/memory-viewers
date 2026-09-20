Parent epic: `0ee95228`. Spec: `2ccde9ee`. Depends on `3cdcaf3b` (presentation-api).

## Scope

- Rust server crate `presentation-viewer` under `memory-viewers/presentation-viewer/`,
  matching the shape of `memory-viewers/spec-viewer`.
- Dioxus shell frontend under `memory-viewers/presentation-viewer/frontend/dioxus/`:
  a **deck browser**. Index at `/` lists all decks from `.presentation/`; `/deck/{id}`
  renders the deck by iframing its built Slidev SPA.
- Server serves built Slidev `dist/` bundles per deck and surfaces build staleness
  (deck revision vs artifact revision) in the index.
- Port **3003**.
- Register in `viewer-ctl.toml`: a `[[server]]` entry (package `presentation-viewer`,
  port 3003, `source_dir = "memory-viewers/presentation-viewer"`) and a `[[frontend]]`
  entry (`serves = "presentation-viewer"`, trunk release build, matching the other
  Dioxus frontends).
- Add `.vscode` tasks mirroring the other managed viewers (`start`, `open`, `prepare`).
- Add a deep-link route entry so the Clickable Reference Policy can cite decks:
  `http://localhost:3003/deck/{id}`.

## Definition of done

- `viewer-ctl prepare presentation-viewer` and `viewer-ctl start presentation-viewer` work.
- Index lists decks; a deck renders and is navigable.
- Shared managed-viewer Playwright suites under
  `viewer-api/viewer-api/frontend/dioxus/e2e/shared/` pass against this viewer.
- Manual verification in an external fullscreen Chromium browser with screenshots and the
  recorded window resolution.
