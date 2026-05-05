//! Ticket dependency graph — thin wrapper around the shared
//! [`viewer_api_dioxus::Graph3D`] component.
//!
//! Fetches the ticket subgraph from the backend, runs the 2-D force-directed
//! layout, lifts the result to a shared [`Layout3D`] (depth → Z), and renders
//! ticket cards as children of `<Graph3D>` so the shared renderer can position
//! them every frame via `data-node-idx` lookups.
//!
//! # Debug logging
//!
//! Every lifecycle event (mount, unmount, cache hit, debounce, fetch) is
//! emitted as a `tracing::debug!` record with `target = "ticket_viewer::graph3d"`.
//! Open the viewer with `?log=debug` to route them to the browser console:
//!
//! ```text
//! http://localhost:3002/workspace/default?log=ticket_viewer=debug
//! ```

use std::collections::HashMap;

use dioxus::prelude::*;

use viewer_api_dioxus::{can_use_webgpu_graph3d, CameraCommand, EdgeRef3D, Layout3D, Node3D};

use crate::components::ticket_card;
use crate::layout::GraphLayout;

/// Tracing target used by all events in this module.
const T: &str = "ticket_viewer::graph3d";


/// Re-export so existing call sites (`crate::graph3d::can_use_webgpu`) keep
/// working unchanged.
pub fn can_use_webgpu() -> bool { can_use_webgpu_graph3d() }

/// Build a shared [`Layout3D`] from the 2-D force-directed layout.
pub fn lift_2d(gl: GraphLayout) -> Layout3D {
    let scale = 1.0 / 100.0_f32;

    let nodes: Vec<Node3D> = gl
        .nodes
        .iter()
        .map(|gn| Node3D {
            id:    gn.id.clone(),
            label: gn.title.clone(),
            state: gn.state.clone(),
            x:     gn.x as f32 * scale,
            y:    -(gn.y as f32 * scale),
            z:     0.0,
        })
        .collect();

    let idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let edges: Vec<EdgeRef3D> = gl
        .edges
        .iter()
        .filter_map(|e| {
            let &from_idx = idx.get(e.from.as_str())?;
            let &to_idx   = idx.get(e.to.as_str())?;
            Some(EdgeRef3D { from_idx, to_idx, kind: e.kind.clone() })
        })
        .collect();

    Layout3D::new(nodes, edges)
}

#[derive(Props, Clone, PartialEq)]
pub struct Graph3DProps {
    pub workspace: String,
    pub root_id:   String,
    pub on_select: EventHandler<String>,
    /// Optional graph-preview selection — highlights this node with an ember
    /// border effect without changing the primary ticket-list selection.
    #[props(optional)]
    pub selected_node_id: Option<String>,
}

#[component]
pub fn Graph3D(props: Graph3DProps) -> Element {
    let workspace = props.workspace.clone();
    let root_id   = props.root_id.clone();
    let on_select = props.on_select;
    let selected_node_id = props.selected_node_id.clone();

    // ── Fetch service + cache ─────────────────────────────────────────────
    // Graph3D never issues its own HTTP requests.  It reads the shared cache
    // (populated by GraphFetchService) and re-renders whenever the service
    // bumps its version counter after a fetch completes.
    let svc: crate::graph_fetch::GraphFetchService =
        use_context::<crate::graph_fetch::GraphFetchService>();
    let cache: crate::GraphCache = use_context::<crate::GraphCache>();
    let cache_key = format!("{workspace}:{root_id}");

    // ── Camera command channel ────────────────────────────────────────────
    // Fire a one-time "look straight at the XY plane" reset the first time a
    // layout loads for each root_id, so the 2-D force-directed view starts
    // orthogonal to the graph plane (z = 0 for all nodes).
    let mut cam_cmd:       Signal<Option<CameraCommand>> = use_hook(|| Signal::new(None));
    let mut cam_seq:       Signal<u64>                   = use_hook(|| Signal::new(0_u64));
    let mut last_cam_root: Signal<Option<String>>        = use_hook(|| Signal::new(None));

    // ── Lifecycle logging ─────────────────────────────────────────────────
    tracing::debug!(target: T, rid = %root_id, "mount");
    {
        let rid_drop = root_id.clone();
        use_drop(move || {
            tracing::debug!(target: T, rid = %rid_drop, "unmount");
        });
    }

    // Subscribe to the version signal so Dioxus re-renders this component
    // whenever a background fetch completes and writes a new layout to the
    // cache.  The actual value is not used — the read is purely for reactivity.
    let _ver = svc.version();
    let fetch_state = svc.state_for(&workspace, &root_id);
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&format!("[graph3d] render rid={root_id} ver={_ver}").into());
    tracing::debug!(target: T, rid = %root_id, ver = _ver, state = ?fetch_state, "render");

    // Try to get the layout from the cache.  If absent, show the spinner.
    // GraphFetchService.ensure_fetched() was already called by TicketListPage
    // when selected_id changed; we do NOT start a second fetch here.
    let layout = match cache.get(&cache_key) {
        Some(layout) => {
            tracing::debug!(target: T, rid = %root_id, nodes = layout.nodes.len(), "cache_hit");
            layout
        }
        None => {
            match fetch_state {
                crate::graph_fetch::GraphFetchState::Error { message, attempts } => {
                    tracing::warn!(target: T, rid = %root_id, attempts, error = %message, "graph_fetch_failed");
                    let svc_retry = svc.clone();
                    let ws_retry = workspace.clone();
                    let rid_retry = root_id.clone();
                    return rsx! {
                        div {
                            style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); max-width: 560px; color: #ddd; font-size: 13px; font-family: sans-serif; background: rgba(25, 28, 34, 0.92); border: 1px solid rgba(255, 120, 120, 0.35); border-radius: 8px; padding: 12px 14px; box-shadow: 0 8px 24px rgba(0,0,0,0.4);",
                            div { style: "font-weight: 600; color: #ff9d9d; margin-bottom: 6px;", "Failed to load graph" }
                            div { style: "color: #f0c7c7; margin-bottom: 4px;", "{message}" }
                            div { style: "color: #a8a8a8; font-size: 12px; margin-bottom: 10px;", "Attempts: {attempts}" }
                            button {
                                style: "padding: 6px 10px; border-radius: 6px; border: 1px solid rgba(255,255,255,0.2); background: rgba(255,255,255,0.08); color: #efefef; cursor: pointer;",
                                onclick: move |_| {
                                    svc_retry.retry(&ws_retry, &rid_retry);
                                },
                                "Retry"
                            }
                        }
                    };
                }
                _ => {
                    tracing::debug!(target: T, rid = %root_id, state = ?fetch_state, "waiting_for_cache");
                    return rsx! {
                        div {
                            style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: #aaa; font-size: 14px; font-family: sans-serif;",
                            "Loading graph\u{2026}"
                        }
                    };
                }
            }
        }
    };

    let nodes = layout.nodes.clone();
    let node_count = nodes.len();

    // Issue a one-time camera reset the first time a layout for this root_id
    // becomes available, so the view starts orthogonal to the flat z=0 plane.
    if last_cam_root.read().as_deref() != Some(root_id.as_str()) {
        last_cam_root.set(Some(root_id.clone()));
        cam_cmd.set(Some(CameraCommand::ResetTo { yaw: 0.0, pitch: 0.0 }));
        let next_seq = *cam_seq.peek() + 1;
        cam_seq.set(next_seq);
    }

    rsx! {
        viewer_api_dioxus::Graph3D {
            layout: layout,
            camera_command: *cam_cmd.read(),
            camera_command_seq: *cam_seq.read(),
            div {
                id: "graph3d-nodes",
                style: "position: absolute; inset: 0; pointer-events: none;",
                for (idx, node) in nodes.iter().enumerate() {
                    {
                        let node_id    = node.id.clone();
                        let title      = node.label.clone().unwrap_or_else(|| "Untitled".into());
                        let state_str  = node.state.clone().unwrap_or_else(|| "new".into());
                        let color      = ticket_card::state_color(Some(state_str.as_str()));
                        let short_id   = if node_id.len() > 8 { node_id[..8].to_string() } else { node_id.clone() };
                        let node_id_click = node_id.clone();
                        let is_selected = selected_node_id.as_deref() == Some(node_id.as_str());
                        let card_class = if is_selected {
                            "content node-card-selected"
                        } else {
                            "content"
                        };
                        rsx! {
                            div {
                                key: "{node_id}",
                                class: "{card_class}",
                                "data-node-idx": "{idx}",
                                style: "position: absolute; top: 0; left: 0; pointer-events: auto; transform-origin: center center; display: none; width: 220px; box-sizing: border-box; border: 1px solid rgba(200,200,200,0.35); border-left: 3px solid {color}; border-radius: 6px; background: rgba(30,30,40,0.92); backdrop-filter: blur(2px); padding: 6px 8px; cursor: pointer; overflow: hidden; font-family: sans-serif; box-shadow: 0 2px 8px rgba(0,0,0,0.5);",
                                onclick: move |evt: Event<MouseData>| {
                                    evt.stop_propagation();
                                    on_select.call(node_id_click.clone());
                                },
                                div {
                                    style: "font-size: 12px; font-weight: 600; color: #e8e8f0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                                    "{title}"
                                }
                                div {
                                    style: "display: flex; align-items: center; gap: 6px; margin-top: 3px;",
                                    span {
                                        style: "font-size: 10px; color: {color}; font-weight: 500;",
                                        "{state_str}"
                                    }
                                    span {
                                        style: "font-size: 9px; color: #888;",
                                        "{short_id}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div {
                style: "position: absolute; bottom: 12px; left: 50%; transform: translateX(-50%); font-size: 11px; color: rgba(255,255,255,0.3); font-family: sans-serif; pointer-events: none; white-space: nowrap;",
                "Left-drag: orbit \u{00b7} Right-drag: pan \u{00b7} Scroll: zoom \u{00b7} Click card: open"
            }
            if node_count > 0 {
                div {
                    style: "position: absolute; top: 12px; right: 12px; font-size: 11px; color: rgba(255,255,255,0.35); font-family: sans-serif; pointer-events: none;",
                    "{node_count} nodes"
                }
            }
        }
    }
}
