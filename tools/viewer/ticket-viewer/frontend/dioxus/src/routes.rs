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
use crate::components::ticket_content::TicketContent;
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
    // ── View mode: "split" | "graph" | "content" ─────────────────────
    let mut view_mode: Signal<String> = use_signal(|| "split".to_string());
    // ── Selected file: (ticket_id, relative_path) ────────────────────
    // Set when the user clicks a file sub-row in the sidebar.
    let mut selected_file: Signal<Option<(String, String)>> = use_signal(|| None);
    // ── Responsive panel priority system ─────────────────────────────
    // Minimum widths per panel (px) define the auto-collapse thresholds.
    // Priority (highest = collapses last):
    //   P0 — TicketContent  (never collapses — gets all remaining space)
    //   P1 — DepGraph       (collapses second, in split mode only)
    //   P2 — TicketDetail   (collapses first — lowest priority)
    //   sidebar             (managed separately by existing toggle button)
    //
    // Thresholds are the minimum total viewport width needed to show each
    // additional panel at its minimum size, from narrowest to widest:
    //   DETAIL : sidebar(240) + graph(280) + content(360) + detail(280) = 1160
    //   GRAPH  : sidebar(240) + graph(280) + content(360)               =  880
    const DETAIL_COLLAPSE_PX: u32 = 1160;
    const GRAPH_COLLAPSE_PX: u32 = 880;
    // Live viewport width — updated by the resize listener registered below.
    let mut window_width: Signal<u32> = use_signal(|| {
        let w: u32 = {
            #[cfg(target_arch = "wasm32")]
            {
                web_sys::window()
                    .and_then(|w| w.inner_width().ok())
                    .and_then(|v| v.as_f64())
                    .map(|f| f as u32)
                    .unwrap_or(1200)
            }
            #[cfg(not(target_arch = "wasm32"))]
            { 1200 }
        };
        w
    });
    // Keep the resize EventListener alive in a Signal so it is not dropped.
    #[cfg(target_arch = "wasm32")]
    let mut _resize_guard: Signal<Option<gloo_events::EventListener>> = use_signal(|| None);
    // Manual overrides: None = auto from viewport width; Some(b) = force state.
    let mut detail_panel_override: Signal<Option<bool>> = use_signal(|| None);
    let mut graph_panel_override: Signal<Option<bool>> = use_signal(|| None);
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

    // ── Graph prefetch — fill GraphCache via the central fetch service ────
    // 1. When selected_id changes: immediately trigger a fetch for that ticket
    //    so the graph is ready as fast as possible.
    // 2. When the ticket list loads/changes: speculatively fetch the first N
    //    tickets so switching to them is instant.
    // Both paths use GraphFetchService.ensure_fetched which deduplicates
    // concurrent requests and never spawns a duplicate fetch for a key that
    // is already in-flight or cached.
    #[cfg(target_arch = "wasm32")]
    {
        let svc = use_context::<crate::graph_fetch::GraphFetchService>();
        let ws_sel = workspace.clone();
        // Reactive on selected_id: fires immediately when a ticket is clicked.
        use_effect(move || {
            if let Some(ref id) = *selected_id.read() {
                svc.ensure_fetched(&ws_sel, id);
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        const PREFETCH_N: usize = 8;
        let svc = use_context::<crate::graph_fetch::GraphFetchService>();
        let ws_pre = workspace.clone();
        // Reactive on tickets: fires when the list loads or filter changes.
        use_effect(move || {
            let tix = tickets.read();
            for t in tix.iter().take(PREFETCH_N) {
                svc.ensure_fetched(&ws_pre, &t.id);
            }
        });
    }

    // ── Register window resize listener (WASM only) ──────────────────────
    #[cfg(target_arch = "wasm32")]
    use_effect(move || {
        if let Some(win) = web_sys::window() {
            let listener = gloo_events::EventListener::new(&win, "resize", move |_| {
                if let Some(w) = web_sys::window() {
                    if let Ok(iw) = w.inner_width() {
                        if let Some(v) = iw.as_f64() {
                            window_width.set(v as u32);
                        }
                    }
                }
            });
            _resize_guard.set(Some(listener));
        }
    });

    // ── Derived values ────────────────────────────────────────────────────
    let ticket_count = tickets.read().len();
    let ws_for_new = workspace.clone();
    let ws_for_detail = workspace.clone();
    let ws_for_content = workspace.clone();
    // Toggle batch button picks up an `.btn-active` modifier when checkbox mode is on.
    let batch_btn_class = if *show_checkboxes.read() {
        "btn btn-secondary btn-active"
    } else {
        "btn btn-secondary"
    };

    // ── Panel collapse states (derived from viewport width + user override) ──
    let win_w = *window_width.read();
    let detail_is_collapsed = detail_panel_override.read().unwrap_or(win_w < DETAIL_COLLAPSE_PX);
    let graph_panel_collapsed = graph_panel_override.read().unwrap_or(win_w < GRAPH_COLLAPSE_PX);

    rsx! {
        SearchBar { workspace: workspace.clone() }
        Layout {
            // ── Header ────────────────────────────────────────────────────
            header: rsx! {
                Header {
                    left: rsx! {
                        // Sidebar toggle — always visible on desktop; also serves as the
                        // mobile hamburger.  Collapses / expands the ticket list panel.
                        button {
                            class: "btn btn-icon",
                            aria_label: if *sidebar_collapsed.read() { "Open sidebar" } else { "Close sidebar" },
                            onclick: move |_| {
                                if *sidebar_collapsed.read() {
                                    sidebar_collapsed.set(false);
                                } else {
                                    // On desktop: toggle collapse.
                                    // On mobile: open the drawer.
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        use web_sys::window;
                                        let is_mobile = window()
                                            .and_then(|w| w.inner_width().ok())
                                            .and_then(|v| v.as_f64())
                                            .map(|f| f < 640.0)
                                            .unwrap_or(false);
                                        if is_mobile {
                                            mobile_sidebar_open.set(true);
                                        } else {
                                            sidebar_collapsed.toggle();
                                        }
                                    }
                                    #[cfg(not(target_arch = "wasm32"))]
                                    sidebar_collapsed.toggle();
                                }
                            },
                            HamburgerIcon {}
                        }
                        span { class: "header-icon", "🎫" }
                        span { class: "header-title", "{workspace}" }
                        span {
                            class: "header-subtitle",
                            "{workspace}"
                        }
                    },
                    right: rsx! {
                        // Settings — opens the theme / settings panel.
                        button {
                            class: "btn btn-icon",
                            aria_label: "Settings",
                            title: "Settings",
                            onclick: move |_| {
                                let cur = *show_theme_settings.read();
                                show_theme_settings.set(!cur);
                            },
                            "⚙"
                        }
                        // Batch select toggle — secondary, takes `btn-active` when on.
                        button {
                            class: "{batch_btn_class}",
                            aria_pressed: if *show_checkboxes.read() { "true" } else { "false" },
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
                    workspace: workspace.clone(),
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
                    on_select_file: move |(tid, path): (String, String)| {
                        selected_id.set(Some(tid.clone()));
                        selected_file.set(Some((tid, path)));
                        mobile_sidebar_open.set(false);
                        // Switch to content view so the file is immediately visible.
                        if view_mode.read().as_str() == "graph" {
                            view_mode.set("content".to_string());
                        }
                    },
                    selected_file: selected_file.read().clone(),
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
                    on_new_ticket: move |_| {
                        nav.push(Route::NewTicketPage { workspace: ws_for_new.clone() });
                    },
                }
            }

            // ── Main panel — dep graph + ticket detail ──────────────────
            div {
                class: "content",
                style: "display: flex; flex-direction: column; overflow: hidden;",
                if let Some(ref id) = *selected_id.read() {
                    // ── View mode toggle bar ───────────────────────────
                    {
                        let vm = view_mode.read().clone();
                        let btn_style = |active: bool| format!(
                            "padding: 3px 10px; font-size: 11px; font-weight: 600; \
                             border: 1px solid var(--border-subtle); \
                             border-radius: 4px; cursor: pointer; \
                             background: {}; color: {};",
                            if active { "var(--accent-blue)" } else { "var(--bg-secondary)" },
                            if active { "#fff" } else { "var(--text-muted)" },
                        );
                        // Panel toggle buttons: highlighted when the panel is collapsed
                        // (indicating the user can click to expand it).
                        let pnl_btn_style = |collapsed: bool| format!(
                            "padding: 3px 8px; font-size: 10px; font-weight: 500; \
                             border: 1px solid var(--border-subtle); \
                             border-radius: 4px; cursor: pointer; \
                             background: {}; color: {};",
                            if collapsed {
                                "color-mix(in srgb, var(--accent-blue) 25%, var(--bg-secondary))"
                            } else {
                                "var(--bg-secondary)"
                            },
                            if collapsed { "var(--accent-blue)" } else { "var(--text-muted)" },
                        );
                        let g_collapsed = graph_panel_collapsed;
                        let d_collapsed = detail_is_collapsed;
                        rsx! {
                            div {
                                style: "
                                    display: flex;
                                    align-items: center;
                                    gap: 4px;
                                    padding: 4px 10px;
                                    border-bottom: 1px solid var(--border-subtle);
                                    background: var(--bg-primary);
                                    flex-shrink: 0;
                                ",
                                span {
                                    style: "font-size: 11px; color: var(--text-muted); margin-right: 4px;",
                                    "View:"
                                }
                                button {
                                    style: "{btn_style(vm == \"graph\")}",
                                    onclick: move |_| view_mode.set("graph".to_string()),
                                    "Graph"
                                }
                                button {
                                    style: "{btn_style(vm == \"split\")}",
                                    onclick: move |_| view_mode.set("split".to_string()),
                                    "Split"
                                }
                                button {
                                    style: "{btn_style(vm == \"content\")}",
                                    onclick: move |_| view_mode.set("content".to_string()),
                                    "Content"
                                }
                                // Push panel toggles to the right edge.
                                div { style: "flex: 1; min-width: 0;" }
                                // Graph panel toggle — only meaningful in split mode.
                                if vm == "split" {
                                    button {
                                        title: if g_collapsed {
                                            "Show graph panel (auto-collapsed)"
                                        } else {
                                            "Collapse graph panel"
                                        },
                                        style: "{pnl_btn_style(g_collapsed)}",
                                        onclick: move |_| {
                                            let auto = *window_width.read() < GRAPH_COLLAPSE_PX;
                                            let cur = graph_panel_override.read().unwrap_or(auto);
                                            graph_panel_override.set(Some(!cur));
                                        },
                                        if g_collapsed { "⬡ Graph ›" } else { "⬡ Graph ‹" }
                                    }
                                }
                                // Detail panel toggle — shown in split and content modes.
                                if vm != "graph" {
                                    button {
                                        title: if d_collapsed {
                                            "Show details panel (auto-collapsed)"
                                        } else {
                                            "Collapse details panel"
                                        },
                                        style: "{pnl_btn_style(d_collapsed)}",
                                        onclick: move |_| {
                                            let auto = *window_width.read() < DETAIL_COLLAPSE_PX;
                                            let cur = detail_panel_override.read().unwrap_or(auto);
                                            detail_panel_override.set(Some(!cur));
                                        },
                                        if d_collapsed { "☰ Details ›" } else { "☰ Details ‹" }
                                    }
                                }
                            }
                        }
                    }
                    // ── Ticket content area ────────────────────────────
                    div {
                        style: "display: flex; flex-direction: row; flex: 1; overflow: hidden; min-height: 0;",
                        // Dep graph — shown in "graph" and "split" modes,
                        // and not auto-collapsed by the priority panel system.
                        if view_mode.read().as_str() != "content" && !graph_panel_collapsed {
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
                        }
                        // Collapsed graph strip — narrow left edge when graph is
                        // priority-collapsed in split mode.
                        if view_mode.read().as_str() == "split" && graph_panel_collapsed {
                            div {
                                style: "
                                    width: 32px; min-width: 32px; height: 100%;
                                    background: var(--panel-bg-strong);
                                    backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                                    -webkit-backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                                    border-right: 1px solid var(--border-color);
                                    display: flex; flex-direction: column;
                                    align-items: center; justify-content: center;
                                    flex-shrink: 0; cursor: pointer;
                                ",
                                title: "Show graph panel",
                                onclick: move |_| {
                                    let auto = *window_width.read() < GRAPH_COLLAPSE_PX;
                                    let cur = graph_panel_override.read().unwrap_or(auto);
                                    graph_panel_override.set(Some(!cur));
                                },
                                div {
                                    style: "
                                        writing-mode: vertical-lr;
                                        font-size: 10px; font-weight: 600;
                                        color: var(--text-muted);
                                        letter-spacing: 0.08em; text-transform: uppercase;
                                        user-select: none; padding: 8px 0;
                                    ",
                                    "⬡ Graph ›"
                                }
                            }
                        }
                        // Ticket content (description / asset viewer) — shown in
                        // "content" and "split" modes.
                        if view_mode.read().as_str() != "graph" {
                            {
                                // Determine the active asset path for this ticket.
                                let active_asset = selected_file.read().as_ref()
                                    .filter(|(tid, _)| tid == id)
                                    .map(|(_, path)| path.clone());
                                // Look up the ticket fields from the summary list for
                                // the TOML tab.
                                let fields = tickets.read()
                                    .iter()
                                    .find(|t| &t.id == id)
                                    .map(|t| t.fields.clone())
                                    .unwrap_or(serde_json::Value::Object(Default::default()));
                                rsx! {
                                    div {
                                        key: "content-{id}",
                                        style: "flex: 1; min-width: 0; overflow: hidden; display: flex; flex-direction: column;",
                                        TicketContent {
                                            workspace: ws_for_content.clone(),
                                            ticket_id: id.clone(),
                                            fields,
                                            asset_path: active_asset,
                                        }
                                    }
                                }
                            }
                        }
                        // Ticket detail (metadata / state machine) — shown in
                        // content/split modes when not priority-collapsed.
                        if view_mode.read().as_str() != "graph" && !detail_is_collapsed {
                            TicketDetail {
                                key: "{id}",
                                workspace: ws_for_detail.clone(),
                                id: id.clone(),
                            }
                        }
                        // Collapsed detail strip — narrow right edge giving the user
                        // a click target to re-expand the detail panel.
                        if view_mode.read().as_str() != "graph" && detail_is_collapsed {
                            div {
                                style: "
                                    width: 32px; min-width: 32px; height: 100%;
                                    background: var(--panel-bg-strong);
                                    backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                                    -webkit-backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                                    border-left: 1px solid var(--border-color);
                                    display: flex; flex-direction: column;
                                    align-items: center; justify-content: center;
                                    flex-shrink: 0; cursor: pointer;
                                ",
                                title: "Show details panel",
                                onclick: move |_| {
                                    let auto = *window_width.read() < DETAIL_COLLAPSE_PX;
                                    let cur = detail_panel_override.read().unwrap_or(auto);
                                    detail_panel_override.set(Some(!cur));
                                },
                                div {
                                    style: "
                                        writing-mode: vertical-rl;
                                        transform: rotate(180deg);
                                        font-size: 10px; font-weight: 600;
                                        color: var(--text-muted);
                                        letter-spacing: 0.08em; text-transform: uppercase;
                                        user-select: none; padding: 8px 0;
                                    ",
                                    "Details ›"
                                }
                            }
                        }
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

