//! SearchBar — floating full-text search overlay for the ticket viewer.
//!
//! # Features
//!
//! * Triggered by **Ctrl+K** / **Cmd+K** / **/** at the document level.
//! * Input accepts free-text or `field:value` predicates
//!   (e.g. `state:new priority:high`).
//! * Delegates to `GET /api/tickets?workspace=<ws>&query=<q>` (existing FTS).
//! * Shows ranked results in a floating panel (max 8 items).
//! * State/type **filter-chip facets** narrow results client-side.
//! * Clicking a result navigates to [`Route::TicketDetailPage`].
//! * Last 5 searches persisted in `localStorage` under
//!   `ticket-viewer:{ws}:recent-searches`; surfaced when the bar opens empty.
//! * Dismissed by **Escape** or a click outside the panel.
//!
//! # Usage
//!
//! Mount `SearchBar` once per workspace page, passing the current workspace
//! name.  It renders nothing when closed and a floating overlay when open.
//!
//! ```rust,ignore
//! rsx! {
//!     SearchBar { workspace: workspace.clone() }
//!     // …rest of page
//! }
//! ```
//!
//! # Predicate syntax tooltip
//!
//! The input shows a helper hint: `state:<value>  priority:<value>  type:<value>  <free text>`
//! as a placeholder, making the supported predicate syntax self-documenting.

use dioxus::prelude::*;
use gloo_events::EventListener;
use viewer_api_dioxus::set_hash_param;
use wasm_bindgen::JsCast as _;

use crate::backend::{HttpTicketBackend, TicketBackend, TicketSummary};
use crate::routes::Route;

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_RESULTS: usize = 8;
const MAX_RECENT: usize = 5;

// ── localStorage helpers ──────────────────────────────────────────────────────

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn recent_key(workspace: &str) -> String {
    format!("ticket-viewer:{workspace}:recent-searches")
}

fn load_recent(workspace: &str) -> Vec<String> {
    let store = match local_storage() {
        Some(s) => s,
        None => return vec![],
    };
    let raw = match store.get_item(&recent_key(workspace)).ok().flatten() {
        Some(r) => r,
        None => return vec![],
    };
    serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
}

fn save_recent(workspace: &str, query: &str) {
    let store = match local_storage() {
        Some(s) => s,
        None => return,
    };
    let mut recents = load_recent(workspace);
    // Remove duplicates, prepend new entry, truncate.
    recents.retain(|r| r != query);
    recents.insert(0, query.to_string());
    recents.truncate(MAX_RECENT);
    if let Ok(json) = serde_json::to_string(&recents) {
        let _ = store.set_item(&recent_key(workspace), &json);
    }
}

// ── Facet helpers ─────────────────────────────────────────────────────────────

fn ticket_state(t: &TicketSummary) -> &str {
    t.state.as_deref().unwrap_or("")
}

fn ticket_type(t: &TicketSummary) -> &str {
    t.ticket_type.as_deref().unwrap_or("")
}

/// Collect unique non-empty values for `key_fn` across all items.
fn unique_values<F: Fn(&TicketSummary) -> &str>(items: &[TicketSummary], key_fn: F) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .map(|t| key_fn(t))
        .filter(|s| !s.is_empty())
        .filter(|s| seen.insert(s.to_string()))
        .map(|s| s.to_string())
        .collect()
}

// ── State color ────────────────────────────────────────────────────────────────

fn state_color(state: &str) -> (&'static str, &'static str) {
    match state {
        "new" => ("#2d2d4a", "#a0a0c8"),
        "ready" => ("#1a3d28", "#86efac"),
        "in-implementation" => ("#3d2e1a", "#fbbf24"),
        "in-review" => ("#361a4a", "#c084fc"),
        "done" => ("#1a3d28", "#4ade80"),
        "cancelled" => ("#3d1a1a", "#f87171"),
        _ => ("#2a2a3a", "#9ca3af"),
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct SearchBarProps {
    pub workspace: String,
}

// ── Component ─────────────────────────────────────────────────────────────────

/// Floating full-text search bar triggered by Ctrl+K / Cmd+K / /.
///
/// Renders nothing when closed.  When open it renders a full-screen backdrop
/// with a floating card containing the input, filter chips, and a result list.
#[component]
pub fn SearchBar(props: SearchBarProps) -> Element {
    let workspace = props.workspace.clone();

    // ── Open/closed state ────────────────────────────────────────────
    let mut open: Signal<bool> = use_signal(|| false);

    // ── Query typed in the input ─────────────────────────────────────
    let mut query: Signal<String> = use_signal(String::new);

    // ── Search results from the server ───────────────────────────────
    let mut results: Signal<Vec<TicketSummary>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| false);
    let mut search_err: Signal<Option<String>> = use_signal(|| None);

    // ── Active facet filters (None = no filter) ───────────────────────
    let mut state_filter: Signal<Option<String>> = use_signal(|| None);
    let mut type_filter: Signal<Option<String>> = use_signal(|| None);

    // ── Recent searches from localStorage ────────────────────────────
    let mut recents: Signal<Vec<String>> = use_signal(|| load_recent(&workspace));

    // ── Hover tracking for recent / result rows ───────────────────────
    let mut hovered_recent: Signal<Option<usize>> = use_signal(|| None);
    let mut hovered_result: Signal<Option<usize>> = use_signal(|| None);

    // ── Keyboard shortcut — open on Ctrl+K / Cmd+K / /  ─────────────
    // Holds the EventListener so it auto-removes on component drop.
    let mut _keydown_listener: Signal<Option<EventListener>> = use_signal(|| None);
    {
        let ws_open = workspace.clone();
        use_effect(move || {
            let window = match web_sys::window() {
                Some(w) => w,
                None => return,
            };
            let ws_for_listener = ws_open.clone();
            let listener = gloo_events::EventListener::new(
                &window,
                "keydown",
                move |evt| {
                    let ke = match evt.dyn_ref::<web_sys::KeyboardEvent>() {
                        Some(k) => k,
                        None => return,
                    };

                    // Ignore events inside text inputs / textareas (other than
                    // our own search input which has no id we can check cheaply).
                    if let Some(target) = ke.target()
                        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                    {
                        let tag = target.tag_name().to_lowercase();
                        let is_input = matches!(tag.as_str(), "input" | "textarea" | "select");
                        // `/` must not fire when the user is typing in any input.
                        if is_input && ke.key() == "/" {
                            return;
                        }
                        // Ctrl+K / Cmd+K still fires even inside inputs to avoid
                        // confusion, but that is standard browser behaviour.
                    }

                    let ctrl_or_cmd = ke.ctrl_key() || ke.meta_key();
                    let key = ke.key();

                    let should_open = (ctrl_or_cmd && key == "k") || key == "/";

                    if should_open && !open() {
                        ke.prevent_default();
                        // Reload recents when opening.
                        recents.set(load_recent(&ws_for_listener));
                        state_filter.set(None);
                        type_filter.set(None);
                        results.set(vec![]);
                        query.set(String::new());
                        open.set(true);
                    } else if key == "Escape" && open() {
                        open.set(false);
                    }
                },
            );
            _keydown_listener.set(Some(listener));
        });
    }

    // ── Trigger search whenever query changes (debounce via spawn) ────
    {
        let ws_search = workspace.clone();
        use_effect(move || {
            let q = query.read().clone();
            if q.trim().is_empty() {
                results.set(vec![]);
                loading.set(false);
                search_err.set(None);
                return;
            }
            loading.set(true);
            search_err.set(None);
            let ws = ws_search.clone();
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                match backend
                    .list_tickets(&ws, None, Some(q.trim()), Some(MAX_RESULTS as u32))
                    .await
                {
                    Ok(resp) => {
                        results.set(resp.items);
                        loading.set(false);
                    }
                    Err(e) => {
                        search_err.set(Some(e));
                        loading.set(false);
                    }
                }
            });
        });
    }

    // ── Close on Escape (also handled in keydown above) & backdrop click ─
    let on_close = move |_: MouseEvent| {
        open.set(false);
    };

    // ── Navigate to ticket + save recent search ───────────────────────
    let nav = use_navigator();

    // ── Render: nothing when closed ───────────────────────────────────
    if !*open.read() {
        return rsx! {};
    }

    // ── Derived: filtered results ──────────────────────────────────────
    let all_results = results.read().clone();

    let filtered: Vec<TicketSummary> = all_results
        .iter()
        .filter(|t| {
            if let Some(sf) = state_filter.read().as_deref() {
                if ticket_state(t) != sf {
                    return false;
                }
            }
            if let Some(tf) = type_filter.read().as_deref() {
                if ticket_type(t) != tf {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    let state_facets = unique_values(&all_results, ticket_state);
    let type_facets = unique_values(&all_results, ticket_type);

    let q_display = query.read().clone();
    let is_empty_query = q_display.trim().is_empty();

    // ── Render: open overlay ──────────────────────────────────────────
    rsx! {
        // Full-screen backdrop — clicks outside the card close the panel.
        div {
            style: "
                position: fixed;
                inset: 0;
                z-index: 500;
                background: rgba(0, 0, 0, 0.55);
                display: flex;
                flex-direction: column;
                align-items: center;
                padding-top: 10vh;
            ",
            // Clicking the backdrop itself (not the card) closes the overlay.
            onclick: on_close,

            // Floating search card — stop propagation so clicks inside don't
            // bubble up to the backdrop and close the panel.
            div {
                style: "
                    background: #1e1e2d;
                    border: 1px solid rgba(100, 100, 200, 0.35);
                    border-radius: 12px;
                    width: min(640px, 92vw);
                    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.6);
                    overflow: hidden;
                    display: flex;
                    flex-direction: column;
                    font-family: sans-serif;
                    color: #e0e0e8;
                ",
                onclick: move |evt| evt.stop_propagation(),

                // ── Input row ─────────────────────────────────────────
                div {
                    style: "
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        padding: 0.75rem 1rem;
                        border-bottom: 1px solid rgba(100, 100, 200, 0.2);
                    ",
                    // Search icon
                    span {
                        style: "color: #9999bb; font-size: 1rem; user-select: none;",
                        "🔍"
                    }
                    input {
                        r#type: "text",
                        autofocus: true,
                        placeholder: "Search… or state:<value>  priority:<value>  type:<value>",
                        value: "{q_display}",
                        style: "
                            flex: 1;
                            background: transparent;
                            border: none;
                            outline: none;
                            color: #e0e0e8;
                            font-size: 1rem;
                            caret-color: #6666cc;
                        ",
                        oninput: move |evt| {
                            query.set(evt.value().to_string());
                            // Reset facet filters when query changes.
                            state_filter.set(None);
                            type_filter.set(None);
                        },
                        onkeydown: move |evt| {
                            if evt.key() == Key::Escape {
                                open.set(false);
                            }
                        },
                    }
                    // Dismiss hint
                    span {
                        style: "
                            font-size: 11px;
                            color: #6666aa;
                            white-space: nowrap;
                            user-select: none;
                        ",
                        "Esc to close"
                    }
                }

                // ── Facet chips (only when we have results) ───────────
                if !state_facets.is_empty() || !type_facets.is_empty() {
                    div {
                        style: "
                            display: flex;
                            flex-wrap: wrap;
                            gap: 0.4rem;
                            padding: 0.5rem 1rem;
                            border-bottom: 1px solid rgba(100, 100, 200, 0.15);
                        ",
                        // State chips
                        for s in state_facets.iter() {
                            {
                                let s_val = s.clone();
                                let active = *state_filter.read() == Some(s_val.clone());
                                let (bg, fg) = state_color(&s_val);
                                let chip_border = if active {
                                    "2px solid #8080ff"
                                } else {
                                    "1px solid rgba(180,180,255,0.2)"
                                };
                                rsx! {
                                    button {
                                        style: "
                                            background: {bg};
                                            color: {fg};
                                            border: {chip_border};
                                            border-radius: 20px;
                                            padding: 2px 10px;
                                            font-size: 11px;
                                            font-weight: 600;
                                            cursor: pointer;
                                            letter-spacing: 0.03em;
                                        ",
                                        onclick: {
                                            let s2 = s_val.clone();
                                            move |_| {
                                                if *state_filter.read() == Some(s2.clone()) {
                                                    state_filter.set(None);
                                                } else {
                                                    state_filter.set(Some(s2.clone()));
                                                }
                                            }
                                        },
                                        "{s_val}"
                                    }
                                }
                            }
                        }
                        // Type chips
                        for t in type_facets.iter() {
                            {
                                let t_val = t.clone();
                                let active = *type_filter.read() == Some(t_val.clone());
                                let chip_bg = if active { "#2d2d5a" } else { "#252540" };
                                let chip_border = if active {
                                    "2px solid #8080ff"
                                } else {
                                    "1px solid rgba(180,180,255,0.2)"
                                };
                                rsx! {
                                    button {
                                        style: "
                                            background: {chip_bg};
                                            color: #b0b0e8;
                                            border: {chip_border};
                                            border-radius: 20px;
                                            padding: 2px 10px;
                                            font-size: 11px;
                                            font-weight: 600;
                                            cursor: pointer;
                                            letter-spacing: 0.03em;
                                        ",
                                        onclick: {
                                            let t2 = t_val.clone();
                                            move |_| {
                                                if *type_filter.read() == Some(t2.clone()) {
                                                    type_filter.set(None);
                                                } else {
                                                    type_filter.set(Some(t2.clone()));
                                                }
                                            }
                                        },
                                        "{t_val}"
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Result list ───────────────────────────────────────
                div {
                    style: "overflow-y: auto; max-height: 360px;",

                    // Loading indicator
                    if *loading.read() {
                        div {
                            style: "padding: 1rem; color: #9999bb; font-size: 0.9rem; text-align: center;",
                            "Searching…"
                        }
                    }

                    // Error
                    if let Some(err) = search_err.read().as_deref() {
                        div {
                            style: "
                                padding: 0.75rem 1rem;
                                color: #ff8080;
                                font-size: 0.85rem;
                            ",
                            "Error: {err}"
                        }
                    }

                    // Recent searches (shown when input is empty and there are no results)
                    if is_empty_query && !recents.read().is_empty() {
                        div {
                            style: "padding: 0.5rem 0;",
                            div {
                                style: "
                                    padding: 0.25rem 1rem;
                                    font-size: 11px;
                                    color: #6666aa;
                                    text-transform: uppercase;
                                    letter-spacing: 0.08em;
                                ",
                                "Recent searches"
                            }
                            for (r_idx, recent) in recents.read().iter().enumerate() {
                                {
                                    let r = recent.clone();
                                    let r2 = r.clone();
                                    let is_hovered = *hovered_recent.read() == Some(r_idx);
                                    let row_bg = if is_hovered { "rgba(80,80,140,0.25)" } else { "transparent" };
                                    rsx! {
                                        button {
                                            style: "
                                                display: flex;
                                                align-items: center;
                                                gap: 0.5rem;
                                                width: 100%;
                                                background: {row_bg};
                                                border: none;
                                                padding: 0.5rem 1rem;
                                                color: #c0c0e8;
                                                font-size: 0.9rem;
                                                cursor: pointer;
                                                text-align: left;
                                            ",
                                            onmouseenter: move |_| hovered_recent.set(Some(r_idx)),
                                            onmouseleave: move |_| hovered_recent.set(None),
                                            onclick: move |_| {
                                                state_filter.set(None);
                                                type_filter.set(None);
                                                query.set(r2.clone());
                                            },
                                            span { style: "color: #6666aa; font-size: 0.85rem;", "↩" }
                                            span { "{r}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Results
                    if !filtered.is_empty() {
                        for (res_idx, ticket) in filtered.iter().enumerate() {
                            {
                                let id = ticket.id.clone();
                                let ws_nav = workspace.clone();
                                let q_save = query.read().clone();
                                let ws_save = workspace.clone();
                                let title = ticket
                                    .title
                                    .as_deref()
                                    .unwrap_or("(untitled)")
                                    .to_string();
                                let state = ticket.state.as_deref().unwrap_or("").to_string();
                                let t_type = ticket_type(ticket).to_string();
                                let (state_bg, state_fg) = state_color(&state);
                                let id_short = if id.len() >= 8 { &id[..8] } else { &id };
                                let id_short = id_short.to_string();
                                let is_hovered = *hovered_result.read() == Some(res_idx);
                                let row_bg = if is_hovered { "rgba(80,80,160,0.2)" } else { "transparent" };
                                rsx! {
                                    button {
                                        style: "
                                            display: flex;
                                            flex-direction: column;
                                            gap: 0.2rem;
                                            width: 100%;
                                            background: {row_bg};
                                            border: none;
                                            border-bottom: 1px solid rgba(100,100,200,0.1);
                                            padding: 0.6rem 1rem;
                                            color: #e0e0e8;
                                            cursor: pointer;
                                            text-align: left;
                                        ",
                                        onmouseenter: move |_| hovered_result.set(Some(res_idx)),
                                        onmouseleave: move |_| hovered_result.set(None),
                                        onclick: move |_| {
                                            let trimmed = q_save.trim().to_string();
                                            if !trimmed.is_empty() {
                                                save_recent(&ws_save, &trimmed);
                                            }
                                            open.set(false);
                                            // Pre-select the ticket in TicketListPage via URL hash.
                                            // The TicketListStore hashchange listener picks this up.
                                            set_hash_param("id", &id);
                                            nav.push(Route::TicketListPage {
                                                workspace: ws_nav.clone(),
                                            });
                                        },
                                        // Title row
                                        div {
                                            style: "
                                                display: flex;
                                                align-items: center;
                                                gap: 0.5rem;
                                                font-size: 0.9rem;
                                                font-weight: 600;
                                            ",
                                            if !state.is_empty() {
                                                span {
                                                    style: "
                                                        background: {state_bg};
                                                        color: {state_fg};
                                                        border-radius: 10px;
                                                        padding: 1px 8px;
                                                        font-size: 10px;
                                                        font-weight: 700;
                                                        white-space: nowrap;
                                                    ",
                                                    "{state}"
                                                }
                                            }
                                            span { "{title}" }
                                        }
                                        // Meta row
                                        div {
                                            style: "
                                                display: flex;
                                                align-items: center;
                                                gap: 0.75rem;
                                                font-size: 11px;
                                                color: #7777aa;
                                            ",
                                            span { "{id_short}" }
                                            if !t_type.is_empty() {
                                                span { "· {t_type}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Empty state (query entered but no results)
                    if !is_empty_query && !*loading.read() && filtered.is_empty() && search_err.read().is_none() {
                        div {
                            style: "padding: 1.5rem 1rem; color: #9999bb; font-size: 0.9rem; text-align: center;",
                            "No tickets found for \"{q_display}\"."
                        }
                    }
                }

                // ── Footer hint ───────────────────────────────────────
                div {
                    style: "
                        padding: 0.5rem 1rem;
                        border-top: 1px solid rgba(100,100,200,0.15);
                        font-size: 11px;
                        color: #55556a;
                        display: flex;
                        gap: 1rem;
                    ",
                    span { "↩ open ticket" }
                    span { "Esc close" }
                    span { "Ctrl+K toggle" }
                }
            }
        }
    }
}
