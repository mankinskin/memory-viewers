//! Spec graph page — full 3-D view of every spec with parent + code-ref edges.
//!
//! Fetches `/api/specs/graph`, lays out nodes via the user-selected algorithm
//! and renders via the shared [`viewer_api_dioxus::Graph3D`] canvas.
//!
//! The right-hand settings panel lets the user pick a layout algorithm
//! (rings-by-depth, force-directed, sphere, grid) and tune its parameters.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use dioxus::prelude::*;
use viewer_api_dioxus::{CameraCommand, EdgeRef3D, Layout3D, Node3D};
use wasm_bindgen_futures::spawn_local;

use crate::api;
use crate::types::{SpecGraphEdge, SpecGraphNode};

// ── Algorithm selection ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutAlgorithm {
    RingsByDepth,
    ForceDirected,
    Sphere,
    Grid,
    Tree2D,
}

impl LayoutAlgorithm {
    pub const ALL: &'static [LayoutAlgorithm] = &[
        LayoutAlgorithm::RingsByDepth,
        LayoutAlgorithm::ForceDirected,
        LayoutAlgorithm::Sphere,
        LayoutAlgorithm::Grid,
        LayoutAlgorithm::Tree2D,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LayoutAlgorithm::RingsByDepth  => "Rings by depth",
            LayoutAlgorithm::ForceDirected => "Force-directed",
            LayoutAlgorithm::Sphere        => "Sphere",
            LayoutAlgorithm::Grid          => "Grid",
            LayoutAlgorithm::Tree2D        => "2D Tree",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "rings"  => Some(LayoutAlgorithm::RingsByDepth),
            "force"  => Some(LayoutAlgorithm::ForceDirected),
            "sphere" => Some(LayoutAlgorithm::Sphere),
            "grid"   => Some(LayoutAlgorithm::Grid),
            "tree2d" => Some(LayoutAlgorithm::Tree2D),
            _        => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LayoutAlgorithm::RingsByDepth  => "rings",
            LayoutAlgorithm::ForceDirected => "force",
            LayoutAlgorithm::Sphere        => "sphere",
            LayoutAlgorithm::Grid          => "grid",
            LayoutAlgorithm::Tree2D        => "tree2d",
        }
    }

    /// Preferred orbit angle for this layout, used by the "Reset camera"
    /// button.  2-D layouts return a top-down perspective so the entire
    /// hierarchy is visible at once; 3-D layouts return the default
    /// three-quarter view.
    pub fn preferred_camera(self) -> CameraCommand {
        match self {
            // Look straight down the +Y axis at the XZ plane.
            LayoutAlgorithm::Tree2D => CameraCommand::ResetTo {
                yaw:   0.0,
                pitch: std::f32::consts::FRAC_PI_2 - 0.001,
            },
            _ => CameraCommand::ResetToDefault,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutParams {
    /// Overall scale multiplier applied to all positions (0.2 .. 4.0).
    pub spread:     f32,
    /// Vertical jitter / row-height for stack-based layouts (0.0 .. 4.0).
    pub y_spacing:  f32,
    /// Force-directed: number of iterations (10 .. 400).
    pub iterations: u32,
    /// Force-directed: edge target length (0.5 .. 6.0).
    pub link_dist:  f32,
    /// Force-directed: node-node repulsion strength (0.5 .. 8.0).
    pub repulsion:  f32,
}

impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            spread:     1.0,
            y_spacing:  0.8,
            iterations: 120,
            link_dist:  3.0,
            repulsion:  2.0,
        }
    }
}

// ── Layout entry-point ───────────────────────────────────────────────────────

pub fn build_layout(
    algo:   LayoutAlgorithm,
    params: LayoutParams,
    nodes:  &[SpecGraphNode],
    edges:  &[SpecGraphEdge],
) -> Layout3D {
    if nodes.is_empty() {
        return Layout3D::default();
    }
    let positioned = match algo {
        LayoutAlgorithm::RingsByDepth  => layout_rings(nodes, edges, params),
        LayoutAlgorithm::ForceDirected => layout_force(nodes, edges, params),
        LayoutAlgorithm::Sphere        => layout_sphere(nodes, params),
        LayoutAlgorithm::Grid          => layout_grid(nodes, params),
        LayoutAlgorithm::Tree2D        => layout_tree_2d(nodes, edges, params),
    };
    let idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let out_edges: Vec<EdgeRef3D> = edges
        .iter()
        .filter_map(|e| {
            let &from_idx = idx.get(e.from.as_str())?;
            let &to_idx   = idx.get(e.to.as_str())?;
            Some(EdgeRef3D { from_idx, to_idx, kind: e.kind.clone() })
        })
        .collect();
    Layout3D::new(positioned, out_edges)
}

// ── Rings-by-depth (the original layout) ────────────────────────────────────

fn layout_rings(
    nodes:  &[SpecGraphNode],
    edges:  &[SpecGraphEdge],
    params: LayoutParams,
) -> Vec<Node3D> {
    let idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut has_parent: HashSet<usize> = HashSet::new();
    for e in edges {
        if e.kind != "parent" { continue; }
        let (Some(&from), Some(&to)) = (idx.get(e.from.as_str()), idx.get(e.to.as_str()))
        else { continue };
        children.entry(from).or_default().push(to);
        has_parent.insert(to);
    }

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
    for d in depth.iter_mut() {
        if *d == u32::MAX { *d = 0; }
    }

    let mut by_depth: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, &d) in depth.iter().enumerate() {
        by_depth.entry(d).or_default().push(i);
    }

    let mut out = vec![
        Node3D { id: String::new(), label: None, state: None, x: 0.0, y: 0.0, z: 0.0 };
        nodes.len()
    ];
    for (&d, members) in &by_depth {
        let n = members.len() as f32;
        let radius = ((n * 0.55 + 2.5).min(18.0)) * params.spread;
        let z = -(d as f32) * 4.5 * params.spread;
        for (k, &i) in members.iter().enumerate() {
            let theta = if n > 0.0 {
                (k as f32) / n * std::f32::consts::TAU
            } else { 0.0 };
            let x = radius * theta.cos();
            let y = ((k % 3) as f32 - 1.0) * params.y_spacing;
            let nd = &nodes[i];
            out[i] = Node3D {
                id:    nd.id.clone(),
                label: nd.title.clone(),
                state: nd.state.clone(),
                x, y, z,
            };
        }
    }
    out
}

// ── Force-directed (Fruchterman–Reingold variant in 3-D) ────────────────────

fn layout_force(
    nodes:  &[SpecGraphNode],
    edges:  &[SpecGraphEdge],
    params: LayoutParams,
) -> Vec<Node3D> {
    let n = nodes.len();
    let idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| (nd.id.as_str(), i))
        .collect();

    // Deterministic initial positions on a sphere.
    let mut pos: Vec<[f32; 3]> = (0..n)
        .map(|i| {
            let phi = (1.0 - 2.0 * (i as f32 + 0.5) / n as f32).acos();
            let theta = std::f32::consts::PI * (1.0 + 5.0_f32.sqrt()) * (i as f32);
            let r = 4.0 * params.spread;
            [r * phi.sin() * theta.cos(),
             r * phi.sin() * theta.sin(),
             r * phi.cos()]
        })
        .collect();

    let edges_idx: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| {
            let &a = idx.get(e.from.as_str())?;
            let &b = idx.get(e.to.as_str())?;
            Some((a, b))
        })
        .collect();

    let k = params.link_dist.max(0.1);
    let rep = params.repulsion;
    let iters = params.iterations.max(1);
    let mut temperature = 0.6_f32 * params.spread;

    for _ in 0..iters {
        let mut disp = vec![[0.0_f32; 3]; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i][0] - pos[j][0];
                let dy = pos[i][1] - pos[j][1];
                let dz = pos[i][2] - pos[j][2];
                let dist2 = (dx * dx + dy * dy + dz * dz).max(0.01);
                let dist = dist2.sqrt();
                let f = rep * k * k / dist2;
                let ux = dx / dist;
                let uy = dy / dist;
                let uz = dz / dist;
                disp[i][0] += ux * f;
                disp[i][1] += uy * f;
                disp[i][2] += uz * f;
                disp[j][0] -= ux * f;
                disp[j][1] -= uy * f;
                disp[j][2] -= uz * f;
            }
        }

        for &(a, b) in &edges_idx {
            if a == b { continue; }
            let dx = pos[a][0] - pos[b][0];
            let dy = pos[a][1] - pos[b][1];
            let dz = pos[a][2] - pos[b][2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);
            let f = dist * dist / k;
            let ux = dx / dist;
            let uy = dy / dist;
            let uz = dz / dist;
            disp[a][0] -= ux * f;
            disp[a][1] -= uy * f;
            disp[a][2] -= uz * f;
            disp[b][0] += ux * f;
            disp[b][1] += uy * f;
            disp[b][2] += uz * f;
        }

        for i in 0..n {
            let d = disp[i];
            let mag = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(0.001);
            let cap = mag.min(temperature);
            pos[i][0] += d[0] / mag * cap;
            pos[i][1] += d[1] / mag * cap;
            pos[i][2] += d[2] / mag * cap;
        }
        temperature *= 0.97;
    }

    nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| Node3D {
            id:    nd.id.clone(),
            label: nd.title.clone(),
            state: nd.state.clone(),
            x: pos[i][0],
            y: pos[i][1],
            z: pos[i][2],
        })
        .collect()
}

// ── Sphere (Fibonacci lattice on a sphere) ───────────────────────────────────

fn layout_sphere(nodes: &[SpecGraphNode], params: LayoutParams) -> Vec<Node3D> {
    let n = nodes.len() as f32;
    let radius = (n.sqrt() * 0.9 + 4.0) * params.spread;
    let golden = std::f32::consts::PI * (1.0 + 5.0_f32.sqrt());
    nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| {
            let t = (i as f32 + 0.5) / n;
            let phi = (1.0 - 2.0 * t).acos();
            let theta = golden * (i as f32);
            Node3D {
                id:    nd.id.clone(),
                label: nd.title.clone(),
                state: nd.state.clone(),
                x: radius * phi.sin() * theta.cos(),
                y: radius * phi.cos(),
                z: radius * phi.sin() * theta.sin(),
            }
        })
        .collect()
}

// ── Grid (rectangular lattice in the XZ plane) ───────────────────────────────

fn layout_grid(nodes: &[SpecGraphNode], params: LayoutParams) -> Vec<Node3D> {
    let n = nodes.len();
    let cols = ((n as f32).sqrt().ceil() as usize).max(1);
    let cell = 2.5 * params.spread;
    let half_w = (cols as f32 - 1.0) * 0.5 * cell;
    let rows = (n + cols - 1) / cols;
    let half_d = (rows as f32 - 1.0) * 0.5 * cell;
    nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| {
            let col = i % cols;
            let row = i / cols;
            Node3D {
                id:    nd.id.clone(),
                label: nd.title.clone(),
                state: nd.state.clone(),
                x: col as f32 * cell - half_w,
                y: ((row % 2) as f32 - 0.5) * params.y_spacing,
                z: row as f32 * cell - half_d,
            }
        })
        .collect()
}

// ── State color helper ───────────────────────────────────────────────────────

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

// ── 2-D Tree (tidy tree in the XZ plane) ─────────────────────────────────────
//
// Builds a forest from the parent-edges and runs a simple post-order
// "subtree-width" tidy-tree pass.  Each subtree is allocated a horizontal
// span equal to the sum of its leaf widths; the parent is centred above
// its children.  Result is in the XZ plane (Y=0) so the camera can look
// straight down for a true 2-D view.
fn layout_tree_2d(
    nodes:  &[SpecGraphNode],
    edges:  &[SpecGraphEdge],
    params: LayoutParams,
) -> Vec<Node3D> {
    let n = nodes.len();
    let idx: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Build parent-edge adjacency.  An edge `from -> to` of kind "parent"
    // means `from` is the parent of `to`.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut has_parent = vec![false; n];
    for e in edges {
        if e.kind != "parent" { continue; }
        let (Some(&from), Some(&to)) = (idx.get(e.from.as_str()), idx.get(e.to.as_str()))
        else { continue };
        if from == to { continue; }
        children[from].push(to);
        has_parent[to] = true;
    }
    // Stable child order so re-runs produce deterministic layouts.
    for cs in children.iter_mut() {
        cs.sort();
    }

    // Roots are nodes without parents (orphans become single-node roots).
    let mut roots: Vec<usize> = (0..n).filter(|&i| !has_parent[i]).collect();
    roots.sort();

    // Post-order subtree-width pass (in leaf-units).  Cycle-safe via
    // visited bitmap.
    let mut width  = vec![1.0_f32; n];
    let mut depth  = vec![0_u32;   n];
    let mut visited = vec![false;  n];
    fn measure(
        i: usize,
        d: u32,
        children: &[Vec<usize>],
        width: &mut [f32],
        depth: &mut [u32],
        visited: &mut [bool],
    ) {
        if visited[i] { return; }
        visited[i] = true;
        depth[i] = d;
        let mut sum = 0.0_f32;
        for &c in &children[i] {
            measure(c, d + 1, children, width, depth, visited);
            sum += width[c];
        }
        if !children[i].is_empty() {
            width[i] = sum.max(1.0);
        }
    }
    for &r in &roots {
        measure(r, 0, &children, &mut width, &mut depth, &mut visited);
    }
    // Any node not reached (cycle members) gets depth 0, width 1 \u2014 they
    // join the synthetic root row.
    for i in 0..n {
        if !visited[i] {
            roots.push(i);
            visited[i] = true;
        }
    }

    // Pre-order assignment pass: each subtree gets an [x, x+width] slot
    // and its root sits at the slot centre.
    let col_step = 2.5 * params.spread;
    let row_step = 2.5 * params.spread.max(0.4);
    let mut x_pos = vec![0.0_f32; n];

    fn assign(
        i: usize,
        slot_start: f32,
        children: &[Vec<usize>],
        width: &[f32],
        x_pos: &mut [f32],
    ) {
        x_pos[i] = slot_start + width[i] * 0.5;
        let mut cursor = slot_start;
        for &c in &children[i] {
            assign(c, cursor, children, width, x_pos);
            cursor += width[c];
        }
    }

    let mut cursor = 0.0_f32;
    for &r in &roots {
        assign(r, cursor, &children, &width, &mut x_pos);
        cursor += width[r];
    }
    let total_width: f32 = roots.iter().map(|&r| width[r]).sum();
    let centre_offset = total_width * 0.5;

    nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| {
            let x = (x_pos[i] - centre_offset) * col_step;
            let z = depth[i] as f32 * row_step;
            Node3D {
                id:    nd.id.clone(),
                label: nd.title.clone(),
                state: nd.state.clone(),
                x,
                y: 0.0,
                z,
            }
        })
        .collect()
}

// ── Main page ────────────────────────────────────────────────────────────────

#[component]
pub fn SpecGraphPage() -> Element {
    let mut raw:    Signal<Option<(Vec<SpecGraphNode>, Vec<SpecGraphEdge>)>> = use_signal(|| None);
    let mut error:  Signal<Option<String>> = use_signal(|| None);

    // Draft (edited) values — what the panel widgets bind to.
    let mut draft_algo:   Signal<LayoutAlgorithm> = use_signal(|| LayoutAlgorithm::ForceDirected);
    let mut draft_params: Signal<LayoutParams>    = use_signal(LayoutParams::default);
    let mut draft_show_edges: Signal<bool>        = use_signal(|| true);

    // Committed values — what is actually fed into build_layout.
    let mut committed_algo:   Signal<LayoutAlgorithm> = use_signal(|| LayoutAlgorithm::ForceDirected);
    let mut committed_params: Signal<LayoutParams>    = use_signal(LayoutParams::default);
    let mut committed_show_edges: Signal<bool>        = use_signal(|| true);

    let mut auto_apply: Signal<bool> = use_signal(|| true);
    let mut panel_open: Signal<bool> = use_signal(|| true);

    // Imperative camera-command channel.  `camera_seq` is bumped whenever
    // we want Graph3D to re-apply `camera_cmd`.  See `CameraCommand` docs.
    let mut camera_cmd: Signal<Option<CameraCommand>> = use_signal(|| None);
    let mut camera_seq: Signal<u64>                   = use_signal(|| 0);

    // When the committed algorithm changes (auto-apply or Apply press),
    // automatically reset the camera to that algorithm's preferred angle
    // so 2-D layouts get a top-down view.  We track the last-applied algo
    // so we don't fire on every render.
    let mut last_cam_algo: Signal<LayoutAlgorithm> = use_hook(|| Signal::new(LayoutAlgorithm::ForceDirected));

    // Preview sidebar state.  When `Some(id)`, the right-side preview
    // panel is open and fetching that spec's summary.
    let mut preview_id: Signal<Option<String>> = use_signal(|| None);

    let nav = use_navigator();

    use_effect(move || {
        spawn_local(async move {
            match api::get_graph().await {
                Ok(resp) => raw.set(Some((resp.nodes, resp.edges))),
                Err(e)   => error.set(Some(e)),
            }
        });
    });

    if let Some(msg) = error.read().clone() {
        return rsx! {
            div {
                class: "empty-state",
                style: "color: var(--error);",
                "Failed to load graph: {msg}"
            }
        };
    }

    let Some((nodes_raw, edges_raw)) = raw.read().clone() else {
        return rsx! {
            div {
                class: "empty-state",
                "Loading graph\u{2026}"
            }
        };
    };

    let cur_algo   = *committed_algo.read();
    let cur_params = *committed_params.read();

    // Auto-reset camera when the committed algorithm changes \u2014 this is
    // what makes "switch to 2-D Tree" snap straight to top-down without
    // requiring a manual Reset Camera click.
    if cur_algo != *last_cam_algo.peek() {
        last_cam_algo.set(cur_algo);
        camera_cmd.set(Some(cur_algo.preferred_camera()));
        let next_seq = *camera_seq.peek() + 1;
        camera_seq.set(next_seq);
    }

    let edges_for_layout: Vec<SpecGraphEdge> = if *committed_show_edges.read() {
        edges_raw.clone()
    } else {
        Vec::new()
    };
    let l = build_layout(cur_algo, cur_params, &nodes_raw, &edges_for_layout);
    let nodes = l.nodes.clone();
    let node_count = nodes.len();
    let edge_count = l.edges.len();

    let d_algo   = *draft_algo.read();
    let d_params = *draft_params.read();
    let d_show_edges = *draft_show_edges.read();
    let is_auto = *auto_apply.read();
    let dirty = d_algo != cur_algo || d_params != cur_params || d_show_edges != *committed_show_edges.read();
    let apply_disabled = is_auto || !dirty;

    let cam_cmd_now = *camera_cmd.read();
    let cam_seq_now = *camera_seq.read();

    rsx! {
        div { class: "graph-overlay",
            viewer_api_dioxus::Graph3D {
                layout: l,
                container_id: "spec-graph3d-container".to_string(),
                container_style: "position: absolute; inset: 0; overflow: hidden; user-select: none; cursor: grab;".to_string(),
                camera_command: cam_cmd_now,
                camera_command_seq: cam_seq_now,
                div {
                    id: "spec-graph3d-nodes",
                    class: "graph-nodes-layer",
                    for (i, n) in nodes.iter().enumerate() {
                        {
                            let id    = n.id.clone();
                            let title = n.label.clone().unwrap_or_else(|| "Untitled".into());
                            let state = n.state.clone().unwrap_or_else(|| "draft".into());
                            let color = state_color(Some(state.as_str()));
                            let id_click = id.clone();
                            rsx! {
                                div {
                                    key: "{id}",
                                    "data-node-idx": "{i}",
                                    class: "graph-node-card",
                                    style: "border-left: 3px solid {color};",
                                    onclick: move |evt: Event<MouseData>| {
                                        evt.stop_propagation();
                                        // Click opens the preview sidebar; a
                                        // "View details" button inside the
                                        // sidebar performs the full navigation.
                                        preview_id.set(Some(id_click.clone()));
                                    },
                                    div {
                                        class: "graph-node-card__title",
                                        "{title}"
                                    }
                                    div {
                                        class: "graph-node-card__meta",
                                        span {
                                            class: "graph-node-card__state",
                                            style: "color: {color};",
                                            "{state}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div {
                    class: "graph-controls-hint",
                    "Left-drag: orbit \u{00b7} Right-drag: pan \u{00b7} Scroll: zoom \u{00b7} Click card: open"
                }
                if node_count > 0 {
                    div {
                        class: "graph-count-badge",
                        "{node_count} specs \u{00b7} {edge_count} edges"
                    }
                }
                button {
                    class: "graph-settings-toggle",
                    "data-testid": "graph-settings-toggle",
                    "data-graph-passthrough": "false",
                    aria_label: "Toggle graph settings",
                    onclick: move |evt: Event<MouseData>| {
                        evt.stop_propagation();
                        let v = *panel_open.read();
                        panel_open.set(!v);
                    },
                    if *panel_open.read() { "\u{2715} Settings" } else { "\u{2699} Settings" }
                }
                if *panel_open.read() {
                    div {
                        class: "graph-settings-panel",
                        "data-testid": "graph-settings-panel",
                        "data-graph-passthrough": "false",
                        onclick: move |evt: Event<MouseData>| evt.stop_propagation(),
                        onmousedown: move |evt: Event<MouseData>| evt.stop_propagation(),
                        onwheel: move |evt: Event<WheelData>| evt.stop_propagation(),

                        h3 { class: "graph-settings-panel__title", "Graph settings" }

                        // ── Auto-apply toggle ──
                        div { class: "graph-settings-section",
                            label { class: "graph-settings-label graph-settings-label--inline",
                                input {
                                    r#type: "checkbox",
                                    "data-testid": "graph-toggle-auto-apply",
                                    checked: is_auto,
                                    onchange: move |evt| {
                                        let v = evt.checked();
                                        auto_apply.set(v);
                                        // Switching auto-apply on commits any pending draft.
                                        if v {
                                            committed_algo.set(*draft_algo.read());
                                            committed_params.set(*draft_params.read());
                                            committed_show_edges.set(*draft_show_edges.read());
                                        }
                                    },
                                }
                                " Auto-apply"
                            }
                        }

                        // ── Layout algorithm ──
                        div { class: "graph-settings-section",
                            label { class: "graph-settings-label", "Layout algorithm" }
                            select {
                                class: "graph-settings-select",
                                "data-testid": "graph-algo-select",
                                value: d_algo.as_str(),
                                onchange: move |evt| {
                                    if let Some(a) = LayoutAlgorithm::from_str_opt(&evt.value()) {
                                        draft_algo.set(a);
                                        if *auto_apply.read() {
                                            committed_algo.set(a);
                                        }
                                    }
                                },
                                for a in LayoutAlgorithm::ALL.iter() {
                                    option { value: a.as_str(), "{a.label()}" }
                                }
                            }
                        }

                        // ── Spread ──
                        div { class: "graph-settings-section",
                            label { class: "graph-settings-label",
                                "Spread "
                                span { class: "graph-settings-value", "{d_params.spread:.2}" }
                            }
                            input {
                                r#type: "range",
                                min: "0.2", max: "4.0", step: "0.05",
                                value: "{d_params.spread}",
                                oninput: move |evt| {
                                    if let Ok(v) = evt.value().parse::<f32>() {
                                        let mut p = *draft_params.read();
                                        p.spread = v;
                                        draft_params.set(p);
                                        if *auto_apply.read() { committed_params.set(p); }
                                    }
                                },
                            }
                        }

                        if matches!(d_algo, LayoutAlgorithm::RingsByDepth | LayoutAlgorithm::Grid) {
                            div { class: "graph-settings-section",
                                label { class: "graph-settings-label",
                                    "Y spacing "
                                    span { class: "graph-settings-value", "{d_params.y_spacing:.2}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "0.0", max: "4.0", step: "0.05",
                                    value: "{d_params.y_spacing}",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<f32>() {
                                            let mut p = *draft_params.read();
                                            p.y_spacing = v;
                                            draft_params.set(p);
                                            if *auto_apply.read() { committed_params.set(p); }
                                        }
                                    },
                                }
                            }
                        }

                        if matches!(d_algo, LayoutAlgorithm::ForceDirected) {
                            div { class: "graph-settings-section",
                                label { class: "graph-settings-label",
                                    "Iterations "
                                    span { class: "graph-settings-value", "{d_params.iterations}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "10", max: "400", step: "10",
                                    value: "{d_params.iterations}",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<u32>() {
                                            let mut p = *draft_params.read();
                                            p.iterations = v;
                                            draft_params.set(p);
                                            if *auto_apply.read() { committed_params.set(p); }
                                        }
                                    },
                                }
                            }
                            div { class: "graph-settings-section",
                                label { class: "graph-settings-label",
                                    "Link distance "
                                    span { class: "graph-settings-value", "{d_params.link_dist:.2}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "0.5", max: "6.0", step: "0.1",
                                    value: "{d_params.link_dist}",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<f32>() {
                                            let mut p = *draft_params.read();
                                            p.link_dist = v;
                                            draft_params.set(p);
                                            if *auto_apply.read() { committed_params.set(p); }
                                        }
                                    },
                                }
                            }
                            div { class: "graph-settings-section",
                                label { class: "graph-settings-label",
                                    "Repulsion "
                                    span { class: "graph-settings-value", "{d_params.repulsion:.2}" }
                                }
                                input {
                                    r#type: "range",
                                    min: "0.5", max: "8.0", step: "0.1",
                                    value: "{d_params.repulsion}",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<f32>() {
                                            let mut p = *draft_params.read();
                                            p.repulsion = v;
                                            draft_params.set(p);
                                            if *auto_apply.read() { committed_params.set(p); }
                                        }
                                    },
                                }
                            }
                        }

                        // ── Show edges ──
                        div { class: "graph-settings-section",
                            label { class: "graph-settings-label graph-settings-label--inline",
                                input {
                                    r#type: "checkbox",
                                    "data-testid": "graph-toggle-edges",
                                    checked: d_show_edges,
                                    onchange: move |evt| {
                                        let v = evt.checked();
                                        draft_show_edges.set(v);
                                        if *auto_apply.read() { committed_show_edges.set(v); }
                                    },
                                }
                                " Show edges"
                            }
                        }

                        // ── Apply / Reset ──
                        div { class: "graph-settings-section graph-settings-actions",
                            button {
                                class: "graph-settings-apply",
                                "data-testid": "graph-settings-apply",
                                disabled: apply_disabled,
                                onclick: move |evt: Event<MouseData>| {
                                    evt.stop_propagation();
                                    committed_algo.set(*draft_algo.read());
                                    committed_params.set(*draft_params.read());
                                    committed_show_edges.set(*draft_show_edges.read());
                                },
                                "Apply"
                            }
                            button {
                                class: "graph-settings-reset",
                                "data-testid": "graph-settings-reset",
                                onclick: move |evt: Event<MouseData>| {
                                    evt.stop_propagation();
                                    let p = LayoutParams::default();
                                    draft_params.set(p);
                                    if *auto_apply.read() { committed_params.set(p); }
                                },
                                "Reset"
                            }
                        }

                        // ── Reset camera ──
                        div { class: "graph-settings-section",
                            button {
                                class: "graph-settings-reset",
                                "data-testid": "graph-reset-camera",
                                onclick: move |evt: Event<MouseData>| {
                                    evt.stop_propagation();
                                    let cmd = (*committed_algo.read()).preferred_camera();
                                    camera_cmd.set(Some(cmd));
                                    let next_seq = *camera_seq.peek() + 1;
                                    camera_seq.set(next_seq);
                                },
                                "Reset camera"
                            }
                        }
                    }
                }
            }
            // ── Preview sidebar ──
            if let Some(pid) = preview_id.read().clone() {
                SpecPreviewSidebar {
                    spec_id: pid.clone(),
                    on_close: move |_| { preview_id.set(None); },
                    on_view_details: move |id: String| {
                        preview_id.set(None);
                        nav.push(crate::routes::Route::SpecDetailPage { id });
                    },
                }
            }
        }
    }
}

// ── Preview sidebar ──────────────────────────────────────────────────────────

/// Right-side overlay that previews a spec's title, state and an excerpt
/// of its body, with a "View details" button that hands off to the full
/// detail route.  The sidebar is opt-out of camera gestures via
/// `data-graph-passthrough="false"`.
#[component]
fn SpecPreviewSidebar(
    spec_id: String,
    on_close: EventHandler<()>,
    on_view_details: EventHandler<String>,
) -> Element {
    let mut full: Signal<Option<crate::types::SpecFullResponse>> = use_signal(|| None);
    let mut load_err: Signal<Option<String>> = use_signal(|| None);

    let spec_id_load = spec_id.clone();
    use_effect(use_reactive!(|spec_id_load| {
        full.set(None);
        load_err.set(None);
        let id = spec_id_load.clone();
        spawn_local(async move {
            match api::get_spec_full(&id).await {
                Ok(resp) => full.set(Some(resp)),
                Err(e)   => load_err.set(Some(e)),
            }
        });
    }));

    let spec_id_open = spec_id.clone();

    rsx! {
        aside {
            class: "spec-preview",
            "data-testid": "spec-preview",
            "data-graph-passthrough": "false",
            onclick:     move |evt: Event<MouseData>| evt.stop_propagation(),
            onmousedown: move |evt: Event<MouseData>| evt.stop_propagation(),
            onwheel:     move |evt: Event<WheelData>| evt.stop_propagation(),

            header { class: "spec-preview__header",
                h3 { class: "spec-preview__title",
                    {
                        full.read().as_ref()
                            .and_then(|f| f.spec.fields.get("title")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()))
                            .unwrap_or_else(|| spec_id.clone())
                    }
                }
                button {
                    class: "spec-preview__close",
                    "data-testid": "spec-preview-close",
                    aria_label: "Close preview",
                    onclick: move |_| on_close.call(()),
                    "\u{2715}"
                }
            }
            div { class: "spec-preview__body",
                if let Some(err) = load_err.read().clone() {
                    p { class: "spec-preview__error", "Failed to load: {err}" }
                } else if let Some(f) = full.read().clone() {
                    {
                        let state = f.spec.fields.get("state")
                            .and_then(|v| v.as_str())
                            .unwrap_or("draft")
                            .to_string();
                        let color = state_color(Some(state.as_str()));
                        let excerpt: String = {
                            let body = f.body.trim();
                            // Skip leading H1 if present.
                            let body = body.lines()
                                .skip_while(|l| l.starts_with('#') || l.trim().is_empty())
                                .collect::<Vec<_>>()
                                .join("\n");
                            let trimmed: String = body.chars().take(420).collect();
                            if body.chars().count() > 420 {
                                format!("{trimmed}\u{2026}")
                            } else {
                                trimmed
                            }
                        };
                        rsx! {
                            div { class: "spec-preview__meta",
                                span {
                                    class: "spec-preview__state",
                                    style: "color: {color}; border-color: {color};",
                                    "{state}"
                                }
                            }
                            pre { class: "spec-preview__excerpt", "{excerpt}" }
                        }
                    }
                } else {
                    p { class: "spec-preview__loading", "Loading\u{2026}" }
                }
            }
            footer { class: "spec-preview__footer",
                button {
                    class: "spec-preview__details",
                    "data-testid": "spec-preview-details",
                    onclick: move |_| on_view_details.call(spec_id_open.clone()),
                    "View details \u{2192}"
                }
            }
        }
    }
}
