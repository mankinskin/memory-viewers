//! Dioxus Router route definitions for the ticket viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                         → WorkspacePickerPage (list workspaces)
//!   /workspace/:ws            → TicketListPage (tickets in workspace)
//!   /workspace/:ws/new        → NewTicketPage (create ticket — modal overlay)
//!   /workspace/:ws/ticket/:id → TicketDetailPage (single ticket + dep graph)

use dioxus::prelude::*;

use viewer_api_dioxus::{HamburgerIcon, Header, Layout, Sidebar};

use crate::backend::{HttpTicketBackend, TicketBackend, TicketSummary, WorkspaceInfo};
use crate::components::create_ticket::CreateTicketModal;
use crate::components::dep_graph::DepGraph;
use crate::components::search::SearchBar;
use crate::components::ticket_detail::TicketDetail;
use crate::sse::use_sse;

// ── Route enum ────────────────────────────────────────────────────────────────

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[route("/")]
    WorkspacePickerPage,

    #[route("/workspace/:workspace")]
    TicketListPage { workspace: String },

    #[route("/workspace/:workspace/new")]
    NewTicketPage { workspace: String },

    #[route("/workspace/:workspace/ticket/:id")]
    TicketDetailPage { workspace: String, id: String },
}

// ── Pages ─────────────────────────────────────────────────────────────────────

#[component]
pub fn WorkspacePickerPage() -> Element {
    let mut workspaces: Signal<Option<Vec<WorkspaceInfo>>> = use_signal(|| None);
    let mut error: Signal<Option<String>> = use_signal(|| None);
    let mut hovered: Signal<Option<usize>> = use_signal(|| None);

    use_effect(move || {
        spawn(async move {
            let backend = HttpTicketBackend::new(None);
            match backend.list_workspaces().await {
                Ok(resp) => workspaces.set(Some(resp.workspaces)),
                Err(e) => error.set(Some(e)),
            }
        });
    });

    rsx! {
        div {
            style: "
                min-height: 100vh;
                background: #1a1a2e;
                font-family: sans-serif;
                color: #e0e0e8;
                display: flex;
                flex-direction: column;
                align-items: center;
                padding: 3rem 1.5rem;
                box-sizing: border-box;
            ",

            h1 {
                style: "margin: 0 0 0.5rem 0; font-size: 2rem; font-weight: 700;",
                "Ticket Viewer"
            }
            p {
                style: "margin: 0 0 2.5rem 0; color: #9999bb; font-size: 1rem;",
                "Select a workspace to begin."
            }

            // ── Loading state ─────────────────────────────────────────────
            if workspaces.read().is_none() && error.read().is_none() {
                div {
                    style: "color: #9999bb; font-size: 0.95rem;",
                    "Loading workspaces…"
                }
            }

            // ── Error state ───────────────────────────────────────────────
            if let Some(err) = error.read().as_deref() {
                div {
                    style: "
                        background: rgba(220,50,50,0.15);
                        border: 1px solid rgba(220,50,50,0.4);
                        border-radius: 8px;
                        padding: 1rem 1.5rem;
                        color: #ff8080;
                        max-width: 480px;
                        width: 100%;
                    ",
                    "Failed to load workspaces: {err}"
                }
            }

            // ── Workspace cards ───────────────────────────────────────────
            if let Some(ws_list) = workspaces.read().as_ref() {
                div {
                    style: "display: flex; flex-direction: column; gap: 0.75rem; width: 100%; max-width: 480px;",
                    if ws_list.is_empty() {
                        p {
                            style: "color: #9999bb; text-align: center;",
                            "No workspaces found."
                        }
                    }
                    for (idx, ws) in ws_list.iter().enumerate() {
                        {
                            let is_hovered = *hovered.read() == Some(idx);
                            let card_bg = if is_hovered { "#1e2a4a" } else { "#16213e" };
                            let card_border = if is_hovered { "rgba(130,130,220,0.55)" } else { "rgba(100,100,180,0.25)" };
                            let ws_name = ws.name.clone();
                            rsx! {
                                Link {
                                    to: Route::TicketListPage { workspace: ws_name.clone() },
                                    style: "text-decoration: none;",
                                    div {
                                        style: "
                                            background: {card_bg};
                                            border: 1px solid {card_border};
                                            border-radius: 10px;
                                            padding: 1.1rem 1.5rem;
                                            cursor: pointer;
                                            color: #e0e0e8;
                                            font-size: 1rem;
                                            font-weight: 600;
                                        ",
                                        onmouseenter: move |_| hovered.set(Some(idx)),
                                        onmouseleave: move |_| hovered.set(None),
                                        "{ws_name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

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
                        Link {
                            to: Route::WorkspacePickerPage,
                            class: "header-subtitle",
                            style: "margin-left: 8px; font-size: 12px; color: var(--text-muted); text-decoration: none;",
                            "← workspaces"
                        }
                    },
                    right: rsx! {
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

                // ── Filter controls ────────────────────────────────────────
                div {
                    style: "
                        padding: 8px 12px;
                        border-bottom: 1px solid var(--border-subtle);
                        display: flex;
                        flex-direction: column;
                        gap: 6px;
                    ",
                    input {
                        r#type: "text",
                        placeholder: "Filter tickets…",
                        style: "
                            width: 100%;
                            padding: 6px 10px;
                            border-radius: 6px;
                            border: 1px solid var(--border-subtle);
                            background: var(--bg-secondary);
                            color: var(--text-primary);
                            font-size: 12px;
                            box-sizing: border-box;
                            outline: none;
                        ",
                        value: "{filter.read()}",
                        oninput: move |e| filter.set(e.value()),
                    }
                    div {
                        style: "display: flex; flex-wrap: wrap; gap: 4px;",
                        for (lab, val) in [("All", ""), ("new", "new"), ("ready", "ready"), ("impl", "in-implementation"), ("review", "in-review"), ("done", "done"), ("cancelled", "cancelled")].into_iter() {
                            {
                                let is_active = state_filter.read().as_str() == val;
                                let chip_bg = if is_active { "var(--accent-blue)" } else { "var(--bg-secondary)" };
                                let v = val.to_string();
                                rsx! {
                                    button {
                                        key: "{val}",
                                        style: "
                                            padding: 2px 8px;
                                            border-radius: 10px;
                                            border: 1px solid var(--border-subtle);
                                            background: {chip_bg};
                                            color: var(--text-primary);
                                            font-size: 10px;
                                            cursor: pointer;
                                            font-weight: 600;
                                        ",
                                        onclick: move |_| state_filter.set(v.clone()),
                                        "{lab}"
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Loading state ──────────────────────────────────────────
                if *loading.read() {
                    div {
                        class: "sidebar-loading",
                        "Loading tickets…"
                    }
                }

                // ── Error state ────────────────────────────────────────────
                if let Some(err) = list_error.read().clone() {
                    div {
                        style: "padding: 12px; color: var(--error); font-size: 12px;",
                        "Failed to load: {err}"
                    }
                }

                // ── Ticket list ────────────────────────────────────────────
                if !*loading.read() {
                    if tickets.read().is_empty() {
                        div {
                            class: "sidebar-empty",
                            "No tickets in this workspace."
                        }
                    }
                    {
                        let mut sorted = tickets.read().clone();
                        match sort_key.read().as_str() {
                            "title" => sorted.sort_by(|a, b| a.title.cmp(&b.title)),
                            "state" => sorted.sort_by(|a, b| a.state.cmp(&b.state)),
                            "created_at" => sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
                            _ => sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
                        }
                        rsx! {
                            for ticket in sorted {
                                {
                                    let tid = ticket.id.clone();
                                    let tid_click = tid.clone();
                                    let title = ticket.title.clone().unwrap_or_else(|| "Untitled".into());
                                    let state = ticket.state.clone().unwrap_or_else(|| "new".into());
                                    let is_selected = *selected_id.read() == Some(tid.clone());

                                    let (state_bg, state_fg) = match state.as_str() {
                                        "new" => ("rgba(100,100,160,0.15)", "#a0a0c8"),
                                        "ready" => ("rgba(61,160,96,0.15)", "#86efac"),
                                        "in-implementation" => ("rgba(249,115,22,0.15)", "#fbbf24"),
                                        "in-review" => ("rgba(192,90,210,0.15)", "#c084fc"),
                                        "done" => ("rgba(74,222,128,0.15)", "#4ade80"),
                                        "cancelled" => ("rgba(248,113,113,0.15)", "#f87171"),
                                        _ => ("rgba(80,80,100,0.15)", "#9ca3af"),
                                    };

                                    let row_bg = if is_selected {
                                        "var(--bg-active)"
                                    } else {
                                        "transparent"
                                    };

                                    rsx! {
                                        button {
                                            key: "{tid}",
                                            style: "
                                                display: flex;
                                                flex-direction: column;
                                                gap: 4px;
                                                width: 100%;
                                                padding: 10px 14px;
                                                border: none;
                                                border-bottom: 1px solid var(--border-subtle);
                                                background: {row_bg};
                                                color: var(--text-primary);
                                                cursor: pointer;
                                                text-align: left;
                                                min-height: 44px;
                                            ",
                                            onclick: move |_| {
                                                selected_id.set(Some(tid_click.clone()));
                                                // Close mobile drawer when a ticket is selected.
                                                mobile_sidebar_open.set(false);
                                            },
                                            span {
                                                style: "font-size: 13px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                                "{title}"
                                            }
                                            span {
                                                style: "
                                                    display: inline-block;
                                                    font-size: 10px;
                                                    font-weight: 600;
                                                    padding: 1px 7px;
                                                    border-radius: 10px;
                                                    background: {state_bg};
                                                    color: {state_fg};
                                                ",
                                                "{state}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
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

