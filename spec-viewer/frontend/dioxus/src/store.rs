//! Per-page UI state store for [`SpecListPage`].
//!
//! State is persisted to `localStorage` under the key `spec-viewer:ui`
//! and restored on mount.  The selected spec ID is additionally mirrored
//! into the URL hash as `#id=<spec_id>` for deep-link support.

use dioxus::prelude::*;
use gloo_events::EventListener;
use serde::{
    Deserialize,
    Serialize,
};
use viewer_api_dioxus::{
    get_hash_param,
    remove_hash_param,
    set_hash_param,
    Camera,
    ColonSegmented,
    Layout3D,
    PathCodec,
};

use crate::{
    components::spec_graph::{
        LayoutAlgorithm,
        LayoutParams,
    },
    types::{
        SpecGraphEdge,
        SpecGraphNode,
    },
};

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
        let mut open_spec_id: Signal<Option<String>> =
            use_signal(|| hash_id.or(saved.open_spec_id.clone()));
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
        // Sync URL hash.  The spec id is encoded via [`ColonSegmented`] so
        // hierarchical ids (e.g. `auth:login`) appear as path segments
        // (`auth/login`) and round-trip back to the original id on reload.
        match snapshot.open_spec_id.as_deref() {
            Some(id) => set_hash_param("id", &ColonSegmented.encode(id)),
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
        get_hash_param("id")
            .filter(|s| !s.is_empty())
            .and_then(|s| ColonSegmented.decode(&s))
    }
}

/// Shared graph-page state kept alive above the router so the global spec
/// graph does not reset to defaults on every route transition.
#[derive(Clone, Copy)]
pub struct SpecGraphStore {
    pub raw: Signal<Option<(Vec<SpecGraphNode>, Vec<SpecGraphEdge>)>>,
    pub error: Signal<Option<String>>,
    pub draft_algo: Signal<LayoutAlgorithm>,
    pub draft_params: Signal<LayoutParams>,
    pub draft_show_edges: Signal<bool>,
    pub center_camera_on_selected_node: Signal<bool>,
    pub zoom_to_selected_node: Signal<bool>,
    pub selected_node_zoom_factor: Signal<f32>,
    pub auto_layout_selected_node: Signal<bool>,
    pub committed_algo: Signal<LayoutAlgorithm>,
    pub committed_params: Signal<LayoutParams>,
    pub committed_show_edges: Signal<bool>,
    pub auto_apply: Signal<bool>,
    pub panel_open: Signal<bool>,
    pub current_layout: Signal<Option<Layout3D>>,
    pub current_camera: Signal<Option<Camera>>,
    pub layout_generation: Signal<u64>,
    pub applied_layout_generation: Signal<u64>,
}

impl SpecGraphStore {
    pub fn use_store() -> Self {
        Self {
            raw: use_signal(|| None),
            error: use_signal(|| None),
            draft_algo: use_signal(|| LayoutAlgorithm::ForceDirected),
            draft_params: use_signal(LayoutParams::default),
            draft_show_edges: use_signal(|| true),
            center_camera_on_selected_node: use_signal(|| false),
            zoom_to_selected_node: use_signal(|| false),
            selected_node_zoom_factor: use_signal(|| 4.0),
            auto_layout_selected_node: use_signal(|| false),
            committed_algo: use_signal(|| LayoutAlgorithm::ForceDirected),
            committed_params: use_signal(LayoutParams::default),
            committed_show_edges: use_signal(|| true),
            auto_apply: use_signal(|| true),
            panel_open: use_signal(|| true),
            current_layout: use_signal(|| None),
            current_camera: use_signal(|| None),
            layout_generation: use_signal(|| 0),
            applied_layout_generation: use_signal(|| 0),
        }
    }

    pub fn mark_layout_dirty(self) {
        let mut layout_generation = self.layout_generation;
        let next = *layout_generation.peek() + 1;
        layout_generation.set(next);
    }
}
