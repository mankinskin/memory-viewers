# Dioxus theme settings: CRT Effect controls + Glass Panels sliders

## Problem

The canonical theme settings spec requires the "Glass Panels" section (§2 row 16,
**default open**) with Opacity + Blur sliders, and the "CRT Effect" section (§2 row 17,
**default open**) with a toggle, five sliders (H Scanlines, V Scanlines, Edge Shadow,
Flicker, Line Width), and a Scanline Color picker. The Dioxus port has neither.

## Acceptance criteria

### Glass Panels

1. Section renders between Background Smoke and CRT Effect, default open.
2. Two sliders: Opacity (0–100%, maps to 0.0–0.4 alpha) and Blur (0–100%, maps to 0–16 px).
3. Slider changes update the corresponding CSS variables (`--glass-bg-opacity`,
   `--glass-blur-radius`, or whatever the existing TS port uses) live.

### CRT Effect

1. Section renders last, default open.
2. Toggle controls `EffectSettings.crtEnabled`.
3. When enabled: 5 sliders (each 0–100%, step 1) wired to:
   `crtScanlinesH`, `crtScanlinesV`, `crtEdgeShadow`, `crtFlicker`, `crtLineWidth`.
4. Scanline Color uses an HTML color picker that reads/writes the `[r,g,b]` 0–255 triplet
   in `EffectSettings.crtColor`.
5. All changes propagate to the CRT post-processing shader uniforms.

## Implementation notes

- TS reference: `ThemeSettings.tsx` lines 650–720 (Glass + CRT blocks).
- Existing iOS toggle CSS already in `viewer-api.css`.
