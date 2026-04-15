//! Dioxus Router route definitions for the ticket viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                         → WorkspacePickerPage (list workspaces)
//!   /workspace/:ws            → TicketListPage (tickets in workspace)
//!   /workspace/:ws/new        → NewTicketPage (create ticket — modal overlay)
//!   /workspace/:ws/ticket/:id → TicketDetailPage (single ticket + dep graph)

use dioxus::prelude::*;

use crate::backend::{HttpTicketBackend, TicketBackend, WorkspaceInfo};
use crate::components::create_ticket::CreateTicketModal;
use crate::components::dep_graph::DepGraph;
use crate::components::ticket_detail::TicketDetail;

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
    let ws = workspace.clone();

    rsx! {
        div {
            style: "padding: 2rem; font-family: sans-serif; color: #e0e0e8;",
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem;",
                h2 { style: "margin: 0;", "Workspace: {workspace}" }
                button {
                    style: "
                        padding: 8px 18px; border-radius: 6px; border: none;
                        background: #3b82f6; color: white;
                        cursor: pointer; font-size: 14px; font-weight: 600;
                    ",
                    onclick: move |_| {
                        nav.push(Route::NewTicketPage { workspace: ws.clone() });
                    },
                    "+ New Ticket"
                }
            }
            p { "Ticket list will be rendered here." }
            Link { to: Route::WorkspacePickerPage, "← Back" }
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

/// Ticket detail page — shows ticket metadata and the dependency graph.
///
/// The `DepGraph` component occupies the full viewport (it renders inside the
/// transparent `#ui-root` overlay sitting on top of `#webgpu-canvas`).
#[component]
pub fn TicketDetailPage(workspace: String, id: String) -> Element {
    rsx! {
        // Dep-graph fills the entire overlay area.
        DepGraph {
            workspace: workspace.clone(),
            root_id: id.clone(),
        }

        // Inline-editing sidebar — fixed left panel above the graph.
        TicketDetail {
            workspace: workspace.clone(),
            id: id.clone(),
        }

        // Back-link HUD — pinned to top-left, above the graph.
        div {
            style: "
                position: absolute;
                top: 12px; left: 12px;
                z-index: 100;
                pointer-events: auto;
            ",
            Link {
                to: Route::TicketListPage { workspace: workspace.clone() },
                style: "
                    color: rgba(200,200,220,0.85);
                    font-family: sans-serif;
                    font-size: 12px;
                    text-decoration: none;
                    background: rgba(30,30,45,0.7);
                    padding: 4px 10px;
                    border-radius: 4px;
                ",
                "← {id}"
            }
        }
    }
}

