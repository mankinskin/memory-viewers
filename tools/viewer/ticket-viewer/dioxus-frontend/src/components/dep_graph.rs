//! DepGraph — ticket dependency graph view.
//!
//! Renders the ticket dependency graph for a root ticket:
//!
//! * Nodes are real DOM `TicketCard` elements positioned by a force-directed
//!   layout algorithm (ported from log-viewer HypergraphView layout.ts).
//! * Edges are drawn as canvas lines on the `#webgpu-canvas` element that
//!   sits behind the UI layer in `ViewerShell`.
//! * Pan/zoom via mouse-wheel + background-drag.
//! * Drag individual nodes to reposition.
//! * Click a node to open that ticket.
//! * SSE subscription via `EventSource` — refetches the subgraph on
//!   `edge.upsert` / `edge.delete` events so the layout stays current.
//!   The `gloo_events::EventListener` wrapper removes the listener on drop
//!   (no `Closure::forget()` calls anywhere in this module).

use dioxus::prelude::*;
use wasm_bindgen::JsCast;

use crate::backend::{HttpTicketBackend, TicketBackend};
use crate::graph::{draw_edges, GraphLayout};
use crate::routes::Route;

use super::ticket_card::TicketCard;

// ── Interaction state ──────────────────────────────────────────────────────

/// What is the mouse currently doing in the dep-graph view?
#[derive(Debug, Clone, PartialEq)]
enum DragKind {
    /// Background pan (mouse-down on empty space).
    Pan,
    /// Node drag (mouse-down on a specific TicketCard).
    Node(String),
}

#[derive(Debug, Clone)]
struct DragState {
    kind: DragKind,
    /// Client coordinates where the drag started.
    start_client_x: f64,
    start_client_y: f64,
    /// Pan offset at the moment the drag started.
    start_pan_x: f64,
    start_pan_y: f64,
    /// Node position at drag start (only relevant for DragKind::Node).
    start_node_x: f64,
    start_node_y: f64,
}

// ── Canvas size helper ─────────────────────────────────────────────────────

/// Return the current pixel dimensions of `#webgpu-canvas`, defaulting to
/// `(800, 600)` before the canvas has been laid out.
fn canvas_size() -> (f64, f64) {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("webgpu-canvas"))
        .and_then(|el| el.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .map(|c| (c.client_width() as f64, c.client_height() as f64))
        .unwrap_or((800.0, 600.0))
}

// ── SSE subscription ───────────────────────────────────────────────────────

/// Attach an `EventSource` to `/api/stream?workspace=<ws>` and return the
/// source + listeners.  Dropping them closes the connection and removes the
/// handlers automatically (no `Closure::forget()` calls).
///
/// `fetch_trigger` is a `Signal<u32>` (Copy).  Interior mutation via
/// `Signal::with_mut` takes `&self`, so the closures satisfy `Fn`.
fn subscribe_sse(
    workspace: &str,
    fetch_trigger: Signal<u32>,
) -> Option<(web_sys::EventSource, [gloo_events::EventListener; 2])> {
    let url = format!("/api/stream?workspace={workspace}");
    let es = web_sys::EventSource::new(&url).ok()?;
    let mut ft1 = fetch_trigger;
    let mut ft2 = fetch_trigger;
    let l1 = gloo_events::EventListener::new(&es, "edge.upsert", move |_| {
        ft1.with_mut(|v| *v += 1);
    });
    let l2 = gloo_events::EventListener::new(&es, "edge.delete", move |_| {
        ft2.with_mut(|v| *v += 1);
    });
    Some((es, [l1, l2]))
}

// ── Props ──────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct DepGraphProps {
    pub workspace: String,
    pub root_id: String,
}

// ── Component ──────────────────────────────────────────────────────────────

/// Ticket dependency graph — DOM nodes + canvas edges.
#[component]
pub fn DepGraph(props: DepGraphProps) -> Element {
    let workspace = props.workspace.clone();
    let root_id = props.root_id.clone();

    // ── Fetch / layout state ───────────────────────────────────────────
    let mut layout: Signal<Option<GraphLayout>> = use_signal(|| None);
    let mut load_error: Signal<Option<String>> = use_signal(|| None);

    // Counter incremented by SSE edge events and "retry" button to
    // trigger a re-fetch without changing the component key.
    let mut fetch_trigger: Signal<u32> = use_signal(|| 0_u32);

    // ── Camera state ──────────────────────────────────────────────────
    let mut pan_x: Signal<f64> = use_signal(|| 0.0_f64);
    let mut pan_y: Signal<f64> = use_signal(|| 0.0_f64);
    let mut zoom: Signal<f64> = use_signal(|| 1.0_f64);

    // ── Drag state ────────────────────────────────────────────────────
    let mut drag: Signal<Option<DragState>> = use_signal(|| None);

    // SSE resources kept alive while this component lives.
    let mut _sse_handle: Signal<Option<(web_sys::EventSource, [gloo_events::EventListener; 2])>> =
        use_signal(|| None);

    // ── Async data fetch triggered by fetch_trigger ────────────────────
    {
        let workspace_fetch = workspace.clone();
        let root_fetch = root_id.clone();
        use_effect(move || {
            // Reading fetch_trigger subscribes this effect to its changes.
            let _trigger = fetch_trigger();
            let ws = workspace_fetch.clone();
            let rid = root_fetch.clone();
            let mut layout_w = layout;
            let mut err_w = load_error;
            spawn(async move {
                let backend = HttpTicketBackend::new(None);
                match backend.get_subgraph(&ws, &rid, 4).await {
                    Ok(resp) => {
                        let new_layout = GraphLayout::build(resp.nodes, resp.edges);
                        layout_w.set(Some(new_layout));
                        err_w.set(None);
                    }
                    Err(e) => {
                        err_w.set(Some(e));
                    }
                }
            });
        });
    }

    // ── SSE subscription — set up once per workspace ──────────────────
    {
        let ws_sse = workspace.clone();
        use_effect(move || {
            let handle = subscribe_sse(&ws_sse, fetch_trigger);
            _sse_handle.with_mut(|h| *h = handle);
        });
    }

    // ── Canvas edge redraw whenever layout / camera changes ────────────
    use_effect(move || {
        let pan = (*pan_x.read(), *pan_y.read());
        let z = *zoom.read();
        if let Some(l) = layout.read().as_ref() {
            draw_edges(l, pan.0, pan.1, z);
        } else {
            // Clear the canvas when there is no layout.
            crate::graph::draw_edges(&GraphLayout::default(), 0.0, 0.0, 1.0);
        }
    });

    // ── Interaction handlers ──────────────────────────────────────────

    let mut on_wheel = move |evt: Event<WheelData>| {
        evt.prevent_default();
        let delta = evt.delta().strip_units().y;
        let factor = if delta < 0.0 { 1.12 } else { 0.89 };
        zoom.with_mut(|z| {
            *z = (*z * factor).clamp(0.1, 8.0);
        });
    };

    // Background mouse-down → start pan.
    let mut on_container_mousedown = move |evt: Event<MouseData>| {
        let client = evt.client_coordinates();
        drag.with_mut(|d| {
            *d = Some(DragState {
                kind: DragKind::Pan,
                start_client_x: client.x,
                start_client_y: client.y,
                start_pan_x: *pan_x.read(),
                start_pan_y: *pan_y.read(),
                start_node_x: 0.0,
                start_node_y: 0.0,
            });
        });
    };

    let mut on_mousemove = move |evt: Event<MouseData>| {
        let Some(state) = drag.read().clone() else {
            return;
        };
        let client = evt.client_coordinates();
        let z = *zoom.read();
        let dx = (client.x - state.start_client_x) / z;
        let dy = (client.y - state.start_client_y) / z;
        match &state.kind {
            DragKind::Pan => {
                pan_x.with_mut(|v| *v = state.start_pan_x + dx);
                pan_y.with_mut(|v| *v = state.start_pan_y + dy);
            }
            DragKind::Node(node_id) => {
                let nid = node_id.clone();
                layout.with_mut(|opt| {
                    if let Some(l) = opt {
                        if let Some(node) = l.nodes.iter_mut().find(|n| n.id == nid) {
                            node.x = state.start_node_x + dx;
                            node.y = state.start_node_y + dy;
                        }
                    }
                });
            }
        }
    };

    let mut on_mouseup = move |_: Event<MouseData>| {
        drag.with_mut(|d| *d = None);
    };

    // ── Render ────────────────────────────────────────────────────────

    let (cw, ch) = canvas_size();
    let z = *zoom.read();
    let px = *pan_x.read();
    let py = *pan_y.read();
    let cursor = if drag.read().is_some() {
        "cursor: grabbing;"
    } else {
        "cursor: grab;"
    };

    rsx! {
        div {
            // Full-screen absolute container on top of the canvas.
            style: "
                position: absolute;
                top: 0; left: 0;
                width: 100%; height: 100%;
                overflow: hidden;
                {cursor};
                user-select: none;
            ",
            onwheel: on_wheel,
            onmousedown: on_container_mousedown,
            onmousemove: on_mousemove,
            onmouseup: on_mouseup,
            onmouseleave: on_mouseup,

            // ── Loading / error states ─────────────────────────────────
            if load_error.read().is_some() {
                div {
                    style: "
                        position: absolute; top: 50%; left: 50%;
                        transform: translate(-50%, -50%);
                        color: #ef4444; font-family: sans-serif; font-size: 14px;
                        text-align: center; pointer-events: auto;
                    ",
                    p { "Failed to load dependency graph:" }
                    p { "{load_error.read().as_deref().unwrap_or(\"\")}" }
                    button {
                        style: "margin-top:8px; padding:6px 14px; cursor:pointer; border-radius:4px;",
                        onclick: move |_| { fetch_trigger += 1; },
                        "Retry"
                    }
                }
            }

            if layout.read().is_none() && load_error.read().is_none() {
                div {
                    style: "
                        position: absolute; top: 50%; left: 50%;
                        transform: translate(-50%, -50%);
                        color: #9ca3af; font-family: sans-serif; font-size: 14px;
                    ",
                    "Loading dependency graph…"
                }
            }

            // ── Ticket card DOM nodes ──────────────────────────────────
            // Each card is a full DOM element positioned in layout space.
            // Closures are created per-node so no ownership issues arise.
            {
                layout.read().as_ref().map(|l| {
                    rsx! {
                        for node in l.nodes.iter() {
                            {
                                let node_id = node.id.clone();
                                let node_id_drag = node.id.clone();
                                let ws = workspace.clone();
                                let nav = use_navigator();
                                let nx = node.x;
                                let ny = node.y;
                                let node_title = node.title.clone();
                                let node_state = node.state.clone();
                                let node_depth = node.depth;
                                rsx! {
                                    TicketCard {
                                        key: "{node.id}",
                                        id: node_id.clone(),
                                        title: node_title,
                                        state: node_state,
                                        depth: node_depth,
                                        layout_x: nx,
                                        layout_y: ny,
                                        pan_x: px,
                                        pan_y: py,
                                        zoom: z,
                                        canvas_w: cw,
                                        canvas_h: ch,
                                        on_click: move |_| {
                                            nav.push(Route::TicketDetailPage {
                                                workspace: ws.clone(),
                                                id: node_id.clone(),
                                            });
                                        },
                                        on_drag_start: move |(_, cx, cy): (String, f64, f64)| {
                                            drag.with_mut(|d| {
                                                *d = Some(DragState {
                                                    kind: DragKind::Node(node_id_drag.clone()),
                                                    start_client_x: cx,
                                                    start_client_y: cy,
                                                    start_pan_x: *pan_x.read(),
                                                    start_pan_y: *pan_y.read(),
                                                    start_node_x: nx,
                                                    start_node_y: ny,
                                                });
                                            });
                                        },
                                    }
                                }
                            }
                        }
                    }
                })
            }

            // ── HUD controls ───────────────────────────────────────────
            div {
                style: "
                    position: absolute; bottom: 16px; right: 16px;
                    display: flex; gap: 6px;
                    pointer-events: auto;
                ",
                button {
                    style: graph_btn_style(),
                    title: "Zoom in",
                    onclick: move |_| zoom.with_mut(|z| *z = (*z * 1.2).min(8.0)),
                    "+"
                }
                button {
                    style: graph_btn_style(),
                    title: "Zoom out",
                    onclick: move |_| zoom.with_mut(|z| *z = (*z / 1.2).max(0.1)),
                    "−"
                }
                button {
                    style: graph_btn_style(),
                    title: "Reset view",
                    onclick: move |_| {
                        pan_x.set(0.0);
                        pan_y.set(0.0);
                        zoom.set(1.0);
                    },
                    "⌂"
                }
            }

            // ── Node count badge ───────────────────────────────────────
            if let Some(l) = layout.read().as_ref() {
                div {
                    style: "
                        position: absolute; top: 12px; left: 12px;
                        color: rgba(200,200,210,0.7);
                        font-family: monospace; font-size: 11px;
                        pointer-events: none;
                    ",
                    "{l.nodes.len()} nodes · {l.edges.len()} edges"
                }
            }
        }
    }
}

fn graph_btn_style() -> &'static str {
    "
        background: rgba(40,40,55,0.85);
        color: #e0e0e8;
        border: 1px solid rgba(200,200,220,0.25);
        border-radius: 4px;
        padding: 4px 10px;
        font-size: 16px;
        cursor: pointer;
        line-height: 1;
    "
}
