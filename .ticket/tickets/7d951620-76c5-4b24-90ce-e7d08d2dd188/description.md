# Problem

`RESEARCH_VIEWER_ARCHITECTURE.md` mixes architectural hypotheses, future APIs, example code, and absolute performance claims. It does not define a stable contract that tickets and validation evidence can prove.

# Scope

- Discover the owning viewer-api, Graph3D, WebGPU overlay, tracing, and browser-policy specs.
- Create or update one focused aligned-structure:v2 specification for cross-viewer browser validation and performance evidence.
- Classify claims as implemented, partial, experimental, or future.
- Define supported browser/WASM and eventual Tauri/native environments.
- Replace claims such as zero-copy, GC-free, nearly-native, fixed FPS, and no-code-change scalability with measurable dependent expectations or explicit non-goals.
- Define ValidationSpec guards for browser conformance, log correlation, accessibility/visual checks, frame-time budgets, leak/soak behavior, and environment metadata.
- Link the umbrella tracker and all implementation tickets created from it.

# Acceptance criteria

- [ ] The owning spec uses aligned-structure:v2 and contains Motivation, Dependent expectation, Guards, Positions, and Governing-rule requirement.
- [ ] Every performance statement names workload, warm-up, metric, percentile, budget, environment class, and evidence source.
- [ ] Experimental APIs such as HTML-to-texture/browser element capture are feature-detected and have fallback/non-goal language.
- [ ] Browser/WASM and native Tauri contracts are separated.
- [ ] The spec links exact ticket manifests and test-api ValidationSpec IDs.
- [ ] `spec refs validate` and spec health pass.

# Implementation steps

1. Inspect neighboring specs and choose the smallest owning contract.
2. Reconcile `RESEARCH_VIEWER_ARCHITECTURE.md` with actual viewer-api capabilities.
3. Author measurable expectations and non-goals.
4. Add guard IDs and positions for current implementations.
5. Link tickets and validate spec health/references.

# Done condition

Another engineer can implement and review every child ticket without relying on unsupported prose in the research memo.