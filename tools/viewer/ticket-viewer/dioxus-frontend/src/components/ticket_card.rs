//! TicketCard — DOM element rendered as a graph node.
//!
//! Each card is positioned absolutely by the DepGraph layout engine.
//! The card's border-left colour signals the ticket state.  Clicking opens
//! the TicketDetailPage; pressing mouse-down starts a drag.

use dioxus::prelude::*;

/// Map a ticket state string to a CSS hex colour for the card's left border.
pub fn state_color(state: Option<&str>) -> &'static str {
    match state {
        Some("new") => "#6b7280",
        Some("ready") => "#8b5cf6",
        Some("in-implementation") => "#f59e0b",
        Some("in-review") => "#ec4899",
        Some("done") => "#10b981",
        Some("cancelled") => "#ef4444",
        _ => "#9ca3af",
    }
}

// ── Props ──────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct TicketCardProps {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub depth: usize,
    /// Layout-space pixel position of the card centre.
    pub layout_x: f64,
    pub layout_y: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
    pub canvas_w: f64,
    pub canvas_h: f64,
    pub on_click: EventHandler<String>,
    pub on_drag_start: EventHandler<(String, f64, f64)>,
}

// ── Component ──────────────────────────────────────────────────────────────

/// A single ticket node rendered as an absolutely-positioned DOM element.
#[component]
pub fn TicketCard(props: TicketCardProps) -> Element {
    let color = state_color(props.state.as_deref());
    let title = props.title.as_deref().unwrap_or("Untitled");
    let state_label = props.state.as_deref().unwrap_or("—");

    // Convert layout-space position to screen pixels relative to the
    // dep-graph container (which has its origin at the container centre).
    // The caller supplies canvas_w/h so we can place the origin at (50%, 50%).
    let screen_x = props.canvas_w / 2.0 + (props.layout_x + props.pan_x) * props.zoom;
    let screen_y = props.canvas_h / 2.0 + (props.layout_y + props.pan_y) * props.zoom;

    use crate::graph::{CARD_H, CARD_W};
    let card_w_px = CARD_W * props.zoom.clamp(0.4, 2.5);
    let card_h_px = CARD_H * props.zoom.clamp(0.4, 2.5);

    // Position the card so its centre aligns with screen_x / screen_y.
    let left = screen_x - card_w_px / 2.0;
    let top = screen_y - card_h_px / 2.0;

    let font_scale = props.zoom.clamp(0.55, 1.5);

    let id = props.id.clone();
    let id_drag = props.id.clone();

    rsx! {
        div {
            key: "{props.id}",
            style: "
                position: absolute;
                left: {left}px;
                top: {top}px;
                width: {card_w_px}px;
                height: {card_h_px}px;
                box-sizing: border-box;
                border: 1px solid rgba(200,200,200,0.4);
                border-left: 3px solid {color};
                border-radius: 6px;
                background: rgba(30,30,40,0.88);
                backdrop-filter: blur(2px);
                padding: 6px 8px;
                cursor: pointer;
                user-select: none;
                overflow: hidden;
                font-family: sans-serif;
                box-shadow: 0 2px 8px rgba(0,0,0,0.5);
                transition: box-shadow 0.1s;
                z-index: 10;
            ",
            onmousedown: move |evt: Event<MouseData>| {
                evt.stop_propagation();
                let client = evt.client_coordinates();
                props.on_drag_start.call((id_drag.clone(), client.x, client.y));
            },
            onclick: move |evt: Event<MouseData>| {
                evt.stop_propagation();
                props.on_click.call(id.clone());
            },
            div {
                style: "
                    font-size: {13.0 * font_scale}px;
                    font-weight: 600;
                    color: #e8e8ee;
                    overflow: hidden;
                    text-overflow: ellipsis;
                    white-space: nowrap;
                    margin-bottom: 4px;
                ",
                "{title}"
            }
            div {
                style: "
                    font-size: {10.0 * font_scale}px;
                    color: {color};
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                ",
                "{state_label}"
            }
            div {
                style: "
                    font-size: {9.0 * font_scale}px;
                    color: rgba(180,180,190,0.6);
                    margin-top: 2px;
                    overflow: hidden;
                    text-overflow: ellipsis;
                    white-space: nowrap;
                ",
                "{props.id}"
            }
        }
    }
}
