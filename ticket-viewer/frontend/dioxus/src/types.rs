//! API request and response types for the ticket viewer.
//!
//! Kept separate from the HTTP client so components can import types without
//! pulling in the transport layer.

use serde::{
    Deserialize,
    Serialize,
};

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

// ── Ticket file tree ──────────────────────────────────────────────────────────

/// A single user-visible file belonging to a ticket.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TicketFileEntry {
    /// Relative path within the ticket folder, e.g. `"description.md"` or
    /// `"assets/design/plan.md"`.
    pub path: String,
    /// Display name (just the filename), e.g. `"plan.md"`.
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketFilesResponse {
    pub id: String,
    pub workspace: String,
    pub files: Vec<TicketFileEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketAssetResponse {
    pub id: String,
    pub workspace: String,
    pub path: String,
    pub content: String,
}

// ── Graph / Subgraph ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct GraphNodeItem {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub depth: usize,
    #[serde(rename = "type", default)]
    pub ticket_type: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HistoryEntry {
    pub rev: u64,
    pub ts: String,
    pub author: Option<String>,
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketHistoryResponse {
    pub id: String,
    pub workspace: String,
    pub count: u64,
    pub entries: Vec<HistoryEntry>,
}

// ── Batch operations ──────────────────────────────────────────────────────────

/// A single command within a batch request sent to `POST /api/batch`.
///
/// The `op` field is the discriminator; matches the server-side `BatchCommand`
/// serde representation.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BatchCommand {
    Update {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        state: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        from_state: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "std::collections::BTreeMap::is_empty"
        )]
        fields: std::collections::BTreeMap<String, serde_json::Value>,
    },
    Close {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_state: Option<String>,
    },
    Cancel {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

/// Request body for `POST /api/batch`.
#[derive(Debug, Clone, Serialize)]
pub struct BatchRequest {
    pub workspace: String,
    pub commands: Vec<BatchCommand>,
}

/// Per-command result returned in the batch response.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchCommandResult {
    /// The raw JSON result value for this command (ticket object, etc.).
    pub result: Option<serde_json::Value>,
    /// Error message if this command failed (non-transactional reporting).
    pub error: Option<String>,
}

/// Response body from `POST /api/batch`.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchResponse {
    pub workspace: String,
    pub status: String,
    pub count: usize,
    pub results: Vec<serde_json::Value>,
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
