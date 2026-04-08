//! Dioxus Router route definitions for the ticket viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                         → WorkspacePickerPage (list workspaces)
//!   /workspace/:ws            → TicketListPage (tickets in workspace)
//!   /workspace/:ws/ticket/:id → TicketDetailPage (single ticket)

use dioxus::prelude::*;

// ── Route enum ────────────────────────────────────────────────────────────────

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[route("/")]
    WorkspacePickerPage,

    #[route("/workspace/:workspace")]
    TicketListPage { workspace: String },

    #[route("/workspace/:workspace/ticket/:id")]
    TicketDetailPage { workspace: String, id: String },
}

// ── Page stubs ────────────────────────────────────────────────────────────────
// These are intentionally minimal scaffolds. Component ports (TicketTree, etc.)
// will be wired in by downstream tickets in Track 1.

#[component]
pub fn WorkspacePickerPage() -> Element {
    rsx! {
        div {
            style: "padding: 2rem; font-family: sans-serif;",
            h1 { "Ticket Viewer" }
            p { "Select a workspace to begin." }
        }
    }
}

#[component]
pub fn TicketListPage(workspace: String) -> Element {
    rsx! {
        div {
            style: "padding: 2rem; font-family: sans-serif;",
            h2 { "Workspace: {workspace}" }
            p { "Ticket list will be rendered here." }
            Link { to: Route::WorkspacePickerPage, "← Back" }
        }
    }
}

#[component]
pub fn TicketDetailPage(workspace: String, id: String) -> Element {
    rsx! {
        div {
            style: "padding: 2rem; font-family: sans-serif;",
            h2 { "Ticket: {id}" }
            p { "Workspace: {workspace}" }
            p { "Detail content will be rendered here." }
            Link {
                to: Route::TicketListPage { workspace: workspace.clone() },
                "← Back to list"
            }
        }
    }
}
