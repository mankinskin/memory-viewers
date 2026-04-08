//! Force-directed 2D layout engine for the ticket dependency graph.
//!
//! Ported from the log-viewer HypergraphView layout.ts (spring-electrical
//! simulation). Nodes start on a circle, then the simulation runs until
//! equilibrium producing a centred layout.

use std::collections::HashMap;

use crate::backend::{GraphEdgeItem, GraphNodeItem};

pub const CARD_W: f64 = 160.0;
pub const CARD_H: f64 = 72.0;

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
    /// Layout position (centre of card), layout-space pixels.
    pub x: f64,
    pub y: f64,
    /// Force-simulation velocity (cleared after simulation).
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

// ── Layout construction ────────────────────────────────────────────────────

impl GraphLayout {
    /// Build a layout from raw API items and run the force simulation.
    pub fn build(api_nodes: Vec<GraphNodeItem>, api_edges: Vec<GraphEdgeItem>) -> Self {
        let n = api_nodes.len();

        // Initialise positions on a circle with slight jitter so the
        // simulation has something to work with even for linear chains.
        let nodes: Vec<GraphNode> = api_nodes
            .into_iter()
            .enumerate()
            .map(|(i, node)| {
                let angle = (i as f64 / n.max(1) as f64) * std::f64::consts::TAU;
                let r = 150.0 + n as f64 * 25.0;
                // depth-based horizontal nudge for hierarchical graphs
                let depth_offset = node.depth as f64 * 40.0;
                GraphNode {
                    id: node.id,
                    title: node.title,
                    state: node.state,
                    depth: node.depth,
                    x: angle.cos() * r + depth_offset,
                    y: angle.sin() * r,
                    vx: 0.0,
                    vy: 0.0,
                }
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
        layout.simulate(200);
        layout.centre();
        layout
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

    /// Translate all nodes so the centre of mass is at (0, 0).
    fn centre(&mut self) {
        let (cx, cy) = self.centre_of_mass();
        for n in &mut self.nodes {
            n.x -= cx;
            n.y -= cy;
        }
    }

    /// Run `steps` simulation iterations.
    pub fn simulate(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step();
        }
    }

    /// One force-directed simulation step (spring-electrical model).
    fn step(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }

        // Map node id → index for fast edge lookup.
        let id_to_idx: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), i))
            .collect();

        let mut fx = vec![0.0_f64; n];
        let mut fy = vec![0.0_f64; n];

        // ── Repulsion (Coulomb / electric) ────────────────────────────────
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

        // ── Attraction (spring) along edges ───────────────────────────────
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

        // ── Integration ───────────────────────────────────────────────────
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

// ── Canvas edge drawing ────────────────────────────────────────────────────

/// Draw all graph edges on the `#webgpu-canvas` element.
///
/// `pan_x/pan_y` are in layout-space pixels.  `zoom` scales both axes.
/// The origin of layout-space maps to the canvas centre.
pub fn draw_edges(layout: &GraphLayout, pan_x: f64, pan_y: f64, zoom: f64) {
    use wasm_bindgen::JsCast;

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    let canvas_el = match document.get_element_by_id("webgpu-canvas") {
        Some(el) => el,
        None => return,
    };
    let canvas: web_sys::HtmlCanvasElement = match canvas_el.dyn_into() {
        Ok(c) => c,
        Err(_) => return,
    };

    // Keep canvas pixel dimensions in sync with its CSS display size.
    let w = canvas.client_width() as u32;
    let h = canvas.client_height() as u32;
    if w == 0 || h == 0 {
        return;
    }
    if canvas.width() != w {
        canvas.set_width(w);
    }
    if canvas.height() != h {
        canvas.set_height(h);
    }

    let ctx: web_sys::CanvasRenderingContext2d = match canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|c| c.dyn_into().ok())
    {
        Some(c) => c,
        None => return,
    };

    ctx.clear_rect(0.0, 0.0, w as f64, h as f64);

    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;

    // Build id → screen-position map (anchor at card centre).
    let positions: HashMap<&str, (f64, f64)> = layout
        .nodes
        .iter()
        .map(|n| {
            let sx = cx + (n.x + pan_x) * zoom;
            let sy = cy + (n.y + pan_y) * zoom;
            (n.id.as_str(), (sx, sy))
        })
        .collect();

    ctx.set_line_width(1.5 * zoom.max(0.5));
    ctx.set_global_alpha(0.75);

    for edge in &layout.edges {
        let Some(&(x1, y1)) = positions.get(edge.from.as_str()) else {
            continue;
        };
        let Some(&(x2, y2)) = positions.get(edge.to.as_str()) else {
            continue;
        };

        // Edge colour keyed on kind.
        let color = if edge.kind == "depends_on" {
            "#5b8dd9"
        } else {
            "#a0a0a0"
        };
        #[allow(deprecated)]
        ctx.set_stroke_style(&js_sys::JsString::from(color).into());

        // Line body.
        ctx.begin_path();
        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        ctx.stroke();

        // Arrowhead at target end.
        let angle = (y2 - y1).atan2(x2 - x1);
        let arrow_len = 10.0 * zoom.clamp(0.5, 2.0);
        let spread = 0.42_f64;
        ctx.begin_path();
        ctx.move_to(x2, y2);
        ctx.line_to(
            x2 - arrow_len * (angle - spread).cos(),
            y2 - arrow_len * (angle - spread).sin(),
        );
        ctx.move_to(x2, y2);
        ctx.line_to(
            x2 - arrow_len * (angle + spread).cos(),
            y2 - arrow_len * (angle + spread).sin(),
        );
        ctx.stroke();
    }

    ctx.set_global_alpha(1.0);
}
