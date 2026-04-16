//! API request and response types for the ticket viewer.
//!
//! Mirrors the TypeScript types in `tools/viewer/ticket-viewer/frontend/src/types.ts`.
//! Kept separate from the HTTP client so components can import types without
//! pulling in the transport layer.

use serde::{Deserialize, Serialize};

// ── Workspace ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceInfo {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspacesResponse {
    pub workspaces: Vec<WorkspaceInfo>,
}

// ── Ticket ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TicketSummary {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    #[serde(rename = "type", default)]
    pub ticket_type: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketsResponse {
    pub workspace: String,
    pub items: Vec<TicketSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketDetail {
    pub id: String,
    pub created_at: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketDetailResponse {
    pub workspace: String,
    pub ticket: TicketDetail,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketDescriptionResponse {
    pub id: String,
    pub workspace: String,
    pub description: Option<String>,
}

// ── Graph / Subgraph ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct GraphNodeItem {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub depth: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphEdgeItem {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubgraphStats {
    pub nodes_returned: usize,
    pub edges_returned: usize,
    pub max_depth_reached: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphSubgraphResponse {
    pub workspace: String,
    pub nodes: Vec<GraphNodeItem>,
    pub edges: Vec<GraphEdgeItem>,
    pub truncated: bool,
    pub stats: SubgraphStats,
}

// ── Schema ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct FieldDef {
    pub field_type: String,
    pub required: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransitionDef {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EdgeRuleDef {
    pub directed: bool,
    pub acyclic_enforced: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypeSchema {
    pub type_id: String,
    pub states: Vec<String>,
    pub transitions: Vec<TransitionDef>,
    pub fields: std::collections::BTreeMap<String, FieldDef>,
    pub edge_rules: std::collections::BTreeMap<String, EdgeRuleDef>,
    pub required_states: Vec<String>,
    pub terminal_states: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaListResponse {
    pub workspace: String,
    pub types: Vec<TypeSchema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchemaDetailResponse {
    pub workspace: String,
    pub schema: TypeSchema,
}

// ── Mutation requests ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default)]
pub struct TicketPatch {
    pub workspace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateTicketRequest {
    #[serde(rename = "type")]
    pub type_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<std::collections::BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTicketResponse {
    pub workspace: String,
    pub ticket: TicketDetail,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeMutationBody {
    pub from_id: String,
    pub to_id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ── History ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryEntry {
    pub rev: u64,
    pub ts: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketHistoryResponse {
    pub id: String,
    pub workspace: String,
    pub count: u64,
    pub entries: Vec<HistoryEntry>,
}

// ── State colours ─────────────────────────────────────────────────────────────

/// Returns `(background, foreground)` CSS colour pair for a ticket state.
///
/// Used by badges, cards, and search results throughout the UI.
pub fn state_colors(state: &str) -> (&'static str, &'static str) {
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

/// Returns a single accent colour for an optional state string.
///
/// Convenience wrapper used by graph node borders and other simple indicators.
pub fn state_accent(state: Option<&str>) -> &'static str {
    match state {
        Some("new") => "#a0a0c8",
        Some("ready") => "#86efac",
        Some("in-implementation") => "#fbbf24",
        Some("in-review") => "#c084fc",
        Some("done") => "#4ade80",
        Some("cancelled") => "#f87171",
        _ => "#9ca3af",
    }
}
