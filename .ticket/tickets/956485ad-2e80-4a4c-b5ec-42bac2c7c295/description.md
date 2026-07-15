# Goal

Establish one implementation-ready validation program for the shared viewer-api platform and the ticket-viewer, spec-viewer, log-viewer, and doc-viewer operational frontends. The program must prove browser behavior, API integration, WASM/frontend observability, accessibility and rendering quality, performance regressions, and eventual native Tauri behavior against the architecture direction recorded in `RESEARCH_VIEWER_ARCHITECTURE.md`.

# Current evidence and boundaries

Existing work must be reused rather than duplicated:

- `80631f3c` moved ticket-viewer Playwright ownership into memory-viewers.
- `363e26d6` introduced per-test browser/backend correlation primitives.
- `8f349d96` owns the WASM tracing file sink.
- `b480632a` owns structured WASM tracing.
- `099ac71e` owns validation of the existing browser profile capture and WASM micro-benchmarks.
- `08c86dbd` owns graph-improvement interaction and screenshot coverage.
- `93b8a331` owns the design for mapping Playwright/TypeScript results and artifacts into test-api.
- `ef3f4a91` remains the broader cross-architecture profiling and benchmark tracker.

This tracker owns the integration and missing platform contracts, not reimplementation of those slices.

# Workstreams

1. Define and implement a canonical shared Playwright harness and one all-viewer orchestration command.
2. Add enforceable CI lanes with retained reports, traces, screenshots, correlated logs, and validation evidence.
3. Harden end-to-end WASM log correlation and make per-test frontend/backend records directly queryable.
4. Add accessibility, responsive-layout, keyboard/focus, and visual-regression gates.
5. Turn profile capture and browser micro-benchmarks into regression budgets with machine-qualified baselines.
6. Add soak/leak telemetry for JS/WASM heap growth, DOM/resource growth, long tasks, and frame-time drift.
7. Define reproducible software-GPU and hardware-GPU browser lanes with explicit environment metadata.
8. Convert architecture claims into a focused spec contract with measurable acceptance criteria and declared experimental assumptions.
9. Add a deferred native Tauri/WebDriver lane for desktop-only IPC, lifecycle, packaging, and native GPU behavior.

# Acceptance criteria

- [ ] Every child workstream has a focused ticket, component, risk, implementation steps, validation plan, and clear done condition.
- [ ] One documented command runs release-browser validation for ticket-viewer, spec-viewer, log-viewer, and doc-viewer without stale paths or omitted viewers.
- [ ] Required CI lanes execute the browser checks and retain Playwright HTML, trace, screenshot, frontend-log, and benchmark artifacts.
- [ ] Each browser test can be correlated to frontend WASM logs, backend logs/spans, and test-api evidence by one stable identifier.
- [ ] Accessibility, keyboard/focus, responsive viewports, and maintained visual baselines are enforced rather than left to optional inspection.
- [ ] Performance checks have explicit warm-up, workloads, percentile metrics, budgets, baseline comparison rules, and environment identity.
- [ ] Leak/soak checks detect monotonic resource growth across repeated load/interact/unload cycles.
- [ ] Software-rendered CI measurements are not presented as native-GPU performance evidence; hardware results record adapter, driver, browser, OS, viewport, and refresh rate.
- [ ] The architecture spec labels hypotheses and experimental APIs, and replaces unsupported absolute claims with measurable contracts.
- [ ] Native Tauri checks are isolated from fast browser/WASM validation and cover only desktop-specific behavior.

# Execution order

1. Architecture/spec contract and canonical harness.
2. Correlated observability and test-api result mapping.
3. CI orchestration and UX quality gates.
4. Performance budgets, reproducible GPU lanes, then leak/soak runs.
5. Native Tauri lane after the browser/WASM contract is stable.

# Non-goals

- Rewriting existing viewer feature tests that already provide useful coverage.
- Treating screenshots attached for human inspection as visual-regression assertions.
- Using smoothed FPS alone as a performance or leak metric.
- Requiring Tauri/Appium in the fast pull-request lane.