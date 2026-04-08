//! TicketBackend trait and its HTTP implementation.
//!
//! The trait abstracts the REST API surface served by `ticket-viewer` /
//! `ticket-http`. The `HttpTicketBackend` implementation calls those endpoints
//! using the browser Fetch API via `gloo-net`.

use serde::Deserialize;
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
}
