//! Per-workspace UI state store for [`TicketListPage`].
//!
//! State is persisted to `localStorage` under the key
//! `ticket-viewer:{workspace}:ui` and restored on mount.  The active ticket
//! ID is additionally mirrored into the URL hash as `#id=<ticket_id>` for
//! deep-link support.
//!
//! ## Usage
//!
//! ```rust,ignore
//! #[component]
//! pub fn TicketListPage(workspace: String) -> Element {
//!     let store = TicketListStore::use_store(&workspace);
//!     let ws_persist = workspace.clone();
//!     use_effect(move || { store.persist(&ws_persist); });
//!     // ...
//! }
//! ```
//!
//! ## Hash ↔ signal loop prevention
//!
//! [`set_hash_param`] triggers a `hashchange` event.  The `hashchange`
//! listener inside [`TicketListStore::use_store`] reads the new hash and
//! calls `Signal::set` only when the value actually differs from the current
//! one (`Signal::set` is a no-op when the value is unchanged).  Because the
//! browser does not fire `hashchange` when the new hash equals the old hash,
//! the cycle terminates after at most one extra no-op round-trip.

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
};

// ── localStorage key ──────────────────────────────────────────────────────────
#[cfg(target_arch = "wasm32")]
fn ls_key(workspace: &str) -> String {
    format!("ticket-viewer:{workspace}:ui")
}

// ── Serialisable snapshot ─────────────────────────────────────────────────────

/// Snapshot of the per-workspace UI state that is persisted to localStorage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceUiState {
    /// Text filter applied to the ticket title / ID.
    #[serde(default)]
    pub filter: String,
    /// State filter (e.g. `"new"`, `"in-implementation"`, …).
    #[serde(default)]
    pub state_filter: String,
    /// Sort key for the ticket list.
    #[serde(default = "default_sort_key")]
    pub sort_key: String,
    /// Currently selected ticket ID, if any.
    #[serde(default)]
    pub open_ticket_id: Option<String>,
    /// Active tab inside the ticket detail panel.
    #[serde(default = "default_active_tab")]
    pub active_tab: String,
}

impl Default for WorkspaceUiState {
    fn default() -> Self {
        Self {
            filter: String::new(),
            state_filter: String::new(),
            sort_key: default_sort_key(),
            open_ticket_id: None,
            active_tab: default_active_tab(),
        }
    }
}

fn default_sort_key() -> String {
    "updated_at".to_string()
}

fn default_active_tab() -> String {
    "description".to_string()
}

// ── localStorage helpers ──────────────────────────────────────────────────────

/// Read and deserialise the saved workspace UI state from `localStorage`.
///
/// Returns [`WorkspaceUiState::default`] on any error (missing key, JSON
/// parse failure, or unavailable `localStorage`).
pub fn load_from_local_storage(workspace: &str) -> WorkspaceUiState {
    #[cfg(target_arch = "wasm32")]
    {
        let key = ls_key(workspace);
        let raw = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(&key).ok().flatten());

        if let Some(json) = raw {
            if let Ok(state) = serde_json::from_str::<WorkspaceUiState>(&json) {
                return state;
            }
        }
        WorkspaceUiState::default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = workspace;
        WorkspaceUiState::default()
    }
}

/// Serialise `state` and write it to `localStorage` under
/// `ticket-viewer:{workspace}:ui`.
///
/// Ignores quota errors silently.
pub fn save_to_local_storage(
    workspace: &str,
    state: &WorkspaceUiState,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let key = ls_key(workspace);
        if let Ok(json) = serde_json::to_string(state) {
            if let Some(storage) =
                web_sys::window().and_then(|w| w.local_storage().ok().flatten())
            {
                let _ = storage.set_item(&key, &json);
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (workspace, state);
    }
}

// ── TicketListStore ───────────────────────────────────────────────────────────

/// Reactive per-workspace UI store for `TicketListPage`.
///
/// All fields are Dioxus [`Signal`]s, so any component or effect that reads
/// them will automatically re-run when they change.
///
/// Call [`TicketListStore::use_store`] **once** near the top of the
/// `TicketListPage` component. Then add a `use_effect` that drives
/// [`TicketListStore::persist`] to flush changes back to `localStorage` and
/// the URL hash:
///
/// ```rust,ignore
/// let store = TicketListStore::use_store(&workspace);
/// let ws = workspace.clone();
/// use_effect(move || { store.persist(&ws); });
/// ```
#[derive(Clone, Copy)]
pub struct TicketListStore {
    /// Full-text / ID search filter.
    pub filter: Signal<String>,
    /// Ticket state filter (empty string = show all).
    pub state_filter: Signal<String>,
    /// Sort column key.
    pub sort_key: Signal<String>,
    /// Currently open ticket ID, or `None`.
    pub open_ticket_id: Signal<Option<String>>,
    /// Active tab inside the detail panel (`"description"`, `"history"`, …).
    pub active_tab: Signal<String>,
    /// RAII guard that keeps the `hashchange` listener alive for the
    /// lifetime of the component.  Never read directly.
    _hash_listener: Signal<Option<EventListener>>,
}

impl TicketListStore {
    /// Initialise the store inside a Dioxus component.
    ///
    /// 1. Loads saved state from `localStorage` (`ticket-viewer:{workspace}:ui`).
    /// 2. Checks `#id=…` in the URL hash and, if present, uses it as the
    ///    initial `open_ticket_id` (deep-link priority over localStorage).
    /// 3. Registers a `hashchange` listener on `window` so that browser
    ///    back / forward navigation updates `open_ticket_id` reactively.
    pub fn use_store(workspace: &str) -> Self {
        let saved = load_from_local_storage(workspace);

        // URL hash takes priority for the ticket ID (supports deep-links).
        let hash_id = get_hash_param("id");
        let initial_id = hash_id.or(saved.open_ticket_id.clone());

        let filter = use_signal(|| saved.filter);
        let state_filter = use_signal(|| saved.state_filter);
        let sort_key = use_signal(|| saved.sort_key);
        let mut open_ticket_id: Signal<Option<String>> =
            use_signal(|| initial_id);
        let active_tab = use_signal(|| saved.active_tab);

        // ── hashchange listener ───────────────────────────────────────────────
        // Stored in a Signal so it is dropped automatically when the component
        // unmounts (RAII).  The closure captures `open_ticket_id` by copy
        // (Signal<T> is Copy) so no lifetime issues arise.
        let _hash_listener: Signal<Option<EventListener>> = use_signal(|| {
            #[cfg(target_arch = "wasm32")]
            {
                web_sys::window().map(|window| {
                    EventListener::new(&window, "hashchange", move |_event| {
                        let id = get_hash_param("id");
                        // Only mutate when the value actually changed to
                        // prevent re-render loops.
                        if *open_ticket_id.peek() != id {
                            open_ticket_id.set(id);
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
            sort_key,
            open_ticket_id,
            active_tab,
            _hash_listener,
        }
    }

    /// Persist the current signal values to `localStorage` and mirror the
    /// active ticket ID into the URL hash (`#id=<id>` or removes `id` when
    /// `None`).
    ///
    /// Call this inside a `use_effect` — Dioxus will re-run the effect
    /// automatically whenever any of the signals read by this method change.
    pub fn persist(
        &self,
        workspace: &str,
    ) {
        let state = WorkspaceUiState {
            filter: self.filter.read().clone(),
            state_filter: self.state_filter.read().clone(),
            sort_key: self.sort_key.read().clone(),
            open_ticket_id: self.open_ticket_id.read().clone(),
            active_tab: self.active_tab.read().clone(),
        };
        save_to_local_storage(workspace, &state);

        // Keep the URL hash in sync for deep-link support.
        match state.open_ticket_id.as_deref() {
            Some(id) => set_hash_param("id", id),
            None => remove_hash_param("id"),
        }
    }
}
