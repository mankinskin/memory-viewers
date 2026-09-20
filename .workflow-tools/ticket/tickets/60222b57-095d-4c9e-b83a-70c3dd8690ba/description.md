Parent epic: `0ee95228`. Spec: `2ccde9ee`. Depends on `89b0c64a` (Phase 1 on a stock theme).

Phase 1 deliberately ships on a **stock Slidev theme** to prove the loop without theme
work. This ticket replaces it with the repo's own theme pack.

## Scope

1. Custom Slidev theme package under
   `memory-viewers/presentation-viewer/frontend/slidev/theme/` (or a publishable local
   theme package, whichever the Slidev theme API prefers).
2. Design tokens: color palette as CSS variables, typography scale, spacing, dark/light.
3. Curated layout presets encoding the retired HTML agent's proven principles:
   - `hero` — full-viewport, dark, animated gradient, large title + subtitle + CTA.
   - `sticky-nav` — sticky navbar with section links and a horizontal scroll-progress bar;
     always a "Start" link first and a CTA link last.
   - `section` — one idea per screen-height section.
   - `cta-bookend` — closing full-viewport section, same dark background as the hero but
     with a **static** gradient, mirroring the hero's visual weight.
   - Plus two-column, chart, diagram, and embed presets.
4. **Preset descriptors**: machine-readable name / purpose / required content slots per
   preset, so an authoring agent picks a layout without reading theme source. This is the
   contract `presentation-api`'s theme registry (`3cdcaf3b` §3.5) consumes from
   `.presentation/themes/<name>/`.
5. Per-deck override support via `theme.override.toml`.
6. Migrate the sample deck from the stock theme to the repo theme; keep the authoring
   contract unchanged.

## Definition of done

- Sample deck renders on the repo theme with every preset exercised.
- Preset descriptors load through `presentation-api`'s theme registry; an unknown preset
  raises `UnknownPreset` naming the valid set.
- Playwright E2E green with per-slide screenshots; verified in an external fullscreen
  Chromium browser (record the window resolution).
- Unblocks deletion of `Presentation.agent 2.md` in ticket `134b953b`.


## Semantic visual grammar (added)

Define a small role-based visual vocabulary — goal, input, evidence, decision, tool, verification, output, risk, unresolved-question — and map each role consistently to color, edge style, and typography across every preset. Preserve non-color/text distinctions (do not rely on color alone) so the mapping stays accessible and machine-readable for authoring agents.