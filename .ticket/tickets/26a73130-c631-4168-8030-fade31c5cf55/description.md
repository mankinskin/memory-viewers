# Problem

Spec-viewer and ticket-viewer currently launch materially different browser/GPU configurations. SwiftShader, headless bundled Chromium, headed system Chrome, and native hardware results are not comparable, yet the architecture roadmap relies on performance evidence.

# Scope

- Define named browser environment profiles: functional DOM/WASM, deterministic software WebGPU, and dedicated hardware WebGPU.
- Centralize launch arguments and capability probes in the shared harness.
- Record browser version/channel, headless/headed mode, WebGPU availability, adapter info, driver/backend where exposed, OS, CPU, viewport, display resolution, device scale factor, and refresh rate.
- Skip or block with an explicit reason when required capabilities are unavailable; never silently fall back for performance evidence.
- Define which assertions are valid in each profile.
- Provide a hardware-run playbook suitable for a dedicated runner and manual external fullscreen Chromium validation.

# Acceptance criteria

- [ ] Ticket-viewer and spec-viewer can run the same named functional and software-WebGPU profiles.
- [ ] Capability probing proves a nonblank WebGPU/canvas result before GPU assertions run.
- [ ] Software-rendered results are labeled and excluded from native-GPU claims.
- [ ] Hardware performance evidence records adapter/driver/browser/display metadata.
- [ ] A missing WebGPU adapter produces a blocked result with diagnostics rather than a passing skipped test.
- [ ] Environment manifests are attached to Playwright and test-api executions.
- [ ] Profile selection is documented and reused by CI/performance tickets.

# Implementation steps

1. Define profile schema and supported assertions.
2. Centralize launch configuration and probes.
3. Migrate viewer configs onto named profiles.
4. Add environment manifest capture.
5. Validate software and one hardware run.