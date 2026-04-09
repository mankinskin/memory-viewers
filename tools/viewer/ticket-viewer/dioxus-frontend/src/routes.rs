//! Dioxus Router route definitions for the ticket viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                         → WorkspacePickerPage (list workspaces)
//!   /workspace/:ws            → TicketListPage (tickets in workspace)
//!   /workspace/:ws/new        → NewTicketPage (create ticket — modal overlay)
//!   /workspace/:ws/ticket/:id → TicketDetailPage (single ticket + dep graph)

use dioxus::prelude::*;

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
    rsx! {
        div {
            style: "padding: 2rem; font-family: sans-serif; color: #e0e0e8;",
            h1 { "Ticket Viewer" }
            p { "Select a workspace to begin." }
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

