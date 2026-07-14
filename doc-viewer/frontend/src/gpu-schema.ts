/**
 * DOC_VIEWER_SCHEMA — AppSchema for the doc-viewer application.
 *
 * Bridges the viewer-api WgpuOverlay component with doc-viewer's concrete
 * CSS selectors, theme signals, and effect settings.
 */

import type { AppSchema, SchemaSelectorEntry } from '@context-engine/viewer-api-frontend';
import { effectSettings, themeColors } from './theme';

// ---------------------------------------------------------------------------
// Element kind constants — passed to the shader for per-category glow style.
// ---------------------------------------------------------------------------

const KIND_STRUCTURAL = 0;
const KIND_FX_SPARK   = 8;
const KIND_FX_EMBER   = 9;
const KIND_FX_BEAM    = 10;
const KIND_FX_GLITTER = 11;

// ---------------------------------------------------------------------------
// Selector registry
//   0-3 : structural UI regions  (low-intensity border glow)
//   4-7 : effect preview containers (drive particle spawning in ThemeSettings)
// ---------------------------------------------------------------------------

const DOC_VIEWER_SELECTORS: ReadonlyArray<SchemaSelectorEntry> = [
    // --- structural regions ---
    { sel: '.header',       hue: 0.00, kind: KIND_STRUCTURAL },
    { sel: '.sidebar',      hue: 0.08, kind: KIND_STRUCTURAL },
    { sel: '.filter-panel', hue: 0.16, kind: KIND_STRUCTURAL },
    { sel: '.doc-viewer',   hue: 0.24, kind: KIND_STRUCTURAL },
    // --- effect preview containers (theme settings panel) ---
    { sel: '.effect-preview-sparks',  hue: 0.60, kind: KIND_FX_SPARK,   priority: true },
    { sel: '.effect-preview-embers',  hue: 0.65, kind: KIND_FX_EMBER,   priority: true },
    { sel: '.effect-preview-beams',   hue: 0.70, kind: KIND_FX_BEAM,    priority: true },
    { sel: '.effect-preview-glitter', hue: 0.75, kind: KIND_FX_GLITTER, priority: true },
];

export const DOC_VIEWER_SCHEMA: AppSchema = {
    selectors: DOC_VIEWER_SELECTORS,
    // The dual-package Signal type conflict is resolved with the same cast
    // used by themeSettingsStore — the shapes are identical.
    effectSettings: effectSettings as AppSchema['effectSettings'],
    themeColors: themeColors as AppSchema['themeColors'],
    getCurrentViewId: () => 0,
    isActive3DView:   () => false,
};
