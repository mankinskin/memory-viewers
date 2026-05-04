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

use crate::types::{GraphEdgeItem, GraphNodeItem};

// ── Data types ─────────────────────────────────────────────────────────────

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
    /// Kept for API compatibility with Graph3D path; unused in 2-D layout.
    pub vx: f64,
    pub vy: f64,
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
    /// Build a hierarchical layout from raw API items.
    ///
    /// Nodes are placed in rows by BFS depth (depth 0 at the top).  Within
    /// each row they are sorted by priority then title, giving a stable
    /// left-to-right ordering with higher-priority tickets on the left.
    pub fn build(api_nodes: Vec<GraphNodeItem>, api_edges: Vec<GraphEdgeItem>) -> Self {
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
                vx: 0.0,
                vy: 0.0,
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
        layout
    }

    /// Place nodes in a grid where each row corresponds to a BFS depth level.
    ///
    /// * Y axis: `depth * LAYER_SPACING` — deeper nodes are lower on screen.
    /// * X axis: nodes within a row are sorted by priority then title and
    ///   distributed evenly, centred around x = 0.
    fn hierarchical_layout(&mut self) {
        // Pixel spacing constants.
        // Larger values spread nodes further apart so edge beams are clearly
        // visible between adjacent cards.
        const LAYER_SPACING: f64 = 280.0; // vertical gap between depth rows
        const COL_SPACING: f64 = 360.0;   // horizontal gap between nodes in a row

        // Group node indices by depth.
        let mut by_depth: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            by_depth.entry(node.depth).or_default().push(i);
        }

        let mut depths: Vec<usize> = by_depth.keys().copied().collect();
        depths.sort_unstable();

        for depth in depths {
            let indices = by_depth.get_mut(&depth).unwrap();

            // Sort within the row: priority (ascending = more important first),
            // then alphabetically by title as a stable tiebreaker.
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
            let total_width = (n as f64 - 1.0) * COL_SPACING;
            let start_x = -total_width / 2.0;
            let y = depth as f64 * LAYER_SPACING;

            for (slot, &node_idx) in indices.iter().enumerate() {
                self.nodes[node_idx].x = start_x + slot as f64 * COL_SPACING;
                self.nodes[node_idx].y = y;
            }
        }

        // Translate so the layout is centred vertically around y = 0.
        self.centre_y();
    }

    /// Return the centre of mass of all node positions.
    pub fn centre_of_mass(&self) -> (f64, f64) {
        if self.nodes.is_empty() {
            return (0.0, 0.0);
        }
        let cx = self.nodes.iter().map(|n| n.x).sum::<f64>() / self.nodes.len() as f64;
        let cy = self.nodes.iter().map(|n| n.y).sum::<f64>() / self.nodes.len() as f64;
        (cx, cy)
    }

    /// Translate all nodes so the vertical centre of mass is at y = 0.
    /// X is kept as-is (nodes are already horizontally centred per row).
    fn centre_y(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let cy = self.nodes.iter().map(|n| n.y).sum::<f64>() / self.nodes.len() as f64;
        for n in &mut self.nodes {
            n.y -= cy;
        }
    }

    /// Run `steps` force-simulation iterations (kept for the 3-D GPU path).
    pub fn simulate(&mut self, steps: usize) {
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
