//! Dioxus Router route definitions for the spec viewer SPA.
//!
//! Route hierarchy:
//!
//!   /                → Redirect to /specs
//!   /specs           → SpecListPage (flat list + detail panel)
//!   /specs/tree      → SpecTreePage (hierarchical parent-child tree)
//!   /specs/graph     → SpecGraphPage (full-screen spec graph)
//!   /specs/:id       → SpecDetailPage (detail page navigated directly)

mod detail;
mod graph;
mod list;
mod tree;

use dioxus::prelude::*;

pub use detail::SpecDetailPage;
pub use graph::SpecGraphPage;
pub use list::SpecListPage;
pub use tree::SpecTreePage;

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[redirect("/", || Route::SpecListPage {})]
    #[route("/specs")]
    SpecListPage {},

    #[route("/specs/tree")]
    SpecTreePage {},

    #[route("/specs/graph")]
    SpecGraphPage {},

    #[route("/specs/:id")]
    SpecDetailPage { id: String },
}
