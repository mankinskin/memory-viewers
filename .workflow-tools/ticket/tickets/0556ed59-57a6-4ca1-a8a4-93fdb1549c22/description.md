# Problem

Repository policy requires Playwright for browser-facing changes, but no checked-in workflow currently runs the viewer Playwright or WASM browser suites. Reports, traces, screenshots, logs, and benchmark output therefore depend on local discipline.

# Scope

- Add path-filtered pull-request browser validation for affected viewer-api and memory-viewers code.
- Add scheduled/on-demand extended browser, performance, and soak lanes.
- Invoke the canonical all-viewer runner rather than duplicating commands in workflow YAML.
- Install and cache Rust/WASM, Trunk, viewer-ctl, Node, Playwright browser, and required GPU/software-rendering dependencies reproducibly.
- Retain Playwright HTML reports, traces, screenshots, video when enabled, frontend/backend correlated logs, environment manifests, and test-api execution IDs.
- Record blocked outcomes when required browser/GPU capabilities are unavailable rather than silently skipping.

# Acceptance criteria

- [ ] A frontend/server-interface pull request triggers the relevant release-browser lane.
- [ ] Ticket-viewer and spec-viewer cannot be omitted from the all-viewer gate.
- [ ] WASM browser tests run in a declared lane.
- [ ] Nightly/on-demand jobs cover performance and leak/soak work without making the fast lane excessively slow.
- [ ] Failed jobs upload all diagnostic artifacts with a stable retention policy.
- [ ] Each job emits a machine-readable environment manifest and records ValidationExecution outcomes in test-api.
- [ ] Workflow concurrency prevents stale runs from consuming shared viewer ports or artifact names.
- [ ] Documentation names local equivalents for every CI command.

# Implementation steps

1. Define fast, release-browser, nightly-performance, and hardware-manual profiles.
2. Wire path filters and the canonical runner.
3. Add toolchain/browser caching and deterministic server lifecycle.
4. Upload diagnostics and record test-api evidence.
5. Prove one intentional failure retains actionable artifacts.