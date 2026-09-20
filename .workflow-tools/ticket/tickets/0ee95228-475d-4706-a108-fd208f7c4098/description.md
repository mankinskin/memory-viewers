# Epic: Presentation System

Governing spec: `presentation-system` (`2ccde9ee-85ac-4c87-9601-f6099f5be01c`).

## Goal

Turn a text script into an interactive, web-native presentation cheaply and repeatably.
"Cheaply" = low token cost (templates + skills so a mid-tier model can author), low infra
cost (static output, no runtime service required for a built deck), low authoring effort
(one command from script to deck).

## Confirmed architecture

- Rendering stack: **Slidev** (Vue 3 + Vite, markdown-first).
- Domain name: `presentation`. Crates live under `memory-viewers/presentation-viewer/`.
- Deck store: `.presentation/` at repo root; each deck = `deck.toml` + `slides.md`.
- Viewer: Rust server + Dioxus shell (deck browser) iframing built Slidev decks;
  index at `/`, deck at `/deck/{id}`; port **3003**; registered in `viewer-ctl.toml`.
- Interactivity v1: ECharts/Chart.js, Mermaid, agent-authored TS/Vue components,
  Rust-compiled WASM modules, shared repo graph component compiled to WASM standalone.
- Outputs: dev server w/ hot reload, static SPA build, presenter mode + speaker notes,
  managed viewer. (No PDF export in v1.)
- Theming: repo theme pack + optional per-deck override + curated layout presets.
- Skills: vendor `marcoshaber99/slidev-skills`, `yoanbernabeu/slidev-skills`,
  `neversight/slidev-syntax-guide`, `zarazhangrui/frontend-slides`; author one
  repo-local presentation-workflow skill on top.
- Agent surface: a Presentation agent mode + a `.agents/prompts/` workflow prompt.
- `Presentation.agent 2.md` is retired; its hero/sticky-nav/CTA-bookend principles
  migrate into the theme pack presets.
- Validation: Playwright E2E + per-slide screenshots (AGENTS.md browser rules).

## Phasing

**Phase 1 — working deck end-to-end.** Filesystem + npm scripts only, no MCP/CLI surface.
Slidev toolchain, stock-theme sample deck, and Playwright verification. This foundation is near
complete; the custom theme pack and its presets remain later work.

**Phase 2 — formalize the domain.** `presentation-api` + `presentation` facade crate
(cli/mcp/http bins per the workflow-tools domain crate contract) + `.presentation/`
store schema, then the viewer, then skills/agent/first real deck.

**Next track — specification-derived conceptual decks.** Build on the Phase 1 static-deck proof
and the Phase 2 domain ownership model. The live human audience consumes digestible conceptual
presentations of authoritative specifications; implementation and documentation disagreement
are structured sidecar signals rather than alternative authorities. Generated outputs are
explicitly overwritten by the generator and reviewed as Git patches.

## Proposed Work Packages and Dependencies

- **Conceptual input and provenance contracts:** Define claim/citation classification,
  source-lock content, disagreement sidecars, visual provenance, presenter-note coverage, and
  declared generated-output boundaries. This is the dependency for every following package.
- **Topology extraction adapters:** Normalize specification facts plus separate typed Git
  containment and Cargo workspace/crate membership/dependency projections with fixtures. It
  depends on the conceptual-input contract and remains separate from `presentation-api`.
- **Managed conceptual generation:** Deterministically materialize generated sources, notes,
  locks, and sidecars; enforce stale and explicit replace behavior, path containment, legacy
  singleton `.presentation/deck.toml` discovery/migration, and deterministic cross-repository
  imports. It depends on the contracts and extraction adapters.
- **Presentation API and static evidence integration:** Connect managed generation to
  `presentation-api` persistence, materialization, builds, and traceability. It depends on
  managed conceptual generation and proves the output with per-slide static E2E validation.
- **Topology visual preset:** Define the later theme contract for structural slides: required
  legend, node/edge roles, density limits, and baseline screenshots. It depends on stable typed
  topology projections and precedes flagship structural slides.

The next track excludes cross-language parsing and telemetry-derived normativity. Declarative
workflow content is normative; durable session telemetry is illustrative only and may later
serve as end-to-end test data.

## First real deck

Workflow-tools suite introduction and overview.

## Acceptance criteria

- AC1: Documented single command turns a text script into a rendered Slidev deck.
- AC2: `.presentation/` holds decks as `deck.toml` + `slides.md`, readable via `presentation-api`.
- AC3: `presentation-viewer` on port 3003, deck index at `/`, deck at `/deck/{id}`,
  registered in `viewer-ctl.toml` as server + frontend.
- AC4: Workflow-tools intro deck builds and passes Playwright E2E with per-slide screenshots.
- AC5: Four vendored skills + repo-local presentation-workflow skill under `.agents/skills/`,
  recorded in `skills-lock.json`.
- AC6: `Presentation.agent 2.md` removed, principles represented as theme-pack presets.

## Remaining decisions

- The serialization schema for source locks, claim citations, and disagreement sidecars.
- The first topology preset's measurable density limits and baseline viewport dimensions.
