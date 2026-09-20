# Dioxus theme settings: per-effect controls (Sparks/Embers/Beams/Glitter/Cinder)

## Problem

The shared canonical theme settings spec (`viewer-api/theme-settings`) requires per-effect
sections (10–14 in §2: Metal Sparks, Embers/Ash, Angelic Beams, Glitter, Cinder Palette)
with their own toggles, color rows, and sliders. The current Dioxus port only exposes a
single master GPU toggle in the "Effects" section.

## Acceptance criteria

1. Each of the five sections renders in the order defined by the canonical spec, between the
   GPU Rendering section and the Background Smoke section.
2. Each section has:
   - A toggle row using `.theme-toggle-row` + `.toggle-switch` (existing iOS slider style).
   - When enabled: the canonical ColorRows for that effect (see spec table).
   - When enabled: sliders matching the slider-range table (Speed/Count/Size for sparks,
     embers, glitter; Speed/Height/Count/Drift for beams; Size for cinder).
3. Slider mins/maxes/steps match the canonical spec exactly (incl. beam Height min=10,
   beam Count max=1024 with "All" label when 0).
4. Toggling a per-effect switch persists to localStorage (`viewer-api-effects`) and updates
   the GPU shader uniforms via the existing `EffectSettings` plumbing.
5. Visual parity verified against the TS reference (`tools/viewer/viewer-api/frontend/ts/src/components/ThemeSettings/ThemeSettings.tsx`) for both ticket-viewer and spec-viewer.

## Implementation notes

- Mirror the structure already used in the TS `ThemeSettings.tsx` lines 350–600.
- `EffectSettings` already exists in `tools/viewer/viewer-api/frontend/dioxus/src/store/theme.rs` (or needs to be ported alongside).
- Use the existing `.theme-color-row` / `.theme-slider-row` / `.theme-color-picker` /
  `.theme-color-hex` / `.theme-color-reset` CSS — already in `viewer-api.css`.

## Out of scope

- Smoke section (separate ticket).
- CRT slider expansion (separate ticket).
- Saved themes panel (existing ticket `17358907…`).
