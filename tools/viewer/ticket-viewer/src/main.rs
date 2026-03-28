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
//! - `STATIC_DIR` — Path to pre-built SPA static files (default: <manifest>/static)

use std::{env, path::PathBuf, sync::Arc};
use tracing::info;
use viewer_api::{display_host, init_tracing, with_static_files};

use ticket_api::workspace::WorkspaceConfig;
use ticket_api::storage::store::TicketStore;
use ticket_http::serve::{WorkspaceRegistry, StreamBroker, AppState};

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
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"));

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

    let index_root = options.index_root.unwrap_or_else(|| {
        let (path, _source) = ticket_api::workspace::resolve_workspace();
        path
    });

    // Build the workspace registry.
    //
    // Only open a TicketStore directly when we need a `single_opened` registry
    // (explicit --workspace flag, or no workspaces configured at all). In the
    // multi-workspace case, the registry opens each workspace lazily so we
    // never strand an unused TicketStore that would hold a TantivySearchIndex
    // and a RedbIndexStore alive for the server's entire lifetime.
    let registry = if options.workspace.is_some() {
        let store = TicketStore::open(&index_root).expect("failed to open ticket store");
        WorkspaceRegistry::single_opened(Arc::new(store))
    } else {
        let config = WorkspaceConfig::load();
        if config.workspaces.is_empty() {
            let store = TicketStore::open(&index_root).expect("failed to open ticket store");
            WorkspaceRegistry::single_opened(Arc::new(store))
        } else {
            WorkspaceRegistry::from_config(&config)
        }
    };

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
