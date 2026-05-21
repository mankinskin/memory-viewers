use dioxus::prelude::*;

use viewer_api_dioxus::{
    LayoutMode,
    PanelResizer,
    Projection,
};

use crate::{
    components::{
        dep_graph::DepGraph,
        ticket_content::TicketContent,
        ticket_detail::TicketDetail,
    },
    types::{
        TicketRef,
        TicketSummary,
    },
};

use super::{
    DETAIL_COLLAPSE_PX,
    GRAPH_COLLAPSE_PX,
};

pub(super) fn render_selected_main_panel(
    active_workspace: String,
    selected_ticket: TicketRef,
    tickets: Signal<Vec<TicketSummary>>,
    mut graph_content_ticket: Signal<Option<TicketRef>>,
    mut view_mode: Signal<String>,
    graph_panel_collapsed: bool,
    detail_is_collapsed: bool,
    mut graph_layout_mode: Signal<LayoutMode>,
    mut graph_projection: Signal<Projection>,
    mut graph_panel_width: Signal<f64>,
    mut detail_panel_width: Signal<f64>,
    selected_file: Signal<Option<(TicketRef, String)>>,
    mut graph_panel_override: Signal<Option<bool>>,
    mut detail_panel_override: Signal<Option<bool>>,
    window_width: Signal<u32>,
) -> Element {
    let view_mode_value = view_mode.read().clone();

    rsx! {
        {render_view_mode_bar(
            view_mode,
            view_mode_value.clone(),
            graph_panel_collapsed,
            detail_is_collapsed,
            graph_panel_override,
            detail_panel_override,
            window_width,
        )}
        div {
            style: "display: flex; flex-direction: row; flex: 1; overflow: hidden; min-height: 0;",
            if view_mode_value.as_str() != "content" && !graph_panel_collapsed {
                {
                    let graph_root = selected_ticket.clone();
                    let graph_style = if view_mode_value.as_str() == "graph" {
                        "flex: 1; position: relative; min-width: 0; overflow: hidden;".to_string()
                    } else {
                        format!(
                            "width: {}px; flex-shrink: 0; position: relative; min-width: 0; overflow: hidden;",
                            *graph_panel_width.read()
                        )
                    };
                    rsx! {
                        div {
                            key: "{graph_root.key()}",
                            style: "{graph_style}",
                            DepGraph {
                                workspace: graph_root.workspace.clone(),
                                root_id: graph_root.id.clone(),
                                selected_node_id: graph_content_ticket.read().as_ref().map(|ticket| ticket.id.clone()),
                                layout_mode: *graph_layout_mode.read(),
                                projection: *graph_projection.read(),
                                on_layout_mode_change: move |mode| graph_layout_mode.set(mode),
                                on_projection_change: move |projection| graph_projection.set(projection),
                                on_select: move |ticket_ref: TicketRef| graph_content_ticket.set(Some(ticket_ref)),
                            }
                        }
                    }
                }
                if view_mode_value.as_str() == "split" {
                    PanelResizer {
                        on_resize: move |delta: f64| {
                            let width = (*graph_panel_width.read() + delta).max(150.0);
                            graph_panel_width.set(width);
                        },
                    }
                }
            }
            if view_mode_value.as_str() == "split" && graph_panel_collapsed {
                {render_graph_strip(graph_panel_override, window_width)}
            }
            if view_mode_value.as_str() != "graph" {
                {render_content_panel(
                    active_workspace.clone(),
                    selected_ticket.clone(),
                    tickets,
                    graph_content_ticket,
                    selected_file,
                )}
            }
            if view_mode_value.as_str() != "graph" && !detail_is_collapsed {
                {
                    let detail_ticket = graph_content_ticket
                        .read()
                        .clone()
                        .unwrap_or_else(|| selected_ticket.clone());
                    rsx! {
                        PanelResizer {
                            key: "detail-{detail_ticket.key()}",
                            on_resize: move |delta: f64| {
                                let width = (*detail_panel_width.read() - delta).max(150.0);
                                detail_panel_width.set(width);
                            },
                        }
                        div {
                            style: "width: {detail_panel_width}px; flex-shrink: 0; overflow: hidden; height: 100%;",
                            TicketDetail {
                                workspace: detail_ticket.workspace.clone(),
                                id: detail_ticket.id.clone(),
                            }
                        }
                    }
                }
            }
            if view_mode_value.as_str() != "graph" && detail_is_collapsed {
                {render_detail_strip(detail_panel_override, window_width)}
            }
        }
    }
}

pub(super) fn render_empty_main_panel() -> Element {
    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                align-items: center;
                justify-content: center;
                height: 100%;
                width: 100%;
                color: var(--text-muted);
                gap: 8px;
                padding: 2rem;
                text-align: center;
            ",
            span { style: "font-size: 2rem;", "🎫" }
            span { style: "font-size: 14px;", "Select a ticket from the sidebar to view details." }
        }
    }
}

fn render_view_mode_bar(
    mut view_mode: Signal<String>,
    view_mode_value: String,
    graph_panel_collapsed: bool,
    detail_is_collapsed: bool,
    mut graph_panel_override: Signal<Option<bool>>,
    mut detail_panel_override: Signal<Option<bool>>,
    window_width: Signal<u32>,
) -> Element {
    let button_style = |active: bool| {
        format!(
            "padding: 3px 10px; font-size: 11px; font-weight: 600; \
             border: 1px solid var(--border-subtle); \
             border-radius: 4px; cursor: pointer; \
             background: {}; color: {};",
            if active {
                "var(--accent-blue)"
            } else {
                "var(--bg-secondary)"
            },
            if active { "#fff" } else { "var(--text-muted)" },
        )
    };
    let panel_button_style = |collapsed: bool| {
        format!(
            "padding: 3px 8px; font-size: 10px; font-weight: 500; \
             border: 1px solid var(--border-subtle); \
             border-radius: 4px; cursor: pointer; \
             background: {}; color: {};",
            if collapsed {
                "color-mix(in srgb, var(--accent-blue) 25%, var(--bg-secondary))"
            } else {
                "var(--bg-secondary)"
            },
            if collapsed {
                "var(--accent-blue)"
            } else {
                "var(--text-muted)"
            },
        )
    };

    rsx! {
        div {
            style: "
                display: flex;
                align-items: center;
                gap: 4px;
                padding: 4px 10px;
                border-bottom: 1px solid var(--border-subtle);
                background: var(--bg-primary);
                flex-shrink: 0;
            ",
            span {
                style: "font-size: 11px; color: var(--text-muted); margin-right: 4px;",
                "View:"
            }
            button {
                style: "{button_style(view_mode_value == \"graph\")}",
                onclick: move |_| view_mode.set("graph".to_string()),
                "Graph"
            }
            button {
                style: "{button_style(view_mode_value == \"split\")}",
                onclick: move |_| view_mode.set("split".to_string()),
                "Split"
            }
            button {
                style: "{button_style(view_mode_value == \"content\")}",
                onclick: move |_| view_mode.set("content".to_string()),
                "Content"
            }
            div { style: "flex: 1; min-width: 0;" }
            if view_mode_value == "split" {
                button {
                    title: if graph_panel_collapsed {
                        "Show graph panel (auto-collapsed)"
                    } else {
                        "Collapse graph panel"
                    },
                    style: "{panel_button_style(graph_panel_collapsed)}",
                    onclick: move |_| {
                        let auto = *window_width.read() < GRAPH_COLLAPSE_PX;
                        let current = graph_panel_override.read().unwrap_or(auto);
                        graph_panel_override.set(Some(!current));
                    },
                    if graph_panel_collapsed { "⬡ Graph ›" } else { "⬡ Graph ‹" }
                }
            }
            if view_mode_value != "graph" {
                button {
                    title: if detail_is_collapsed {
                        "Show details panel (auto-collapsed)"
                    } else {
                        "Collapse details panel"
                    },
                    style: "{panel_button_style(detail_is_collapsed)}",
                    onclick: move |_| {
                        let auto = *window_width.read() < DETAIL_COLLAPSE_PX;
                        let current = detail_panel_override.read().unwrap_or(auto);
                        detail_panel_override.set(Some(!current));
                    },
                    if detail_is_collapsed { "☰ Details ›" } else { "☰ Details ‹" }
                }
            }
        }
    }
}

fn render_content_panel(
    active_workspace: String,
    selected_ticket: TicketRef,
    tickets: Signal<Vec<TicketSummary>>,
    graph_content_ticket: Signal<Option<TicketRef>>,
    selected_file: Signal<Option<(TicketRef, String)>>,
) -> Element {
    let content_ticket = graph_content_ticket
        .read()
        .clone()
        .unwrap_or_else(|| selected_ticket.clone());
    let active_asset = selected_file
        .read()
        .as_ref()
        .filter(|(ticket_ref, _)| ticket_ref == &content_ticket)
        .map(|(_, path)| path.clone());
    let content_key = format!(
        "content-{}-{}",
        content_ticket.key(),
        active_asset.as_deref().unwrap_or("description.md")
    );
    let fields = tickets
        .read()
        .iter()
        .find(|ticket| {
            ticket.resolved_ticket_ref(&active_workspace) == content_ticket
        })
        .map(|ticket| ticket.fields.clone())
        .unwrap_or(serde_json::Value::Object(Default::default()));

    rsx! {
        div {
            key: "{content_key}",
            style: "flex: 1; min-width: 0; overflow: hidden; display: flex; flex-direction: column;",
            TicketContent {
                workspace: content_ticket.workspace,
                ticket_id: content_ticket.id,
                fields,
                asset_path: active_asset,
            }
        }
    }
}

fn render_graph_strip(
    mut graph_panel_override: Signal<Option<bool>>,
    window_width: Signal<u32>,
) -> Element {
    rsx! {
        div {
            style: "
                width: 32px; min-width: 32px; height: 100%;
                background: var(--panel-bg-strong);
                backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                -webkit-backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                border-right: 1px solid var(--border-color);
                display: flex; flex-direction: column;
                align-items: center; justify-content: center;
                flex-shrink: 0; cursor: pointer;
            ",
            title: "Show graph panel",
            onclick: move |_| {
                let auto = *window_width.read() < GRAPH_COLLAPSE_PX;
                let current = graph_panel_override.read().unwrap_or(auto);
                graph_panel_override.set(Some(!current));
            },
            div {
                style: "
                    writing-mode: vertical-lr;
                    font-size: 10px; font-weight: 600;
                    color: var(--text-muted);
                    letter-spacing: 0.08em; text-transform: uppercase;
                    user-select: none; padding: 8px 0;
                ",
                "⬡ Graph ›"
            }
        }
    }
}

fn render_detail_strip(
    mut detail_panel_override: Signal<Option<bool>>,
    window_width: Signal<u32>,
) -> Element {
    rsx! {
        div {
            style: "
                width: 32px; min-width: 32px; height: 100%;
                background: var(--panel-bg-strong);
                backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                -webkit-backdrop-filter: blur(var(--panel-blur)) saturate(var(--panel-saturate));
                border-left: 1px solid var(--border-color);
                display: flex; flex-direction: column;
                align-items: center; justify-content: center;
                flex-shrink: 0; cursor: pointer;
            ",
            title: "Show details panel",
            onclick: move |_| {
                let auto = *window_width.read() < DETAIL_COLLAPSE_PX;
                let current = detail_panel_override.read().unwrap_or(auto);
                detail_panel_override.set(Some(!current));
            },
            div {
                style: "
                    writing-mode: vertical-rl;
                    transform: rotate(180deg);
                    font-size: 10px; font-weight: 600;
                    color: var(--text-muted);
                    letter-spacing: 0.08em; text-transform: uppercase;
                    user-select: none; padding: 8px 0;
                ",
                "Details ›"
            }
        }
    }
}
