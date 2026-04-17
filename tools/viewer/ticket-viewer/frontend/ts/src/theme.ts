/**
 * Ticket-viewer theme store.
 *
 * Uses createThemeStore from viewer-api for CSS variable injection and
 * localStorage persistence. Exposes a ThemeSettingsStore object that can
 * be passed to the shared ThemeSettings component.
 */
import { signal, effect } from '@preact/signals';
import {
  createThemeStore,
  DEFAULT_THEME,
  DEFAULT_EFFECT_SETTINGS_OFF,
  type ThemePreset,
  type EffectSettings,
  type ThemeColors,
  type ThemeSettingsStore,
  type SavedTheme,
} from '@context-engine/viewer-api-frontend';

// ── Theme store ───────────────────────────────────────────────────────────────

const _store = createThemeStore('ticket-viewer-theme', DEFAULT_THEME, true);
export const themeColors = _store.colors;

// ── Preset definitions ────────────────────────────────────────────────────────

/** Dark Dracula-like theme to match the original hardcoded ticket-viewer colors. */
const DARK_COLORS: ThemeColors = {
  ...DEFAULT_THEME,
  bgPrimary: '#1a1b26',
  bgSecondary: '#1e2030',
  bgTertiary: '#222436',
  bgHover: '#2a2e47',
  bgActive: '#313548',
  textPrimary: '#c0caf5',
  textSecondary: '#787c99',
  textMuted: '#565f89',
  borderColor: '#2f3354',
  borderSubtle: '#252840',
  accentBlue: '#7aa2f7',
  accentGreen: '#9ece6a',
  accentOrange: '#ff9e64',
  accentPurple: '#bb9af7',
  accentYellow: '#e0af68',
  levelTrace: '#3b4261',
  levelDebug: '#2d4f2d',
  levelInfo: '#1e3a50',
  levelWarn: '#4a3800',
  levelError: '#4a1a1a',
  levelTraceText: '#a9b1d6',
  levelDebugText: '#9ece6a',
  levelInfoText: '#7aa2f7',
  levelWarnText: '#e0af68',
  levelErrorText: '#f7768e',
  spanEnterText: '#73daca',
  spanExitText: '#ff9e64',
};

/** Paper: a light, off-white theme for daytime use. */
const PAPER_COLORS: ThemeColors = {
  ...DEFAULT_THEME,
  bgPrimary: '#f5f0eb',
  bgSecondary: '#faf8f5',
  bgTertiary: '#ffffff',
  bgHover: '#e8e2db',
  bgActive: '#dcd5cc',
  textPrimary: '#1a1512',
  textSecondary: '#4a4440',
  textMuted: '#7a7470',
  borderColor: '#ccc4bc',
  borderSubtle: '#e0d9d2',
};

/** Dark, high-contrast Scratchboard theme. */
const SCRATCHBOARD_COLORS: ThemeColors = {
  ...DEFAULT_THEME,
  bgPrimary: '#0f0f0f',
  bgSecondary: '#161616',
  bgTertiary: '#1e1e1e',
  bgHover: '#252525',
  bgActive: '#2d2d2d',
  textPrimary: '#f0f0f0',
  textSecondary: '#a0a0a0',
  textMuted: '#606060',
  borderColor: '#303030',
  borderSubtle: '#202020',
  accentBlue: '#58a6ff',
  accentGreen: '#3fb950',
  accentOrange: '#f0883e',
  accentPurple: '#d2a8ff',
  accentYellow: '#cca700',
};

export const TICKET_PRESETS: ThemePreset[] = [
  { name: 'Arcadia', description: 'Warm marble default', colors: DEFAULT_THEME },
  { name: 'Dark', description: 'Dark Dracula-style', colors: DARK_COLORS },
  { name: 'Paper', description: 'Light off-white', colors: PAPER_COLORS },
  { name: 'Scratchboard', description: 'High-contrast dark', colors: SCRATCHBOARD_COLORS },
];

// ── Effect settings ───────────────────────────────────────────────────────────

const EFFECT_KEY = 'ticket-viewer-effects';
function loadEffects(): EffectSettings {
  try {
    const raw = localStorage.getItem(EFFECT_KEY);
    if (raw) return { ...DEFAULT_EFFECT_SETTINGS_OFF, ...JSON.parse(raw) as Partial<EffectSettings> };
  } catch { /* ignore */ }
  return { ...DEFAULT_EFFECT_SETTINGS_OFF };
}
export const effectSettings = signal<EffectSettings>(loadEffects());
effect(() => {
  try { localStorage.setItem(EFFECT_KEY, JSON.stringify(effectSettings.value)); } catch { /* storage full */ }
});

// ── Saved themes ──────────────────────────────────────────────────────────────

const SAVED_KEY = 'ticket-viewer-saved-themes';
function loadSaved(): SavedTheme[] {
  try {
    const raw = localStorage.getItem(SAVED_KEY);
    if (raw) return JSON.parse(raw) as SavedTheme[];
  } catch { /* ignore */ }
  return [];
}
function persist(themes: SavedTheme[]) {
  try { localStorage.setItem(SAVED_KEY, JSON.stringify(themes)); } catch { /* storage full */ }
}
export const savedThemes = signal<SavedTheme[]>(loadSaved());

// ── Store functions ───────────────────────────────────────────────────────────

function updateColor<K extends keyof ThemeColors>(key: K, value: string) {
  _store.updateColor(key, value);
}

function applyPreset(preset: ThemePreset) {
  _store.applyPreset(preset.colors);
}

function resetTheme() {
  _store.reset();
}

function saveTheme(name: string, thumbnail?: string) {
  const theme: SavedTheme = {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    name,
    colors: themeColors.value,
    thumbnail,
    createdAt: Date.now(),
  };
  savedThemes.value = [theme, ...savedThemes.value];
  persist(savedThemes.value);
}

function deleteTheme(id: string) {
  savedThemes.value = savedThemes.value.filter((t) => t.id !== id);
  persist(savedThemes.value);
}

function applySavedTheme(t: SavedTheme) {
  _store.applyPreset(t.colors);
  if (t.effects) effectSettings.value = { ...DEFAULT_EFFECT_SETTINGS_OFF, ...t.effects };
}

function updateSavedTheme(id: string, thumbnail?: string) {
  savedThemes.value = savedThemes.value.map((t) =>
    t.id === id ? { ...t, thumbnail, colors: themeColors.value } : t
  );
  persist(savedThemes.value);
}

function renameSavedTheme(id: string, newName: string) {
  savedThemes.value = savedThemes.value.map((t) =>
    t.id === id ? { ...t, name: newName } : t
  );
  persist(savedThemes.value);
}

function updateEffect<K extends keyof EffectSettings>(key: K, value: EffectSettings[K]) {
  effectSettings.value = { ...effectSettings.value, [key]: value };
}

function exportTheme(name?: string) {
  const data = {
    version: 1,
    name: name ?? 'ticket-theme',
    colors: themeColors.value,
    effects: effectSettings.value,
  };
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${data.name}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

async function importTheme(file: File): Promise<string | null> {
  try {
    const text = await file.text();
    const data = JSON.parse(text) as { colors?: Partial<ThemeColors>; effects?: Partial<EffectSettings> };
    if (data.colors && typeof data.colors === 'object') {
      _store.applyPreset({ ...DEFAULT_THEME, ...data.colors });
    }
    if (data.effects && typeof data.effects === 'object') {
      effectSettings.value = { ...DEFAULT_EFFECT_SETTINGS_OFF, ...data.effects };
    }
    return null;
  } catch (e) {
    return String(e);
  }
}

// ── Exported store ────────────────────────────────────────────────────────────

export const themeSettingsStore: ThemeSettingsStore = {
  themeColors,
  // Cast to resolve dual-package Signal type conflict (ticket-viewer's local
  // @preact/signals-core is a different instance from viewer-api's).
  effectSettings: effectSettings as unknown as ThemeSettingsStore['effectSettings'],
  presets: TICKET_PRESETS,
  defaultTheme: DEFAULT_THEME,
  updateColor,
  applyPreset,
  resetTheme,
  savedThemes: savedThemes as unknown as ThemeSettingsStore['savedThemes'],
  saveTheme,
  deleteTheme,
  applySavedTheme,
  updateSavedTheme,
  renameSavedTheme,
  updateEffect,
  exportTheme,
  importTheme,
};
