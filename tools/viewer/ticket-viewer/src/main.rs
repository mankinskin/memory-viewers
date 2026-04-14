//! Ticket Viewer — single-process HTTP server that serves the SPA frontend
//! and the ticket REST/SSE API together.
//!
//! The viewer imports `ticket-http` as a library and mounts its router
//! directly, eliminating the need for a separate backend process or proxy.
//!
//! # Usage
//!
//! ```bash
//! # Serve the SPA + API on a single port (default 3002)
//! ticket-viewer
//!
//! # Custom port and workspace
//! ticket-viewer --port 3002 --workspace my-project
//! ```
//!
//! # Environment variables
//! - `PORT`       — HTTP listen port (default: 3002)
/// - `STATIC_DIR` — Path to pre-built SPA static files (default: Dioxus build output, then <manifest>/static)

use std::{env, path::PathBuf, sync::Arc};
use tracing::info;
use viewer_api::{display_host, init_tracing, with_static_files};

use ticket_api::storage::store::TicketStore;
use ticket_http::serve::{WorkspaceRegistry, StreamBroker, AppState};

/// Resolve an index root purely from the current working directory.
///
/// The global `~/.ticket-workspaces.toml` is **not** consulted so the server
/// always serves the workspace that is local to where it was launched, not
/// whatever workspace a separate CLI session has set as active.
///
/// Resolution order:
/// 1. `.ticket-workspace` file found walking up from cwd — same as the CLI
///    (absolute path used directly; relative path resolved from the file's
///    directory; workspace name is **not** looked up in the global registry).
/// 2. `.ticket/` directory found walking up from cwd.
/// 3. `<cwd>/.ticket` as the local default (created on first use by the store).
fn resolve_index_root_from_cwd() -> PathBuf {
    // Layer 1: project-local .ticket-workspace file (absolute path only —
    // we do not look up names in the global registry to stay portable).
    if let Some(local_file) = ticket_api::workspace::find_local_workspace_file() {
        if let Ok(content) = std::fs::read_to_string(&local_file) {
            let value = content.trim();
            if !value.is_empty() {
                let p = PathBuf::from(value);
                if p.is_absolute() {
                    return p;
                }
                // Relative path resolved from the file's own directory.
                if let Some(parent) = local_file.parent() {
                    return parent.join(&p);
                }
            }
        }
    }

    // Layer 2: .ticket/ directory found walking up from cwd.
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir: &std::path::Path = &cwd;
        loop {
            let candidate = dir.join(".ticket");
            if candidate.is_dir() {
                return candidate;
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
    }

    // Layer 3: local default — <cwd>/.ticket (no global config fallback).
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".ticket")
}

struct CliOptions {
    port: u16,
    static_dir: PathBuf,
    workspace: Option<String>,
    index_root: Option<PathBuf>,
}

fn parse_cli_options() -> CliOptions {
    let mut port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3002);

    let mut static_dir: PathBuf = env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // Prefer the Dioxus WASM frontend build output (dx build --release).
            // Fall back to the legacy Preact/Vite static/ directory.
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let dioxus_dist = manifest
                .parent()           // tools/viewer/ticket-viewer/..
                .unwrap()           // tools/viewer
                .parent().unwrap()  // tools
                .parent().unwrap()  // workspace root
                .join("target/dx/ticket-viewer-dioxus/release/web/public");
            if dioxus_dist.exists() {
                dioxus_dist
            } else {
                manifest.join("static")
            }
        });

    let mut workspace: Option<String> = None;
    let mut index_root: Option<PathBuf> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                if let Some(value) = args.next() {
                    if let Ok(parsed) = value.parse::<u16>() {
                        port = parsed;
                    }
                }
            }
            "--static-dir" => {
                if let Some(value) = args.next() {
                    static_dir = PathBuf::from(value);
                }
            }
            "--workspace" => {
                workspace = args.next();
            }
            "--index-root" => {
                index_root = args.next().map(PathBuf::from);
            }
            _ => {}
        }
    }

    CliOptions {
        port,
        static_dir,
        workspace,
        index_root,
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_cli_options();

    init_tracing("info");
    info!(
        port = options.port,
        static_dir = %options.static_dir.display(),
        workspace = ?options.workspace,
        "Ticket Viewer starting (single-process mode)"
    );

    let index_root = options.index_root.unwrap_or_else(resolve_index_root_from_cwd);

    info!(index_root = %index_root.display(), "Using ticket index root");

    // Always open a single workspace from the resolved index root.
    // We deliberately do NOT load WorkspaceRegistry::from_config() — the
    // global ~/.ticket-workspaces.toml belongs to the CLI, not to this
    // server, which must serve whatever workspace is local to its cwd.
    let store = TicketStore::open(&index_root).expect("failed to open ticket store");
    let registry = WorkspaceRegistry::single_opened(Arc::new(store));

    // Build the ticket-http AppState and wire up streaming.
    let state = AppState::new(
        Arc::new(registry),
        Arc::new(StreamBroker::new()),
    );

    // Pre-initialize all known workspaces at startup.
    let workspace_names = state.registry.workspace_names();
    for ws in &workspace_names {
        let _ = state.ensure_workspace_runtime(ws);
    }

    // Build the ticket API router from ticket-http (includes /healthz).
    let app = ticket_http::build_router(state);

    let app = with_static_files(app, Some(options.static_dir).filter(|p| p.exists()));

    let addr = format!("0.0.0.0:{}", options.port);
    info!("Listening on http://{}:{}", display_host("0.0.0.0"), options.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
