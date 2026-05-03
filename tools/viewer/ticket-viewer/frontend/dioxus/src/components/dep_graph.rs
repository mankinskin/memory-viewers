//! DepGraph — ticket dependency graph view.
//!
//! Renders the ticket dependency graph for a root ticket.
//!
//! When the browser supports WebGPU this component mounts the GPU-accelerated
//! [`crate::graph3d::Graph3D`] renderer which displays a full 3-D force-directed
//! graph on the existing `#webgpu-canvas` element.  When WebGPU is absent or
//! GPU initialisation fails the component transparently falls back to the DOM
//! SVG path described below.
//!
//! **DOM / SVG fallback path:**
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

use crate::api::{HttpTicketBackend, TicketBackend};
use crate::types::{EdgeMutationBody, TicketSummary};
use crate::layout::GraphLayout;
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

// ── Edge interaction state ─────────────────────────────────────────────────

/// A pending edge deletion awaiting user confirmation.
#[derive(Debug, Clone, PartialEq)]
struct RemoveEdge {
    from_id: String,
    to_id: String,
    kind: String,
    /// Human-readable labels shown in the confirmation dialog.
    from_title: String,
    to_title: String,
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
    /// Optional callback invoked when the user clicks a graph node.
    /// When provided the component calls this instead of navigating to
    /// `TicketDetailPage`.
    #[props(optional)]
    pub on_select: Option<EventHandler<String>>,
}

// ── Component ──────────────────────────────────────────────────────────────

/// Ticket dependency graph — GPU 3-D when WebGPU is available, DOM+canvas otherwise.
///
/// The component first checks for WebGPU support via [`crate::graph3d::can_use_webgpu`].
/// If available, it mounts [`crate::graph3d::Graph3D`] which renders on the
/// `#webgpu-canvas` element provided by `ViewerShell`.  Clicking a node in the
/// 3-D view navigates to that ticket via the Dioxus router.
///
/// When WebGPU is absent or the browser blocks GPU access the component falls
/// back to the traditional DOM force-graph + `<canvas>` edge layout.
#[component]
pub fn DepGraph(props: DepGraphProps) -> Element {
    let workspace = props.workspace.clone();
    let root_id = props.root_id.clone();
    let on_select_prop = props.on_select.clone();

    // ── WebGPU 3-D path ────────────────────────────────────────────────
    #[cfg(target_arch = "wasm32")]
    if crate::graph3d::can_use_webgpu() {
        let ws_gpu = workspace.clone();
        let rid_gpu = root_id.clone();
        let nav = use_navigator();
        let on_sel = on_select_prop.clone();
        return rsx! {
            crate::graph3d::Graph3D {
                key: "{rid_gpu}",
                workspace: ws_gpu.clone(),
                root_id: rid_gpu.clone(),
                on_select: move |id: String| {
                    if let Some(ref cb) = on_sel {
                        cb.call(id);
                    } else {
                        nav.push(crate::routes::Route::TicketDetailPage {
                            workspace: ws_gpu.clone(),
                            id,
                        });
                    }
                }
            }
        };
    }

    // ── DOM SVG fallback path ──────────────────────────────────────────

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

    // ── Add-dependency picker state ────────────────────────────────────────
    let mut picker_open: Signal<bool> = use_signal(|| false);
    let mut picker_query: Signal<String> = use_signal(String::new);
    let mut picker_results: Signal<Vec<TicketSummary>> = use_signal(Vec::new);
    let mut picker_selected_id: Signal<Option<String>> = use_signal(|| None);
    let mut picker_selected_title: Signal<Option<String>> = use_signal(|| None);
    let mut picker_kind: Signal<String> = use_signal(|| "depends_on".to_string());
    let mut picker_reason: Signal<String> = use_signal(String::new);
    let mut picker_error: Signal<Option<String>> = use_signal(|| None);
    let mut picker_searching: Signal<bool> = use_signal(|| false);
    // Monotonic generation counter — incremented on every keystroke so that
    // stale debounce tasks spawned by earlier keystrokes can self-cancel.
    let mut debounce_gen: Signal<u32> = use_signal(|| 0u32);

    // ── Remove-edge confirmation state ─────────────────────────────────────
    let mut remove_confirm: Signal<Option<RemoveEdge>> = use_signal(|| None);

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

    // ── Interaction handlers ──────────────────────────────────────────

    // ── Add-dependency handlers ───────────────────────────────────────────

    // Open the picker and reset all transient picker state.
    let mut on_open_picker = move |_: Event<MouseData>| {
        picker_query.set(String::new());
        picker_results.set(Vec::new());
        picker_selected_id.set(None);
        picker_selected_title.set(None);
        picker_kind.set("depends_on".to_string());
        picker_reason.set(String::new());
        picker_error.set(None);
        picker_searching.set(false);
        picker_open.set(true);
    };

    // Debounced input handler: increments the generation counter then spawns
    // an async task (within the Dioxus runtime context) that waits 300 ms and
    // self-cancels when a newer keystroke has already bumped the counter.
    // Using `spawn` directly avoids raw JS `Closure` callbacks which run
    // outside the Dioxus runtime and panic on signal mutation.
    let workspace_search = workspace.clone();
    let mut on_picker_input = move |evt: Event<FormData>| {
        let query = evt.value().clone();
        picker_query.set(query.clone());
        picker_selected_id.set(None);
        picker_selected_title.set(None);
        picker_error.set(None);

        // Bump the generation so any in-flight debounce task self-cancels.
        let gen = *debounce_gen.read() + 1;
        debounce_gen.set(gen);

        if query.trim().is_empty() {
            picker_results.set(Vec::new());
            picker_searching.set(false);
            return;
        }

        // Spawn within the Dioxus runtime — safe to mutate signals here.
        let ws = workspace_search.clone();
        let mut results_w = picker_results;
        let mut searching_w = picker_searching;
        spawn(async move {
            // 300 ms async sleep via js_sys::Promise (no extra crate needed).
            let promise = js_sys::Promise::new(&mut |resolve, _| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        &resolve,
                        300,
                    )
                    .unwrap();
            });
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;

            // If a newer keystroke came in while we were sleeping, abort.
            if *debounce_gen.read() != gen {
                return;
            }

            searching_w.set(true);
            let backend = HttpTicketBackend::new(None);
            if let Ok(resp) = backend
                .list_tickets(&ws, None, Some(&query), Some(20))
                .await
            {
                results_w.set(resp.items);
            }
            searching_w.set(false);
        });
    };

    // Called when the user confirms adding an edge in the picker overlay.
    let workspace_add = workspace.clone();
    let root_add = root_id.clone();
    let mut on_confirm_add = move |_: Event<MouseData>| {
        let Some(to_id) = picker_selected_id.read().clone() else {
            return;
        };
        let from_id = root_add.clone();
        let kind = picker_kind.read().clone();
        let reason_str = picker_reason.read().clone();
        let reason = if reason_str.trim().is_empty() {
            None
        } else {
            Some(reason_str)
        };
        let ws = workspace_add.clone();
        let mut err_w = picker_error;
        let mut open_w = picker_open;
        let mut ft = fetch_trigger;
        spawn(async move {
            let backend = HttpTicketBackend::new(None);
            let body = EdgeMutationBody { from_id, to_id, kind, reason };
            match backend.create_edge(&ws, &body).await {
                Ok(()) => {
                    open_w.set(false);
                    // The SSE stream will emit edge.upsert and trigger the
                    // existing subscription, but we also bump fetch_trigger
                    // for immediate refresh in case SSE has a small delay.
                    ft.with_mut(|v| *v += 1);
                }
                Err(e) => {
                    err_w.set(Some(if e.starts_with("cycle_detected:") {
                        "Adding this edge would create a dependency cycle. \
                         Choose a different target."
                            .to_string()
                    } else {
                        e
                    }));
                }
            }
        });
    };

    // ── Remove-edge handlers ──────────────────────────────────────────────

    let workspace_remove = workspace.clone();
    let mut on_confirm_remove = move |_: Event<MouseData>| {
        let Some(edge) = remove_confirm.read().clone() else {
            return;
        };
        let ws = workspace_remove.clone();
        let mut rc = remove_confirm;
        let mut ft = fetch_trigger;
        spawn(async move {
            let backend = HttpTicketBackend::new(None);
            let body = EdgeMutationBody {
                from_id: edge.from_id,
                to_id: edge.to_id,
                kind: edge.kind,
                reason: None,
            };
            let _ = backend.delete_edge(&ws, &body).await;
            rc.set(None);
            ft.with_mut(|v| *v += 1);
        });
    };

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
                                let on_sel = on_select_prop.clone();
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
                                            if let Some(ref cb) = on_sel {
                                                cb.call(node_id.clone());
                                            } else {
                                                nav.push(Route::TicketDetailPage {
                                                    workspace: ws.clone(),
                                                    id: node_id.clone(),
                                                });
                                            }
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
                button {
                    style: "
                        background: rgba(59,130,246,0.8);
                        color: #fff;
                        border: 1px solid rgba(147,197,253,0.4);
                        border-radius: 4px;
                        padding: 4px 10px;
                        font-size: 13px;
                        cursor: pointer;
                        line-height: 1;
                    ",
                    title: "Add dependency",
                    onclick: on_open_picker,
                    "+ Add dependency"
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

            // ── Adjacency list sidebar ─────────────────────────────────
            // Shows all edges visible in the current subgraph with × buttons
            // to remove them.  Positioned top-right above the zoom HUD.
            {
                layout.read().as_ref().and_then(|l| {
                    if l.edges.is_empty() {
                        return None;
                    }
                    Some(rsx! {
                        div {
                            style: "
                                position: absolute; top: 12px; right: 12px;
                                max-width: 280px; max-height: 220px;
                                background: rgba(18,18,30,0.92);
                                border: 1px solid rgba(200,200,220,0.18);
                                border-radius: 6px;
                                overflow-y: auto;
                                pointer-events: auto;
                                font-family: sans-serif;
                            ",
                            div {
                                style: "
                                    padding: 5px 8px;
                                    font-size: 10px; font-weight: 700;
                                    color: rgba(180,180,210,0.65);
                                    text-transform: uppercase; letter-spacing: 0.6px;
                                    border-bottom: 1px solid rgba(200,200,220,0.12);
                                    background: rgba(28,28,46,0.6);
                                ",
                                "Dependencies"
                            }
                            for edge in l.edges.iter() {
                                {
                                    let from = edge.from.clone();
                                    let to = edge.to.clone();
                                    let kind = edge.kind.clone();
                                    let from_title = l.nodes.iter()
                                        .find(|n| n.id == from)
                                        .and_then(|n| n.title.as_deref())
                                        .unwrap_or(&from)
                                        .to_string();
                                    let to_title = l.nodes.iter()
                                        .find(|n| n.id == to)
                                        .and_then(|n| n.title.as_deref())
                                        .unwrap_or(&to)
                                        .to_string();
                                    let re = RemoveEdge {
                                        from_id: from.clone(),
                                        to_id: to.clone(),
                                        kind: kind.clone(),
                                        from_title: from_title.clone(),
                                        to_title: to_title.clone(),
                                    };
                                    let mut rc = remove_confirm;
                                    rsx! {
                                        div {
                                            key: "{from}-{to}-{kind}",
                                            style: "
                                                display: flex; align-items: center; gap: 6px;
                                                padding: 4px 8px;
                                                border-bottom: 1px solid rgba(200,200,220,0.08);
                                                font-size: 11px; color: #b0b0c8;
                                            ",
                                            span {
                                                style: "
                                                    flex: 1;
                                                    overflow: hidden;
                                                    text-overflow: ellipsis;
                                                    white-space: nowrap;
                                                ",
                                                title: "{from_title} → {to_title}",
                                                "{from_title} → {to_title}"
                                            }
                                            span {
                                                style: "
                                                    background: rgba(100,100,200,0.25);
                                                    border-radius: 3px; padding: 1px 4px;
                                                    font-size: 9px; color: #8080b8;
                                                    flex-shrink: 0;
                                                ",
                                                "{kind}"
                                            }
                                            button {
                                                style: "
                                                    background: none; border: none;
                                                    color: #ef4444; cursor: pointer;
                                                    font-size: 14px; padding: 0 2px;
                                                    line-height: 1; flex-shrink: 0;
                                                ",
                                                title: "Remove this edge",
                                                onclick: move |_| rc.with_mut(|v| *v = Some(re.clone())),
                                                "×"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    })
                })
            }

            // ── Add-dependency picker overlay ──────────────────────────
            if *picker_open.read() {
                div {
                    // Backdrop — click to cancel.
                    style: "
                        position: absolute; inset: 0;
                        background: rgba(0,0,0,0.65);
                        display: flex; align-items: center; justify-content: center;
                        pointer-events: auto;
                        z-index: 50;
                    ",
                    onclick: move |_| picker_open.set(false),

                    div {
                        // Modal card — clicks don't bubble to backdrop.
                        style: "
                            background: #15152a;
                            border: 1px solid rgba(200,200,220,0.25);
                            border-radius: 10px;
                            padding: 20px;
                            width: 420px; max-width: 92%;
                            display: flex; flex-direction: column;
                            gap: 12px;
                            font-family: sans-serif;
                        ",
                        onclick: move |evt| evt.stop_propagation(),

                        h3 {
                            style: "margin: 0; color: #e0e0f0; font-size: 15px;",
                            "Add dependency"
                        }

                        // Search input
                        input {
                            r#type: "text",
                            placeholder: "Search tickets…",
                            value: "{picker_query}",
                            oninput: on_picker_input,
                            autofocus: true,
                            style: "
                                width: 100%; box-sizing: border-box;
                                background: #0e0e1e;
                                border: 1px solid rgba(200,200,220,0.25);
                                border-radius: 5px;
                                padding: 7px 10px;
                                color: #e0e0f0; font-size: 13px;
                                outline: none;
                            ",
                        }

                        // Results list
                        div {
                            style: "
                                max-height: 180px; overflow-y: auto;
                                border: 1px solid rgba(200,200,220,0.12);
                                border-radius: 5px;
                                background: #0e0e1e;
                            ",
                            if *picker_searching.read() {
                                div {
                                    style: "padding: 10px; color: #9ca3af; font-size: 13px;",
                                    "Searching…"
                                }
                            }
                            if !*picker_searching.read() && picker_results.read().is_empty() && !picker_query.read().is_empty() {
                                div {
                                    style: "padding: 10px; color: #9ca3af; font-size: 13px;",
                                    "No tickets found."
                                }
                            }
                            for result in picker_results.read().clone().into_iter() {
                                {
                                    let id = result.id.clone();
                                    let title = result.title.clone().unwrap_or_else(|| id.clone());
                                    let state_label = result.state.clone().unwrap_or_default();
                                    let is_selected = picker_selected_id.read().as_deref() == Some(id.as_str());
                                    let id_cap = id.clone();
                                    let title_cap = title.clone();
                                    let title_disp = title.clone();
                                    let state_disp = state_label.clone();
                                    rsx! {
                                        button {
                                            key: "{id}",
                                            style: if is_selected {
                                                "
                                                    display: flex; align-items: center; gap: 8px;
                                                    width: 100%; text-align: left;
                                                    padding: 7px 10px; border: none; cursor: pointer;
                                                    background: rgba(59,130,246,0.25);
                                                    color: #dde8ff; font-size: 12px;
                                                "
                                            } else {
                                                "
                                                    display: flex; align-items: center; gap: 8px;
                                                    width: 100%; text-align: left;
                                                    padding: 7px 10px; border: none; cursor: pointer;
                                                    background: transparent;
                                                    color: #c8c8e0; font-size: 12px;
                                                "
                                            },
                                            onclick: move |_| {
                                                picker_selected_id.set(Some(id_cap.clone()));
                                                picker_selected_title.set(Some(title_cap.clone()));
                                            },
                                            span {
                                                style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                                "{title_disp}"
                                            }
                                            if !state_disp.is_empty() {
                                                span {
                                                    style: "
                                                        font-size: 10px; color: #7878a0;
                                                        flex-shrink: 0;
                                                    ",
                                                    "{state_disp}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Edge kind selector
                        div {
                            style: "display: flex; flex-direction: column; gap: 4px;",
                            label {
                                style: "font-size: 11px; color: #9090b8; text-transform: uppercase; letter-spacing: 0.5px;",
                                "Edge type"
                            }
                            select {
                                style: "
                                    background: #0e0e1e;
                                    border: 1px solid rgba(200,200,220,0.25);
                                    border-radius: 5px; padding: 6px 8px;
                                    color: #e0e0f0; font-size: 13px;
                                ",
                                onchange: move |evt| picker_kind.set(evt.value()),
                                option { value: "depends_on", "depends_on" }
                                option { value: "linked", "linked" }
                            }
                        }

                        // Reason input (optional)
                        div {
                            style: "display: flex; flex-direction: column; gap: 4px;",
                            label {
                                style: "font-size: 11px; color: #9090b8; text-transform: uppercase; letter-spacing: 0.5px;",
                                "Reason (optional)"
                            }
                            textarea {
                                rows: "2",
                                placeholder: "Why does this dependency exist?",
                                value: "{picker_reason}",
                                oninput: move |evt| picker_reason.set(evt.value()),
                                style: "
                                    background: #0e0e1e;
                                    border: 1px solid rgba(200,200,220,0.25);
                                    border-radius: 5px; padding: 6px 8px;
                                    color: #e0e0f0; font-size: 13px;
                                    resize: vertical; min-height: 48px;
                                    font-family: sans-serif;
                                ",
                            }
                        }

                        // Cycle detection error banner (non-dismissing per spec).
                        if let Some(err) = picker_error.read().clone() {
                            div {
                                style: "
                                    background: rgba(239,68,68,0.12);
                                    border: 1px solid rgba(239,68,68,0.45);
                                    border-radius: 5px; padding: 8px 12px;
                                    color: #ef4444; font-size: 13px;
                                ",
                                "{err}"
                            }
                        }

                        // Action buttons
                        div {
                            style: "display: flex; gap: 8px; justify-content: flex-end;",
                            button {
                                style: "
                                    padding: 7px 16px; border-radius: 5px;
                                    border: 1px solid rgba(200,200,220,0.2);
                                    background: rgba(40,40,60,0.8);
                                    color: #c0c0d8; font-size: 13px; cursor: pointer;
                                ",
                                onclick: move |_| picker_open.set(false),
                                "Cancel"
                            }
                            button {
                                style: if picker_selected_id.read().is_some() {
                                    "
                                        padding: 7px 16px; border-radius: 5px; border: none;
                                        background: rgba(59,130,246,0.85);
                                        color: #fff; font-size: 13px; cursor: pointer;
                                    "
                                } else {
                                    "
                                        padding: 7px 16px; border-radius: 5px; border: none;
                                        background: rgba(59,130,246,0.3);
                                        color: rgba(255,255,255,0.4); font-size: 13px;
                                        cursor: not-allowed;
                                    "
                                },
                                disabled: picker_selected_id.read().is_none(),
                                onclick: on_confirm_add,
                                "Add"
                            }
                        }
                    }
                }
            }

            // ── Remove-edge confirmation dialog ────────────────────────
            if let Some(edge) = remove_confirm.read().clone() {
                div {
                    style: "
                        position: absolute; inset: 0;
                        background: rgba(0,0,0,0.70);
                        display: flex; align-items: center; justify-content: center;
                        pointer-events: auto;
                        z-index: 60;
                    ",
                    div {
                        style: "
                            background: #15152a;
                            border: 1px solid rgba(239,68,68,0.3);
                            border-radius: 10px; padding: 20px;
                            width: 360px; max-width: 92%;
                            font-family: sans-serif;
                            display: flex; flex-direction: column; gap: 12px;
                        ",
                        h3 {
                            style: "margin: 0; color: #e0e0f0; font-size: 15px;",
                            "Remove dependency?"
                        }
                        p {
                            style: "margin: 0; color: #9ca3af; font-size: 13px; line-height: 1.5;",
                            "Remove the \""
                            span { style: "color: #c0c0e0; font-weight: 600;", "{edge.kind}" }
                            "\" edge from \""
                            span { style: "color: #c0c0e0;", "{edge.from_title}" }
                            "\" to \""
                            span { style: "color: #c0c0e0;", "{edge.to_title}" }
                            "\"?"
                        }
                        div {
                            style: "display: flex; gap: 8px; justify-content: flex-end;",
                            button {
                                style: "
                                    padding: 7px 16px; border-radius: 5px;
                                    border: 1px solid rgba(200,200,220,0.2);
                                    background: rgba(40,40,60,0.8);
                                    color: #c0c0d8; font-size: 13px; cursor: pointer;
                                ",
                                onclick: move |_| remove_confirm.set(None),
                                "Cancel"
                            }
                            button {
                                style: "
                                    padding: 7px 16px; border-radius: 5px; border: none;
                                    background: rgba(239,68,68,0.8);
                                    color: #fff; font-size: 13px; cursor: pointer;
                                ",
                                onclick: on_confirm_remove,
                                "Remove"
                            }
                        }
                    }
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
