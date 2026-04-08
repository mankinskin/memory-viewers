//! TicketBackend trait and its HTTP implementation.
//!
//! The trait abstracts the REST API surface served by `ticket-viewer` /
//! `ticket-http`. The `HttpTicketBackend` implementation calls those endpoints
//! using the browser Fetch API via `gloo-net`.

use serde::{Deserialize, Serialize};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

// ── Response types (mirrors tools/viewer/ticket-viewer/frontend/src/types.ts) ─

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceInfo {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspacesResponse {
    pub workspaces: Vec<WorkspaceInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TicketSummary {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub created_at: String,
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

// ── Graph/Subgraph response types ──────────────────────────────────────────

/// A node in the subgraph response (id, title, state, BFS depth).
#[derive(Debug, Clone, Deserialize)]
pub struct GraphNodeItem {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub depth: usize,
}

/// A directed edge in the subgraph response.
#[derive(Debug, Clone, Deserialize)]
pub struct GraphEdgeItem {
    pub from: String,
    pub to: String,
    pub kind: String,
}

/// Stats block returned alongside the subgraph.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SubgraphStats {
    pub nodes_returned: usize,
    pub edges_returned: usize,
    pub max_depth_reached: usize,
}

/// Response from `GET /api/graph/subgraph`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GraphSubgraphResponse {
    pub workspace: String,
    pub nodes: Vec<GraphNodeItem>,
    pub edges: Vec<GraphEdgeItem>,
    pub truncated: bool,
    pub stats: SubgraphStats,
}

// ── Schema response types ─────────────────────────────────────────────────

/// A field definition in the ticket schema.
#[derive(Debug, Clone, Deserialize)]
pub struct FieldDef {
    pub field_type: String,
    pub required: bool,
}

/// A valid state transition.
#[derive(Debug, Clone, Deserialize)]
pub struct TransitionDef {
    pub from: String,
    pub to: String,
}

/// An edge rule definition.
#[derive(Debug, Clone, Deserialize)]
pub struct EdgeRuleDef {
    pub directed: bool,
    pub acyclic_enforced: bool,
}

/// Full schema for one ticket type.
#[derive(Debug, Clone, Deserialize)]
pub struct TypeSchema {
    pub type_id: String,
    pub states: Vec<String>,
    pub transitions: Vec<TransitionDef>,
    pub fields: std::collections::BTreeMap<String, FieldDef>,
    #[allow(dead_code)]
    pub edge_rules: std::collections::BTreeMap<String, EdgeRuleDef>,
    pub required_states: Vec<String>,
    pub terminal_states: Vec<String>,
}

/// Response from `GET /api/schema`.
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaListResponse {
    #[allow(dead_code)]
    pub workspace: String,
    pub types: Vec<TypeSchema>,
}

// ── PATCH ticket request body ─────────────────────────────────────────────

/// Partial-update body sent to `PATCH /api/tickets/{id}`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct TicketPatch {
    pub workspace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<serde_json::Value>,
}

// ── Trait ────────────────────────────────────────────────────────────────────

/// Abstracts over the ticket REST API so components are not hard-wired to a
/// specific transport. Replace with a mock in tests or a direct in-process
/// call when running without a network.
///
/// Note: futures are not required to be `Send` — WASM is single-threaded.
pub trait TicketBackend {
    fn list_workspaces(&self) -> impl std::future::Future<Output = Result<WorkspacesResponse, String>>;

    fn list_tickets(
        &self,
        workspace: &str,
        state: Option<&str>,
        query: Option<&str>,
        limit: Option<u32>,
    ) -> impl std::future::Future<Output = Result<TicketsResponse, String>>;

    fn get_ticket(
        &self,
        workspace: &str,
        id: &str,
    ) -> impl std::future::Future<Output = Result<TicketDetailResponse, String>>;

    fn get_ticket_description(
        &self,
        workspace: &str,
        id: &str,
    ) -> impl std::future::Future<Output = Result<TicketDescriptionResponse, String>>;

    fn get_subgraph(
        &self,
        workspace: &str,
        root: &str,
        depth: u32,
    ) -> impl std::future::Future<Output = Result<GraphSubgraphResponse, String>>;

    fn patch_ticket(
        &self,
        workspace: &str,
        id: &str,
        patch: &TicketPatch,
    ) -> impl std::future::Future<Output = Result<TicketDetailResponse, String>>;

    fn list_schemas(
        &self,
        workspace: &str,
    ) -> impl std::future::Future<Output = Result<SchemaListResponse, String>>;
}

// ── HTTP implementation ───────────────────────────────────────────────────────

/// HTTP client that targets the running `ticket-viewer` server.
/// All paths are relative so they work both in dev (`dx serve` proxy) and
/// in production (same-origin deployment).
#[derive(Clone)]
pub struct HttpTicketBackend {
    /// Optional bearer token forwarded as `Authorization: Bearer <token>`.
    pub token: Option<String>,
}

impl HttpTicketBackend {
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }

    async fn fetch<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T, String> {
        let mut req = gloo_net::http::Request::get(url).header("Accept", "application/json");
        if let Some(ref t) = self.token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("{status} {url}: {body}"));
        }
        resp.json::<T>().await.map_err(|e| e.to_string())
    }
}

impl TicketBackend for HttpTicketBackend {
    async fn list_workspaces(&self) -> Result<WorkspacesResponse, String> {
        self.fetch("/api/workspaces").await
    }

    async fn list_tickets(
        &self,
        workspace: &str,
        state: Option<&str>,
        query: Option<&str>,
        limit: Option<u32>,
    ) -> Result<TicketsResponse, String> {
        let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
        let mut url = format!("/api/tickets?workspace={}", enc(workspace));
        if let Some(s) = state {
            url.push_str(&format!("&state={}", enc(s)));
        }
        if let Some(q) = query {
            url.push_str(&format!("&query={}", enc(q)));
        }
        if let Some(l) = limit {
            url.push_str(&format!("&limit={l}"));
        }
        self.fetch(&url).await
    }

    async fn get_ticket(
        &self,
        workspace: &str,
        id: &str,
    ) -> Result<TicketDetailResponse, String> {
        let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
        let url = format!("/api/tickets/{id}?workspace={}", enc(workspace));
        self.fetch(&url).await
    }

    async fn get_ticket_description(
        &self,
        workspace: &str,
        id: &str,
    ) -> Result<TicketDescriptionResponse, String> {
        let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
        let url = format!(
            "/api/tickets/{id}/description?workspace={}",
            enc(workspace)
        );
        self.fetch(&url).await
    }

    async fn get_subgraph(
        &self,
        workspace: &str,
        root: &str,
        depth: u32,
    ) -> Result<GraphSubgraphResponse, String> {
        let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
        let url = format!(
            "/api/graph/subgraph?workspace={}&root={}&depth={}",
            enc(workspace),
            enc(root),
            depth,
        );
        self.fetch(&url).await
    }

    async fn patch_ticket(
        &self,
        workspace: &str,
        id: &str,
        patch: &TicketPatch,
    ) -> Result<TicketDetailResponse, String> {
        let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
        let url = format!("/api/tickets/{id}?workspace={}", enc(workspace));
        let body = serde_json::to_string(patch).map_err(|e| e.to_string())?;
        let mut req = gloo_net::http::Request::patch(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(ref t) = self.token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = req.body(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("{status} PATCH {url}: {body}"));
        }
        resp.json::<TicketDetailResponse>().await.map_err(|e| e.to_string())
    }

    async fn list_schemas(&self, workspace: &str) -> Result<SchemaListResponse, String> {
        let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
        self.fetch(&format!("/api/schema?workspace={}", enc(workspace))).await
    }
}
