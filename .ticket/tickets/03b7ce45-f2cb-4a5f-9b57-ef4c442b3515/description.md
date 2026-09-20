# Dioxus theme settings: Background Smoke section (3 colors + 10 sliders)

## Problem

The canonical theme settings spec requires a "Background Smoke" section (§2 row 15) with
a master toggle, three ColorRows (Cool/Warm/Moss tones), and ten sliders covering
intensity, speed, three scale factors, three grain controls, vignette, and underglow. The
Dioxus port currently has none of this.

## Acceptance criteria

1. Section renders between the per-particle sections and the Glass Panels section.
2. Toggle controls `EffectSettings.smokeEnabled` and is persisted.
3. ColorRows: `smokeCool`, `smokeWarm`, `smokeMoss` with picker + hex input + reset.
4. Sliders match the canonical range table:
   - Intensity 0–100% · Speed 0–500 · Warm/Cool/Moss Scale 0–200 ·
     Grain Intensity 0–100% · Grain Coarseness 0–100 · Grain Size 0–100 ·
     Vignette 0–100% · Underglow 0–100%.
5. All slider changes propagate to the WebGPU smoke shader uniforms (existing wiring in
   `wgpu_overlay.rs`).

## Implementation notes

- TS reference: `ThemeSettings.tsx` lines 600–650 ("Background Smoke" Section block).
- Reuse the helper pattern proposed in the per-effect ticket so each viewer has one
  uniform `SliderRow` / `ColorRow` Dioxus component.
