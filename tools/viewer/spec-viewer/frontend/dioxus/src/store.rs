//! Per-page UI state store for [`SpecListPage`].
//!
//! State is persisted to `localStorage` under the key `spec-viewer:ui`
//! and restored on mount.  The selected spec ID is additionally mirrored
//! into the URL hash as `#id=<spec_id>` for deep-link support.

use dioxus::prelude::*;
use gloo_events::EventListener;
use serde::{Deserialize, Serialize};
use viewer_api_dioxus::{get_hash_param, remove_hash_param, set_hash_param};

// ── localStorage key ──────────────────────────────────────────────────────────

fn ls_key() -> &'static str {
    "spec-viewer:ui"
}

// ── Serialisable snapshot ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecUiState {
    /// Text filter applied to spec title / slug / component.
    #[serde(default)]
    pub filter: String,
    /// State filter (e.g. `"draft"`, `"reviewed"`, …).
    #[serde(default)]
    pub state_filter: String,
    /// Currently selected spec ID.
    #[serde(default)]
    pub open_spec_id: Option<String>,
    /// Active tab inside the spec detail panel.
    #[serde(default = "default_active_tab")]
    pub active_tab: String,
}

impl Default for SpecUiState {
    fn default() -> Self {
        Self {
            filter: String::new(),
            state_filter: String::new(),
            open_spec_id: None,
            active_tab: default_active_tab(),
        }
    }
}

fn default_active_tab() -> String {
    "body".to_string()
}

// ── Store ─────────────────────────────────────────────────────────────────────

/// Reactive UI state store for the spec list page.
#[derive(Clone, Copy)]
pub struct SpecListStore {
    pub filter: Signal<String>,
    pub state_filter: Signal<String>,
    pub open_spec_id: Signal<Option<String>>,
    pub active_tab: Signal<String>,
    /// RAII guard that keeps the `hashchange` listener alive for the
    /// lifetime of the component.
    _hash_listener: Signal<Option<EventListener>>,
}

impl SpecListStore {
    /// Mount and return the store, restoring persisted state if available.
    pub fn use_store() -> Self {
        let saved = Self::load_from_storage();
        let hash_id = Self::read_hash_id();

        let filter = use_signal(|| saved.filter.clone());
        let state_filter = use_signal(|| saved.state_filter.clone());
        let mut open_spec_id: Signal<Option<String>> = use_signal(|| hash_id.or(saved.open_spec_id.clone()));
        let active_tab = use_signal(|| saved.active_tab.clone());

        // Store the listener in a Signal so it is dropped when the component
        // unmounts (RAII).  Signal<T> is Clone/Copy even when T is not.
        let _hash_listener: Signal<Option<EventListener>> = use_signal(|| {
            #[cfg(target_arch = "wasm32")]
            {
                web_sys::window().map(|window| {
                    EventListener::new(&window, "hashchange", move |_event| {
                        let new_id = Self::read_hash_id();
                        if *open_spec_id.peek() != new_id {
                            open_spec_id.set(new_id);
                        }
                    })
                })
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                None
            }
        });

        Self {
            filter,
            state_filter,
            open_spec_id,
            active_tab,
            _hash_listener,
        }
    }

    /// Flush current state to localStorage and URL hash.
    pub fn persist(&self) {
        let snapshot = SpecUiState {
            filter: self.filter.read().clone(),
            state_filter: self.state_filter.read().clone(),
            open_spec_id: self.open_spec_id.read().clone(),
            active_tab: self.active_tab.read().clone(),
        };
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(json) = serde_json::to_string(&snapshot) {
                if let Some(win) = web_sys::window() {
                    if let Ok(Some(storage)) = win.local_storage() {
                        let _ = storage.set_item(ls_key(), &json);
                    }
                }
            }
        }
        // Sync URL hash.
        match snapshot.open_spec_id.as_deref() {
            Some(id) => set_hash_param("id", id),
            None => remove_hash_param("id"),
        }
    }

    fn load_from_storage() -> SpecUiState {
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item(ls_key()).ok().flatten())
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            SpecUiState::default()
        }
    }

    fn read_hash_id() -> Option<String> {
        get_hash_param("id").filter(|s| !s.is_empty())
    }
}
