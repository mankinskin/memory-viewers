//! Dioxus Router route definitions for the ticket viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                         → Redirect to /workspace/default
//!   /workspace/:ws            → TicketListPage (tickets in workspace)
//!   /workspace/:ws/new        → NewTicketPage (create ticket — modal overlay)
//!   /workspace/:ws/ticket/:id → TicketDetailPage (single ticket + dep graph)

use dioxus::prelude::*;

use viewer_api_dioxus::{HamburgerIcon, Header, Layout, Sidebar, ThemeSettings};

use crate::api::{HttpTicketBackend, TicketBackend};
use crate::types::TicketSummary;
use crate::components::batch_panel::BatchPanel;
use crate::components::create_ticket::CreateTicketModal;
use crate::components::dep_graph::DepGraph;
use crate::components::search::SearchBar;
use crate::components::ticket_detail::TicketDetail;
use crate::components::ticket_tree::TicketTree;
use crate::sse::use_sse;

// ── Route enum ────────────────────────────────────────────────────────────────

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[redirect("/", || Route::TicketListPage { workspace: "default".into() })]

    #[route("/workspace/:workspace")]
    TicketListPage { workspace: String },

    #[route("/workspace/:workspace/new")]
    NewTicketPage { workspace: String },

    #[route("/workspace/:workspace/ticket/:id")]
    TicketDetailPage { workspace: String, id: String },
}

// ── Pages ─────────────────────────────────────────────────────────────────────

#[component]
pub fn TicketListPage(workspace: String) -> Element {
    let nav = use_navigator();

    // ── Per-workspace UI state (filter, sort, open ticket, active tab) ────
    // Loads from localStorage (`ticket-viewer:{workspace}:ui`) + URL hash
    // (`#id=…`) on first render; registers a `hashchange` listener so that
    // browser back / forward navigation updates reactive state automatically.
    let store = crate::store::TicketListStore::use_store(&workspace);

    // Flush signals to localStorage and the URL hash on every state change.
    // Dioxus tracks the signal reads inside `persist` and re-runs the effect
    // automatically whenever filter, sort_key, open_ticket_id, or active_tab
    // changes.
    let ws_persist = workspace.clone();
    use_effect(move || {
        store.persist(&ws_persist);
    });

    // ── Layout state ──────────────────────────────────────────────────────
    let mut sidebar_collapsed = use_signal(|| false);
    let mut mobile_sidebar_open = use_signal(|| false);
    let mut show_theme_settings = use_signal(|| false);

    // ── Ticket list state ─────────────────────────────────────────────────
    let mut tickets: Signal<Vec<TicketSummary>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| true);
    let mut list_error: Signal<Option<String>> = use_signal(|| None);
    // Selected ticket ID — backed by the store for persistence and hash sync.
    // Signal<T> is Copy so this is a zero-cost alias to the same backing cell.
    let mut selected_id = store.open_ticket_id;
    // Mutable aliases for filter/sort signals so closures can call `.set()`.
    let mut filter = store.filter;
    let mut state_filter = store.state_filter;
    let sort_key = store.sort_key;

    // ── Multi-select state ────────────────────────────────────────────────
    let mut selected_ids: Signal<Vec<String>> = use_signal(Vec::new);
    // Whether the batch-selection checkbox mode is active (toggled by header).
    let mut show_checkboxes: Signal<bool> = use_signal(|| false);
    // Incrementing counter to force a ticket-list refresh after batch ops.
    let mut refresh_counter: Signal<u32> = use_signal(|| 0);

    // ── SSE live-update hook ──────────────────────────────────────────────
    // Must be called before any conditional logic (hook ordering rule).
    // Mutates `tickets` in-place on `ticket.upsert` / `ticket.delete`;
    // reconnects automatically with exponential backoff on error.
    use_sse(workspace.clone(), tickets);

    // ── Fetch ticket list on mount / workspace change / filter change ─────
    {
        let ws = workspace.clone();
        use_effect(move || {
            let ws = ws.clone();
            // Read filter signals here so the effect re-runs when they change.
            let query = filter.read().clone();
            let state = state_filter.read().clone();
            // Reading refresh_counter makes the effect re-run when it changes.
            let _rev = *refresh_counter.read();
            loading.set(true);
            list_error.set(None);
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                let q = if query.trim().is_empty() { None } else { Some(query.trim().to_string()) };
                let s = if state.is_empty() { None } else { Some(state) };
                match backend.list_tickets(&ws, s.as_deref(), q.as_deref(), Some(200)).await {
                    Ok(resp) => {
                        tickets.set(resp.items);
                        loading.set(false);
                    }
                    Err(e) => {
                        list_error.set(Some(e));
                        loading.set(false);
                    }
                }
            });
        });
    }

    // ── Derived values ────────────────────────────────────────────────────
    let ticket_count = tickets.read().len();
    let ws_for_new = workspace.clone();
    let ws_for_detail = workspace.clone();
    // Pre-compute for RSX (avoids string literals inside {} format blocks).
    let batch_btn_bg = if *show_checkboxes.read() { "var(--accent-blue)" } else { "var(--bg-secondary)" };

    rsx! {
        SearchBar { workspace: workspace.clone() }
        Layout {
            // ── Header ────────────────────────────────────────────────────
            header: rsx! {
                Header {
                    left: rsx! {
                        // Hamburger — visible on mobile only (CSS hides on desktop).
                        // min 44×44px tap target per WCAG AAA.
                        button {
                            class: "sidebar-hamburger",
                            style: "min-width: 44px; min-height: 44px;",
                            aria_label: "Open sidebar",
                            onclick: move |_| mobile_sidebar_open.set(true),
                            HamburgerIcon {}
                        }
                        span { class: "header-icon", "🎫" }
                        span { class: "header-title", "{workspace}" }
                        span {
                            class: "header-subtitle",
                            style: "margin-left: 8px; font-size: 12px; color: var(--text-muted);",
                            "{workspace}"
                        }
                    },
                    right: rsx! {
                        // Theme settings toggle button.
                        button {
                            style: "
                                padding: 6px 10px; border-radius: 6px;
                                border: 1px solid var(--border-subtle);
                                background: var(--bg-secondary);
                                color: var(--text-primary);
                                cursor: pointer; font-size: 14px;
                                min-height: 32px; margin-right: 6px;
                            ",
                            aria_label: "Theme settings",
                            onclick: move |_| {
                                let cur = *show_theme_settings.read();
                                show_theme_settings.set(!cur);
                            },
                            "🎨"
                        }
                        // "Batch select" toggle — enables checkboxes in the tree.
                        button {
                            style: "
                                padding: 6px 12px; border-radius: 6px;
                                border: 1px solid var(--border-subtle);
                                background: {batch_btn_bg};
                                color: var(--text-primary);
                                cursor: pointer; font-size: 12px; font-weight: 600;
                                min-height: 32px; margin-right: 6px;
                            ",
                            onclick: move |_| {
                                let currently = *show_checkboxes.read();
                                show_checkboxes.set(!currently);
                                if currently {
                                    // Turning off: clear selection.
                                    selected_ids.set(Vec::new());
                                }
                            },
                            "☑ Batch"
                        }
                        button {
                            style: "
                                padding: 6px 14px; border-radius: 6px; border: none;
                                background: var(--accent-blue); color: white;
                                cursor: pointer; font-size: 13px; font-weight: 600;
                                min-height: 32px;
                            ",
                            onclick: move |_| {
                                nav.push(Route::NewTicketPage { workspace: ws_for_new.clone() });
                            },
                            "+ New Ticket"
                        }
                    },
                }
            },

            // ── Sidebar — ticket tree slot ─────────────────────────────────
            Sidebar {
                title: "Tickets",
                badge: if ticket_count > 0 { Some(ticket_count.to_string()) } else { None },
                collapsed: *sidebar_collapsed.read(),
                on_toggle: move |_| sidebar_collapsed.toggle(),
                // Controlled mobile drawer — linked to the hamburger in the Header.
                mobile_open: Some(*mobile_sidebar_open.read()),
                on_mobile_open_change: move |open| mobile_sidebar_open.set(open),

                TicketTree {
                    tickets: tickets.read().clone(),
                    loading: *loading.read(),
                    error: list_error.read().clone(),
                    filter: filter.read().clone(),
                    on_filter_change: move |val: String| filter.set(val),
                    state_filter: state_filter.read().clone(),
                    on_state_filter_change: move |val: String| state_filter.set(val),
                    sort_key: sort_key.read().clone(),
                    selected_id: selected_id.read().clone(),
                    on_select: move |tid: String| {
                        selected_id.set(Some(tid));
                        mobile_sidebar_open.set(false);
                    },
                    show_checkboxes: *show_checkboxes.read(),
                    selected_ids: selected_ids.read().clone(),
                    on_toggle_select: move |tid: String| {
                        let mut ids = selected_ids.write();
                        if let Some(pos) = ids.iter().position(|x| x == &tid) {
                            ids.remove(pos);
                        } else {
                            ids.push(tid);
                        }
                    },
                    on_select_all: move |check: bool| {
                        if check {
                            let ids: Vec<String> = tickets.read().iter().map(|t| t.id.clone()).collect();
                            selected_ids.set(ids);
                        } else {
                            selected_ids.set(Vec::new());
                        }
                    },
                }
            }

            // ── Main panel — dep graph + ticket detail ──────────────────
            div {
                class: "content",
                style: "display: flex; flex-direction: row; overflow: hidden;",
                if let Some(ref id) = *selected_id.read() {
                    // Center: dependency graph fills remaining space
                    div {
                        key: "{id}",
                        style: "flex: 1; position: relative; min-width: 0; overflow: hidden;",
                        DepGraph {
                            workspace: ws_for_detail.clone(),
                            root_id: id.clone(),
                            on_select: move |new_id: String| {
                                selected_id.set(Some(new_id));
                            },
                        }
                    }
                    // Right sidebar: ticket detail editor
                    TicketDetail {
                        key: "{id}",
                        workspace: ws_for_detail.clone(),
                        id: id.clone(),
                    }
                } else {
                    div {
                        style: "
                            display: flex;
                            flex-direction: column;
                            align-items: center;
                            justify-content: center;
                            height: 100%;
                            width: 100%;
                            color: var(--text-muted);
                            gap: 8px;
                            padding: 2rem;
                            text-align: center;
                        ",
                        span { style: "font-size: 2rem;", "🎫" }
                        span { style: "font-size: 14px;", "Select a ticket from the sidebar to view details." }
                    }
                }
            }
        }

        // ── Theme settings panel — floats above everything ──────────────────
        if *show_theme_settings.read() {
            div {
                style: "
                    position: fixed; top: 48px; right: 16px; z-index: 200;
                    max-height: calc(100vh - 64px); overflow-y: auto;
                ",
                ThemeSettings {
                    on_close: move |_| show_theme_settings.set(false),
                }
            }
        }

        // ── Batch panel — floats above everything when tickets are selected ──
        if !selected_ids.read().is_empty() {
            BatchPanel {
                workspace: workspace.clone(),
                selected_ids: selected_ids.read().clone(),
                on_done: move |_| {
                    selected_ids.set(Vec::new());
                    show_checkboxes.set(false);
                    // Bump refresh counter to re-trigger the ticket list fetch.
                    *refresh_counter.write() += 1;
                },
            }
        }
    }
}

/// Standalone page that renders the CreateTicketModal as an overlay.
///
/// On cancel it navigates back to the ticket list.  On successful submit the
/// modal itself navigates to the new ticket's detail page.
#[component]
pub fn NewTicketPage(workspace: String) -> Element {
    let nav = use_navigator();
    let ws_back = workspace.clone();

    rsx! {
        CreateTicketModal {
            workspace: workspace.clone(),
            prefill: None,
            on_cancel: move |_| {
                nav.push(Route::TicketListPage { workspace: ws_back.clone() });
            },
        }
    }
}

/// Ticket detail page — redirects to the list page with the ticket pre-selected.
///
/// The three-panel layout (list + graph + detail) is now rendered entirely by
/// `TicketListPage`, so visiting this URL just sets the hash and redirects.
#[component]
pub fn TicketDetailPage(workspace: String, id: String) -> Element {
    let nav = use_navigator();
    let ws = workspace.clone();
    let tid = id.clone();
    use_effect(move || {
        // Set the URL hash so the store picks up the selected ticket.
        if let Some(win) = web_sys::window() {
            let _ = win.location().set_hash(&format!("id={tid}"));
        }
        nav.replace(Route::TicketListPage { workspace: ws.clone() });
    });
    rsx! {}
}

