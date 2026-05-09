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

mod facets;
mod page;
mod recent;
mod results;

pub use page::SearchBar;
