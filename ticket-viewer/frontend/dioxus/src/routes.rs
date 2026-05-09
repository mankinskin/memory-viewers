//! Dioxus Router route definitions for the ticket viewer SPA.

use dioxus::prelude::*;

mod detail;
mod list;
mod new_ticket;

pub use detail::TicketDetailPage;
pub use list::TicketListPage;
pub use new_ticket::NewTicketPage;

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
