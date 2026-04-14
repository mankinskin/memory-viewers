//! Integration tests verifying that ticket-viewer resolves workspaces
//! identically to the CLI — via `ticket_api::workspace` — and that the
//! HTTP API reflects the resolved store contents.

use std::{collections::BTreeMap, sync::Arc};

use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // for `oneshot`

use ticket_api::{
    model::filesystem::ScanRoot,
    storage::store::TicketStore,
    workspace::{find_local_workspace_file_from, LOCAL_WORKSPACE_FILE},
};
use ticket_http::serve::{AppState, StreamBroker, WorkspaceRegistry};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn open_store(dir: &std::path::Path) -> Arc<TicketStore> {
    let store = Arc::new(TicketStore::open(dir).expect("open store"));
    store
        .add_scan_root(ScanRoot {
            path: dir.join("tickets"),
            label: "default".into(),
        })
        .expect("add scan root");
    store
}

fn build_app(store: Arc<TicketStore>) -> axum::Router {
    let state = AppState::new(
        Arc::new(WorkspaceRegistry::single_opened(store)),
        Arc::new(StreamBroker::new()),
    );
    let _ = state.ensure_workspace_runtime("default");
    ticket_http::build_router(state)
}

async fn get_ticket_count(app: axum::Router) -> (StatusCode, usize) {
    let req = Request::builder()
        .uri("/api/tickets?workspace=default")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.expect("request should succeed");
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024)
        .await
        .expect("read body");
    let payload: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json");
    let count = payload["items"].as_array().map(|a| a.len()).unwrap_or(0);
    (status, count)
}

// ── Workspace resolution tests ───────────────────────────────────────────────

#[test]
fn local_workspace_file_is_found_walking_up() {
    let root = tempfile::tempdir().expect("tempdir");
    let nested = root.path().join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).expect("mkdir");

    // Place .ticket-workspace at root
    std::fs::write(root.path().join(LOCAL_WORKSPACE_FILE), ".ticket\n").expect("write");

    // Walking up from nested dir should find the file at root
    let found = find_local_workspace_file_from(&nested);
    assert!(found.is_some(), "should find .ticket-workspace walking up");
    assert_eq!(
        found.unwrap(),
        root.path().join(LOCAL_WORKSPACE_FILE),
    );
}

#[test]
fn local_workspace_file_not_found_in_empty_tree() {
    let root = tempfile::tempdir().expect("tempdir");
    // No .ticket-workspace anywhere
    let found = find_local_workspace_file_from(root.path());
    // May or may not find one depending on the user's home directory —
    // the important property is that it does NOT find one inside root.
    if let Some(path) = found {
        assert!(
            !path.starts_with(root.path()),
            "should not find .ticket-workspace inside the temp dir",
        );
    }
}

#[test]
fn workspace_file_relative_path_resolves_from_file_parent() {
    let root = tempfile::tempdir().expect("tempdir");
    let ticket_dir = root.path().join(".ticket");
    std::fs::create_dir_all(&ticket_dir).expect("mkdir .ticket");

    // Write a relative path in .ticket-workspace
    std::fs::write(root.path().join(LOCAL_WORKSPACE_FILE), ".ticket").expect("write");

    // Simulate what resolve_workspace does for a local file:
    let local_file = find_local_workspace_file_from(root.path()).expect("should find file");
    let content = std::fs::read_to_string(&local_file).expect("read");
    let value = content.trim();
    let resolved = local_file.parent().unwrap().join(value);

    assert_eq!(resolved, ticket_dir);
}

// ── HTTP API integration tests ───────────────────────────────────────────────

#[tokio::test]
async fn empty_store_returns_zero_tickets() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open_store(dir.path());
    let app = build_app(store);

    let (status, count) = get_ticket_count(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(count, 0, "empty store should return 0 tickets");
}

#[tokio::test]
async fn store_with_tickets_returns_correct_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open_store(dir.path());

    // Create 3 tickets
    for i in 0..3 {
        store
            .create(
                None,
                "tracker-improvement",
                Some(&format!("test ticket {i}")),
                None,
                BTreeMap::new(),
                None,
                None,
            )
            .expect("create ticket");
    }

    let app = build_app(store);
    let (status, count) = get_ticket_count(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(count, 3, "store with 3 tickets should return 3");
}

#[tokio::test]
async fn resolved_workspace_serves_correct_store() {
    // Simulate the exact flow from main.rs:
    // 1. Create a temp dir with .ticket-workspace pointing to .ticket/
    // 2. Open the store at that resolved path
    // 3. Create tickets
    // 4. Verify the API serves them

    let root = tempfile::tempdir().expect("tempdir");
    let ticket_dir = root.path().join(".ticket");
    std::fs::create_dir_all(&ticket_dir).expect("mkdir");

    // Write workspace file (relative path, same as context-engine)
    std::fs::write(root.path().join(LOCAL_WORKSPACE_FILE), ".ticket").expect("write");

    // Resolve the workspace path the same way main.rs does
    let local_file = find_local_workspace_file_from(root.path()).expect("find workspace file");
    let content = std::fs::read_to_string(&local_file).expect("read");
    let value = content.trim();
    let index_root = local_file.parent().unwrap().join(value);

    // Open store at the resolved path and create tickets
    let store = Arc::new(TicketStore::open(&index_root).expect("open store"));
    store
        .add_scan_root(ScanRoot {
            path: index_root.join("tickets"),
            label: "default".into(),
        })
        .expect("add scan root");

    store
        .create(
            None,
            "tracker-improvement",
            Some("resolved workspace ticket"),
            None,
            BTreeMap::new(),
            None,
            None,
        )
        .expect("create ticket");

    // Build the app and query
    let app = build_app(store);
    let (status, count) = get_ticket_count(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(count, 1, "resolved workspace should serve its tickets");
}

#[tokio::test]
async fn separate_workspaces_are_isolated() {
    // Two independent temp dirs, each with their own store.
    // Tickets in one should not appear in the other.

    let dir_a = tempfile::tempdir().expect("tempdir a");
    let dir_b = tempfile::tempdir().expect("tempdir b");

    let store_a = open_store(dir_a.path());
    let store_b = open_store(dir_b.path());

    // Create 2 tickets in store A, 0 in store B
    for i in 0..2 {
        store_a
            .create(
                None,
                "tracker-improvement",
                Some(&format!("store_a ticket {i}")),
                None,
                BTreeMap::new(),
                None,
                None,
            )
            .expect("create in store_a");
    }

    let app_a = build_app(store_a);
    let app_b = build_app(store_b);

    let (_, count_a) = get_ticket_count(app_a).await;
    let (_, count_b) = get_ticket_count(app_b).await;

    assert_eq!(count_a, 2, "store_a should have 2 tickets");
    assert_eq!(count_b, 0, "store_b should have 0 tickets");
}
