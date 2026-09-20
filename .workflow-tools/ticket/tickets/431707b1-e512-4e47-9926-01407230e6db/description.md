# Goal

Add a deferred native desktop validation lane after the browser/WASM contract is stable. This lane proves only behavior that Chromium Playwright cannot establish: Tauri IPC, native window lifecycle, packaging, filesystem permissions, OS integration, and native wgpu backend behavior.

# Scope

- Select the supported Tauri v2 WebDriver/Appium integration for Windows first, with an explicit portability plan for other desktop platforms.
- Reuse browser-level scenario definitions and correlation IDs where possible without pretending Playwright directly controls native internals.
- Validate startup/shutdown, window recreation, IPC request/response and error handling, native filesystem boundaries, packaging/install launch, and native GPU adapter/backend reporting.
- Capture native process logs, frontend logs, screenshots, environment metadata, crash artifacts, and test-api executions.
- Keep native performance baselines distinct from browser/SwiftShader baselines.

# Acceptance criteria

- [ ] A packaged or release-mode Tauri application is started and controlled by an automated desktop driver.
- [ ] At least one IPC success path, one IPC failure path, window lifecycle, and native filesystem boundary are tested.
- [ ] Frontend and native logs share a correlation ID and are attached to the result.
- [ ] Native GPU adapter/backend and display metadata are recorded before performance assertions.
- [ ] The lane has deterministic cleanup and leaves no stale application processes.
- [ ] The lane is scheduled/on-demand, not part of the fast browser PR gate.
- [ ] Platform limitations and unsupported environments are documented as blocked, not silently skipped.

# Dependencies and sequencing

This work begins only after the canonical harness, observability contract, environment profiles, and architecture spec are stable.