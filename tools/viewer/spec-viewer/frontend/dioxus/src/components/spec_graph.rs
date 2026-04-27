//! Spec graph page — full 3-D view of every spec with parent + code-ref edges.
//!
//! Fetches `/api/specs/graph`, lays out nodes deterministically (BFS layers
//! from roots → Z, angle around Y within each layer) and renders via the
//! shared [`viewer_api_dioxus::Graph3D`] canvas.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use dioxus::prelude::*;
use viewer_api_dioxus::{EdgeRef3D, Layout3D, Node3D};
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::types::{SpecGraphEdge, SpecGraphNode};

/// Lay nodes out as a stack of rings: BFS depth from a synthetic root → Z,
/// angle around Y within each ring.
fn layout_3d(nodes: &[SpecGraphNode], edges: &[SpecGraphEdge]) -> Layout3D {
    if nodes.is_empty() {
        return Layout3D::default();
    }

    // index lookup
    let idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // children map from `parent` edges only (tree backbone).
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut has_parent: HashSet<usize> = HashSet::new();
    for e in edges {
        if e.kind != "parent" { continue; }
        let (Some(&from), Some(&to)) = (idx.get(e.from.as_str()), idx.get(e.to.as_str()))
        else { continue };
        children.entry(from).or_default().push(to);
        has_parent.insert(to);
    }

    // BFS depth from roots (nodes with no parent edge in).
    let mut depth = vec![u32::MAX; nodes.len()];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for i in 0..nodes.len() {
        if !has_parent.contains(&i) {
            depth[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(p) = queue.pop_front() {
        let d = depth[p];
        if let Some(cs) = children.get(&p) {
            for &c in cs {
                if depth[c] == u32::MAX {
                    depth[c] = d + 1;
                    queue.push_back(c);
                }
            }
        }
    }
    // Anything still untouched (orphan via code_ref only): put on depth 0.
    for d in depth.iter_mut() {
        if *d == u32::MAX { *d = 0; }
    }

    // group by depth.
    let mut by_depth: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, &d) in depth.iter().enumerate() {
        by_depth.entry(d).or_default().push(i);
    }

    // Place each ring.
    let mut out_nodes = Vec::with_capacity(nodes.len());
    out_nodes.resize(
        nodes.len(),
        Node3D { id: String::new(), label: None, state: None, x: 0.0, y: 0.0, z: 0.0 },
    );
    for (&d, members) in &by_depth {
        let n = members.len() as f32;
        // radius grows with member count, capped.
        let radius = (n * 0.55 + 2.5).min(18.0);
        let z = -(d as f32) * 4.5;
        for (k, &i) in members.iter().enumerate() {
            let theta = if n > 0.0 {
                (k as f32) / n * std::f32::consts::TAU
            } else { 0.0 };
            let x = radius * theta.cos();
            // small y-jitter by index so depth-rings don't perfectly stack.
            let y = ((k % 3) as f32 - 1.0) * 0.8;
            let nd = &nodes[i];
            out_nodes[i] = Node3D {
                id:    nd.id.clone(),
                label: nd.title.clone(),
                state: nd.state.clone(),
                x, y, z: z + radius * theta.sin() * 0.0, // z determined by depth
            };
            // override z (the term above is intentionally zero — kept for clarity)
            out_nodes[i].z = z;
        }
    }

    let out_edges: Vec<EdgeRef3D> = edges
        .iter()
        .filter_map(|e| {
            let &from_idx = idx.get(e.from.as_str())?;
            let &to_idx   = idx.get(e.to.as_str())?;
            Some(EdgeRef3D { from_idx, to_idx, kind: e.kind.clone() })
        })
        .collect();

    Layout3D::new(out_nodes, out_edges)
}

fn state_color(state: Option<&str>) -> &'static str {
    match state.unwrap_or("draft") {
        "draft"        => "#9ca3af",
        "review"       => "#f59e0b",
        "approved"     => "#10b981",
        "implemented"  => "#3b82f6",
        "deprecated"   => "#6b7280",
        _              => "#a78bfa",
    }
}

#[component]
pub fn SpecGraphPage() -> Element {
    let mut layout: Signal<Option<Layout3D>> = use_signal(|| None);
    let mut error:  Signal<Option<String>>   = use_signal(|| None);
    let nav = use_navigator();

    use_effect(move || {
        spawn_local(async move {
            match api::get_graph().await {
                Ok(resp) => layout.set(Some(layout_3d(&resp.nodes, &resp.edges))),
                Err(e)   => error.set(Some(e)),
            }
        });
    });

    if let Some(msg) = error.read().clone() {
        return rsx! {
            div {
                style: "display: flex; align-items: center; justify-content: center; height: 100%; color: #f87171; font-size: 14px; font-family: sans-serif;",
                "Failed to load graph: {msg}"
            }
        };
    }

    let Some(l) = layout.read().clone() else {
        return rsx! {
            div {
                style: "display: flex; align-items: center; justify-content: center; height: 100%; color: #9ca3af; font-size: 14px; font-family: sans-serif;",
                "Loading graph\u{2026}"
            }
        };
    };

    let nodes = l.nodes.clone();
    let node_count = nodes.len();

    rsx! {
        div { style: "position: relative; width: 100%; height: 100%;",
            viewer_api_dioxus::Graph3D {
                layout: l,
                container_id: "spec-graph3d-container".to_string(),
                container_style: "position: absolute; inset: 0; overflow: hidden; user-select: none; cursor: grab;".to_string(),
                div {
                    id: "spec-graph3d-nodes",
                    style: "position: absolute; inset: 0; pointer-events: none;",
                    for (i, n) in nodes.iter().enumerate() {
                        {
                            let id    = n.id.clone();
                            let title = n.label.clone().unwrap_or_else(|| "Untitled".into());
                            let state = n.state.clone().unwrap_or_else(|| "draft".into());
                            let color = state_color(Some(state.as_str()));
                            let nav2  = nav.clone();
                            let id_click = id.clone();
                            rsx! {
                                div {
                                    key: "{id}",
                                    "data-node-idx": "{i}",
                                    style: "position: absolute; top: 0; left: 0; pointer-events: auto; transform-origin: center center; display: none; width: 170px; box-sizing: border-box; border: 1px solid rgba(200,200,200,0.35); border-left: 3px solid {color}; border-radius: 6px; background: rgba(18,16,14,0.65); backdrop-filter: blur(6px) saturate(140%); padding: 6px 8px; cursor: pointer; overflow: hidden; font-family: sans-serif; box-shadow: 0 2px 8px rgba(0,0,0,0.5);",
                                    onclick: move |evt: Event<MouseData>| {
                                        evt.stop_propagation();
                                        nav2.push(crate::routes::Route::SpecDetailPage { id: id_click.clone() });
                                    },
                                    div {
                                        style: "font-size: 12px; font-weight: 600; color: #e5e7eb; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                                        "{title}"
                                    }
                                    div {
                                        style: "display: flex; align-items: center; gap: 6px; margin-top: 3px;",
                                        span { style: "font-size: 10px; color: {color}; font-weight: 500;", "{state}" }
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
                        "{node_count} specs"
                    }
                }
            }
        }
    }
}
