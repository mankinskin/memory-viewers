//! `TicketBackend` trait and its `HttpTicketBackend` implementation.
//!
//! The trait abstracts the REST API surface served by `ticket-viewer` /
//! `ticket-http`. The HTTP implementation calls those endpoints using the
//! browser Fetch API via `gloo-net`.

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;

use crate::types::*;

// ── Trait ────────────────────────────────────────────────────────────────────

/// Abstracts over the ticket REST API so components are not hard-wired to a
/// specific transport.
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

    fn create_edge(
        &self,
        workspace: &str,
        body: &EdgeMutationBody,
    ) -> impl std::future::Future<Output = Result<(), String>>;

    fn delete_edge(
        &self,
        workspace: &str,
        body: &EdgeMutationBody,
    ) -> impl std::future::Future<Output = Result<(), String>>;

    fn create_ticket(
        &self,
        workspace: &str,
        body: &CreateTicketRequest,
    ) -> impl std::future::Future<Output = Result<CreateTicketResponse, String>>;

    fn get_schema_by_type(
        &self,
        workspace: &str,
        type_id: &str,
    ) -> impl std::future::Future<Output = Result<SchemaDetailResponse, String>>;

    fn get_ticket_history(
        &self,
        workspace: &str,
        id: &str,
    ) -> impl std::future::Future<Output = Result<TicketHistoryResponse, String>>;

    fn undo_ticket(
        &self,
        workspace: &str,
        id: &str,
    ) -> impl std::future::Future<Output = Result<TicketDetailResponse, String>>;
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

    /// Send a JSON body with the given HTTP method.
    async fn send_json<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        url: &str,
        body: &str,
    ) -> Result<T, String> {
        let builder = match method {
            "PATCH" => gloo_net::http::Request::patch(url),
            "POST" => gloo_net::http::Request::post(url),
            "DELETE" => gloo_net::http::Request::delete(url),
            _ => gloo_net::http::Request::post(url),
        };
        let mut req = builder
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(ref t) = self.token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = req
            .body(body)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(format!("{status} {method} {url}: {body_text}"));
        }
        resp.json::<T>().await.map_err(|e| e.to_string())
    }
}

fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
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
        self.fetch(&format!("/api/tickets/{id}?workspace={}", enc(workspace))).await
    }

    async fn get_ticket_description(
        &self,
        workspace: &str,
        id: &str,
    ) -> Result<TicketDescriptionResponse, String> {
        self.fetch(&format!("/api/tickets/{id}/description?workspace={}", enc(workspace))).await
    }

    async fn get_subgraph(
        &self,
        workspace: &str,
        root: &str,
        depth: u32,
    ) -> Result<GraphSubgraphResponse, String> {
        self.fetch(&format!(
            "/api/graph/subgraph?workspace={}&root={}&depth={}",
            enc(workspace), enc(root), depth,
        )).await
    }

    async fn patch_ticket(
        &self,
        workspace: &str,
        id: &str,
        patch: &TicketPatch,
    ) -> Result<TicketDetailResponse, String> {
        let url = format!("/api/tickets/{id}?workspace={}", enc(workspace));
        let body = serde_json::to_string(patch).map_err(|e| e.to_string())?;
        self.send_json("PATCH", &url, &body).await
    }

    async fn list_schemas(&self, workspace: &str) -> Result<SchemaListResponse, String> {
        self.fetch(&format!("/api/schema?workspace={}", enc(workspace))).await
    }

    async fn create_edge(&self, workspace: &str, body: &EdgeMutationBody) -> Result<(), String> {
        let url = format!("/api/edges?workspace={}", enc(workspace));
        let json_body = serde_json::to_string(body).map_err(|e| e.to_string())?;
        let mut req = gloo_net::http::Request::post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(ref t) = self.token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = req
            .body(json_body)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status() == 422 {
            return Err("cycle_detected: Adding this edge would create a dependency cycle".to_string());
        }
        if !resp.ok() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(format!("{status} POST {url}: {body_text}"));
        }
        Ok(())
    }

    async fn delete_edge(&self, workspace: &str, body: &EdgeMutationBody) -> Result<(), String> {
        let url = format!("/api/edges?workspace={}", enc(workspace));
        let json_body = serde_json::to_string(body).map_err(|e| e.to_string())?;
        let mut req = gloo_net::http::Request::delete(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(ref t) = self.token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = req
            .body(json_body)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(format!("{status} DELETE {url}: {body_text}"));
        }
        Ok(())
    }

    async fn create_ticket(
        &self,
        workspace: &str,
        body: &CreateTicketRequest,
    ) -> Result<CreateTicketResponse, String> {
        let url = format!("/api/tickets?workspace={}", enc(workspace));
        let json_body = serde_json::to_string(body).map_err(|e| e.to_string())?;
        self.send_json("POST", &url, &json_body).await
    }

    async fn get_schema_by_type(
        &self,
        workspace: &str,
        type_id: &str,
    ) -> Result<SchemaDetailResponse, String> {
        self.fetch(&format!("/api/schema/{}?workspace={}", enc(type_id), enc(workspace))).await
    }

    async fn get_ticket_history(
        &self,
        workspace: &str,
        id: &str,
    ) -> Result<TicketHistoryResponse, String> {
        self.fetch(&format!("/api/tickets/{id}/history?workspace={}", enc(workspace))).await
    }

    async fn undo_ticket(
        &self,
        workspace: &str,
        id: &str,
    ) -> Result<TicketDetailResponse, String> {
        let url = format!("/api/tickets/{id}/undo?workspace={}", enc(workspace));
        let mut req = gloo_net::http::Request::post(&url)
            .header("Accept", "application/json");
        if let Some(ref t) = self.token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("{status} POST {url}: {body}"));
        }
        resp.json::<TicketDetailResponse>().await.map_err(|e| e.to_string())
    }
}
