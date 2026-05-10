//! Per-page UI state store for [`SpecListPage`].
//!
//! State is persisted to `localStorage` under the key `spec-viewer:ui`
//! and restored on mount for the browse filters shown on `/specs`.

use dioxus::prelude::*;
use serde::{
    Deserialize,
    Serialize,
};
use viewer_api_dioxus::{Camera, Layout3D};

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
}

impl Default for SpecUiState {
    fn default() -> Self {
        Self {
            filter: String::new(),
            state_filter: String::new(),
        }
    }
}

// ── Store ─────────────────────────────────────────────────────────────────────

/// Reactive UI state store for the spec list page.
#[derive(Clone, Copy)]
pub struct SpecListStore {
    pub filter: Signal<String>,
    pub state_filter: Signal<String>,
}

impl SpecListStore {
    /// Mount and return the store, restoring persisted state if available.
    pub fn use_store() -> Self {
        let saved = Self::load_from_storage();

        let filter = use_signal(|| saved.filter.clone());
        let state_filter = use_signal(|| saved.state_filter.clone());

        Self {
            filter,
            state_filter,
        }
    }

    /// Flush current state to localStorage.
    pub fn persist(&self) {
        let snapshot = SpecUiState {
            filter: self.filter.read().clone(),
            state_filter: self.state_filter.read().clone(),
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
