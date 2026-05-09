//! Hierarchical 2D layout engine for the ticket dependency graph.
//!
//! Nodes are arranged in depth-based rows (BFS depth 0 = root at top).
//! Within each row nodes are ordered by priority (critical → none) then title.
//! The resulting layout has larger / parent tickets at the top and smaller /
//! leaf tickets at the bottom, matching the conventional dependency-tree view.
//!
//! Used by `graph3d::Layout3D::from_2d()` to compute initial node positions
//! before 3D projection.

use std::collections::HashMap;

use crate::types::{
    GraphEdgeItem,
    GraphNodeItem,
};

// ── Layout mode ────────────────────────────────────────────────────────────

// Re-exported from viewer-api so callers don't need to import two crates.
pub use viewer_api_dioxus::LayoutMode;

/// One node in the rendered graph.  Coordinates are in layout-space pixels
/// centred on (0, 0); the canvas/DOM layer applies pan + zoom on top.
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    /// BFS depth from the root ticket.
    pub depth: usize,
    /// Ticket type (e.g. "tracker-improvement").
    pub ticket_type: Option<String>,
    /// Priority field value (e.g. "high", "critical").
    pub priority: Option<String>,
    /// Layout position (centre of card), layout-space pixels.
    pub x: f64,
    pub y: f64,
    /// Z position — force-directed within each depth layer's XZ plane.
    pub z: f64,
    /// Physics velocities used by force simulations.
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
}

/// One directed dependency edge.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

/// Fully computed layout ready for rendering.
#[derive(Debug, Clone, Default)]
pub struct GraphLayout {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

// ── Priority ordering ──────────────────────────────────────────────────────

/// Maps a priority string to a sort key (lower = more important = further left).
fn priority_order(p: Option<&str>) -> u32 {
    match p {
        Some("critical") => 0,
        Some("high") => 1,
        Some("medium") => 2,
        Some("low") => 3,
        Some("none") | Some("") => 4,
        _ => 5,
    }
}

// ── Layout construction ────────────────────────────────────────────────────

impl GraphLayout {
    /// Build a hierarchical layout from raw API items using the default
    /// [`LayoutMode::Hierarchical3D`] algorithm.
    pub fn build(
        api_nodes: Vec<GraphNodeItem>,
        api_edges: Vec<GraphEdgeItem>,
    ) -> Self {
        Self::build_with_mode(api_nodes, api_edges, LayoutMode::default())
    }

    /// Build a layout using the specified [`LayoutMode`].
    pub fn build_with_mode(
        api_nodes: Vec<GraphNodeItem>,
        api_edges: Vec<GraphEdgeItem>,
        mode: LayoutMode,
    ) -> Self {
        let nodes: Vec<GraphNode> = api_nodes
            .into_iter()
            .map(|node| GraphNode {
                id: node.id,
                title: node.title,
                state: node.state,
                depth: node.depth,
                ticket_type: node.ticket_type,
                priority: node.priority,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
            })
            .collect();

        let edges: Vec<GraphEdge> = api_edges
            .into_iter()
            .map(|e| GraphEdge {
                from: e.from,
                to: e.to,
                kind: e.kind,
            })
            .collect();

        let mut layout = Self { nodes, edges };
        layout.hierarchical_layout();

        // For Flat2D, zero out all Z coords so the graph is a pure top-down
        // 2-D projection (suitable for orthographic view).
        if mode == LayoutMode::Flat2D {
            for nd in &mut layout.nodes {
                nd.z = 0.0;
            }
        }

        layout
    }

    /// Place nodes in a 3-D hierarchical layout:
    ///
    /// * Y axis: `depth * LAYER_SPACING` — deeper dependencies are lower.
    /// * XZ plane: nodes in each layer start on an evenly-spaced circle,
    ///   then a force simulation clusters connected nodes while spreading
    ///   same-layer siblings apart.
    fn hierarchical_layout(&mut self) {
        // Vertical gap between depth layers (dependency hierarchy).
        const LAYER_SPACING: f64 = 300.0;
        // Minimum inter-node arc spacing on the initial placement circle.
        const COL_SPACING: f64 = 360.0;

        // Group node indices by depth.
        let mut by_depth: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            by_depth.entry(node.depth).or_default().push(i);
        }

        let mut depths: Vec<usize> = by_depth.keys().copied().collect();
        depths.sort_unstable();

        for depth in depths {
            let indices = by_depth.get_mut(&depth).unwrap();

            // Sort within the layer: priority then title for a stable seed.
            indices.sort_by(|&a, &b| {
                let pa = priority_order(self.nodes[a].priority.as_deref());
                let pb = priority_order(self.nodes[b].priority.as_deref());
                pa.cmp(&pb).then_with(|| {
                    let ta = self.nodes[a].title.as_deref().unwrap_or("");
                    let tb = self.nodes[b].title.as_deref().unwrap_or("");
                    ta.cmp(tb)
                })
            });

            let n = indices.len();
            let y = depth as f64 * LAYER_SPACING;

            if n == 1 {
                // Single node: place at origin of its layer's XZ plane.
                self.nodes[indices[0]].x = 0.0;
                self.nodes[indices[0]].y = y;
                self.nodes[indices[0]].z = 0.0;
            } else {
                // Spread nodes evenly around a circle in XZ so the force
                // simulation starts from a symmetric, uncrowded seed.
                let radius = (n as f64 * COL_SPACING / std::f64::consts::TAU)
                    .max(COL_SPACING);
                for (slot, &node_idx) in indices.iter().enumerate() {
                    let angle = std::f64::consts::TAU * slot as f64 / n as f64;
                    self.nodes[node_idx].x = radius * angle.cos();
                    self.nodes[node_idx].y = y;
                    self.nodes[node_idx].z = radius * angle.sin();
                }
            }
        }

        // Centre vertically around y = 0.
        self.centre_y();

        // Refine XZ positions: connected nodes attract in the XZ plane while
        // same-layer siblings repel.  Y is frozen throughout.
        self.xz_force_refinement(200);

        // Re-centre XZ after force spread.
        self.centre_xz();
    }

    /// XZ-plane force simulation that keeps depth-layer Y positions fixed.
    ///
    /// * Same-layer nodes repel each other in XZ to prevent crowding.
    /// * Edge-connected nodes attract in XZ so linked subtrees cluster
    ///   toward each other across the 3-D space of their layers.
    /// * Cross-layer repulsion decays with Y distance so nodes primarily
    ///   push away neighbours on the same or adjacent layers.
    fn xz_force_refinement(
        &mut self,
        steps: usize,
    ) {
        let n = self.nodes.len();
        if n <= 1 {
            return;
        }

        // Pre-compute (from_idx, to_idx) for every edge.
        let edge_pairs: Vec<(usize, usize)> = {
            let id_to_idx: HashMap<&str, usize> = self
                .nodes
                .iter()
                .enumerate()
                .map(|(i, nd)| (nd.id.as_str(), i))
                .collect();
            self.edges
                .iter()
                .filter_map(|e| {
                    let i = id_to_idx.get(e.from.as_str()).copied()?;
                    let j = id_to_idx.get(e.to.as_str()).copied()?;
                    Some((i, j))
                })
                .collect()
        };

        // Spring constant: pulls connected nodes toward each other in XZ.
        const SPRING_K: f64 = 0.05;
        // Repulsion strength in the XZ plane.
        const REPULSION: f64 = 80_000.0;
        // Minimum XZ distance used in the repulsion denominator (px).
        const MIN_DIST: f64 = 15.0;
        // Velocity damping per step.
        const DAMPING: f64 = 0.75;
        const DT: f64 = 1.0;

        for _ in 0..steps {
            let mut fx = vec![0.0_f64; n];
            let mut fz = vec![0.0_f64; n];

            // XZ repulsion between every pair of nodes.
            // Repulsion decays for nodes far apart in Y (cross-layer) so the
            // force mainly separates nodes on the same or nearby layers.
            for i in 0..n {
                for j in (i + 1)..n {
                    let dx = self.nodes[i].x - self.nodes[j].x;
                    let dz = self.nodes[i].z - self.nodes[j].z;
                    let dy = (self.nodes[i].y - self.nodes[j].y).abs();
                    let layer_w = 1.0 / (1.0 + dy / 250.0);
                    let d_xz = (dx * dx + dz * dz).sqrt().max(MIN_DIST);
                    let f = REPULSION * layer_w / (d_xz * d_xz);
                    fx[i] += f * dx / d_xz;
                    fx[j] -= f * dx / d_xz;
                    fz[i] += f * dz / d_xz;
                    fz[j] -= f * dz / d_xz;
                }
            }

            // XZ spring attraction along dependency edges.
            // Pulls parent and child toward each other in XZ so connected
            // subtrees group together visually.
            for &(i, j) in &edge_pairs {
                let dx = self.nodes[j].x - self.nodes[i].x;
                let dz = self.nodes[j].z - self.nodes[i].z;
                fx[i] += SPRING_K * dx;
                fx[j] -= SPRING_K * dx;
                fz[i] += SPRING_K * dz;
                fz[j] -= SPRING_K * dz;
            }

            // Integrate X and Z; Y is frozen to preserve depth layers.
            for i in 0..n {
                self.nodes[i].vx = (self.nodes[i].vx + fx[i] * DT) * DAMPING;
                self.nodes[i].vz = (self.nodes[i].vz + fz[i] * DT) * DAMPING;
                self.nodes[i].x += self.nodes[i].vx * DT;
                self.nodes[i].z += self.nodes[i].vz * DT;
            }
        }

        // Zero velocities so the layout is stable when re-used.
        for nd in &mut self.nodes {
            nd.vx = 0.0;
            nd.vz = 0.0;
        }
    }

    /// Translate all nodes so the XZ centre of mass is at (0, 0).
    fn centre_xz(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let n = self.nodes.len() as f64;
        let cx = self.nodes.iter().map(|nd| nd.x).sum::<f64>() / n;
        let cz = self.nodes.iter().map(|nd| nd.z).sum::<f64>() / n;
        for nd in &mut self.nodes {
            nd.x -= cx;
            nd.z -= cz;
        }
    }

    /// Return the centre of mass of all node positions.
    pub fn centre_of_mass(&self) -> (f64, f64) {
        if self.nodes.is_empty() {
            return (0.0, 0.0);
        }
        let cx = self.nodes.iter().map(|n| n.x).sum::<f64>()
            / self.nodes.len() as f64;
        let cy = self.nodes.iter().map(|n| n.y).sum::<f64>()
            / self.nodes.len() as f64;
        (cx, cy)
    }

    /// Translate all nodes so the vertical centre of mass is at y = 0.
    /// X is kept as-is (nodes are already horizontally centred per row).
    fn centre_y(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let cy = self.nodes.iter().map(|n| n.y).sum::<f64>()
            / self.nodes.len() as f64;
        for n in &mut self.nodes {
            n.y -= cy;
        }
    }

    /// Run `steps` force-simulation iterations (kept for the 3-D GPU path).
    pub fn simulate(
        &mut self,
        steps: usize,
    ) {
        for _ in 0..steps {
            self.step();
        }
    }

    /// One force-directed simulation step (spring-electrical model).
    /// Only called from the 3-D GPU path (`graph3d::Layout3D::from_2d`).
    fn step(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }

        let id_to_idx: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), i))
            .collect();

        let mut fx = vec![0.0_f64; n];
        let mut fy = vec![0.0_f64; n];

        // Repulsion
        const REPULSION: f64 = 12_000.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = self.nodes[i].x - self.nodes[j].x;
                let dy = self.nodes[i].y - self.nodes[j].y;
                let dist2 = (dx * dx + dy * dy).max(1.0);
                let dist = dist2.sqrt();
                let force = REPULSION / dist2;
                let fxi = force * dx / dist;
                let fyi = force * dy / dist;
                fx[i] += fxi;
                fy[i] += fyi;
                fx[j] -= fxi;
                fy[j] -= fyi;
            }
        }

        // Spring attraction along edges
        const TARGET_LEN: f64 = 220.0;
        const SPRING_K: f64 = 0.06;
        for edge in &self.edges {
            let Some(&i) = id_to_idx.get(edge.from.as_str()) else {
                continue;
            };
            let Some(&j) = id_to_idx.get(edge.to.as_str()) else {
                continue;
            };
            let dx = self.nodes[j].x - self.nodes[i].x;
            let dy = self.nodes[j].y - self.nodes[i].y;
            let dist = (dx * dx + dy * dy).sqrt().max(0.1);
            let stretch = dist - TARGET_LEN;
            let sfx = SPRING_K * stretch * dx / dist;
            let sfy = SPRING_K * stretch * dy / dist;
            fx[i] += sfx;
            fy[i] += sfy;
            fx[j] -= sfx;
            fy[j] -= sfy;
        }

        // Integration
        const DAMPING: f64 = 0.82;
        const DT: f64 = 0.5;
        for i in 0..n {
            self.nodes[i].vx = (self.nodes[i].vx + fx[i] * DT) * DAMPING;
            self.nodes[i].vy = (self.nodes[i].vy + fy[i] * DT) * DAMPING;
            self.nodes[i].x += self.nodes[i].vx * DT;
            self.nodes[i].y += self.nodes[i].vy * DT;
        }
    }
}
