Parent epic: `0ee95228`. Spec: `2ccde9ee`.

Prove the whole loop with files and npm scripts only — **no Rust, no MCP, no CLI surface**,
and **no custom theme**. Phase 1 runs on a **stock Slidev theme, unmodified**. The repo
theme pack is ticket `60222b57` and lands later.

## Scope

1. Slidev toolchain scaffold (Node + Vite + Slidev) under
   `memory-viewers/presentation-viewer/frontend/slidev/`.
   Vite config includes `vite-plugin-wasm` + `vite-plugin-top-level-await` so WASM
   embedding (ticket `e01dd058`) works later without rebuilding the toolchain.
2. Pick one stock Slidev theme (e.g. `@slidev/theme-seriph` or `-default`) and use it
   **as-is**. Do not fork, restyle, or add custom layouts.
3. Enable ECharts/Chart.js and Mermaid in the toolchain.
4. A synthetic sample deck exercising the stock theme's built-in layouts, a chart, and a
   Mermaid diagram. Keep the deck's authoring shape stable so ticket `60222b57` can swap
   the theme without rewriting content.
5. npm scripts: `dev` (hot reload) and `build` (static SPA); verify presenter mode and
   speaker notes.
6. Playwright E2E over the sample deck with a screenshot captured per slide, per AGENTS.md.
7. Establish the on-disk shape that `presentation-api`'s theme registry will later read
   from `.presentation/themes/<name>/` — even though Phase 1 only has the stock theme,
   document where a theme pack would live.

## Definition of done

- `npm run dev` serves the sample deck with hot reload.
- `npm run build` emits a static SPA.
- Presenter mode and speaker notes work.
- Playwright E2E green with per-slide screenshots, verified in an external fullscreen
  Chromium browser (record the window resolution used).

## Non-goals

No Rust crates, no `.presentation/` store, no viewer server, no skills, no custom theme,
no real content.
