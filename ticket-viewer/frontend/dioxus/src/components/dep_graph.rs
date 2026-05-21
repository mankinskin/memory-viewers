use dioxus::prelude::*;

use crate::{
    layout::LayoutMode,
    types::TicketRef,
};

use viewer_api_dioxus::Projection;

mod edge_list;
mod interactions;
mod page;
mod picker;
mod state;
mod viewport;

// ── Props ──────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct DepGraphProps {
    pub workspace: String,
    pub root_id: String,
    /// Optional callback invoked when the user clicks a graph node.
    /// When provided the component calls this instead of navigating to
    /// `TicketDetailPage`.
    #[props(optional)]
    pub on_select: Option<EventHandler<TicketRef>>,
    /// Graph-preview selection — the node ID currently shown in the content
    /// panel (set by clicking a node). Highlights that card with an ember
    /// border without changing the primary list selection.
    #[props(optional)]
    pub selected_node_id: Option<String>,
    /// Which 3-D layout algorithm to use.
    #[props(default)]
    pub layout_mode: LayoutMode,
    /// Camera projection mode.
    #[props(default)]
    pub projection: Projection,
    /// Callback from the built-in settings overlay when layout mode changes.
    #[props(default)]
    pub on_layout_mode_change: Option<EventHandler<LayoutMode>>,
    /// Callback from the built-in settings overlay when projection changes.
    #[props(default)]
    pub on_projection_change: Option<EventHandler<Projection>>,
}

pub use page::DepGraph;
