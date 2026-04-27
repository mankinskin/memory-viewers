//! Ticket dependency graph — thin wrapper around the shared
//! [`viewer_api_dioxus::Graph3D`] component.
//!
//! Fetches the ticket subgraph from the backend, runs the 2-D force-directed
//! layout, lifts the result to a shared [`Layout3D`] (depth → Z), and renders
//! ticket cards as children of `<Graph3D>` so the shared renderer can position
//! them every frame via `data-node-idx` lookups.

use std::collections::HashMap;

use dioxus::prelude::*;

use viewer_api_dioxus::{can_use_webgpu_graph3d, EdgeRef3D, Layout3D, Node3D};

use crate::api::{HttpTicketBackend, TicketBackend};
use crate::components::ticket_card;
use crate::layout::GraphLayout;

/// Re-export so existing call sites (`crate::graph3d::can_use_webgpu`) keep
/// working unchanged.
pub fn can_use_webgpu() -> bool { can_use_webgpu_graph3d() }

/// Build a shared [`Layout3D`] from the 2-D force-directed layout.
fn lift_2d(gl: GraphLayout) -> Layout3D {
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
            z:     gn.depth as f32 * -4.0,
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
}

#[component]
pub fn Graph3D(props: Graph3DProps) -> Element {
    let workspace = props.workspace.clone();
    let root_id   = props.root_id.clone();
    let on_select = props.on_select;

    let mut layout_sig: Signal<Option<Layout3D>> = use_signal(|| None);
    let mut error_sig:  Signal<Option<String>>   = use_signal(|| None);

    use_effect(move || {
        let ws = workspace.clone();
        let rid = root_id.clone();
        let mut layout_w = layout_sig;
        let mut error_w  = error_sig;
        spawn(async move {
            let backend = HttpTicketBackend::new(None);
            match backend.get_subgraph(&ws, &rid, 4).await {
                Ok(resp) => {
                    let gl = GraphLayout::build(resp.nodes, resp.edges);
                    layout_w.set(Some(lift_2d(gl)));
                }
                Err(e) => error_w.set(Some(format!("Fetch failed: {e}"))),
            }
        });
    });

    if let Some(msg) = error_sig.read().clone() {
        return rsx! {
            div {
                style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: #f88; font-size: 14px; font-family: sans-serif;",
                "{msg}"
            }
        };
    }

    let Some(layout) = layout_sig.read().clone() else {
        return rsx! {
            div {
                style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: #aaa; font-size: 14px; font-family: sans-serif;",
                "Loading graph\u{2026}"
            }
        };
    };

    let nodes = layout.nodes.clone();
    let node_count = nodes.len();

    rsx! {
        viewer_api_dioxus::Graph3D {
            layout: layout,
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
                        rsx! {
                            div {
                                key: "{node_id}",
                                "data-node-idx": "{idx}",
                                style: "position: absolute; top: 0; left: 0; pointer-events: auto; transform-origin: center center; display: none; width: 160px; box-sizing: border-box; border: 1px solid rgba(200,200,200,0.35); border-left: 3px solid {color}; border-radius: 6px; background: rgba(30,30,40,0.92); backdrop-filter: blur(2px); padding: 6px 8px; cursor: pointer; overflow: hidden; font-family: sans-serif; box-shadow: 0 2px 8px rgba(0,0,0,0.5);",
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
